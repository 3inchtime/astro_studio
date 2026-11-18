use crate::api_gateway::{EngineCallError, ImageEngine, PreparedEditImage, ProviderAttemptBody};
use crate::current_timestamp;
use crate::db::Database;
use crate::error::AppError;
use crate::file_manager::{self, PromotedGenerationFiles, StagedGenerationFiles};
use crate::generation_jobs::{
    begin_generation_job_write_transaction, executable_provider_snapshot_is_valid,
    finish_job_in_transaction, get_job_event_in_transaction, get_job_in_transaction,
    load_generation_execution_snapshot_for_stage_in_transaction,
    set_actual_image_count_in_transaction, terminal_message_for_code,
    validate_worker_recovery_for_stage, GenerationJobOptions, GenerationJobRequest,
    GenerationJobRequestKind, GenerationJobTerminalUpdate, PreparedGenerationJob,
    WorkerTransitionError,
};
use crate::generation_worker_lease::{
    assert_worker_transition_authority_in_transaction, WorkerLeaseError, WorkerTransitionAuthority,
};
use crate::image_engines::{provider_for_model, ImageProvider};
use crate::model_registry::{
    image_endpoint_url_for_model_settings, is_gemini_model, normalize_image_model,
    sanitize_request_options_for_model, ImageEndpointKind,
};
use crate::models::*;
use crate::providers;
use cap_fs_ext::DirExt;
use rusqlite::{params, OptionalExtension, Transaction};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::io::{Read, Write};
use std::marker::PhantomData;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tauri::Emitter;

const RECOVERY_STATE_REQUESTING: &str = "requesting";
const RECOVERY_STATE_RESPONSE_READY: &str = "response_ready";

#[derive(Debug, Clone, Copy, Default, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum GenerationLifecycleKind {
    #[default]
    Generate,
    Edit,
}

impl GenerationLifecycleKind {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Generate => "generate",
            Self::Edit => "edit",
        }
    }

    fn endpoint_kind(self) -> ImageEndpointKind {
        match self {
            Self::Generate => ImageEndpointKind::Generate,
            Self::Edit => ImageEndpointKind::Edit,
        }
    }

    fn completed_log_message(self, image_count: usize) -> String {
        match self {
            Self::Generate => format!("Completed — {} image(s) saved", image_count),
            Self::Edit => format!("Edit completed — {} image(s) saved", image_count),
        }
    }

    fn failed_log_message(self, error: &str) -> String {
        match self {
            Self::Generate => format!("Failed: {}", error),
            Self::Edit => format!("Edit failed: {}", error),
        }
    }
}

impl From<GenerationJobRequestKind> for GenerationLifecycleKind {
    fn from(kind: GenerationJobRequestKind) -> Self {
        match kind {
            GenerationJobRequestKind::Generate => Self::Generate,
            GenerationJobRequestKind::Edit => Self::Edit,
        }
    }
}

use crate::generation_execution::GenerationExecutionRequest as GenerationLifecycleRequest;

trait LegacyGenerationEventSink: Send + Sync {
    fn stage_changed(&self) {}
    fn progress(&self, generation_id: &str);
    fn complete(&self, generation_id: &str);
    fn failed(&self, generation_id: &str, sanitized_message: &str);
}

struct TauriLegacyGenerationEventSink<'a, R: tauri::Runtime> {
    app: &'a tauri::AppHandle<R>,
    scope: &'a GenerationProgressContext,
}

impl<R: tauri::Runtime> TauriLegacyGenerationEventSink<'_, R> {
    fn emit(&self, event: &str, status: &str, error: Option<&str>) {
        let mut value = serde_json::json!(self.scope);
        value["status"] = status.into();
        if let Some(error) = error {
            value["error"] = error.into();
        }
        let _ = self.app.emit(event, value);
    }
}

impl<R: tauri::Runtime> LegacyGenerationEventSink for TauriLegacyGenerationEventSink<'_, R> {
    fn stage_changed(&self) {
        crate::generation_execution::publish_current(self.app, &self.scope.job_id);
    }
    fn progress(&self, _: &str) {
        self.emit("generation:progress", "processing", None);
    }
    fn complete(&self, _: &str) {
        self.emit("generation:complete", "completed", None);
    }
    fn failed(&self, _: &str, message: &str) {
        self.emit("generation:failed", "failed", Some(message));
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GenerationExecutionContext {
    pub(crate) generation_id: String,
    pub(crate) job_id: String,
    pub(crate) conversation_id: String,
    pub(crate) provider_kind: String,
    pub(crate) model: String,
    pub(crate) endpoint_url: String,
    pub(crate) provider_profile_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum GenerationExecutionError {
    Engine(EngineCallError),
    Local {
        code: String,
        sanitized_message: String,
        stage: String,
    },
}

impl GenerationExecutionError {
    pub(crate) fn code(&self) -> &str {
        match self {
            Self::Engine(error) => &error.code,
            Self::Local { code, .. } => code,
        }
    }

    pub(crate) fn sanitized_message(&self) -> &str {
        match self {
            Self::Engine(error) => &error.sanitized_message,
            Self::Local {
                sanitized_message, ..
            } => sanitized_message,
        }
    }

    fn response_artifact() -> Self {
        Self::Local {
            code: "recovery_failed".to_string(),
            sanitized_message: "The provider response could not be verified".to_string(),
            stage: "response_artifact".to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GenerationTerminalDisposition {
    pub(crate) status: GenerationJobStatus,
    pub(crate) error_code: String,
    pub(crate) retryable: bool,
    pub(crate) preserve_response_ready: bool,
}

#[derive(Debug)]
enum GenerationSuccessTransition {
    Completed(GenerateResult),
    CancelRequested,
}

#[derive(Debug)]
pub(crate) enum FencedGenerationSuccessTransition {
    Completed {
        result: GenerateResult,
        event: Box<GenerationJobEvent>,
    },
    CancelRequested,
}

type PrecreatedLocalOutcome = GenerationSuccessTransition;

/// Verified response envelope. Raw bodies are deliberately not serializable or
/// debuggable outside the app-owned response store.
pub(crate) struct ProviderAttemptResponse {
    pub(crate) body_text: String,
    pub(crate) response_file: String,
    pub(crate) response_size: u64,
    pub(crate) response_sha256: String,
    pub(crate) requested_image_count: u8,
}

/// Narrow, already-verified input for the response-ready SQL transition.
/// Private fields ensure callers cannot bypass response artifact verification.
struct VerifiedResponseCommit {
    response_file: String,
    response_size: u64,
    response_sha256: String,
    requested_image_count: u8,
    job_id: String,
    generation_id: String,
}

struct PersistedFencedResponseProjection {
    request_state: String,
    expected_response_file: Option<String>,
    response_file: Option<String>,
    response_size: Option<i64>,
    response_sha256: Option<String>,
    recovery_updated_at: String,
    job_status: String,
    job_stage: String,
    last_heartbeat_at: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct TerminalRecoveryProjection {
    request_kind: String,
    request_state: String,
    output_format: String,
    expected_response_file: Option<String>,
    response_file: Option<String>,
    response_size: Option<i64>,
    response_sha256: Option<String>,
    created_at: String,
    updated_at: String,
}

/// Sealed, pure-data success projection derived from a live promoted-file
/// guard. The lifetime prevents cleanup/disarm while the SQL commit is using
/// the projection, without passing an I/O-capable guard into the transaction.
struct PromotedImageCommit<'a> {
    generation_id: String,
    images: Vec<GeneratedImage>,
    _guard: PhantomData<&'a PromotedGenerationFiles>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ResponseVerificationEvent {
    BeforeBodyHash,
    BeforeFileMetadata,
}

/// Observes the exact boundaries before verified-response CPU or filesystem
/// work. Implementations must be non-blocking and must not panic.
trait ResponseVerificationObserver: Send + Sync {
    fn observe(&self, event: ResponseVerificationEvent);
}

fn observe_response_verification(
    observer: Option<&dyn ResponseVerificationObserver>,
    event: ResponseVerificationEvent,
) {
    if let Some(observer) = observer {
        observer.observe(event);
    }
}

#[derive(Debug, Clone)]
pub(crate) struct GenerationExecutionSnapshot {
    pub(crate) context: GenerationExecutionContext,
    pub(crate) request: GenerationJobRequest,
    pub(crate) runtime_options: GptImageRequestOptions,
    pub(crate) created_at: String,
    pub(crate) output_format: String,
}

pub(crate) struct ProviderExecutionCredentials {
    api_key: String,
}

impl ProviderExecutionCredentials {
    pub(crate) fn new(api_key: String) -> Self {
        Self { api_key }
    }

    pub(crate) fn expose_for_engine(&self) -> &str {
        &self.api_key
    }
}

enum PreparedProviderPayload {
    Generate,
    Edit(Vec<PreparedEditImage>),
}

/// Sealed preparation result that must be fully produced before the provider
/// HTTP future is created. It deliberately has no `Debug`/`Serialize` surface.
pub(crate) struct PreparedProviderAttempt {
    context: GenerationExecutionContext,
    request: GenerationJobRequest,
    payload: PreparedProviderPayload,
    overlay: Option<PreparedEditImage>,
}

#[derive(Clone, Default)]
pub(crate) struct CancellationProbe {
    cancelled: tokio::sync::watch::Sender<bool>,
}

impl CancellationProbe {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn cancel(&self) {
        self.cancelled.send_replace(true);
    }

    pub(crate) fn is_cancelled(&self) -> bool {
        *self.cancelled.borrow()
    }

    async fn until_cancelled(&self) {
        let mut changes = self.cancelled.subscribe();
        let _ = changes.wait_for(|cancelled| *cancelled).await;
    }

    fn error(stage: &str) -> GenerationExecutionError {
        GenerationExecutionError::Local {
            code: "cancelled_by_user".to_string(),
            sanitized_message: "The generation was cancelled".to_string(),
            stage: stage.to_string(),
        }
    }

    pub(crate) fn checkpoint(&self, stage: &str) -> Result<(), GenerationExecutionError> {
        if self.is_cancelled() {
            Err(Self::error(stage))
        } else {
            Ok(())
        }
    }
}

#[async_trait::async_trait]
pub(crate) trait ImageResponseDecoder: Send + Sync {
    async fn decode_and_download(
        &self,
        response: &ProviderAttemptResponse,
        cancellation: &CancellationProbe,
    ) -> Result<Vec<Vec<u8>>, GenerationExecutionError>;
}

pub(crate) struct EngineImageResponseDecoder {
    engine: Arc<crate::api_gateway::GptImageEngine>,
}

impl EngineImageResponseDecoder {
    pub(crate) fn new(engine: Arc<crate::api_gateway::GptImageEngine>) -> Self {
        Self { engine }
    }
}

#[async_trait::async_trait]
impl ImageResponseDecoder for EngineImageResponseDecoder {
    async fn decode_and_download(
        &self,
        response: &ProviderAttemptResponse,
        cancellation: &CancellationProbe,
    ) -> Result<Vec<Vec<u8>>, GenerationExecutionError> {
        cancellation.checkpoint("response_decode")?;
        if response.response_size != response.body_text.len() as u64
            || response.response_sha256
                != FileResponseArtifactStore::response_hash(&response.body_text)
            || !(1..=10).contains(&response.requested_image_count)
        {
            return Err(GenerationExecutionError::response_artifact());
        }
        let mut images = self
            .engine
            .decode_images_from_response(&response.body_text, &|| cancellation.is_cancelled())
            .await
            .map_err(|_| {
                if cancellation.is_cancelled() {
                    CancellationProbe::error("response_decode")
                } else {
                    GenerationExecutionError::Local {
                        code: "response_decode_failed".to_string(),
                        sanitized_message: "The provider response could not be decoded".to_string(),
                        stage: "response_decode".to_string(),
                    }
                }
            })?;
        cancellation.checkpoint("response_decode")?;
        images.truncate(response.requested_image_count as usize);
        if images.is_empty() {
            return Err(GenerationExecutionError::Local {
                code: "response_decode_failed".to_string(),
                sanitized_message: "The provider response did not contain an image".to_string(),
                stage: "response_decode".to_string(),
            });
        }
        Ok(images)
    }
}

#[async_trait::async_trait]
pub(crate) trait GenerationFileStore: Send + Sync {
    async fn stage_images(
        &self,
        snapshot: &GenerationExecutionSnapshot,
        images: Vec<Vec<u8>>,
        cancellation: &CancellationProbe,
    ) -> Result<StagedGenerationFiles, GenerationExecutionError>;
}

#[derive(Clone)]
pub(crate) struct LocalGenerationFileStore {
    root_dir: PathBuf,
}

impl LocalGenerationFileStore {
    pub(crate) fn new(root_dir: PathBuf) -> Self {
        Self { root_dir }
    }
}

#[async_trait::async_trait]
impl GenerationFileStore for LocalGenerationFileStore {
    async fn stage_images(
        &self,
        snapshot: &GenerationExecutionSnapshot,
        images: Vec<Vec<u8>>,
        cancellation: &CancellationProbe,
    ) -> Result<StagedGenerationFiles, GenerationExecutionError> {
        cancellation.checkpoint("image_staging")?;
        let root_dir = self.root_dir.clone();
        let generation_id = snapshot.context.generation_id.clone();
        let output_format = snapshot.output_format.clone();
        let created_at = snapshot.created_at.clone();
        let staged = tokio::task::spawn_blocking(move || {
            file_manager::FileManager::new(root_dir).stage_generation_images(
                &generation_id,
                &images,
                &output_format,
                &created_at,
            )
        })
        .await
        .map_err(|_| GenerationExecutionError::Local {
            code: "image_save_failed".to_string(),
            sanitized_message: "The generated images could not be staged".to_string(),
            stage: "image_staging".to_string(),
        })?
        .map_err(|_| GenerationExecutionError::Local {
            code: "image_save_failed".to_string(),
            sanitized_message: "The generated images could not be staged".to_string(),
            stage: "image_staging".to_string(),
        })?;
        cancellation.checkpoint("image_staging")?;
        Ok(staged)
    }
}

#[async_trait::async_trait]
pub(crate) trait ResponseArtifactStore: Send + Sync {
    fn expected_response_path(
        &self,
        context: &GenerationExecutionContext,
    ) -> Result<PathBuf, GenerationExecutionError>;

    async fn persist_verified_response(
        &self,
        context: &GenerationExecutionContext,
        body: ProviderAttemptBody,
    ) -> Result<ProviderAttemptResponse, GenerationExecutionError>;

    async fn load_verified_response(
        &self,
        context: &GenerationExecutionContext,
        path: &Path,
    ) -> Result<ProviderAttemptResponse, GenerationExecutionError>;
}

#[derive(Clone)]
pub(crate) struct FileResponseArtifactStore {
    root_dir: PathBuf,
}

struct PreparedResponseDirectory {
    directory: cap_std::fs::Dir,
    absolute_directory: PathBuf,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum ResponseDirectoryOpenStage {
    Jobs,
    Job,
}

impl FileResponseArtifactStore {
    const ENVELOPE_VERSION: u64 = 1;
    const MAX_RESPONSE_BODY_BYTES: u64 =
        crate::api_gateway::MAX_PROVIDER_RESPONSE_BODY_BYTES as u64;
    const MAX_RESPONSE_ENVELOPE_BYTES: u64 = Self::MAX_RESPONSE_BODY_BYTES * 2;

    pub(crate) fn new(root_dir: PathBuf) -> Self {
        Self { root_dir }
    }

    fn identity_segment_is_safe(value: &str) -> bool {
        !value.is_empty()
            && value.len() <= 128
            && value
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    }

    pub(crate) fn response_path(
        &self,
        context: &GenerationExecutionContext,
    ) -> Result<PathBuf, GenerationExecutionError> {
        Ok(self.root_dir.join(Self::response_relative_path(context)?))
    }

    fn response_relative_path(
        context: &GenerationExecutionContext,
    ) -> Result<PathBuf, GenerationExecutionError> {
        if !Self::identity_segment_is_safe(&context.job_id)
            || !Self::identity_segment_is_safe(&context.generation_id)
        {
            return Err(GenerationExecutionError::response_artifact());
        }
        Ok(PathBuf::from("generation-jobs")
            .join(&context.job_id)
            .join(format!("{}.response.json", context.generation_id)))
    }

    fn response_hash(body_text: &str) -> String {
        format!("{:x}", Sha256::digest(body_text.as_bytes()))
    }

    fn sync_cap_directory(
        _directory: &cap_std::fs::Dir,
        _absolute_path: &Path,
    ) -> Result<(), GenerationExecutionError> {
        #[cfg(unix)]
        {
            _directory
                .try_clone()
                .and_then(|directory| directory.into_std_file().sync_all())
                .map_err(|_| GenerationExecutionError::response_artifact())?;
        }
        #[cfg(windows)]
        {
            // Windows does not guarantee FlushFileBuffers for directory handles.
            // Attempt it for filesystems that support it, but do not reject a
            // successfully installed artifact solely for that platform limit.
            let _ = file_manager::sync_directory(_absolute_path);
        }
        #[cfg(not(any(unix, windows)))]
        {
            let _ = (_directory, _absolute_path);
        }
        Ok(())
    }

    fn create_cap_directory_with_hook<F>(
        parent: &cap_std::fs::Dir,
        parent_absolute: &Path,
        name: &str,
        stage: ResponseDirectoryOpenStage,
        before_open: &mut F,
    ) -> Result<cap_std::fs::Dir, GenerationExecutionError>
    where
        F: FnMut(ResponseDirectoryOpenStage),
    {
        match parent.create_dir(name) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(_) => return Err(GenerationExecutionError::response_artifact()),
        }
        let metadata = parent
            .symlink_metadata(name)
            .map_err(|_| GenerationExecutionError::response_artifact())?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(GenerationExecutionError::response_artifact());
        }
        before_open(stage);
        let directory = parent
            .open_dir_nofollow(name)
            .map_err(|_| GenerationExecutionError::response_artifact())?;
        Self::sync_cap_directory(parent, parent_absolute)?;
        Self::sync_cap_directory(&directory, &parent_absolute.join(name))?;
        Ok(directory)
    }

    fn prepare_response_directory_with_hook<F>(
        root_dir: &Path,
        context: &GenerationExecutionContext,
        before_open: &mut F,
    ) -> Result<PreparedResponseDirectory, GenerationExecutionError>
    where
        F: FnMut(ResponseDirectoryOpenStage),
    {
        file_manager::create_dir_all_durable(root_dir)
            .map_err(|_| GenerationExecutionError::response_artifact())?;
        let root_metadata = std::fs::symlink_metadata(root_dir)
            .map_err(|_| GenerationExecutionError::response_artifact())?;
        if root_metadata.file_type().is_symlink() || !root_metadata.is_dir() {
            return Err(GenerationExecutionError::response_artifact());
        }
        Self::response_relative_path(context)?;
        let canonical_root = root_dir
            .canonicalize()
            .map_err(|_| GenerationExecutionError::response_artifact())?;
        let root =
            cap_std::fs::Dir::open_ambient_dir(&canonical_root, cap_std::ambient_authority())
                .map_err(|_| GenerationExecutionError::response_artifact())?;
        let jobs = Self::create_cap_directory_with_hook(
            &root,
            &canonical_root,
            "generation-jobs",
            ResponseDirectoryOpenStage::Jobs,
            before_open,
        )?;
        let jobs_absolute = canonical_root.join("generation-jobs");
        let job = Self::create_cap_directory_with_hook(
            &jobs,
            &jobs_absolute,
            &context.job_id,
            ResponseDirectoryOpenStage::Job,
            before_open,
        )?;
        Ok(PreparedResponseDirectory {
            directory: job,
            absolute_directory: jobs_absolute.join(&context.job_id),
        })
    }

    fn prepare_response_directory(
        root_dir: &Path,
        context: &GenerationExecutionContext,
    ) -> Result<PreparedResponseDirectory, GenerationExecutionError> {
        Self::prepare_response_directory_with_hook(root_dir, context, &mut |_| {})
    }

    fn decode_verified_envelope(
        path: &Path,
        bytes: &[u8],
    ) -> Result<ProviderAttemptResponse, GenerationExecutionError> {
        let envelope: serde_json::Value = serde_json::from_slice(bytes)
            .map_err(|_| GenerationExecutionError::response_artifact())?;
        let object = envelope
            .as_object()
            .filter(|object| object.len() == 5)
            .ok_or_else(GenerationExecutionError::response_artifact)?;
        if object.get("version").and_then(serde_json::Value::as_u64) != Some(Self::ENVELOPE_VERSION)
        {
            return Err(GenerationExecutionError::response_artifact());
        }
        let body_text = object
            .get("body_text")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(GenerationExecutionError::response_artifact)?;
        let response_size = object
            .get("body_size")
            .and_then(serde_json::Value::as_u64)
            .ok_or_else(GenerationExecutionError::response_artifact)?;
        let response_sha256 = object
            .get("body_sha256")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(GenerationExecutionError::response_artifact)?;
        let requested_image_count = object
            .get("requested_image_count")
            .and_then(serde_json::Value::as_u64)
            .and_then(|value| u8::try_from(value).ok())
            .filter(|value| (1..=10).contains(value))
            .ok_or_else(GenerationExecutionError::response_artifact)?;
        if response_size != body_text.len() as u64
            || response_size > Self::MAX_RESPONSE_BODY_BYTES
            || response_sha256.len() != 64
            || response_sha256 != Self::response_hash(body_text)
        {
            return Err(GenerationExecutionError::response_artifact());
        }
        Ok(ProviderAttemptResponse {
            body_text: body_text.to_string(),
            response_file: path.to_string_lossy().to_string(),
            response_size,
            response_sha256: response_sha256.to_string(),
            requested_image_count,
        })
    }

    fn load_verified_response_sync_with_hook<F>(
        root_dir: &Path,
        context: &GenerationExecutionContext,
        path: &Path,
        before_open: &mut F,
    ) -> Result<ProviderAttemptResponse, GenerationExecutionError>
    where
        F: FnMut(ResponseDirectoryOpenStage),
    {
        let root_metadata = std::fs::symlink_metadata(root_dir)
            .map_err(|_| GenerationExecutionError::response_artifact())?;
        if root_metadata.file_type().is_symlink() || !root_metadata.is_dir() {
            return Err(GenerationExecutionError::response_artifact());
        }
        let canonical_root = root_dir
            .canonicalize()
            .map_err(|_| GenerationExecutionError::response_artifact())?;
        let relative_path = Self::response_relative_path(context)?;
        let raw_expected_path = root_dir.join(&relative_path);
        let canonical_expected_path = canonical_root.join(&relative_path);
        if path != raw_expected_path && path != canonical_expected_path {
            return Err(GenerationExecutionError::response_artifact());
        }
        let file_name = relative_path
            .file_name()
            .and_then(|value| value.to_str())
            .ok_or_else(GenerationExecutionError::response_artifact)?;
        let root =
            cap_std::fs::Dir::open_ambient_dir(&canonical_root, cap_std::ambient_authority())
                .map_err(|_| GenerationExecutionError::response_artifact())?;
        before_open(ResponseDirectoryOpenStage::Jobs);
        let jobs = root
            .open_dir_nofollow("generation-jobs")
            .map_err(|_| GenerationExecutionError::response_artifact())?;
        before_open(ResponseDirectoryOpenStage::Job);
        let directory = jobs
            .open_dir_nofollow(&context.job_id)
            .map_err(|_| GenerationExecutionError::response_artifact())?;
        let mut options = cap_std::fs::OpenOptions::new();
        options.read(true);
        cap_fs_ext::OpenOptionsFollowExt::follow(&mut options, cap_fs_ext::FollowSymlinks::No);
        let file = directory
            .open_with(file_name, &options)
            .map_err(|_| GenerationExecutionError::response_artifact())?;
        let metadata = file
            .metadata()
            .map_err(|_| GenerationExecutionError::response_artifact())?;
        let maximum_envelope_size = Self::MAX_RESPONSE_ENVELOPE_BYTES;
        if !metadata.is_file() || metadata.len() > maximum_envelope_size {
            return Err(GenerationExecutionError::response_artifact());
        }
        let mut bytes = Vec::with_capacity(
            usize::try_from(metadata.len().min(maximum_envelope_size)).unwrap_or_default(),
        );
        file.take(maximum_envelope_size.saturating_add(1))
            .read_to_end(&mut bytes)
            .map_err(|_| GenerationExecutionError::response_artifact())?;
        if bytes.len() as u64 > maximum_envelope_size {
            return Err(GenerationExecutionError::response_artifact());
        }
        Self::decode_verified_envelope(path, &bytes)
    }

    fn load_verified_response_sync(
        root_dir: &Path,
        context: &GenerationExecutionContext,
        path: &Path,
    ) -> Result<ProviderAttemptResponse, GenerationExecutionError> {
        Self::load_verified_response_sync_with_hook(root_dir, context, path, &mut |_| {})
    }

    pub(crate) async fn load_verified_response(
        &self,
        context: &GenerationExecutionContext,
        path: &Path,
    ) -> Result<ProviderAttemptResponse, GenerationExecutionError> {
        self.response_path(context)?;
        let root_dir = self.root_dir.clone();
        let context = context.clone();
        let path = path.to_path_buf();
        tokio::task::spawn_blocking(move || {
            Self::load_verified_response_sync(&root_dir, &context, &path)
        })
        .await
        .map_err(|_| GenerationExecutionError::response_artifact())?
    }

    fn write_verified_response_sync(
        root_dir: &Path,
        context: &GenerationExecutionContext,
        response_path: &Path,
        body: ProviderAttemptBody,
    ) -> Result<ProviderAttemptResponse, GenerationExecutionError> {
        let response_size = body.body_text.len() as u64;
        if response_size > Self::MAX_RESPONSE_BODY_BYTES
            || !(1..=10).contains(&body.requested_image_count)
        {
            return Err(GenerationExecutionError::response_artifact());
        }
        let response_sha256 = Self::response_hash(&body.body_text);
        let envelope = serde_json::json!({
            "version": Self::ENVELOPE_VERSION,
            "body_text": body.body_text,
            "body_size": response_size,
            "body_sha256": response_sha256,
            "requested_image_count": body.requested_image_count,
        });
        let bytes = serde_json::to_vec(&envelope)
            .map_err(|_| GenerationExecutionError::response_artifact())?;
        if bytes.len() as u64 > Self::MAX_RESPONSE_ENVELOPE_BYTES {
            return Err(GenerationExecutionError::response_artifact());
        }
        let relative_path = Self::response_relative_path(context)?;
        if response_path != root_dir.join(&relative_path) {
            return Err(GenerationExecutionError::response_artifact());
        }
        let file_name = relative_path
            .file_name()
            .and_then(|value| value.to_str())
            .ok_or_else(GenerationExecutionError::response_artifact)?;
        let prepared = Self::prepare_response_directory(root_dir, context)?;
        let temporary_name = format!(".{file_name}.{}.tmp", uuid::Uuid::new_v4());
        let mut options = cap_std::fs::OpenOptions::new();
        options.write(true).create_new(true);
        cap_fs_ext::OpenOptionsFollowExt::follow(&mut options, cap_fs_ext::FollowSymlinks::No);
        #[cfg(unix)]
        {
            use cap_std::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut temporary_file = prepared
            .directory
            .open_with(&temporary_name, &options)
            .map_err(|_| GenerationExecutionError::response_artifact())?;
        let write_result = temporary_file
            .write_all(&bytes)
            .and_then(|()| temporary_file.sync_all());
        drop(temporary_file);
        if write_result.is_err() {
            let _ = prepared.directory.remove_file(&temporary_name);
            let _ = Self::sync_cap_directory(&prepared.directory, &prepared.absolute_directory);
            return Err(GenerationExecutionError::response_artifact());
        }

        match prepared
            .directory
            .hard_link(&temporary_name, &prepared.directory, file_name)
        {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                let _ = prepared.directory.remove_file(&temporary_name);
                let _ = Self::sync_cap_directory(&prepared.directory, &prepared.absolute_directory);
                let existing = Self::load_verified_response_sync(root_dir, context, response_path)?;
                if existing.response_size == response_size
                    && existing.response_sha256 == response_sha256
                    && existing.requested_image_count == body.requested_image_count
                {
                    return Ok(existing);
                }
                return Err(GenerationExecutionError::response_artifact());
            }
            Err(_) => {
                let _ = prepared.directory.remove_file(&temporary_name);
                let _ = Self::sync_cap_directory(&prepared.directory, &prepared.absolute_directory);
                return Err(GenerationExecutionError::response_artifact());
            }
        }
        if Self::sync_cap_directory(&prepared.directory, &prepared.absolute_directory).is_err() {
            let _ = prepared.directory.remove_file(&temporary_name);
            return Err(GenerationExecutionError::response_artifact());
        }

        let verified = match Self::load_verified_response_sync(root_dir, context, response_path) {
            Ok(verified)
                if verified.response_size == response_size
                    && verified.response_sha256 == response_sha256
                    && verified.requested_image_count == body.requested_image_count =>
            {
                verified
            }
            _ => {
                let _ = prepared.directory.remove_file(&temporary_name);
                let _ = Self::sync_cap_directory(&prepared.directory, &prepared.absolute_directory);
                return Err(GenerationExecutionError::response_artifact());
            }
        };
        let _ = prepared.directory.remove_file(&temporary_name);
        let _ = Self::sync_cap_directory(&prepared.directory, &prepared.absolute_directory);
        Ok(verified)
    }
}

#[async_trait::async_trait]
impl ResponseArtifactStore for FileResponseArtifactStore {
    fn expected_response_path(
        &self,
        context: &GenerationExecutionContext,
    ) -> Result<PathBuf, GenerationExecutionError> {
        self.response_path(context)
    }

    async fn persist_verified_response(
        &self,
        context: &GenerationExecutionContext,
        body: ProviderAttemptBody,
    ) -> Result<ProviderAttemptResponse, GenerationExecutionError> {
        let root_dir = self.root_dir.clone();
        let response_path = self.response_path(context)?;
        let context = context.clone();
        tokio::task::spawn_blocking(move || {
            Self::write_verified_response_sync(&root_dir, &context, &response_path, body)
        })
        .await
        .map_err(|_| GenerationExecutionError::response_artifact())?
    }

    async fn load_verified_response(
        &self,
        context: &GenerationExecutionContext,
        path: &Path,
    ) -> Result<ProviderAttemptResponse, GenerationExecutionError> {
        FileResponseArtifactStore::load_verified_response(self, context, path).await
    }
}

fn source_image_invalid() -> GenerationExecutionError {
    GenerationExecutionError::Local {
        code: "source_image_invalid".to_string(),
        sanitized_message: "A source image is unavailable or invalid".to_string(),
        stage: "source_image_revalidation".to_string(),
    }
}

fn prepare_persisted_edit_image_with_hook<F>(
    path_text: &str,
    before_parent_open: F,
) -> Result<PreparedEditImage, GenerationExecutionError>
where
    F: FnOnce(),
{
    use crate::source_images::{
        source_image_media_type_for_path, validate_source_image_data, MAX_SOURCE_IMAGE_BYTES,
    };

    let path = Path::new(path_text);
    if path_text.is_empty() || path_text.chars().any(char::is_control) || !path.is_absolute() {
        return Err(source_image_invalid());
    }
    let parent = path
        .parent()
        .filter(|parent| parent.is_absolute())
        .ok_or_else(source_image_invalid)?;
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty() && !name.chars().any(char::is_control))
        .ok_or_else(source_image_invalid)?;
    let expected_parent_identity =
        same_file::Handle::from_path(parent).map_err(|_| source_image_invalid())?;
    let canonical_parent = parent.canonicalize().map_err(|_| source_image_invalid())?;
    if canonical_parent != parent {
        return Err(source_image_invalid());
    }
    let path_metadata = std::fs::symlink_metadata(path).map_err(|_| source_image_invalid())?;
    if path_metadata.file_type().is_symlink() || !path_metadata.file_type().is_file() {
        return Err(source_image_invalid());
    }
    before_parent_open();
    let directory =
        cap_std::fs::Dir::open_ambient_dir(&canonical_parent, cap_std::ambient_authority())
            .map_err(|_| source_image_invalid())?;
    let opened_parent_identity = same_file::Handle::from_file(
        directory
            .try_clone()
            .map_err(|_| source_image_invalid())?
            .into_std_file(),
    )
    .map_err(|_| source_image_invalid())?;
    if opened_parent_identity != expected_parent_identity {
        return Err(source_image_invalid());
    }
    let entry_metadata = directory
        .symlink_metadata(file_name)
        .map_err(|_| source_image_invalid())?;
    if entry_metadata.file_type().is_symlink() || !entry_metadata.is_file() {
        return Err(source_image_invalid());
    }
    let mut options = cap_std::fs::OpenOptions::new();
    options.read(true);
    cap_fs_ext::OpenOptionsFollowExt::follow(&mut options, cap_fs_ext::FollowSymlinks::No);
    let file = directory
        .open_with(file_name, &options)
        .map_err(|_| source_image_invalid())?;
    let metadata = file.metadata().map_err(|_| source_image_invalid())?;
    if !metadata.is_file() || metadata.len() == 0 || metadata.len() > MAX_SOURCE_IMAGE_BYTES {
        return Err(source_image_invalid());
    }
    let expected_size = metadata.len();
    let mut bytes = Vec::with_capacity(usize::try_from(expected_size).unwrap_or_default());
    file.take(MAX_SOURCE_IMAGE_BYTES.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|_| source_image_invalid())?;
    if bytes.len() as u64 != expected_size || bytes.len() as u64 > MAX_SOURCE_IMAGE_BYTES {
        return Err(source_image_invalid());
    }
    source_image_media_type_for_path(path).map_err(|_| source_image_invalid())?;
    validate_source_image_data(path, &bytes).map_err(|_| source_image_invalid())?;
    Ok(PreparedEditImage::new(file_name.to_string(), bytes))
}

fn prepare_persisted_edit_image(
    path_text: &str,
) -> Result<PreparedEditImage, GenerationExecutionError> {
    prepare_persisted_edit_image_with_hook(path_text, || {})
}

async fn prepare_edit_source_images(
    source_image_paths: &[String],
) -> Result<Vec<PreparedEditImage>, GenerationExecutionError> {
    if source_image_paths.is_empty() {
        return Err(source_image_invalid());
    }
    let source_image_paths = source_image_paths.to_vec();
    tokio::task::spawn_blocking(move || {
        source_image_paths
            .iter()
            .map(|path| prepare_persisted_edit_image(path))
            .collect()
    })
    .await
    .map_err(|_| source_image_invalid())?
}

pub(crate) async fn prepare_provider_attempt(
    snapshot: &GenerationExecutionSnapshot,
) -> Result<PreparedProviderAttempt, GenerationExecutionError> {
    let payload = match snapshot.request.kind {
        GenerationJobRequestKind::Generate => {
            if !snapshot.request.source_image_paths.is_empty() {
                return Err(GenerationExecutionError::Engine(
                    EngineCallError::provider_configuration_invalid(),
                ));
            }
            PreparedProviderPayload::Generate
        }
        GenerationJobRequestKind::Edit => PreparedProviderPayload::Edit(
            prepare_edit_source_images(&snapshot.request.source_image_paths).await?,
        ),
    };
    let overlay = if let Some(path) = snapshot
        .request
        .options
        .advanced_options
        .as_ref()
        .and_then(|a| a.markup_path.as_ref().or(a.mask_path.as_ref()))
    {
        let overlay = prepare_persisted_edit_image(path)?;
        let PreparedProviderPayload::Edit(images) = &payload else {
            return Err(source_image_invalid());
        };
        let source = images.first().ok_or_else(source_image_invalid)?;
        crate::source_images::validate_edit_overlay(source.bytes(), overlay.bytes())
            .map_err(|_| source_image_invalid())?;
        Some(overlay)
    } else {
        None
    };
    Ok(PreparedProviderAttempt {
        overlay,
        context: snapshot.context.clone(),
        request: snapshot.request.clone(),
        payload,
    })
}

fn response_path_is_valid(path: &Path) -> bool {
    let value = path.to_string_lossy();
    !value.is_empty() && value.len() <= 32_768 && !value.chars().any(char::is_control)
}

fn response_path_matches_expected(
    expected: &Path,
    actual: &Path,
    observer: Option<&dyn ResponseVerificationObserver>,
) -> bool {
    if !response_path_is_valid(expected) || !response_path_is_valid(actual) {
        return false;
    }
    observe_response_verification(observer, ResponseVerificationEvent::BeforeFileMetadata);
    let Ok(metadata) = std::fs::symlink_metadata(actual) else {
        return false;
    };
    if metadata.file_type().is_symlink()
        || !metadata.is_file()
        || metadata.len() == 0
        || metadata.len() > FileResponseArtifactStore::MAX_RESPONSE_ENVELOPE_BYTES
    {
        return false;
    }
    expected == actual
}

fn verified_response_shape_matches(
    response: &ProviderAttemptResponse,
    requested_image_count: u8,
    expected_path: Option<&Path>,
    observer: Option<&dyn ResponseVerificationObserver>,
) -> bool {
    let actual_path = Path::new(&response.response_file);
    if response.requested_image_count != requested_image_count
        || !(1..=10).contains(&response.requested_image_count)
        || response.response_size != response.body_text.len() as u64
        || response.response_size > FileResponseArtifactStore::MAX_RESPONSE_BODY_BYTES
        || response.response_sha256.len() != 64
    {
        return false;
    }
    observe_response_verification(observer, ResponseVerificationEvent::BeforeBodyHash);
    response.response_sha256 == FileResponseArtifactStore::response_hash(&response.body_text)
        && response_path_matches_expected(
            expected_path.unwrap_or(actual_path),
            actual_path,
            observer,
        )
}

impl VerifiedResponseCommit {
    fn verify(
        context: &GenerationExecutionContext,
        expected_path: &Path,
        response: &ProviderAttemptResponse,
        observer: Option<&dyn ResponseVerificationObserver>,
    ) -> Result<Self, AppError> {
        let response_root = expected_path
            .parent()
            .and_then(Path::parent)
            .and_then(Path::parent)
            .ok_or(AppError::GenerationJobInvalidSnapshot)?;
        Self::verify_at_store_root(context, expected_path, response_root, response, observer)
    }

    fn verify_with_store(
        context: &GenerationExecutionContext,
        artifact_store: &FileResponseArtifactStore,
        response: &ProviderAttemptResponse,
        observer: Option<&dyn ResponseVerificationObserver>,
    ) -> Result<Self, AppError> {
        let expected_path = artifact_store
            .response_path(context)
            .map_err(|_| AppError::GenerationJobInvalidSnapshot)?;
        Self::verify_at_store_root(
            context,
            &expected_path,
            &artifact_store.root_dir,
            response,
            observer,
        )
    }

    fn verify_at_store_root(
        context: &GenerationExecutionContext,
        expected_path: &Path,
        response_root: &Path,
        response: &ProviderAttemptResponse,
        observer: Option<&dyn ResponseVerificationObserver>,
    ) -> Result<Self, AppError> {
        if !response_path_matches_context(expected_path, context)
            || !verified_response_shape_matches(
                response,
                response.requested_image_count,
                Some(expected_path),
                observer,
            )
        {
            return Err(AppError::GenerationJobInvalidSnapshot);
        }
        let verified_file = FileResponseArtifactStore::load_verified_response_sync(
            response_root,
            context,
            expected_path,
        )
        .map_err(|_| AppError::GenerationJobInvalidSnapshot)?;
        if verified_file.response_file != response.response_file
            || verified_file.response_size != response.response_size
            || verified_file.response_sha256 != response.response_sha256
            || verified_file.requested_image_count != response.requested_image_count
            || verified_file.body_text != response.body_text
        {
            return Err(AppError::GenerationJobInvalidSnapshot);
        }
        Ok(Self {
            response_file: verified_file.response_file,
            response_size: verified_file.response_size,
            response_sha256: verified_file.response_sha256,
            requested_image_count: verified_file.requested_image_count,
            job_id: context.job_id.clone(),
            generation_id: context.generation_id.clone(),
        })
    }

    fn matches_context(&self, context: &GenerationExecutionContext) -> bool {
        self.job_id == context.job_id && self.generation_id == context.generation_id
    }
}

fn response_path_matches_context(path: &Path, context: &GenerationExecutionContext) -> bool {
    if path.components().any(|component| {
        matches!(
            component,
            std::path::Component::CurDir | std::path::Component::ParentDir
        )
    }) {
        return false;
    }
    let expected_file_name = format!("{}.response.json", context.generation_id);
    path.file_name().and_then(|value| value.to_str()) == Some(expected_file_name.as_str())
        && path
            .parent()
            .and_then(Path::file_name)
            .and_then(|value| value.to_str())
            == Some(context.job_id.as_str())
        && path
            .parent()
            .and_then(Path::parent)
            .and_then(Path::file_name)
            .and_then(|value| value.to_str())
            == Some("generation-jobs")
}

pub(crate) fn validate_provider_execution_snapshot(
    snapshot: &GenerationExecutionSnapshot,
) -> Result<(), GenerationExecutionError> {
    let expected_provider_kind = match provider_for_model(&snapshot.context.model) {
        ImageProvider::OpenAi => "openai",
        ImageProvider::Gemini => "gemini",
        ImageProvider::Xai => "xai",
        ImageProvider::Seedream => "seedream",
        ImageProvider::Unknown => "unresolved",
    };
    if snapshot.context.provider_kind != expected_provider_kind
        || snapshot.request.model != snapshot.context.model
        || snapshot.request.conversation_id != snapshot.context.conversation_id
        || snapshot.output_format != snapshot.runtime_options.output_format
        || snapshot.request.options.advanced_options != snapshot.runtime_options.advanced_options
        || snapshot
            .runtime_options
            .advanced_options
            .as_ref()
            .is_some_and(|a| {
                a.stream
                    .is_some_and(|v| v != snapshot.runtime_options.stream)
                    || a.partial_images
                        .is_some_and(|v| v != snapshot.runtime_options.partial_images)
            })
        || crate::model_registry::validate_request_options(
            &snapshot.context.model,
            &snapshot.runtime_options,
            snapshot.request.source_image_paths.len(),
        )
        .is_err()
        || !executable_provider_snapshot_is_valid(
            &snapshot.context.provider_kind,
            &snapshot.context.provider_profile_id,
            &snapshot.context.endpoint_url,
        )
    {
        return Err(GenerationExecutionError::Engine(
            EngineCallError::provider_configuration_invalid(),
        ));
    }
    Ok(())
}

pub(crate) async fn perform_provider_http_attempt(
    engine: &dyn ImageEngine,
    snapshot: &GenerationExecutionSnapshot,
    credentials: &ProviderExecutionCredentials,
    prepared: &PreparedProviderAttempt,
) -> Result<ProviderAttemptBody, GenerationExecutionError> {
    if prepared.context != snapshot.context || prepared.request != snapshot.request {
        return Err(GenerationExecutionError::Engine(
            EngineCallError::provider_configuration_invalid(),
        ));
    }
    validate_provider_execution_snapshot(snapshot)?;
    if credentials.expose_for_engine().is_empty() {
        return Err(GenerationExecutionError::Engine(
            EngineCallError::provider_configuration_invalid(),
        ));
    }

    let mut runtime_options = snapshot.runtime_options.clone();
    runtime_options.prepared_overlay = prepared.overlay.clone();
    let body = match snapshot.request.kind {
        GenerationJobRequestKind::Generate => {
            if !matches!(&prepared.payload, PreparedProviderPayload::Generate) {
                return Err(GenerationExecutionError::Engine(
                    EngineCallError::provider_configuration_invalid(),
                ));
            }
            engine
                .generate(
                    &snapshot.context.model,
                    credentials.expose_for_engine(),
                    &snapshot.context.endpoint_url,
                    &snapshot.request.prompt,
                    &runtime_options,
                )
                .await
        }
        GenerationJobRequestKind::Edit => {
            let PreparedProviderPayload::Edit(source_images) = &prepared.payload else {
                return Err(GenerationExecutionError::Engine(
                    EngineCallError::provider_configuration_invalid(),
                ));
            };
            if source_images.len() != snapshot.request.source_image_paths.len()
                || source_images.is_empty()
            {
                return Err(GenerationExecutionError::Engine(
                    EngineCallError::provider_configuration_invalid(),
                ));
            }
            engine
                .edit(
                    &snapshot.context.model,
                    credentials.expose_for_engine(),
                    &snapshot.context.endpoint_url,
                    &snapshot.request.prompt,
                    source_images,
                    &runtime_options,
                )
                .await
        }
    }
    .map_err(GenerationExecutionError::Engine)?;

    if body.requested_image_count != snapshot.runtime_options.image_count {
        return Err(GenerationExecutionError::response_artifact());
    }
    Ok(body)
}

pub(crate) async fn persist_provider_attempt_response(
    artifact_store: &dyn ResponseArtifactStore,
    snapshot: &GenerationExecutionSnapshot,
    body: ProviderAttemptBody,
) -> Result<ProviderAttemptResponse, GenerationExecutionError> {
    if body.requested_image_count != snapshot.runtime_options.image_count
        || !(1..=10).contains(&body.requested_image_count)
        || body.body_text.len() as u64 > FileResponseArtifactStore::MAX_RESPONSE_BODY_BYTES
    {
        return Err(GenerationExecutionError::response_artifact());
    }
    let expected_path = artifact_store.expected_response_path(&snapshot.context)?;
    let expected_size = body.body_text.len() as u64;
    let expected_hash = FileResponseArtifactStore::response_hash(&body.body_text);
    let persisted_response = artifact_store
        .persist_verified_response(&snapshot.context, body)
        .await?;
    if !verified_response_shape_matches(
        &persisted_response,
        snapshot.runtime_options.image_count,
        Some(&expected_path),
        None,
    ) || persisted_response.response_size != expected_size
        || persisted_response.response_sha256 != expected_hash
    {
        return Err(GenerationExecutionError::response_artifact());
    }
    let loaded_response = artifact_store
        .load_verified_response(&snapshot.context, &expected_path)
        .await?;
    if !verified_response_shape_matches(
        &loaded_response,
        snapshot.runtime_options.image_count,
        Some(&expected_path),
        None,
    ) || loaded_response.response_size != expected_size
        || loaded_response.response_sha256 != expected_hash
        || loaded_response.response_file != persisted_response.response_file
        || loaded_response.response_size != persisted_response.response_size
        || loaded_response.response_sha256 != persisted_response.response_sha256
        || loaded_response.requested_image_count != persisted_response.requested_image_count
        || loaded_response.body_text != persisted_response.body_text
    {
        return Err(GenerationExecutionError::response_artifact());
    }
    Ok(loaded_response)
}

fn lifecycle_database_error(context: &str, error: rusqlite::Error) -> AppError {
    AppError::Database {
        message: format!("{context}: {error}"),
    }
}

fn lifecycle_lock_error() -> AppError {
    AppError::Database {
        message: "Lock generation lifecycle database failed".to_string(),
    }
}

fn repository_execution_error(error: &AppError, stage: &str) -> GenerationExecutionError {
    let sanitized_message = match error.stable_code() {
        "database_error" => "The generation state could not be updated",
        "file_system_error" => "Generated images could not be saved",
        "generation_job_invalid_transition" => "The generation state changed before completion",
        "generation_job_invalid_snapshot" | "generation_job_corrupt_persisted_data" => {
            "The generation state is invalid"
        }
        _ => "The generation could not be completed",
    };
    GenerationExecutionError::Local {
        code: error.stable_code().to_string(),
        sanitized_message: sanitized_message.to_string(),
        stage: stage.to_string(),
    }
}

fn load_matching_execution_snapshot(
    tx: &Transaction<'_>,
    context: &GenerationExecutionContext,
) -> Result<GenerationExecutionSnapshot, AppError> {
    let job = get_job_in_transaction(tx, &context.job_id)?;
    if !matches!(
        job.stage,
        GenerationJobStage::Preparing
            | GenerationJobStage::ProviderRequest
            | GenerationJobStage::ResponseReady
            | GenerationJobStage::LocalProcessing
    ) {
        return Err(AppError::GenerationJobInvalidTransition);
    }
    let snapshot = load_generation_execution_snapshot_for_stage_in_transaction(
        tx,
        &context.job_id,
        job.stage,
    )?;
    if snapshot.context != *context {
        return Err(AppError::GenerationJobInvalidSnapshot);
    }
    Ok(snapshot)
}

fn lifecycle_worker_timestamp(now_ms: i64) -> Result<String, WorkerTransitionError> {
    if now_ms < 0 {
        return Err(WorkerLeaseError::InvalidTiming.into());
    }
    chrono::DateTime::<chrono::Utc>::from_timestamp_millis(now_ms)
        .map(|timestamp| timestamp.to_rfc3339_opts(chrono::SecondsFormat::Secs, true))
        .ok_or_else(|| WorkerLeaseError::TimeOverflow.into())
}

fn promote_verified_response_in_transaction(
    tx: &Transaction<'_>,
    context: &GenerationExecutionContext,
    response: &VerifiedResponseCommit,
) -> Result<(), AppError> {
    let snapshot = load_matching_execution_snapshot(tx, context)?;
    if !response.matches_context(context)
        || response.requested_image_count != snapshot.runtime_options.image_count
    {
        return Err(AppError::GenerationJobInvalidSnapshot);
    }
    let (request_state, response_file): (String, Option<String>) = tx
        .query_row(
            "SELECT request_state, response_file FROM generation_recoveries
             WHERE generation_id = ?1",
            params![context.generation_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(|error| lifecycle_database_error("Read generation recovery failed", error))?
        .ok_or(AppError::GenerationJobCorruptPersistedData)?;
    if request_state != RECOVERY_STATE_REQUESTING || response_file.is_some() {
        return Err(AppError::GenerationJobInvalidTransition);
    }

    let updated = tx
        .execute(
            "UPDATE generation_recoveries
             SET request_state = ?1, response_file = ?2, updated_at = ?3
             WHERE generation_id = ?4 AND request_state = ?5 AND response_file IS NULL",
            params![
                RECOVERY_STATE_RESPONSE_READY,
                response.response_file,
                current_timestamp(),
                context.generation_id,
                RECOVERY_STATE_REQUESTING,
            ],
        )
        .map_err(|error| lifecycle_database_error("Promote verified response failed", error))?;
    if updated != 1 {
        return Err(AppError::GenerationJobInvalidTransition);
    }
    let job = get_job_in_transaction(tx, &context.job_id)?;
    if job.status != GenerationJobStatus::Running || job.generation_id != context.generation_id {
        return Err(AppError::GenerationJobInvalidTransition);
    }
    if job.stage == GenerationJobStage::ProviderRequest {
        let updated = tx.execute("UPDATE generation_recoveries SET response_size=?1,response_sha256=?2 WHERE generation_id=?3 AND expected_response_file=?4", params![response.response_size, response.response_sha256, context.generation_id, response.response_file])?;
        if updated != 1 {
            return Err(AppError::GenerationJobInvalidTransition);
        }
        tx.execute("UPDATE generation_jobs SET stage='local_processing',last_heartbeat_at=?1 WHERE id=?2 AND stage='provider_request'", params![current_timestamp(), context.job_id])?;
        validate_worker_recovery_for_stage(
            tx,
            &context.generation_id,
            GenerationJobStage::LocalProcessing,
        )?;
    }
    Ok(())
}

fn promote_verified_response(
    db: &Database,
    context: &GenerationExecutionContext,
    expected_path: &Path,
    response: &ProviderAttemptResponse,
) -> Result<(), AppError> {
    promote_verified_response_with_optional_observer(db, context, expected_path, response, None)
}

#[cfg(test)]
fn promote_verified_response_with_observer(
    db: &Database,
    context: &GenerationExecutionContext,
    expected_path: &Path,
    response: &ProviderAttemptResponse,
    observer: &dyn ResponseVerificationObserver,
) -> Result<(), AppError> {
    promote_verified_response_with_optional_observer(
        db,
        context,
        expected_path,
        response,
        Some(observer),
    )
}

fn promote_verified_response_with_optional_observer(
    db: &Database,
    context: &GenerationExecutionContext,
    expected_path: &Path,
    response: &ProviderAttemptResponse,
    observer: Option<&dyn ResponseVerificationObserver>,
) -> Result<(), AppError> {
    let response = VerifiedResponseCommit::verify(context, expected_path, response, observer)?;
    let mut conn = db.conn.lock().map_err(|_| lifecycle_lock_error())?;
    let tx = begin_generation_job_write_transaction(&mut conn)?;
    promote_verified_response_in_transaction(&tx, context, &response)?;
    tx.commit()
        .map_err(|error| lifecycle_database_error("Commit verified response failed", error))
}

#[cfg(test)]
fn promote_verified_response_fenced_with_optional_observer(
    db: &Database,
    artifact_store: &FileResponseArtifactStore,
    context: &GenerationExecutionContext,
    response: &ProviderAttemptResponse,
    authority: &WorkerTransitionAuthority,
    now_ms: i64,
    observer: Option<&dyn ResponseVerificationObserver>,
) -> Result<GenerationJobEvent, WorkerTransitionError> {
    promote_verified_response_fenced_with_optional_observer_and_transaction_time(
        db,
        artifact_store,
        context,
        response,
        authority,
        || now_ms,
        observer,
    )
}

fn promote_verified_response_fenced_with_optional_observer_and_transaction_time<Now>(
    db: &Database,
    artifact_store: &FileResponseArtifactStore,
    context: &GenerationExecutionContext,
    response: &ProviderAttemptResponse,
    authority: &WorkerTransitionAuthority,
    now: Now,
    observer: Option<&dyn ResponseVerificationObserver>,
) -> Result<GenerationJobEvent, WorkerTransitionError>
where
    Now: FnOnce() -> i64,
{
    let response =
        VerifiedResponseCommit::verify_with_store(context, artifact_store, response, observer)?;
    let response_size = i64::try_from(response.response_size)
        .map_err(|_| AppError::GenerationJobInvalidSnapshot)?;
    let mut conn = db.conn.lock().map_err(|_| lifecycle_lock_error())?;
    let tx = begin_generation_job_write_transaction(&mut conn)?;
    let now_ms = now();
    let timestamp = lifecycle_worker_timestamp(now_ms)?;

    // This must remain the first database operation after BEGIN IMMEDIATE and
    // the side-effect-free transaction-time sample.
    assert_worker_transition_authority_in_transaction(&tx, authority, now_ms)?;

    let snapshot = load_generation_execution_snapshot_for_stage_in_transaction(
        &tx,
        &context.job_id,
        GenerationJobStage::ProviderRequest,
    )?;
    if snapshot.context != *context
        || !response.matches_context(context)
        || response.requested_image_count != snapshot.runtime_options.image_count
    {
        return Err(AppError::GenerationJobInvalidSnapshot.into());
    }

    let expected_response_file = response.response_file.as_str();
    let updated_recovery = tx
        .execute(
            "UPDATE generation_recoveries
                SET request_state = ?1,
                    response_file = ?2,
                    response_size = ?3,
                    response_sha256 = ?4,
                    updated_at = ?5
              WHERE generation_id = ?6
                AND request_state = ?7
                AND expected_response_file = ?2
                AND response_file IS NULL
                AND response_size IS NULL
                AND response_sha256 IS NULL
                AND updated_at <= ?5",
            params![
                RECOVERY_STATE_RESPONSE_READY,
                response.response_file,
                response_size,
                response.response_sha256,
                timestamp,
                context.generation_id,
                RECOVERY_STATE_REQUESTING,
            ],
        )
        .map_err(|error| {
            lifecycle_database_error("Promote fenced verified response failed", error)
        })?;
    if updated_recovery != 1 {
        return Err(AppError::GenerationJobInvalidTransition.into());
    }

    let updated_job = tx
        .execute(
            "UPDATE generation_jobs
                SET stage = 'response_ready', last_heartbeat_at = ?1
              WHERE id = ?2
                AND generation_id = ?3
                AND status = 'running'
                AND stage = 'provider_request'
                AND started_at IS NOT NULL
                AND started_at <= ?1
                AND last_heartbeat_at IS NOT NULL
                AND last_heartbeat_at <= ?1",
            params![timestamp, context.job_id, context.generation_id],
        )
        .map_err(|error| lifecycle_database_error("Advance fenced response stage failed", error))?;
    if updated_job != 1 {
        return Err(AppError::GenerationJobInvalidTransition.into());
    }

    let persisted = tx
        .query_row(
            "SELECT r.request_state, r.expected_response_file, r.response_file,
                    r.response_size, r.response_sha256, r.updated_at,
                    j.status, j.stage, j.last_heartbeat_at
               FROM generation_recoveries r
               JOIN generation_jobs j ON j.generation_id = r.generation_id
              WHERE r.generation_id = ?1 AND j.id = ?2",
            params![context.generation_id, context.job_id],
            |row| {
                Ok(PersistedFencedResponseProjection {
                    request_state: row.get(0)?,
                    expected_response_file: row.get(1)?,
                    response_file: row.get(2)?,
                    response_size: row.get(3)?,
                    response_sha256: row.get(4)?,
                    recovery_updated_at: row.get(5)?,
                    job_status: row.get(6)?,
                    job_stage: row.get(7)?,
                    last_heartbeat_at: row.get(8)?,
                })
            },
        )
        .optional()
        .map_err(|error| {
            lifecycle_database_error("Verify fenced response promotion failed", error)
        })?
        .ok_or(AppError::GenerationJobCorruptPersistedData)?;
    if persisted.request_state != RECOVERY_STATE_RESPONSE_READY
        || persisted.expected_response_file.as_deref() != Some(expected_response_file)
        || persisted.response_file.as_deref() != Some(response.response_file.as_str())
        || persisted.response_size != Some(response_size)
        || persisted.response_sha256.as_deref() != Some(response.response_sha256.as_str())
        || persisted.recovery_updated_at != timestamp
        || persisted.job_status != "running"
        || persisted.job_stage != "response_ready"
        || persisted.last_heartbeat_at.as_deref() != Some(timestamp.as_str())
    {
        return Err(AppError::GenerationJobCorruptPersistedData.into());
    }
    validate_worker_recovery_for_stage(
        &tx,
        &context.generation_id,
        GenerationJobStage::ResponseReady,
    )?;

    let event = get_job_event_in_transaction(&tx, &context.job_id)?;
    tx.commit().map_err(|error| {
        lifecycle_database_error("Commit fenced verified response failed", error)
    })?;
    Ok(event)
}

#[cfg(test)]
pub(crate) fn promote_verified_response_fenced(
    db: &Database,
    artifact_store: &FileResponseArtifactStore,
    context: &GenerationExecutionContext,
    response: &ProviderAttemptResponse,
    authority: &WorkerTransitionAuthority,
    now_ms: i64,
) -> Result<GenerationJobEvent, WorkerTransitionError> {
    promote_verified_response_fenced_with_transaction_time(
        db,
        artifact_store,
        context,
        response,
        authority,
        || now_ms,
    )
}

/// Verifies the response artifact without holding the database lock, then
/// samples worker time exactly once after the immediate write transaction is
/// held and before the first authority read.
pub(crate) fn promote_verified_response_fenced_with_transaction_time<Now>(
    db: &Database,
    artifact_store: &FileResponseArtifactStore,
    context: &GenerationExecutionContext,
    response: &ProviderAttemptResponse,
    authority: &WorkerTransitionAuthority,
    now: Now,
) -> Result<GenerationJobEvent, WorkerTransitionError>
where
    Now: FnOnce() -> i64,
{
    // The production adapter must pass its managed app-owned store. Unlike the
    // compatibility API, this fenced path never accepts a caller-selected path.
    promote_verified_response_fenced_with_optional_observer_and_transaction_time(
        db,
        artifact_store,
        context,
        response,
        authority,
        now,
        None,
    )
}

fn finalize_generation_failure_in_transaction(
    tx: &Transaction<'_>,
    context: &GenerationExecutionContext,
    error: &GenerationExecutionError,
    disposition: &GenerationTerminalDisposition,
) -> Result<(), AppError> {
    if error.code() != disposition.error_code
        || !matches!(
            disposition.status,
            GenerationJobStatus::Failed
                | GenerationJobStatus::Interrupted
                | GenerationJobStatus::Cancelled
        )
        || (disposition.status == GenerationJobStatus::Cancelled
            && (disposition.error_code != "cancelled_by_user" || disposition.retryable))
    {
        return Err(AppError::GenerationJobInvalidSnapshot);
    }
    load_matching_execution_snapshot(tx, context)?;
    let (request_state, response_file): (String, Option<String>) = tx
        .query_row(
            "SELECT request_state, response_file FROM generation_recoveries
             WHERE generation_id = ?1",
            params![context.generation_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(|query_error| {
            lifecycle_database_error("Read terminal generation recovery failed", query_error)
        })?
        .ok_or(AppError::GenerationJobCorruptPersistedData)?;
    let response_ready = request_state == RECOVERY_STATE_RESPONSE_READY && response_file.is_some();
    let requesting = request_state == RECOVERY_STATE_REQUESTING && response_file.is_none();
    if !response_ready && !requesting {
        return Err(AppError::GenerationJobCorruptPersistedData);
    }
    if disposition.preserve_response_ready {
        let bound_request = requesting && tx.query_row("SELECT expected_response_file IS NOT NULL FROM generation_recoveries WHERE generation_id=?1", [&context.generation_id], |row| row.get::<_,bool>(0))?
            && get_job_in_transaction(tx, &context.job_id)?.stage == GenerationJobStage::ProviderRequest;
        if !response_ready && !bound_request {
            return Err(AppError::GenerationJobInvalidTransition);
        }
    } else {
        let deleted = tx
            .execute(
                "DELETE FROM generation_recoveries WHERE generation_id = ?1",
                params![context.generation_id],
            )
            .map_err(|query_error| {
                lifecycle_database_error("Delete terminal generation recovery failed", query_error)
            })?;
        if deleted != 1 {
            return Err(AppError::GenerationJobCorruptPersistedData);
        }
    }

    finish_job_in_transaction(
        tx,
        &GenerationJobTerminalUpdate {
            job_id: context.job_id.clone(),
            expected_status: GenerationJobStatus::Running,
            status: disposition.status.clone(),
            finished_at: current_timestamp(),
            error_code: Some(disposition.error_code.clone()),
            error_message: None,
            retryable: disposition.retryable,
        },
    )?;
    Ok(())
}

#[cfg(test)]
fn finalize_generation_failure(
    db: &Database,
    context: &GenerationExecutionContext,
    error: &GenerationExecutionError,
    disposition: &GenerationTerminalDisposition,
) -> Result<(), AppError> {
    let mut conn = db.conn.lock().map_err(|_| lifecycle_lock_error())?;
    let tx = begin_generation_job_write_transaction(&mut conn)?;
    finalize_generation_failure_in_transaction(&tx, context, error, disposition)?;
    tx.commit().map_err(|query_error| {
        lifecycle_database_error("Commit generation failure failed", query_error)
    })
}

#[cfg(test)]
pub(crate) fn finalize_generation_failure_fenced(
    db: &Database,
    context: &GenerationExecutionContext,
    error: &GenerationExecutionError,
    disposition: &GenerationTerminalDisposition,
    authority: &WorkerTransitionAuthority,
    now_ms: i64,
) -> Result<GenerationJobEvent, WorkerTransitionError> {
    finalize_generation_failure_fenced_with_transaction_time(
        db,
        context,
        error,
        disposition,
        authority,
        || now_ms,
    )
}

/// Samples worker time exactly once after the immediate write transaction is
/// held, while preserving the terminal projection's existing rollback rules.
pub(crate) fn finalize_generation_failure_fenced_with_transaction_time<Now>(
    db: &Database,
    context: &GenerationExecutionContext,
    error: &GenerationExecutionError,
    disposition: &GenerationTerminalDisposition,
    authority: &WorkerTransitionAuthority,
    now: Now,
) -> Result<GenerationJobEvent, WorkerTransitionError>
where
    Now: FnOnce() -> i64,
{
    if error.code() != disposition.error_code
        || !matches!(
            disposition.status,
            GenerationJobStatus::Failed | GenerationJobStatus::Interrupted
        )
    {
        return Err(AppError::GenerationJobInvalidSnapshot.into());
    }
    let event = {
        let mut conn = db.conn.lock().map_err(|_| lifecycle_lock_error())?;
        let tx = begin_generation_job_write_transaction(&mut conn)?;
        let now_ms = now();
        let timestamp = lifecycle_worker_timestamp(now_ms)?;

        // This must remain the first database operation after BEGIN IMMEDIATE
        // and the side-effect-free transaction-time sample.
        assert_worker_transition_authority_in_transaction(&tx, authority, now_ms)?;

        let job = get_job_in_transaction(&tx, &context.job_id)?;
        let expected_stage = match (job.stage, disposition.preserve_response_ready) {
            (GenerationJobStage::Preparing, false)
            | (GenerationJobStage::ProviderRequest, false)
            | (GenerationJobStage::ResponseReady, false)
            | (GenerationJobStage::LocalProcessing, true) => job.stage,
            _ => return Err(AppError::GenerationJobInvalidTransition.into()),
        };
        let snapshot = load_generation_execution_snapshot_for_stage_in_transaction(
            &tx,
            &context.job_id,
            expected_stage,
        )?;
        if snapshot.context != *context
            || job.generation_id != context.generation_id
            || job.cancel_requested_at.is_some()
            || !terminal_timestamp_not_before_job(&job, &timestamp, false)
        {
            return Err(AppError::GenerationJobInvalidTransition.into());
        }
        validate_worker_recovery_for_stage(&tx, &context.generation_id, expected_stage)?;
        let recovery_before = load_terminal_recovery_projection(&tx, &context.generation_id)?
            .ok_or(AppError::GenerationJobCorruptPersistedData)?;
        validate_terminal_recovery_timestamp(&recovery_before, &timestamp)?;
        let (generation_status_before, generation_error_before, request_metadata_before): (
            String,
            Option<String>,
            String,
        ) = tx
            .query_row(
                "SELECT status, error_message, request_metadata
                   FROM generations WHERE id = ?1",
                params![context.generation_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()
            .map_err(|query_error| {
                lifecycle_database_error(
                    "Read fenced failure generation projection failed",
                    query_error,
                )
            })?
            .ok_or(AppError::GenerationJobCorruptPersistedData)?;
        let existing_images: i64 = tx
            .query_row(
                "SELECT COUNT(*) FROM images WHERE generation_id = ?1",
                params![context.generation_id],
                |row| row.get(0),
            )
            .map_err(|query_error| {
                lifecycle_database_error(
                    "Count fenced failed generation images failed",
                    query_error,
                )
            })?;
        if generation_status_before != "running"
            || generation_error_before.is_some()
            || existing_images != 0
        {
            return Err(AppError::GenerationJobCorruptPersistedData.into());
        }

        if !disposition.preserve_response_ready {
            let deleted = tx
                .execute(
                    "DELETE FROM generation_recoveries WHERE generation_id = ?1",
                    params![context.generation_id],
                )
                .map_err(|query_error| {
                    lifecycle_database_error("Delete fenced failure recovery failed", query_error)
                })?;
            if deleted != 1 {
                return Err(AppError::GenerationJobCorruptPersistedData.into());
            }
        }
        finish_job_in_transaction(
            &tx,
            &GenerationJobTerminalUpdate {
                job_id: context.job_id.clone(),
                expected_status: GenerationJobStatus::Running,
                status: disposition.status.clone(),
                finished_at: timestamp.clone(),
                error_code: Some(disposition.error_code.clone()),
                error_message: None,
                retryable: disposition.retryable,
            },
        )?;

        let mut expected_job = job.clone();
        expected_job.status = disposition.status.clone();
        expected_job.stage = GenerationJobStage::Terminal;
        expected_job.finished_at = Some(timestamp.clone());
        expected_job.error_code = Some(disposition.error_code.clone());
        expected_job.error_message =
            Some(terminal_message_for_code(&disposition.error_code).to_string());
        expected_job.retryable = disposition.retryable;
        let persisted_job = get_job_in_transaction(&tx, &context.job_id)?;
        let expected_status = match disposition.status {
            GenerationJobStatus::Failed => "failed",
            GenerationJobStatus::Interrupted => "interrupted",
            _ => return Err(AppError::GenerationJobInvalidSnapshot.into()),
        };
        let (generation_status, generation_error, request_metadata, image_count): (
            String,
            Option<String>,
            String,
            i64,
        ) = tx
            .query_row(
                "SELECT g.status, g.error_message, g.request_metadata,
                        (SELECT COUNT(*) FROM images i WHERE i.generation_id = g.id)
                   FROM generations g WHERE g.id = ?1",
                params![context.generation_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .optional()
            .map_err(|query_error| {
                lifecycle_database_error("Verify fenced failure projection failed", query_error)
            })?
            .ok_or(AppError::GenerationJobCorruptPersistedData)?;
        let recovery_after = load_terminal_recovery_projection(&tx, &context.generation_id)?;
        let recovery_is_exact = if disposition.preserve_response_ready {
            recovery_after.as_ref() == Some(&recovery_before)
        } else {
            recovery_after.is_none()
        };
        if persisted_job != expected_job
            || generation_status != expected_status
            || generation_error.as_deref()
                != Some(terminal_message_for_code(&disposition.error_code))
            || request_metadata != request_metadata_before
            || image_count != 0
            || !recovery_is_exact
        {
            return Err(AppError::GenerationJobCorruptPersistedData.into());
        }

        let event = get_job_event_in_transaction(&tx, &context.job_id)?;
        if event.status != disposition.status
            || event.stage != GenerationJobStage::Terminal
            || event.finished_at.as_deref() != Some(timestamp.as_str())
            || event.cancel_requested_at.is_some()
            || event.error_code.as_deref() != Some(disposition.error_code.as_str())
            || event.error_message.as_deref()
                != Some(terminal_message_for_code(&disposition.error_code))
            || event.retryable != disposition.retryable
        {
            return Err(AppError::GenerationJobCorruptPersistedData.into());
        }
        tx.commit().map_err(|query_error| {
            lifecycle_database_error("Commit fenced generation failure failed", query_error)
        })?;
        event
    };
    Ok(event)
}

fn generation_image_projection_is_valid(image: &GeneratedImage, generation_id: &str) -> bool {
    let id_is_valid = !image.id.is_empty()
        && image.id.len() <= 128
        && image
            .id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'));
    let path_is_valid = |path: &str| {
        !path.is_empty() && path.len() <= 32_768 && !path.chars().any(char::is_control)
    };
    image.generation_id == generation_id
        && id_is_valid
        && path_is_valid(&image.file_path)
        && path_is_valid(&image.thumbnail_path)
        && image.width > 0
        && image.height > 0
        && image.file_size > 0
}

impl<'a> PromotedImageCommit<'a> {
    fn from_promoted(
        promoted: &'a PromotedGenerationFiles,
        generation_id: &str,
    ) -> Result<Self, AppError> {
        if !promoted.matches_generation(generation_id) {
            return Err(AppError::GenerationJobInvalidSnapshot);
        }
        let images = promoted.saved_images();
        let mut image_ids = HashSet::with_capacity(images.len());
        if images.is_empty()
            || images.len() > 10
            || images.iter().any(|image| {
                !generation_image_projection_is_valid(image, generation_id)
                    || !image_ids.insert(image.id.as_str())
            })
        {
            return Err(AppError::GenerationJobInvalidSnapshot);
        }
        Ok(Self {
            generation_id: generation_id.to_string(),
            images,
            _guard: PhantomData,
        })
    }

    fn images(&self) -> &[GeneratedImage] {
        &self.images
    }
}

fn commit_generation_success_in_transaction(
    tx: &Transaction<'_>,
    context: &GenerationExecutionContext,
    request: &GenerationJobRequest,
    promoted: &PromotedImageCommit<'_>,
) -> Result<GenerationSuccessTransition, AppError> {
    let snapshot = load_matching_execution_snapshot(tx, context)?;
    commit_generation_success_projection_in_transaction(
        tx,
        context,
        request,
        promoted,
        &snapshot,
        current_timestamp(),
    )
}

fn commit_generation_success_projection_in_transaction(
    tx: &Transaction<'_>,
    context: &GenerationExecutionContext,
    request: &GenerationJobRequest,
    promoted: &PromotedImageCommit<'_>,
    snapshot: &GenerationExecutionSnapshot,
    finished_at: String,
) -> Result<GenerationSuccessTransition, AppError> {
    if snapshot.request != *request || promoted.generation_id != context.generation_id {
        return Err(AppError::GenerationJobInvalidSnapshot);
    }
    let job = get_job_in_transaction(tx, &context.job_id)?;
    if job.generation_id != context.generation_id {
        return Err(AppError::GenerationJobInvalidSnapshot);
    }
    if job.cancel_requested_at.is_some() {
        return Ok(GenerationSuccessTransition::CancelRequested);
    }
    let requested_image_count = snapshot.runtime_options.image_count;
    let images = promoted.images();
    let actual_image_count =
        u8::try_from(images.len()).map_err(|_| AppError::GenerationJobInvalidSnapshot)?;
    if !(1..=requested_image_count).contains(&actual_image_count) {
        return Err(AppError::GenerationJobInvalidSnapshot);
    }

    let (request_state, response_file): (String, Option<String>) = tx
        .query_row(
            "SELECT request_state, response_file FROM generation_recoveries
             WHERE generation_id = ?1",
            params![context.generation_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(|error| {
            lifecycle_database_error("Read successful generation recovery failed", error)
        })?
        .ok_or(AppError::GenerationJobCorruptPersistedData)?;
    if request_state != RECOVERY_STATE_RESPONSE_READY || response_file.is_none() {
        return Err(AppError::GenerationJobInvalidTransition);
    }
    let existing_images: i64 = tx
        .query_row(
            "SELECT COUNT(*) FROM images WHERE generation_id = ?1",
            params![context.generation_id],
            |row| row.get(0),
        )
        .map_err(|error| {
            lifecycle_database_error("Count existing generation images failed", error)
        })?;
    if existing_images != 0 {
        return Err(AppError::GenerationJobCorruptPersistedData);
    }

    for image in images {
        tx.execute(
            "INSERT INTO images (
                id, generation_id, file_path, thumbnail_path, width, height, file_size, created_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                image.id,
                image.generation_id,
                image.file_path,
                image.thumbnail_path,
                image.width,
                image.height,
                image.file_size,
                snapshot.created_at,
            ],
        )
        .map_err(|error| {
            lifecycle_database_error("Insert completed generation image failed", error)
        })?;
    }
    set_actual_image_count_in_transaction(tx, &context.generation_id, actual_image_count)?;
    let deleted_recovery = tx
        .execute(
            "DELETE FROM generation_recoveries
             WHERE generation_id = ?1 AND request_state = ?2 AND response_file IS NOT NULL",
            params![context.generation_id, RECOVERY_STATE_RESPONSE_READY],
        )
        .map_err(|error| {
            lifecycle_database_error("Delete completed generation recovery failed", error)
        })?;
    if deleted_recovery != 1 {
        return Err(AppError::GenerationJobCorruptPersistedData);
    }
    finish_job_in_transaction(
        tx,
        &GenerationJobTerminalUpdate {
            job_id: context.job_id.clone(),
            expected_status: GenerationJobStatus::Running,
            status: GenerationJobStatus::Completed,
            finished_at,
            error_code: None,
            error_message: None,
            retryable: false,
        },
    )?;
    Ok(GenerationSuccessTransition::Completed(GenerateResult {
        generation_id: context.generation_id.clone(),
        conversation_id: context.conversation_id.clone(),
        images: images.to_vec(),
    }))
}

fn terminal_timestamp_not_before_job(
    job: &GenerationJob,
    timestamp: &str,
    require_cancel_request: bool,
) -> bool {
    job.started_at
        .as_deref()
        .is_some_and(|started_at| timestamp >= started_at)
        && job
            .last_heartbeat_at
            .as_deref()
            .is_some_and(|heartbeat_at| timestamp >= heartbeat_at)
        && (!require_cancel_request
            || job
                .cancel_requested_at
                .as_deref()
                .is_some_and(|cancel_requested_at| timestamp >= cancel_requested_at))
}

fn load_terminal_recovery_projection(
    tx: &Transaction<'_>,
    generation_id: &str,
) -> Result<Option<TerminalRecoveryProjection>, AppError> {
    tx.query_row(
        "SELECT request_kind, request_state, output_format, expected_response_file,
                response_file, response_size, response_sha256, created_at, updated_at
           FROM generation_recoveries WHERE generation_id = ?1",
        params![generation_id],
        |row| {
            Ok(TerminalRecoveryProjection {
                request_kind: row.get(0)?,
                request_state: row.get(1)?,
                output_format: row.get(2)?,
                expected_response_file: row.get(3)?,
                response_file: row.get(4)?,
                response_size: row.get(5)?,
                response_sha256: row.get(6)?,
                created_at: row.get(7)?,
                updated_at: row.get(8)?,
            })
        },
    )
    .optional()
    .map_err(|error| lifecycle_database_error("Read fenced terminal recovery failed", error))
}

fn validate_terminal_recovery_timestamp(
    recovery: &TerminalRecoveryProjection,
    terminal_timestamp: &str,
) -> Result<(), AppError> {
    let parse_canonical = |value: &str| {
        chrono::DateTime::parse_from_rfc3339(value)
            .ok()
            .filter(|timestamp| {
                timestamp
                    .with_timezone(&chrono::Utc)
                    .to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
                    == value
            })
    };
    let created_at =
        parse_canonical(&recovery.created_at).ok_or(AppError::GenerationJobCorruptPersistedData)?;
    let updated_at =
        parse_canonical(&recovery.updated_at).ok_or(AppError::GenerationJobCorruptPersistedData)?;
    let terminal_at =
        parse_canonical(terminal_timestamp).ok_or(AppError::GenerationJobInvalidTransition)?;
    if updated_at < created_at {
        return Err(AppError::GenerationJobCorruptPersistedData);
    }
    if terminal_at < updated_at {
        return Err(AppError::GenerationJobInvalidTransition);
    }
    Ok(())
}

#[cfg(test)]
pub(crate) fn acknowledge_generation_cancellation_fenced(
    db: &Database,
    context: &GenerationExecutionContext,
    authority: &WorkerTransitionAuthority,
    now_ms: i64,
) -> Result<GenerationJobEvent, WorkerTransitionError> {
    acknowledge_generation_cancellation_fenced_with_transaction_time(db, context, authority, || {
        now_ms
    })
}

/// Samples worker time exactly once after the immediate write transaction is
/// held and before cancellation authority or durable job state is read.
pub(crate) fn acknowledge_generation_cancellation_fenced_with_transaction_time<Now>(
    db: &Database,
    context: &GenerationExecutionContext,
    authority: &WorkerTransitionAuthority,
    now: Now,
) -> Result<GenerationJobEvent, WorkerTransitionError>
where
    Now: FnOnce() -> i64,
{
    let event = {
        let mut conn = db.conn.lock().map_err(|_| lifecycle_lock_error())?;
        let tx = begin_generation_job_write_transaction(&mut conn)?;
        let now_ms = now();
        let timestamp = lifecycle_worker_timestamp(now_ms)?;

        // This must remain the first database operation after BEGIN IMMEDIATE
        // and the side-effect-free transaction-time sample.
        assert_worker_transition_authority_in_transaction(&tx, authority, now_ms)?;

        let job = get_job_in_transaction(&tx, &context.job_id)?;
        let expected_stage = match job.stage {
            GenerationJobStage::Preparing
            | GenerationJobStage::ProviderRequest
            | GenerationJobStage::RetryBackoff
            | GenerationJobStage::ResponseReady
            | GenerationJobStage::LocalProcessing => job.stage,
            _ => return Err(AppError::GenerationJobInvalidTransition.into()),
        };
        let snapshot = load_generation_execution_snapshot_for_stage_in_transaction(
            &tx,
            &context.job_id,
            expected_stage,
        )?;
        if snapshot.context != *context
            || job.generation_id != context.generation_id
            || job.cancel_requested_at.is_none()
            || !terminal_timestamp_not_before_job(&job, &timestamp, true)
        {
            return Err(AppError::GenerationJobInvalidTransition.into());
        }
        validate_worker_recovery_for_stage(&tx, &context.generation_id, expected_stage)?;
        let recovery = load_terminal_recovery_projection(&tx, &context.generation_id)?
            .ok_or(AppError::GenerationJobCorruptPersistedData)?;
        validate_terminal_recovery_timestamp(&recovery, &timestamp)?;
        let (generation_status_before, generation_error_before, request_metadata_before): (
            String,
            Option<String>,
            String,
        ) = tx
            .query_row(
                "SELECT status, error_message, request_metadata
                   FROM generations WHERE id = ?1",
                params![context.generation_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()
            .map_err(|error| {
                lifecycle_database_error("Read fenced cancelled generation failed", error)
            })?
            .ok_or(AppError::GenerationJobCorruptPersistedData)?;
        let existing_images: i64 = tx
            .query_row(
                "SELECT COUNT(*) FROM images WHERE generation_id = ?1",
                params![context.generation_id],
                |row| row.get(0),
            )
            .map_err(|error| {
                lifecycle_database_error("Count fenced cancelled generation images failed", error)
            })?;
        if generation_status_before != "running"
            || generation_error_before.is_some()
            || existing_images != 0
        {
            return Err(AppError::GenerationJobCorruptPersistedData.into());
        }

        let deleted = tx
            .execute(
                "DELETE FROM generation_recoveries WHERE generation_id = ?1",
                params![context.generation_id],
            )
            .map_err(|error| {
                lifecycle_database_error("Delete fenced cancelled recovery failed", error)
            })?;
        if deleted != 1 {
            return Err(AppError::GenerationJobCorruptPersistedData.into());
        }
        finish_job_in_transaction(
            &tx,
            &GenerationJobTerminalUpdate {
                job_id: context.job_id.clone(),
                expected_status: GenerationJobStatus::Running,
                status: GenerationJobStatus::Cancelled,
                finished_at: timestamp.clone(),
                error_code: Some("cancelled_by_user".to_string()),
                error_message: None,
                retryable: false,
            },
        )?;

        let mut expected_job = job.clone();
        expected_job.status = GenerationJobStatus::Cancelled;
        expected_job.stage = GenerationJobStage::Terminal;
        expected_job.finished_at = Some(timestamp.clone());
        expected_job.error_code = Some("cancelled_by_user".to_string());
        expected_job.error_message = Some("The operation was cancelled".to_string());
        expected_job.retryable = false;
        if get_job_in_transaction(&tx, &context.job_id)? != expected_job {
            return Err(AppError::GenerationJobCorruptPersistedData.into());
        }
        let event = get_job_event_in_transaction(&tx, &context.job_id)?;
        let (generation_status, generation_error, request_metadata, image_count): (
            String,
            Option<String>,
            String,
            i64,
        ) = tx
            .query_row(
                "SELECT g.status, g.error_message,
                        g.request_metadata,
                        (SELECT COUNT(*) FROM images i WHERE i.generation_id = g.id)
                   FROM generations g WHERE g.id = ?1",
                params![context.generation_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .optional()
            .map_err(|error| {
                lifecycle_database_error("Verify fenced cancelled generation failed", error)
            })?
            .ok_or(AppError::GenerationJobCorruptPersistedData)?;
        if load_terminal_recovery_projection(&tx, &context.generation_id)?.is_some()
            || generation_status != "cancelled"
            || generation_error.as_deref() != Some("The operation was cancelled")
            || request_metadata != request_metadata_before
            || image_count != 0
            || event.status != GenerationJobStatus::Cancelled
            || event.stage != GenerationJobStage::Terminal
            || event.finished_at.as_deref() != Some(timestamp.as_str())
            || event.cancel_requested_at != job.cancel_requested_at
            || event.error_code.as_deref() != Some("cancelled_by_user")
            || event.error_message.as_deref() != Some("The operation was cancelled")
            || event.retryable
        {
            return Err(AppError::GenerationJobCorruptPersistedData.into());
        }
        tx.commit().map_err(|error| {
            lifecycle_database_error("Commit fenced generation cancellation failed", error)
        })?;
        event
    };
    Ok(event)
}

fn verify_fenced_success_projection_in_transaction(
    tx: &Transaction<'_>,
    context: &GenerationExecutionContext,
    snapshot: &GenerationExecutionSnapshot,
    images: &[GeneratedImage],
    job_before: &GenerationJob,
    finished_at: &str,
    request_metadata_before: &str,
) -> Result<(), AppError> {
    let mut expected_job = job_before.clone();
    expected_job.status = GenerationJobStatus::Completed;
    expected_job.stage = GenerationJobStage::Terminal;
    expected_job.finished_at = Some(finished_at.to_string());
    expected_job.error_code = None;
    expected_job.error_message = None;
    expected_job.retryable = false;
    if get_job_in_transaction(tx, &context.job_id)? != expected_job {
        return Err(AppError::GenerationJobCorruptPersistedData);
    }
    let (generation_status, generation_error, request_metadata, recovery_count): (
        String,
        Option<String>,
        String,
        i64,
    ) = tx
        .query_row(
            "SELECT g.status, g.error_message, g.request_metadata,
                    (SELECT COUNT(*) FROM generation_recoveries r
                      WHERE r.generation_id = g.id)
               FROM generations g WHERE g.id = ?1",
            params![context.generation_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .optional()
        .map_err(|error| {
            lifecycle_database_error("Verify fenced successful generation failed", error)
        })?
        .ok_or(AppError::GenerationJobCorruptPersistedData)?;
    let mut metadata: serde_json::Value = serde_json::from_str(&request_metadata)
        .map_err(|_| AppError::GenerationJobCorruptPersistedData)?;
    let metadata_before: serde_json::Value = serde_json::from_str(request_metadata_before)
        .map_err(|_| AppError::GenerationJobCorruptPersistedData)?;
    let actual_image_count = metadata
        .as_object_mut()
        .and_then(|object| object.remove("actual_image_count"))
        .and_then(|value| value.as_u64());
    if generation_status != "completed"
        || generation_error.is_some()
        || recovery_count != 0
        || actual_image_count != Some(images.len() as u64)
        || metadata != metadata_before
    {
        return Err(AppError::GenerationJobCorruptPersistedData);
    }

    let mut statement = tx
        .prepare(
            "SELECT id, generation_id, file_path, thumbnail_path, width, height, file_size,
                    created_at
               FROM images WHERE generation_id = ?1 ORDER BY id",
        )
        .map_err(|error| {
            lifecycle_database_error("Prepare fenced successful image verification failed", error)
        })?;
    let persisted = statement
        .query_map(params![context.generation_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, i32>(4)?,
                row.get::<_, i32>(5)?,
                row.get::<_, i64>(6)?,
                row.get::<_, String>(7)?,
            ))
        })
        .map_err(|error| lifecycle_database_error("Query fenced successful images failed", error))?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|error| lifecycle_database_error("Read fenced successful images failed", error))?;
    let mut expected = images
        .iter()
        .map(|image| {
            (
                image.id.clone(),
                image.generation_id.clone(),
                image.file_path.clone(),
                image.thumbnail_path.clone(),
                image.width,
                image.height,
                image.file_size,
                snapshot.created_at.clone(),
            )
        })
        .collect::<Vec<_>>();
    expected.sort_by(|left, right| left.0.cmp(&right.0));
    if persisted != expected {
        return Err(AppError::GenerationJobCorruptPersistedData);
    }
    Ok(())
}

#[cfg(test)]
pub(crate) fn commit_generation_success_fenced(
    db: &Database,
    context: &GenerationExecutionContext,
    request: &GenerationJobRequest,
    promoted: PromotedGenerationFiles,
    authority: &WorkerTransitionAuthority,
    now_ms: i64,
) -> Result<FencedGenerationSuccessTransition, WorkerTransitionError> {
    commit_generation_success_fenced_with_transaction_time(
        db,
        context,
        request,
        promoted,
        authority,
        || now_ms,
    )
}

/// Keeps promoted-file ownership and cleanup outside the database lock while
/// sampling worker time exactly once after the immediate transaction is held.
pub(crate) fn commit_generation_success_fenced_with_transaction_time<Now>(
    db: &Database,
    context: &GenerationExecutionContext,
    request: &GenerationJobRequest,
    mut promoted: PromotedGenerationFiles,
    authority: &WorkerTransitionAuthority,
    now: Now,
) -> Result<FencedGenerationSuccessTransition, WorkerTransitionError>
where
    Now: FnOnce() -> i64,
{
    let promoted_projection =
        PromotedImageCommit::from_promoted(&promoted, &context.generation_id)?;
    let outcome = (|| -> Result<FencedGenerationSuccessTransition, WorkerTransitionError> {
        let mut conn = db.conn.lock().map_err(|_| lifecycle_lock_error())?;
        let tx = begin_generation_job_write_transaction(&mut conn)?;
        let now_ms = now();
        let timestamp = lifecycle_worker_timestamp(now_ms)?;

        // This must remain the first database operation after BEGIN IMMEDIATE
        // and the side-effect-free transaction-time sample.
        assert_worker_transition_authority_in_transaction(&tx, authority, now_ms)?;

        let snapshot = load_generation_execution_snapshot_for_stage_in_transaction(
            &tx,
            &context.job_id,
            GenerationJobStage::LocalProcessing,
        )?;
        if snapshot.context != *context {
            return Err(AppError::GenerationJobInvalidSnapshot.into());
        }
        validate_worker_recovery_for_stage(
            &tx,
            &context.generation_id,
            GenerationJobStage::LocalProcessing,
        )?;
        let recovery = load_terminal_recovery_projection(&tx, &context.generation_id)?
            .ok_or(AppError::GenerationJobCorruptPersistedData)?;
        validate_terminal_recovery_timestamp(&recovery, &timestamp)?;
        let job = get_job_in_transaction(&tx, &context.job_id)?;
        if !terminal_timestamp_not_before_job(&job, &timestamp, false) {
            return Err(AppError::GenerationJobInvalidTransition.into());
        }
        let request_metadata_before: String = tx
            .query_row(
                "SELECT request_metadata FROM generations WHERE id = ?1",
                params![context.generation_id],
                |row| row.get(0),
            )
            .optional()
            .map_err(|error| {
                lifecycle_database_error("Read fenced success metadata failed", error)
            })?
            .ok_or(AppError::GenerationJobCorruptPersistedData)?;

        match commit_generation_success_projection_in_transaction(
            &tx,
            context,
            request,
            &promoted_projection,
            &snapshot,
            timestamp.clone(),
        )? {
            GenerationSuccessTransition::Completed(result) => {
                verify_fenced_success_projection_in_transaction(
                    &tx,
                    context,
                    &snapshot,
                    &result.images,
                    &job,
                    &timestamp,
                    &request_metadata_before,
                )?;
                let event = get_job_event_in_transaction(&tx, &context.job_id)?;
                tx.commit().map_err(|error| {
                    lifecycle_database_error("Commit fenced generation success failed", error)
                })?;
                Ok(FencedGenerationSuccessTransition::Completed {
                    result,
                    event: Box::new(event),
                })
            }
            GenerationSuccessTransition::CancelRequested => {
                tx.rollback().map_err(|error| {
                    lifecycle_database_error("Rollback fenced cancelled success failed", error)
                })?;
                Ok(FencedGenerationSuccessTransition::CancelRequested)
            }
        }
    })();
    drop(promoted_projection);
    match outcome {
        Ok(completed @ FencedGenerationSuccessTransition::Completed { .. }) => {
            promoted.disarm_cleanup();
            Ok(completed)
        }
        Ok(FencedGenerationSuccessTransition::CancelRequested) => {
            drop(promoted);
            Ok(FencedGenerationSuccessTransition::CancelRequested)
        }
        Err(error) => {
            drop(promoted);
            Err(error)
        }
    }
}

fn commit_precreated_generation_success(
    db: &Database,
    context: &GenerationExecutionContext,
    request: &GenerationJobRequest,
    promoted: &PromotedImageCommit<'_>,
) -> Result<GenerationSuccessTransition, AppError> {
    let mut conn = db.conn.lock().map_err(|_| lifecycle_lock_error())?;
    let tx = begin_generation_job_write_transaction(&mut conn)?;
    match commit_generation_success_in_transaction(&tx, context, request, promoted) {
        Ok(GenerationSuccessTransition::Completed(result)) => {
            tx.commit().map_err(|error| {
                lifecycle_database_error("Commit successful generation failed", error)
            })?;
            Ok(GenerationSuccessTransition::Completed(result))
        }
        Ok(GenerationSuccessTransition::CancelRequested) => {
            tx.rollback().map_err(|error| {
                lifecycle_database_error("Rollback cancelled generation success failed", error)
            })?;
            Ok(GenerationSuccessTransition::CancelRequested)
        }
        Err(error) => {
            drop(tx);
            Err(error)
        }
    }
}

#[derive(Clone, Copy)]
struct GenerationOutput<'a> {
    artifact_store: &'a dyn ResponseArtifactStore,
    decoder: &'a dyn ImageResponseDecoder,
    file_store: &'a dyn GenerationFileStore,
}

async fn continue_precreated_generation_after_provider(
    db: &Database,
    output: GenerationOutput<'_>,
    snapshot: &GenerationExecutionSnapshot,
    body: ProviderAttemptBody,
    cancellation: &CancellationProbe,
    events: Option<&dyn LegacyGenerationEventSink>,
) -> Result<PrecreatedLocalOutcome, GenerationExecutionError> {
    let GenerationOutput {
        artifact_store,
        decoder,
        file_store,
    } = output;
    let response = persist_provider_attempt_response(artifact_store, snapshot, body).await?;
    let expected_response_path = artifact_store.expected_response_path(&snapshot.context)?;
    promote_verified_response(db, &snapshot.context, &expected_response_path, &response)
        .map_err(|error| repository_execution_error(&error, "response_ready_commit"))?;
    if let Some(events) = events {
        events.stage_changed();
    }
    let staged =
        resume_verified_response(decoder, file_store, snapshot, &response, cancellation).await?;
    cancellation.checkpoint("image_promotion")?;
    let mut promoted = staged
        .promote()
        .map_err(|_| GenerationExecutionError::Local {
            code: "image_save_failed".to_string(),
            sanitized_message: "Generated images could not be saved".to_string(),
            stage: "image_promotion".to_string(),
        })?;
    let promoted_commit =
        PromotedImageCommit::from_promoted(&promoted, &snapshot.context.generation_id)
            .map_err(|error| repository_execution_error(&error, "success_commit"))?;
    let transition = commit_precreated_generation_success(
        db,
        &snapshot.context,
        &snapshot.request,
        &promoted_commit,
    );
    drop(promoted_commit);
    match transition {
        Ok(GenerationSuccessTransition::Completed(result)) => {
            promoted.disarm_cleanup();
            Ok(GenerationSuccessTransition::Completed(result))
        }
        Ok(GenerationSuccessTransition::CancelRequested) => {
            drop(promoted);
            Ok(GenerationSuccessTransition::CancelRequested)
        }
        Err(error) => {
            drop(promoted);
            Err(repository_execution_error(&error, "success_commit"))
        }
    }
}

pub(crate) async fn resume_verified_response(
    decoder: &dyn ImageResponseDecoder,
    file_store: &dyn GenerationFileStore,
    snapshot: &GenerationExecutionSnapshot,
    response: &ProviderAttemptResponse,
    cancellation: &CancellationProbe,
) -> Result<StagedGenerationFiles, GenerationExecutionError> {
    if response.requested_image_count != snapshot.runtime_options.image_count {
        return Err(GenerationExecutionError::response_artifact());
    }
    cancellation.checkpoint("response_decode")?;
    let images = tokio::select! {
        biased;
        _ = cancellation.until_cancelled() => return Err(CancellationProbe::error("response_decode")),
        result = decoder.decode_and_download(response, cancellation) => result?,
    };
    cancellation.checkpoint("image_staging")?;
    file_store
        .stage_images(snapshot, images, cancellation)
        .await
}

fn normalize_image_moderation(moderation: &str) -> &'static str {
    match moderation {
        "low" => "low",
        _ => DEFAULT_IMAGE_MODERATION,
    }
}

fn normalize_input_fidelity(input_fidelity: &str) -> &'static str {
    match input_fidelity {
        "low" => "low",
        "high" => "high",
        _ => DEFAULT_INPUT_FIDELITY,
    }
}

#[allow(clippy::too_many_arguments)] // Mirrors the persisted generation request fields.
pub(crate) fn image_request_options(
    size: Option<String>,
    quality: Option<String>,
    background: Option<String>,
    output_format: Option<String>,
    output_compression: Option<u8>,
    moderation: Option<String>,
    input_fidelity: Option<String>,
    image_count: Option<u8>,
) -> GptImageRequestOptions {
    GptImageRequestOptions {
        advanced_options: None,
        prepared_overlay: None,
        progress_context: None,
        aspect_ratio: None,
        resolution: None,
        size: size.unwrap_or_else(|| DEFAULT_IMAGE_SIZE.to_string()),
        quality: quality.unwrap_or_else(|| DEFAULT_IMAGE_QUALITY.to_string()),
        background: background.unwrap_or_else(|| DEFAULT_IMAGE_BACKGROUND.to_string()),
        output_format: output_format.unwrap_or_else(|| DEFAULT_OUTPUT_FORMAT.to_string()),
        output_compression: output_compression
            .unwrap_or(DEFAULT_OUTPUT_COMPRESSION)
            .min(100),
        moderation: normalize_image_moderation(
            moderation.as_deref().unwrap_or(DEFAULT_IMAGE_MODERATION),
        )
        .to_string(),
        input_fidelity: normalize_input_fidelity(
            input_fidelity.as_deref().unwrap_or(DEFAULT_INPUT_FIDELITY),
        )
        .to_string(),
        stream: DEFAULT_IMAGE_STREAM,
        partial_images: DEFAULT_PARTIAL_IMAGES,
        image_count: image_count.unwrap_or(DEFAULT_IMAGE_COUNT),
    }
}

pub(crate) fn prepare_generation_job(
    db: &Database,
    request: &GenerationLifecycleRequest,
) -> Result<PreparedGenerationJob, AppError> {
    let stored_model = if request.model.is_none() {
        Some(crate::providers::current_image_model(db)?)
    } else {
        None
    };
    if let Some(model) = request.model.as_deref() {
        if provider_for_model(model) == ImageProvider::Unknown {
            return Err(AppError::Validation {
                message: "Unknown image model.".to_string(),
            });
        }
    }
    let model = normalize_image_model(
        request
            .model
            .as_deref()
            .or(stored_model.as_deref())
            .unwrap_or(&crate::model_catalog::CATALOG.default_image_model),
    )
    .to_string();
    if request.source_generation_id.is_some() {
        crate::generation_recipe::validate_saved_recipe_options(
            &model,
            &GenerationJobOptions {
                size: request.size.clone(),
                quality: request.quality.clone(),
                background: request.background.clone(),
                output_format: request.output_format.clone(),
                output_compression: request.output_compression,
                moderation: request.moderation.clone(),
                input_fidelity: request.input_fidelity.clone(),
                image_count: request.image_count,
                advanced_options: request.advanced_options.clone(),
                aspect_ratio: request.aspect_ratio.clone(),
                resolution: request.resolution.clone(),
                ..Default::default()
            },
            request.source_image_paths.len(),
        )?;
    }
    let mut runtime_options = sanitize_request_options_for_model(
        &model,
        image_request_options(
            request.size.clone(),
            request.quality.clone(),
            request.background.clone(),
            request.output_format.clone(),
            request.output_compression,
            request.moderation.clone(),
            request.input_fidelity.clone(),
            request.image_count,
        ),
    );
    runtime_options.advanced_options = request.advanced_options.clone();
    if let Some(advanced) = &request.advanced_options {
        runtime_options.stream = advanced.stream.unwrap_or(false);
        runtime_options.partial_images = advanced.partial_images.unwrap_or(0);
    }
    if provider_for_model(&model) == ImageProvider::OpenAi
        && request.output_compression.is_some()
        && !["jpeg", "webp"].contains(&runtime_options.output_format.as_str())
    {
        return Err(AppError::Validation {
            message: "Output compression is supported only for JPEG and WebP.".into(),
        });
    }
    runtime_options.aspect_ratio = request.aspect_ratio.clone();
    runtime_options.resolution = request
        .resolution
        .clone()
        .or_else(|| crate::model_registry::default_wire_resolution(&model));
    crate::model_registry::validate_request_options(
        &model,
        &runtime_options,
        request.source_image_paths.len(),
    )?;
    let capability_omits_controls = is_gemini_model(&model);
    let request_options = GenerationJobOptions {
        advanced_options: request.advanced_options.clone(),
        aspect_ratio: runtime_options.aspect_ratio.clone(),
        resolution: runtime_options.resolution.clone(),
        size: request.size.as_ref().map(|_| runtime_options.size.clone()),
        quality: (!capability_omits_controls)
            .then(|| {
                request
                    .quality
                    .as_ref()
                    .map(|_| runtime_options.quality.clone())
            })
            .flatten(),
        background: (!capability_omits_controls)
            .then(|| {
                request
                    .background
                    .as_ref()
                    .map(|_| runtime_options.background.clone())
            })
            .flatten(),
        output_format: (!capability_omits_controls
            || crate::model_registry::image_model(&model).is_some_and(|m| {
                m.profile.protocol == crate::model_catalog::ImageProtocol::GeminiInteractions
            }))
        .then(|| {
            request
                .output_format
                .as_ref()
                .map(|_| runtime_options.output_format.clone())
        })
        .flatten(),
        output_compression: (!capability_omits_controls)
            .then(|| {
                request
                    .output_compression
                    .map(|_| runtime_options.output_compression)
            })
            .flatten(),
        moderation: (!capability_omits_controls)
            .then(|| {
                request
                    .moderation
                    .as_ref()
                    .map(|_| runtime_options.moderation.clone())
            })
            .flatten(),
        input_fidelity: (!capability_omits_controls)
            .then(|| {
                request
                    .input_fidelity
                    .as_ref()
                    .map(|_| runtime_options.input_fidelity.clone())
            })
            .flatten(),
        stream: request.advanced_options.as_ref().and_then(|a| a.stream),
        partial_images: request
            .advanced_options
            .as_ref()
            .and_then(|a| a.partial_images),
        image_count: request.image_count.map(|_| runtime_options.image_count),
    };
    let queued_at = current_timestamp();
    let client_request_id = request
        .client_request_id
        .clone()
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    let seed = format!(
        "compatibility:{}:{client_request_id}",
        request.kind.as_str()
    );
    let job_id = {
        let digest = Sha256::digest(seed.as_bytes());
        let mut bytes = [0u8; 16];
        bytes.copy_from_slice(&digest[0..16]);
        uuid::Uuid::from_bytes(bytes).to_string()
    };
    let generation_id = request
        .client_request_id
        .as_ref()
        .map(|value| {
            let digest = Sha256::digest(value.as_bytes());
            let mut bytes = [0u8; 16];
            bytes.copy_from_slice(&digest[0..16]);
            uuid::Uuid::from_bytes(bytes).to_string()
        })
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    if let Some(source) = &request.source_generation_id {
        let recipe = crate::generation_recipe::load_recipe(db, source)?;
        let value =
            serde_json::to_value(recipe).map_err(|_| AppError::GenerationJobInvalidSnapshot)?;
        if !request.provider_override_confirmed
            && request.provider_profile_id.is_some()
            && value["provider_profile_id"].as_str() == request.provider_profile_id.as_deref()
            && value["warnings"].as_array().is_some_and(|warnings| {
                warnings.iter().any(|warning| {
                    matches!(
                        warning.as_str(),
                        Some("provider_missing" | "endpoint_changed" | "provider_unconfigured")
                    )
                })
            })
        {
            return Err(AppError::Validation { message: "Review the saved recipe before generating: its provider, parameters or source files changed.".into() });
        }
    }
    let active_profile =
        providers::resolve_profile(db, &model, request.provider_profile_id.as_deref());
    if let Some(expected) = &request.confirmed_profile_fingerprint {
        let profile = active_profile.as_ref().map_err(|_| AppError::Validation {
            message: "Confirmed test provider is unavailable.".into(),
        })?;
        if crate::provider_diagnostics::profile_fingerprint(&model, profile)? != *expected {
            return Err(AppError::Validation {
                message: "Provider changed. Review a new test plan before generating.".into(),
            });
        }
    }
    let (provider_kind, provider_profile_id, endpoint_snapshot, status, finished_at, error_code) =
        match active_profile {
            Ok(profile) => {
                let provider_kind = match provider_for_model(&model) {
                    ImageProvider::OpenAi => "openai",
                    ImageProvider::Gemini => "gemini",
                    ImageProvider::Xai => "xai",
                    ImageProvider::Seedream => "seedream",
                    ImageProvider::Unknown => "unresolved",
                };
                let provider_profile_id = profile.id;
                let api_key_is_present = !profile.api_key.trim().is_empty();
                let endpoint_snapshot = image_endpoint_url_for_model_settings(
                    &model,
                    &profile.endpoint_settings,
                    request.kind.endpoint_kind(),
                );
                let provider_snapshot_is_valid = executable_provider_snapshot_is_valid(
                    provider_kind,
                    &provider_profile_id,
                    &endpoint_snapshot,
                );
                if provider_snapshot_is_valid && api_key_is_present {
                    (
                        provider_kind.to_string(),
                        provider_profile_id,
                        endpoint_snapshot,
                        GenerationJobStatus::Queued,
                        None,
                        None,
                    )
                } else if provider_snapshot_is_valid {
                    (
                        provider_kind.to_string(),
                        provider_profile_id,
                        endpoint_snapshot,
                        GenerationJobStatus::Failed,
                        Some(queued_at.clone()),
                        Some("provider_configuration_invalid".to_string()),
                    )
                } else {
                    (
                        "unresolved".to_string(),
                        "unresolved".to_string(),
                        String::new(),
                        GenerationJobStatus::Failed,
                        Some(queued_at.clone()),
                        Some("provider_configuration_invalid".to_string()),
                    )
                }
            }
            Err(AppError::ProviderProfileNotFound { .. }) => (
                "unresolved".to_string(),
                "unresolved".to_string(),
                String::new(),
                GenerationJobStatus::Failed,
                Some(queued_at.clone()),
                Some("provider_profile_missing".to_string()),
            ),
            Err(_) => (
                "unresolved".to_string(),
                "unresolved".to_string(),
                String::new(),
                GenerationJobStatus::Failed,
                Some(queued_at.clone()),
                Some("provider_configuration_invalid".to_string()),
            ),
        };

    let retry_parent = request
        .source_generation_id
        .as_deref()
        .map(|id| crate::generation_recipe::saved_job(db, id))
        .transpose()?
        .flatten()
        .filter(|parent| {
            let saved = serde_json::from_value::<GenerationJobRequest>(parent.request.clone()).ok();
            status == GenerationJobStatus::Queued
                && request.document_id.is_none()
                && parent.source_kind != "canvas"
                && parent.retryable
                && matches!(
                    parent.status,
                    GenerationJobStatus::Failed | GenerationJobStatus::Interrupted
                )
                && parent.provider_profile_id == provider_profile_id
                && parent.endpoint_snapshot == endpoint_snapshot
                && saved.is_some_and(|saved| {
                    // A blank destination means a new conversation, including for a saved recipe.
                    request.conversation_id.as_deref() == Some(saved.conversation_id.as_str())
                        && request
                            .project_id
                            .as_ref()
                            .is_none_or(|id| id == &saved.project_id)
                        && saved.kind.as_str() == request.kind.as_str()
                        && saved.prompt == request.prompt
                        && saved.model == model
                        && saved.source_image_paths == request.source_image_paths
                        && {
                            let normalize_effective_options =
                                |mut options: GenerationJobOptions| {
                                    let effective = sanitize_request_options_for_model(
                                        &model,
                                        image_request_options(
                                            options.size.clone(),
                                            options.quality.clone(),
                                            options.background.clone(),
                                            options.output_format.clone(),
                                            options.output_compression,
                                            options.moderation.clone(),
                                            options.input_fidelity.clone(),
                                            options.image_count,
                                        ),
                                    );
                                    options.size = Some(effective.size);
                                    options.quality = Some(effective.quality);
                                    options.background = Some(effective.background);
                                    options.output_format = Some(effective.output_format);
                                    options.output_compression = Some(effective.output_compression);
                                    options.moderation = Some(effective.moderation);
                                    options.input_fidelity = Some(effective.input_fidelity);
                                    options.image_count = Some(effective.image_count);
                                    options.resolution = options.resolution.or_else(|| {
                                        crate::model_registry::default_wire_resolution(&model)
                                    });
                                    let mut advanced =
                                        options.advanced_options.take().unwrap_or_default();
                                    options.stream =
                                        Some(advanced.stream.or(options.stream).unwrap_or(false));
                                    options.partial_images = Some(
                                        advanced
                                            .partial_images
                                            .or(options.partial_images)
                                            .unwrap_or(0),
                                    );
                                    advanced.stream = None;
                                    advanced.partial_images = None;
                                    options.advanced_options = (advanced
                                        != ImageAdvancedOptions::default())
                                    .then_some(advanced);
                                    options
                                };
                            let old = normalize_effective_options(saved.options);
                            let new = normalize_effective_options(request_options.clone());
                            old == new
                        }
                })
        });
    let mut source_ref = serde_json::json!({ "id": client_request_id, "execution_version": 1 });
    if let Some(document_id) = &request.document_id {
        source_ref["document_id"] = serde_json::json!(document_id);
    }
    if let Some(source_generation_id) = &request.source_generation_id {
        source_ref["source_generation_id"] = serde_json::json!(source_generation_id);
    }
    Ok(PreparedGenerationJob {
        job_id,
        client_request_id: client_request_id.clone(),
        generation_id,
        requested_conversation_id: request.conversation_id.clone(),
        requested_project_id: request.project_id.clone(),
        prompt: request.prompt.clone(),
        model,
        request_kind: request.kind.as_str().to_string(),
        size: runtime_options.size,
        quality: runtime_options.quality,
        background: runtime_options.background,
        output_format: runtime_options.output_format,
        output_compression: i32::from(runtime_options.output_compression),
        moderation: runtime_options.moderation,
        input_fidelity: runtime_options.input_fidelity,
        image_count: i32::from(runtime_options.image_count),
        stream: runtime_options.stream,
        partial_images: runtime_options.partial_images,
        source_image_paths: request.source_image_paths.clone(),
        request_options,
        parent_job_id: retry_parent.as_ref().map(|p| p.id.clone()),
        source_kind: if request.document_id.is_some() {
            "canvas"
        } else {
            request.kind.as_str()
        }
        .into(),
        source_ref,
        provider_kind,
        provider_profile_id,
        endpoint_snapshot,
        status,
        chain_attempt: retry_parent
            .as_ref()
            .map(|p| p.chain_attempt.saturating_add(1))
            .unwrap_or(1),
        auto_attempt: 0,
        max_auto_attempts: 2,
        queued_at,
        finished_at,
        error_code,
        error_message: None,
        retryable: false,
    })
}

fn compatibility_execution_error(
    code: &str,
    sanitized_message: &str,
    stage: &str,
) -> GenerationExecutionError {
    GenerationExecutionError::Local {
        code: code.to_string(),
        sanitized_message: sanitized_message.to_string(),
        stage: stage.to_string(),
    }
}

fn compatibility_error_is_manually_retryable(error: &GenerationExecutionError) -> bool {
    matches!(
        error,
        GenerationExecutionError::Engine(engine_error)
            if engine_error.safe_to_retry
                || engine_error.outcome_ambiguous
                || matches!(
                    engine_error.code.as_str(),
                    "rate_limited"
                        | "provider_unavailable"
                        | "network_before_response"
                        | "provider_outcome_unknown"
                )
    )
}

fn compatibility_response_ready_is_committed(
    db: &Database,
    context: &GenerationExecutionContext,
) -> Result<bool, AppError> {
    let conn = db.conn.lock().map_err(|_| lifecycle_lock_error())?;
    let recovery = conn
        .query_row(
            "SELECT request_state, response_file
             FROM generation_recoveries WHERE generation_id = ?1",
            params![context.generation_id],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?)),
        )
        .optional()
        .map_err(|error| lifecycle_database_error("Read compatibility recovery failed", error))?;
    Ok(matches!(
        recovery,
        Some((state, Some(path)))
            if state == RECOVERY_STATE_RESPONSE_READY && !path.is_empty()
    ))
}

fn compatibility_legacy_error(
    context: &GenerationExecutionContext,
    error: &GenerationExecutionError,
) -> AppError {
    match error.code() {
        "provider_profile_missing" => AppError::ProviderProfileNotFound {
            model: context.model.clone(),
        },
        "provider_configuration_invalid" => AppError::ApiKeyNotSet {
            model: context.model.clone(),
        },
        _ => AppError::Validation {
            message: error.sanitized_message().to_string(),
        },
    }
}

fn terminalize_compatibility_error(
    db: &Database,
    event_sink: &dyn LegacyGenerationEventSink,
    snapshot: &GenerationExecutionSnapshot,
    error: &GenerationExecutionError,
    status: GenerationJobStatus,
    preserve_response_ready: bool,
) -> Result<AppError, AppError> {
    let cancellation_error = compatibility_execution_error(
        "cancelled_by_user",
        "The generation was cancelled",
        "cancellation",
    );
    let cancelled = {
        let mut conn = db.conn.lock().map_err(|_| lifecycle_lock_error())?;
        let tx = begin_generation_job_write_transaction(&mut conn)?;
        let cancelled = get_job_in_transaction(&tx, &snapshot.context.job_id)?
            .cancel_requested_at
            .is_some();
        let effective_error = if cancelled {
            &cancellation_error
        } else {
            error
        };
        let disposition = GenerationTerminalDisposition {
            status: if cancelled {
                GenerationJobStatus::Cancelled
            } else {
                status
            },
            error_code: effective_error.code().to_string(),
            retryable: !cancelled && compatibility_error_is_manually_retryable(effective_error),
            preserve_response_ready: !cancelled && preserve_response_ready,
        };
        finalize_generation_failure_in_transaction(
            &tx,
            &snapshot.context,
            effective_error,
            &disposition,
        )?;
        tx.commit()?;
        cancelled
    };
    let error = if cancelled {
        &cancellation_error
    } else {
        error
    };
    let kind = GenerationLifecycleKind::from(snapshot.request.kind);
    let _ = db.insert_log(
        "generation",
        "error",
        &kind.failed_log_message(error.sanitized_message()),
        Some(&snapshot.context.generation_id),
        None,
        None,
    );
    event_sink.failed(&snapshot.context.generation_id, error.sanitized_message());
    Ok(compatibility_legacy_error(&snapshot.context, error))
}

// Under the process library lock the worker persists
// the same recovery milestones used by the fenced restart worker before HTTP.
fn begin_compatibility_provider_request(
    db: &Database,
    snapshot: &GenerationExecutionSnapshot,
    expected: &Path,
) -> Result<(), AppError> {
    if !response_path_matches_context(expected, &snapshot.context) {
        return Err(AppError::GenerationJobInvalidSnapshot);
    }
    let mut conn = db.conn.lock().map_err(|_| lifecycle_lock_error())?;
    let tx = begin_generation_job_write_transaction(&mut conn)?;
    let current = load_matching_execution_snapshot(&tx, &snapshot.context)?;
    if current.request != snapshot.request {
        return Err(AppError::GenerationJobInvalidSnapshot);
    }
    validate_worker_recovery_for_stage(
        &tx,
        &snapshot.context.generation_id,
        GenerationJobStage::Preparing,
    )?;
    let timestamp = current_timestamp();
    let changed = tx.execute("UPDATE generation_jobs SET stage='provider_request',last_heartbeat_at=?1 WHERE id=?2 AND status='running' AND stage='preparing' AND cancel_requested_at IS NULL", params![timestamp, snapshot.context.job_id])?;
    if changed != 1 {
        return Err(AppError::GenerationJobInvalidTransition);
    }
    let changed = tx.execute("UPDATE generation_recoveries SET expected_response_file=?1,updated_at=?2 WHERE generation_id=?3 AND request_state='requesting' AND expected_response_file IS NULL", params![expected.to_string_lossy(), timestamp, snapshot.context.generation_id])?;
    if changed != 1 {
        return Err(AppError::GenerationJobInvalidTransition);
    }
    validate_worker_recovery_for_stage(
        &tx,
        &snapshot.context.generation_id,
        GenerationJobStage::ProviderRequest,
    )?;
    tx.commit()?;
    Ok(())
}

async fn execute_claimed_generation(
    db: &Database,
    engine: &dyn ImageEngine,
    output: GenerationOutput<'_>,
    event_sink: &dyn LegacyGenerationEventSink,
    snapshot: GenerationExecutionSnapshot,
    cancellation: &CancellationProbe,
) -> Result<GenerateResult, AppError> {
    let artifact_store = output.artifact_store;
    let kind = GenerationLifecycleKind::from(snapshot.request.kind);
    // Read the endpoint and key from the same saved profile. A queued operation
    // must never send a replacement endpoint's credential to its former host.
    let context = &snapshot.context;
    let profile =
        providers::resolve_profile(db, &context.model, Some(&context.provider_profile_id));
    let credentials = match profile {
        Ok(profile)
            if image_endpoint_url_for_model_settings(
                &context.model,
                &profile.endpoint_settings,
                kind.endpoint_kind(),
            ) != context.endpoint_url =>
        {
            Err("provider_endpoint_changed")
        }
        Ok(profile) if !profile.api_key.trim().is_empty() => {
            Ok(ProviderExecutionCredentials::new(profile.api_key))
        }
        Err(AppError::ProviderProfileNotFound { .. }) => Err("provider_profile_missing"),
        _ => Err("provider_configuration_invalid"),
    };
    let credentials = match credentials {
        Ok(credentials) => credentials,
        Err(code) => {
            let error = compatibility_execution_error(
                code,
                crate::generation_jobs::terminal_message_for_code(code),
                "provider_credentials",
            );
            return Err(terminalize_compatibility_error(
                db,
                event_sink,
                &snapshot,
                &error,
                GenerationJobStatus::Failed,
                false,
            )?);
        }
    };
    let prepared_attempt = match prepare_provider_attempt(&snapshot).await {
        Ok(prepared_attempt) => prepared_attempt,
        Err(error) => {
            return Err(terminalize_compatibility_error(
                db,
                event_sink,
                &snapshot,
                &error,
                GenerationJobStatus::Failed,
                false,
            )?);
        }
    };
    let begin = artifact_store
        .expected_response_path(&snapshot.context)
        .and_then(|path| {
            begin_compatibility_provider_request(db, &snapshot, &path)
                .map_err(|error| repository_execution_error(&error, "request_binding"))
        });
    if let Err(error) = begin {
        return Err(terminalize_compatibility_error(
            db,
            event_sink,
            &snapshot,
            &error,
            GenerationJobStatus::Failed,
            false,
        )?);
    }
    event_sink.stage_changed();
    let provider_result = tokio::select! {
        biased;
        _ = cancellation.until_cancelled() => Err(compatibility_execution_error("cancelled_by_user", "The generation was cancelled", "provider_request")),
        result = perform_provider_http_attempt(engine, &snapshot, &credentials, &prepared_attempt) => result,
    };
    let body = match provider_result {
        Ok(body) => body,
        Err(error) => {
            let status = match &error {
                GenerationExecutionError::Engine(engine_error)
                    if engine_error.outcome_ambiguous =>
                {
                    GenerationJobStatus::Interrupted
                }
                _ => GenerationJobStatus::Failed,
            };
            return Err(terminalize_compatibility_error(
                db, event_sink, &snapshot, &error, status, false,
            )?);
        }
    };

    match continue_precreated_generation_after_provider(
        db,
        output,
        &snapshot,
        body,
        cancellation,
        Some(event_sink),
    )
    .await
    {
        Ok(GenerationSuccessTransition::Completed(result)) => {
            let _ = db.insert_log(
                "generation",
                "info",
                &kind.completed_log_message(result.images.len()),
                Some(&snapshot.context.generation_id),
                Some(&serde_json::json!({ "image_count": result.images.len() }).to_string()),
                None,
            );
            event_sink.complete(&snapshot.context.generation_id);
            Ok(result)
        }
        Ok(GenerationSuccessTransition::CancelRequested) => {
            let error = compatibility_execution_error(
                "cancelled_by_user",
                "The generation was cancelled",
                "cancellation",
            );
            Err(terminalize_compatibility_error(
                db,
                event_sink,
                &snapshot,
                &error,
                GenerationJobStatus::Cancelled,
                false,
            )?)
        }
        Err(error) => {
            let mut preserve_response_ready =
                compatibility_response_ready_is_committed(db, &snapshot.context)?;
            if !preserve_response_ready {
                // The envelope can be durable while its response-ready DB
                // publication failed. Keep the bound row for local recovery.
                if let Ok(path) = artifact_store.expected_response_path(&snapshot.context) {
                    preserve_response_ready = artifact_store
                        .load_verified_response(&snapshot.context, &path)
                        .await
                        .is_ok_and(|response| {
                            verified_response_shape_matches(
                                &response,
                                snapshot.runtime_options.image_count,
                                Some(&path),
                                None,
                            )
                        });
                }
            }
            Err(terminalize_compatibility_error(
                db,
                event_sink,
                &snapshot,
                &error,
                GenerationJobStatus::Failed,
                preserve_response_ready,
            )?)
        }
    }
}

fn on_worker_claim(
    db: &Database,
    event_sink: &dyn LegacyGenerationEventSink,
    snapshot: &GenerationExecutionSnapshot,
) -> Result<(), AppError> {
    let kind = GenerationLifecycleKind::from(snapshot.request.kind);
    if kind == GenerationLifecycleKind::Generate {
        let _ = db.insert_log(
            "generation",
            "info",
            &format!(
                "Started — size: {}, quality: {}, background: {}, output_format: {}, image_count: {}",
                snapshot.runtime_options.size,
                snapshot.runtime_options.quality,
                snapshot.runtime_options.background,
                snapshot.runtime_options.output_format,
                snapshot.runtime_options.image_count,
            ),
            Some(&snapshot.context.generation_id),
            Some(
                &serde_json::json!({
                    "model": &snapshot.context.model,
                    "size": &snapshot.runtime_options.size,
                    "quality": &snapshot.runtime_options.quality,
                    "background": &snapshot.runtime_options.background,
                    "output_format": &snapshot.runtime_options.output_format,
                    "output_compression": snapshot.runtime_options.output_compression,
                    "moderation": &snapshot.runtime_options.moderation,
                    "stream": snapshot.runtime_options.stream,
                    "partial_images": snapshot.runtime_options.partial_images,
                    "image_count": snapshot.runtime_options.image_count,
                    "conversation_id": &snapshot.context.conversation_id,
                })
                .to_string(),
            ),
            None,
        );
    }
    event_sink.progress(&snapshot.context.generation_id);

    if let Err(error) = validate_provider_execution_snapshot(snapshot) {
        return Err(terminalize_compatibility_error(
            db,
            event_sink,
            snapshot,
            &error,
            GenerationJobStatus::Failed,
            false,
        )?);
    }
    Ok(())
}

pub(crate) async fn run_claimed_generation(
    app: &tauri::AppHandle<impl tauri::Runtime>,
    db: &Database,
    engine: &crate::api_gateway::GptImageEngine,
    root: &Path,
    snapshot: GenerationExecutionSnapshot,
    cancellation: &CancellationProbe,
) -> Result<GenerateResult, AppError> {
    let scope = snapshot
        .runtime_options
        .progress_context
        .clone()
        .ok_or(AppError::GenerationJobInvalidSnapshot)?;
    let event_sink = TauriLegacyGenerationEventSink { app, scope: &scope };
    on_worker_claim(db, &event_sink, &snapshot)?;
    let artifact_store = FileResponseArtifactStore::new(root.to_path_buf());
    let decoder = EngineImageResponseDecoder::new(Arc::new(engine.clone()));
    let file_store = LocalGenerationFileStore::new(root.to_path_buf());
    execute_claimed_generation(
        db,
        engine,
        GenerationOutput {
            artifact_store: &artifact_store,
            decoder: &decoder,
            file_store: &file_store,
        },
        &event_sink,
        snapshot,
        cancellation,
    )
    .await
}

#[cfg(test)]
#[path = "generation_lifecycle_tests.rs"]
mod tests;
