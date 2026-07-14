use crate::api_gateway::{EngineCallError, ImageEngine, PreparedEditImage, ProviderAttemptBody};
use crate::commands::settings;
use crate::current_timestamp;
use crate::db::Database;
use crate::error::AppError;
use crate::file_manager::{self, PromotedGenerationFiles, StagedGenerationFiles};
use crate::generation_jobs::{
    begin_generation_job_write_transaction, enqueue_job, executable_provider_snapshot_is_valid,
    finish_job_in_transaction, get_job_event_in_transaction, get_job_in_transaction,
    insert_and_claim_exact_job, load_generation_execution_snapshot_for_stage_in_transaction,
    load_generation_execution_snapshot_in_transaction, set_actual_image_count_in_transaction,
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
use cap_fs_ext::DirExt;
use rusqlite::{params, OptionalExtension, Transaction};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::io::{Read, Write};
use std::marker::PhantomData;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::{Emitter, Manager};

const RECOVERY_STATE_REQUESTING: &str = "requesting";
const RECOVERY_STATE_RESPONSE_READY: &str = "response_ready";

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum GenerationLifecycleKind {
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

pub(crate) struct GenerationLifecycleRequest {
    pub(crate) kind: GenerationLifecycleKind,
    pub(crate) prompt: String,
    pub(crate) model: Option<String>,
    pub(crate) source_image_paths: Vec<String>,
    pub(crate) size: Option<String>,
    pub(crate) quality: Option<String>,
    pub(crate) background: Option<String>,
    pub(crate) output_format: Option<String>,
    pub(crate) output_compression: Option<u8>,
    pub(crate) moderation: Option<String>,
    pub(crate) input_fidelity: Option<String>,
    pub(crate) image_count: Option<u8>,
    pub(crate) conversation_id: Option<String>,
    pub(crate) project_id: Option<String>,
}

trait LegacyGenerationEventSink: Send + Sync {
    fn progress(&self, generation_id: &str);
    fn complete(&self, generation_id: &str);
    fn failed(&self, generation_id: &str, sanitized_message: &str);
}

struct TauriLegacyGenerationEventSink<'a> {
    app: &'a tauri::AppHandle,
}

impl LegacyGenerationEventSink for TauriLegacyGenerationEventSink<'_> {
    fn progress(&self, generation_id: &str) {
        let _ = self.app.emit(
            "generation:progress",
            serde_json::json!({ "generation_id": generation_id, "status": "processing" }),
        );
    }

    fn complete(&self, generation_id: &str) {
        let _ = self.app.emit(
            "generation:complete",
            serde_json::json!({ "generation_id": generation_id, "status": "completed" }),
        );
    }

    fn failed(&self, generation_id: &str, sanitized_message: &str) {
        let _ = self.app.emit(
            "generation:failed",
            serde_json::json!({
                "generation_id": generation_id,
                "error": sanitized_message,
            }),
        );
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
pub(crate) enum GenerationSuccessTransition {
    Completed(GenerateResult),
    CancelRequested,
}

pub(crate) type PrecreatedLocalOutcome = GenerationSuccessTransition;

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
pub(crate) struct VerifiedResponseCommit {
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

/// Sealed, pure-data success projection derived from a live promoted-file
/// guard. The lifetime prevents cleanup/disarm while the SQL commit is using
/// the projection, without passing an I/O-capable guard into the transaction.
pub(crate) struct PromotedImageCommit<'a> {
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
}

#[derive(Clone, Default)]
pub(crate) struct CancellationProbe {
    cancelled: Arc<AtomicBool>,
}

impl CancellationProbe {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
    }

    pub(crate) fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }

    pub(crate) fn checkpoint(&self, stage: &str) -> Result<(), GenerationExecutionError> {
        if self.is_cancelled() {
            Err(GenerationExecutionError::Local {
                code: "cancelled_by_user".to_string(),
                sanitized_message: "The generation was cancelled".to_string(),
                stage: stage.to_string(),
            })
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
            || !(1..=4).contains(&response.requested_image_count)
        {
            return Err(GenerationExecutionError::response_artifact());
        }
        let mut images = self
            .engine
            .decode_images_from_response(&response.body_text, &|| cancellation.is_cancelled())
            .await
            .map_err(|_| {
                if cancellation.is_cancelled() {
                    GenerationExecutionError::Local {
                        code: "cancelled_by_user".to_string(),
                        sanitized_message: "The generation was cancelled".to_string(),
                        stage: "response_decode".to_string(),
                    }
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
    const MAX_RESPONSE_BODY_BYTES: u64 = 64 * 1024 * 1024;
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
        directory: &cap_std::fs::Dir,
        _absolute_path: &Path,
    ) -> Result<(), GenerationExecutionError> {
        #[cfg(unix)]
        {
            directory
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
            let _ = (directory, _absolute_path);
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
            .filter(|value| (1..=4).contains(value))
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
            || !(1..=4).contains(&body.requested_image_count)
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
    use crate::commands::generation::{
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
    Ok(PreparedProviderAttempt {
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
        || !(1..=4).contains(&response.requested_image_count)
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

fn validate_provider_execution_snapshot(
    snapshot: &GenerationExecutionSnapshot,
) -> Result<(), GenerationExecutionError> {
    let expected_provider_kind = match provider_for_model(&snapshot.context.model) {
        ImageProvider::OpenAi => "openai",
        ImageProvider::Gemini => "gemini",
    };
    if snapshot.context.provider_kind != expected_provider_kind
        || snapshot.request.model != snapshot.context.model
        || snapshot.request.conversation_id != snapshot.context.conversation_id
        || snapshot.output_format != snapshot.runtime_options.output_format
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
                    &snapshot.runtime_options,
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
                    &snapshot.runtime_options,
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
        || !(1..=4).contains(&body.requested_image_count)
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
    let snapshot = load_generation_execution_snapshot_in_transaction(tx, &context.job_id)?;
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

pub(crate) fn promote_verified_response_in_transaction(
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
    Ok(())
}

pub(crate) fn promote_verified_response(
    db: &Database,
    context: &GenerationExecutionContext,
    expected_path: &Path,
    response: &ProviderAttemptResponse,
) -> Result<(), AppError> {
    promote_verified_response_with_optional_observer(db, context, expected_path, response, None)
}

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

fn promote_verified_response_fenced_with_optional_observer(
    db: &Database,
    artifact_store: &FileResponseArtifactStore,
    context: &GenerationExecutionContext,
    response: &ProviderAttemptResponse,
    authority: &WorkerTransitionAuthority,
    now_ms: i64,
    observer: Option<&dyn ResponseVerificationObserver>,
) -> Result<GenerationJobEvent, WorkerTransitionError> {
    let response =
        VerifiedResponseCommit::verify_with_store(context, artifact_store, response, observer)?;
    let timestamp = lifecycle_worker_timestamp(now_ms)?;
    let response_size = i64::try_from(response.response_size)
        .map_err(|_| AppError::GenerationJobInvalidSnapshot)?;
    let mut conn = db.conn.lock().map_err(|_| lifecycle_lock_error())?;
    let tx = begin_generation_job_write_transaction(&mut conn)?;

    // This must remain the first database operation after BEGIN IMMEDIATE.
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

pub(crate) fn promote_verified_response_fenced(
    db: &Database,
    artifact_store: &FileResponseArtifactStore,
    context: &GenerationExecutionContext,
    response: &ProviderAttemptResponse,
    authority: &WorkerTransitionAuthority,
    now_ms: i64,
) -> Result<GenerationJobEvent, WorkerTransitionError> {
    // The production adapter must pass its managed app-owned store. Unlike the
    // compatibility API, this fenced path never accepts a caller-selected path.
    promote_verified_response_fenced_with_optional_observer(
        db,
        artifact_store,
        context,
        response,
        authority,
        now_ms,
        None,
    )
}

pub(crate) fn finalize_generation_failure_in_transaction(
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
        if !response_ready {
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

pub(crate) fn finalize_generation_failure(
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
            || images.len() > 4
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

pub(crate) fn commit_generation_success_in_transaction(
    tx: &Transaction<'_>,
    context: &GenerationExecutionContext,
    request: &GenerationJobRequest,
    promoted: &PromotedImageCommit<'_>,
) -> Result<GenerationSuccessTransition, AppError> {
    let snapshot = load_matching_execution_snapshot(tx, context)?;
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
            finished_at: current_timestamp(),
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

pub(crate) async fn continue_precreated_generation_after_provider(
    db: &Database,
    artifact_store: &dyn ResponseArtifactStore,
    decoder: &dyn ImageResponseDecoder,
    file_store: &dyn GenerationFileStore,
    snapshot: &GenerationExecutionSnapshot,
    body: ProviderAttemptBody,
    cancellation: &CancellationProbe,
) -> Result<PrecreatedLocalOutcome, GenerationExecutionError> {
    let response = persist_provider_attempt_response(artifact_store, snapshot, body).await?;
    let expected_response_path = artifact_store.expected_response_path(&snapshot.context)?;
    promote_verified_response(db, &snapshot.context, &expected_response_path, &response)
        .map_err(|error| repository_execution_error(&error, "response_ready_commit"))?;
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
    let images = decoder.decode_and_download(response, cancellation).await?;
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
        image_count: image_count.unwrap_or(DEFAULT_IMAGE_COUNT).clamp(1, 4),
    }
}

fn prepare_compatibility_job(
    db: &Database,
    request: &GenerationLifecycleRequest,
) -> Result<PreparedGenerationJob, AppError> {
    let stored_model = if request.model.is_none() {
        db.get_setting(SETTING_IMAGE_MODEL)?
    } else {
        None
    };
    let model = normalize_image_model(
        request
            .model
            .as_deref()
            .or(stored_model.as_deref())
            .unwrap_or(DEFAULT_IMAGE_MODEL),
    )
    .to_string();
    let runtime_options = sanitize_request_options_for_model(
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
    let capability_omits_controls = is_gemini_model(&model);
    let request_options = GenerationJobOptions {
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
        output_format: (!capability_omits_controls)
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
        stream: None,
        partial_images: None,
        image_count: request.image_count.map(|_| runtime_options.image_count),
    };
    let queued_at = current_timestamp();
    let job_id = uuid::Uuid::new_v4().to_string();
    let client_request_id = uuid::Uuid::new_v4().to_string();
    let generation_id = uuid::Uuid::new_v4().to_string();
    let active_profile = settings::active_provider_profile_for_model(db, &model);
    let (provider_kind, provider_profile_id, endpoint_snapshot, status, finished_at, error_code) =
        match active_profile {
            Ok(profile) => {
                let provider_kind = match provider_for_model(&model) {
                    ImageProvider::OpenAi => "openai",
                    ImageProvider::Gemini => "gemini",
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
        parent_job_id: None,
        source_kind: request.kind.as_str().to_string(),
        source_ref: serde_json::json!({ "id": client_request_id }),
        provider_kind,
        provider_profile_id,
        endpoint_snapshot,
        status,
        chain_attempt: 1,
        auto_attempt: 0,
        max_auto_attempts: 2,
        queued_at,
        finished_at,
        error_code,
        error_message: None,
        retryable: false,
    })
}

fn compatibility_initial_failure_message(error_code: &str) -> &'static str {
    match error_code {
        "provider_profile_missing" => "The selected provider profile is unavailable",
        _ => "The image provider configuration is invalid",
    }
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
    let disposition = GenerationTerminalDisposition {
        status,
        error_code: error.code().to_string(),
        retryable: compatibility_error_is_manually_retryable(error),
        preserve_response_ready,
    };
    finalize_generation_failure(db, &snapshot.context, error, &disposition)?;
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

fn start_compatibility_job(
    db: &Database,
    event_sink: &dyn LegacyGenerationEventSink,
    prepared_job: PreparedGenerationJob,
) -> Result<GenerationExecutionSnapshot, AppError> {
    if prepared_job.status == GenerationJobStatus::Failed {
        let error_code = prepared_job
            .error_code
            .as_deref()
            .unwrap_or("provider_configuration_invalid");
        let message = compatibility_initial_failure_message(error_code);
        let resolved_provider_configuration = executable_provider_snapshot_is_valid(
            &prepared_job.provider_kind,
            &prepared_job.provider_profile_id,
            &prepared_job.endpoint_snapshot,
        );
        let generation_id = prepared_job.generation_id.clone();
        let model = prepared_job.model.clone();
        let kind = match prepared_job.request_kind.as_str() {
            "generate" => GenerationLifecycleKind::Generate,
            "edit" => GenerationLifecycleKind::Edit,
            _ => return Err(AppError::GenerationJobInvalidSnapshot),
        };
        {
            let mut conn = db.conn.lock().map_err(|_| lifecycle_lock_error())?;
            let inserted = enqueue_job(&mut conn, &prepared_job)?;
            if inserted.job_id != prepared_job.job_id
                || inserted.generation_id != prepared_job.generation_id
            {
                return Err(AppError::GenerationJobIdempotencyConflict);
            }
        }
        let _ = db.insert_log(
            "generation",
            "error",
            &kind.failed_log_message(message),
            Some(&generation_id),
            None,
            None,
        );
        event_sink.failed(&generation_id, message);
        return Err(match error_code {
            "provider_profile_missing" => AppError::ProviderProfileNotFound { model },
            "provider_configuration_invalid" if resolved_provider_configuration => {
                AppError::ApiKeyNotSet { model }
            }
            _ => AppError::Validation {
                message: message.to_string(),
            },
        });
    }
    if prepared_job.status != GenerationJobStatus::Queued {
        return Err(AppError::GenerationJobInvalidSnapshot);
    }

    let snapshot = {
        let mut conn = db.conn.lock().map_err(|_| lifecycle_lock_error())?;
        insert_and_claim_exact_job(&mut conn, &prepared_job)?
    };
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

    if let Err(error) = validate_provider_execution_snapshot(&snapshot) {
        return Err(terminalize_compatibility_error(
            db,
            event_sink,
            &snapshot,
            &error,
            GenerationJobStatus::Failed,
            false,
        )?);
    }
    Ok(snapshot)
}

async fn execute_claimed_compatibility_job(
    db: &Database,
    engine: &dyn ImageEngine,
    artifact_store: &dyn ResponseArtifactStore,
    decoder: &dyn ImageResponseDecoder,
    file_store: &dyn GenerationFileStore,
    event_sink: &dyn LegacyGenerationEventSink,
    snapshot: GenerationExecutionSnapshot,
) -> Result<GenerateResult, AppError> {
    let kind = GenerationLifecycleKind::from(snapshot.request.kind);
    let api_key = match settings::read_model_provider_api_key(
        db,
        &snapshot.context.model,
        &snapshot.context.provider_profile_id,
    ) {
        Ok(Some(api_key)) => api_key,
        Ok(None) => {
            let error = compatibility_execution_error(
                "provider_configuration_invalid",
                "The image provider configuration is invalid",
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
        Err(AppError::ProviderProfileNotFound { .. }) => {
            let error = compatibility_execution_error(
                "provider_profile_missing",
                "The selected provider profile is unavailable",
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
        Err(_) => {
            let error = compatibility_execution_error(
                "provider_configuration_invalid",
                "The image provider configuration is invalid",
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
    let credentials = ProviderExecutionCredentials::new(api_key);
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
    let body =
        match perform_provider_http_attempt(engine, &snapshot, &credentials, &prepared_attempt)
            .await
        {
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
        artifact_store,
        decoder,
        file_store,
        &snapshot,
        body,
        &CancellationProbe::new(),
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
            let preserve_response_ready =
                compatibility_response_ready_is_committed(db, &snapshot.context)?;
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

async fn execute_compatibility_job(
    db: &Database,
    engine: &dyn ImageEngine,
    artifact_store: &dyn ResponseArtifactStore,
    decoder: &dyn ImageResponseDecoder,
    file_store: &dyn GenerationFileStore,
    event_sink: &dyn LegacyGenerationEventSink,
    prepared_job: PreparedGenerationJob,
) -> Result<GenerateResult, AppError> {
    let snapshot = start_compatibility_job(db, event_sink, prepared_job)?;
    execute_claimed_compatibility_job(
        db,
        engine,
        artifact_store,
        decoder,
        file_store,
        event_sink,
        snapshot,
    )
    .await
}

pub(crate) async fn run_generation_lifecycle(
    app: &tauri::AppHandle,
    db: &Database,
    engine: &crate::api_gateway::GptImageEngine,
    request: GenerationLifecycleRequest,
) -> Result<GenerateResult, AppError> {
    let prepared_job = prepare_compatibility_job(db, &request)?;
    let event_sink = TauriLegacyGenerationEventSink { app };
    let snapshot = start_compatibility_job(db, &event_sink, prepared_job)?;
    let app_data_dir = match app.path().app_data_dir() {
        Ok(app_data_dir) => app_data_dir,
        Err(path_error) => {
            let execution_error = compatibility_execution_error(
                "image_save_failed",
                "Generated images could not be saved",
                "app_data_directory",
            );
            terminalize_compatibility_error(
                db,
                &event_sink,
                &snapshot,
                &execution_error,
                GenerationJobStatus::Failed,
                false,
            )?;
            return Err(AppError::FileSystem {
                message: format!("Get app data dir failed: {path_error}"),
            });
        }
    };
    let artifact_store = FileResponseArtifactStore::new(app_data_dir.clone());
    let decoder = EngineImageResponseDecoder::new(Arc::new(engine.clone()));
    let file_store = LocalGenerationFileStore::new(app_data_dir);
    execute_claimed_compatibility_job(
        db,
        engine,
        &artifact_store,
        &decoder,
        &file_store,
        &event_sink,
        snapshot,
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api_gateway::{PreparedEditImage, ProviderAttemptBody, RetryAfterHint};
    use crate::file_manager::GenerationFileLifecycleObserver;
    use crate::generation_jobs::{
        begin_generation_job_write_transaction, claim_job_in_transaction, enqueue_job, get_job,
        load_generation_execution_snapshot, request_cancel,
        transition_running_job_stage_with_event, GenerationJobOptions, GenerationJobRequest,
        GenerationJobRequestKind, PreparedGenerationJob, WorkerStageTransition,
    };
    use crate::generation_worker_lease::{
        acquire_worker_lease, WorkerLeaseAcquireOutcome, WorkerTransitionAuthority,
    };
    use crate::model_registry::default_endpoint_settings_for_model;
    use crate::models::GenerationJobStatus;
    use image::{DynamicImage, ImageBuffer, ImageFormat, Rgb};
    use std::io::Cursor;
    use std::path::Path;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    fn fixture_execution_context() -> GenerationExecutionContext {
        GenerationExecutionContext {
            generation_id: "generation-1".to_string(),
            job_id: "job-1".to_string(),
            conversation_id: "conversation-1".to_string(),
            provider_kind: "openai".to_string(),
            model: "gpt-image-2".to_string(),
            endpoint_url: "https://example.test/images/generations".to_string(),
            provider_profile_id: "profile-1".to_string(),
        }
    }

    fn jpeg_bytes() -> Vec<u8> {
        let image = DynamicImage::ImageRgb8(ImageBuffer::from_pixel(4, 4, Rgb([24, 96, 180])));
        let mut bytes = Vec::new();
        image
            .write_to(&mut Cursor::new(&mut bytes), ImageFormat::Jpeg)
            .expect("encode jpeg");
        bytes
    }

    fn fixture_execution_snapshot() -> GenerationExecutionSnapshot {
        GenerationExecutionSnapshot {
            context: fixture_execution_context(),
            request: GenerationJobRequest {
                kind: GenerationJobRequestKind::Generate,
                prompt: "draw a nebula".to_string(),
                model: "gpt-image-2".to_string(),
                source_image_paths: Vec::new(),
                options: GenerationJobOptions {
                    image_count: Some(2),
                    ..GenerationJobOptions::default()
                },
                requested_conversation_id: None,
                requested_project_id: None,
                conversation_id: "conversation-1".to_string(),
                project_id: "default".to_string(),
            },
            runtime_options: image_request_options(
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                Some(2),
            ),
            created_at: "2026-04-29T06:18:01Z".to_string(),
            output_format: "png".to_string(),
        }
    }

    struct CompatibilityPreparationFixture {
        db: Arc<Database>,
        root: PathBuf,
    }

    impl CompatibilityPreparationFixture {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!(
                "astro-studio-compatibility-preparation-test-{}",
                uuid::Uuid::new_v4()
            ));
            std::fs::create_dir_all(&root).expect("create compatibility fixture root");
            let db = Arc::new(
                Database::open(&root.join("astro_studio.db")).expect("open compatibility db"),
            );
            db.run_migrations().expect("migrate compatibility db");
            Self { db, root }
        }

        fn save_profiles(&self, model: &str, state: ModelProviderProfilesState) {
            settings::save_model_provider_profiles_state(&self.db, model, state)
                .expect("save compatibility profiles");
        }

        fn cleanup(self) {
            drop(self.db);
            std::fs::remove_dir_all(self.root).ok();
        }
    }

    fn compatibility_request(model: &str) -> GenerationLifecycleRequest {
        GenerationLifecycleRequest {
            kind: GenerationLifecycleKind::Generate,
            prompt: "draw a durable nebula".to_string(),
            model: Some(model.to_string()),
            source_image_paths: Vec::new(),
            size: Some("1536x1024".to_string()),
            quality: Some("high".to_string()),
            background: Some("transparent".to_string()),
            output_format: Some("webp".to_string()),
            output_compression: Some(150),
            moderation: Some("unsupported-moderation".to_string()),
            input_fidelity: Some("unsupported-fidelity".to_string()),
            image_count: Some(9),
            conversation_id: None,
            project_id: Some("default".to_string()),
        }
    }

    #[test]
    fn compatibility_job_snapshots_profile_endpoint_and_normalized_present_options_without_secret()
    {
        let fixture = CompatibilityPreparationFixture::new();
        fixture.save_profiles(
            ENGINE_GPT_IMAGE_2,
            ModelProviderProfilesState {
                active_provider_id: "provider-a".to_string(),
                profiles: vec![ModelProviderProfile {
                    id: "provider-a".to_string(),
                    name: "Provider A".to_string(),
                    api_key: "sk-compatibility-secret".to_string(),
                    endpoint_settings: EndpointSettings {
                        mode: ENDPOINT_MODE_FULL_URL.to_string(),
                        base_url: "https://unused.example.test/v1".to_string(),
                        generation_url: "https://provider-a.example.test/images/generations"
                            .to_string(),
                        edit_url: "https://provider-a.example.test/images/edits".to_string(),
                    },
                }],
            },
        );

        let prepared =
            prepare_compatibility_job(&fixture.db, &compatibility_request(ENGINE_GPT_IMAGE_2))
                .expect("prepare durable compatibility job");

        assert_eq!(prepared.status, GenerationJobStatus::Queued);
        assert_eq!(prepared.provider_kind, "openai");
        assert_eq!(prepared.provider_profile_id, "provider-a");
        assert_eq!(
            prepared.endpoint_snapshot,
            "https://provider-a.example.test/images/generations"
        );
        assert_eq!(prepared.output_compression, 100);
        assert_eq!(prepared.image_count, 4);
        assert_eq!(
            prepared.request_options.output_compression,
            Some(prepared.output_compression as u8)
        );
        assert_eq!(
            prepared.request_options.moderation.as_deref(),
            Some(DEFAULT_IMAGE_MODERATION)
        );
        assert_eq!(
            prepared.request_options.input_fidelity.as_deref(),
            Some(DEFAULT_INPUT_FIDELITY)
        );
        assert_eq!(prepared.request_options.image_count, Some(4));
        let public_snapshot = serde_json::json!({
            "job_id": prepared.job_id,
            "client_request_id": prepared.client_request_id,
            "generation_id": prepared.generation_id,
            "model": prepared.model,
            "provider_kind": prepared.provider_kind,
            "provider_profile_id": prepared.provider_profile_id,
            "endpoint_snapshot": prepared.endpoint_snapshot,
            "source_ref": prepared.source_ref,
        })
        .to_string();
        assert!(!public_snapshot.contains("sk-compatibility-secret"));
        fixture.cleanup();
    }

    #[test]
    fn compatibility_job_preserves_gemini_capability_omissions() {
        let fixture = CompatibilityPreparationFixture::new();
        fixture.save_profiles(
            ENGINE_NANO_BANANA_2,
            ModelProviderProfilesState {
                active_provider_id: "gemini-a".to_string(),
                profiles: vec![ModelProviderProfile {
                    id: "gemini-a".to_string(),
                    name: "Gemini A".to_string(),
                    api_key: "gemini-secret".to_string(),
                    endpoint_settings: default_endpoint_settings_for_model(ENGINE_NANO_BANANA_2),
                }],
            },
        );

        let prepared =
            prepare_compatibility_job(&fixture.db, &compatibility_request(ENGINE_NANO_BANANA_2))
                .expect("prepare Gemini compatibility job");

        assert_eq!(prepared.status, GenerationJobStatus::Queued);
        assert_eq!(prepared.provider_kind, "gemini");
        assert_eq!(prepared.quality, DEFAULT_IMAGE_QUALITY);
        assert_eq!(prepared.background, DEFAULT_IMAGE_BACKGROUND);
        assert_eq!(prepared.output_format, DEFAULT_OUTPUT_FORMAT);
        assert_eq!(
            prepared.output_compression,
            i32::from(DEFAULT_OUTPUT_COMPRESSION)
        );
        assert_eq!(prepared.moderation, DEFAULT_IMAGE_MODERATION);
        assert_eq!(prepared.input_fidelity, DEFAULT_INPUT_FIDELITY);
        assert_eq!(prepared.request_options.size.as_deref(), Some("1536x1024"));
        assert_eq!(prepared.request_options.image_count, Some(4));
        assert_eq!(prepared.request_options.quality, None);
        assert_eq!(prepared.request_options.background, None);
        assert_eq!(prepared.request_options.output_format, None);
        assert_eq!(prepared.request_options.output_compression, None);
        assert_eq!(prepared.request_options.moderation, None);
        assert_eq!(prepared.request_options.input_fidelity, None);
        fixture.cleanup();
    }

    #[test]
    fn compatibility_job_preserves_openai_omitted_option_presence() {
        let fixture = CompatibilityPreparationFixture::new();
        fixture.save_profiles(
            ENGINE_GPT_IMAGE_2,
            ModelProviderProfilesState {
                active_provider_id: "provider-a".to_string(),
                profiles: vec![ModelProviderProfile {
                    id: "provider-a".to_string(),
                    name: "Provider A".to_string(),
                    api_key: "sk-openai-secret".to_string(),
                    endpoint_settings: default_endpoint_settings_for_model(ENGINE_GPT_IMAGE_2),
                }],
            },
        );
        let mut request = compatibility_request(ENGINE_GPT_IMAGE_2);
        request.size = None;
        request.quality = None;
        request.background = None;
        request.output_format = None;
        request.output_compression = None;
        request.moderation = None;
        request.input_fidelity = None;
        request.image_count = None;

        let prepared = prepare_compatibility_job(&fixture.db, &request)
            .expect("prepare omitted OpenAI compatibility options");

        assert_eq!(prepared.status, GenerationJobStatus::Queued);
        assert_eq!(prepared.request_options, GenerationJobOptions::default());
        fixture.cleanup();
    }

    #[test]
    fn compatibility_job_persists_missing_profile_as_initial_failed_sentinel() {
        let fixture = CompatibilityPreparationFixture::new();
        fixture.save_profiles(
            ENGINE_GPT_IMAGE_2,
            ModelProviderProfilesState {
                active_provider_id: String::new(),
                profiles: Vec::new(),
            },
        );

        let prepared =
            prepare_compatibility_job(&fixture.db, &compatibility_request(ENGINE_GPT_IMAGE_2))
                .expect("prepare visible missing-profile failure");

        assert_eq!(prepared.status, GenerationJobStatus::Failed);
        assert_eq!(prepared.provider_kind, "unresolved");
        assert_eq!(prepared.provider_profile_id, "unresolved");
        assert!(prepared.endpoint_snapshot.is_empty());
        assert_eq!(
            prepared.error_code.as_deref(),
            Some("provider_profile_missing")
        );
        assert_eq!(
            prepared.finished_at.as_deref(),
            Some(prepared.queued_at.as_str())
        );
        assert!(!prepared.retryable);
        let job_id = prepared.job_id.clone();
        let generation_id = prepared.generation_id.clone();
        {
            let mut conn = fixture.db.conn.lock().expect("lock compatibility db");
            let persisted =
                enqueue_job(&mut conn, &prepared).expect("persist visible missing-profile failure");
            assert_eq!(persisted.status, GenerationJobStatus::Failed);
            assert_eq!(
                get_job(&conn, &job_id).unwrap().status,
                GenerationJobStatus::Failed
            );
            let generation_status: String = conn
                .query_row(
                    "SELECT status FROM generations WHERE id = ?1",
                    params![generation_id],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(generation_status, "failed");
            let recovery_count: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM generation_recoveries WHERE generation_id = ?1",
                    params![generation_id],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(recovery_count, 0);
        }
        fixture.cleanup();
    }

    #[test]
    fn compatibility_job_persists_unsafe_provider_snapshot_as_secret_free_failure() {
        let fixture = CompatibilityPreparationFixture::new();
        fixture.save_profiles(
            ENGINE_GPT_IMAGE_2,
            ModelProviderProfilesState {
                active_provider_id: "provider-unsafe".to_string(),
                profiles: vec![ModelProviderProfile {
                    id: "provider-unsafe".to_string(),
                    name: "Provider Unsafe".to_string(),
                    api_key: "sk-profile-secret".to_string(),
                    endpoint_settings: EndpointSettings {
                        mode: ENDPOINT_MODE_FULL_URL.to_string(),
                        base_url: "https://unused.example.test/v1".to_string(),
                        generation_url:
                            "https://provider.example.test/images/generations?api_key=sk-endpoint-secret"
                                .to_string(),
                        edit_url: "https://provider.example.test/images/edits".to_string(),
                    },
                }],
            },
        );

        let prepared =
            prepare_compatibility_job(&fixture.db, &compatibility_request(ENGINE_GPT_IMAGE_2))
                .expect("convert unsafe provider snapshot to terminal job");

        assert_eq!(prepared.status, GenerationJobStatus::Failed);
        assert_eq!(prepared.provider_kind, "unresolved");
        assert_eq!(prepared.provider_profile_id, "unresolved");
        assert!(prepared.endpoint_snapshot.is_empty());
        assert_eq!(
            prepared.error_code.as_deref(),
            Some("provider_configuration_invalid")
        );
        let job_id = prepared.job_id.clone();
        {
            let mut conn = fixture.db.conn.lock().expect("lock compatibility db");
            enqueue_job(&mut conn, &prepared).expect("persist unsafe provider failure");
            let persisted_public: String = conn
                .query_row(
                    "SELECT request_json || source_ref_json || endpoint_snapshot
                     FROM generation_jobs WHERE id = ?1",
                    params![job_id],
                    |row| row.get(0),
                )
                .unwrap();
            assert!(!persisted_public.contains("sk-profile-secret"));
            assert!(!persisted_public.contains("sk-endpoint-secret"));
            let (status, recovery_count, queued_count): (String, i64, i64) = conn
                .query_row(
                    "SELECT j.status,
                            (SELECT COUNT(*) FROM generation_recoveries r
                             WHERE r.generation_id = j.generation_id),
                            (SELECT COUNT(*) FROM generation_jobs q WHERE q.status = 'queued')
                     FROM generation_jobs j WHERE j.id = ?1",
                    params![job_id],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )
                .unwrap();
            assert_eq!(status, "failed");
            assert_eq!(recovery_count, 0);
            assert_eq!(queued_count, 0);
        }
        fixture.cleanup();
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    enum ObservedLegacyEvent {
        Progress(String),
        Complete(String),
        Failed(String, String),
    }

    struct CompatibilityEventSink {
        events: Mutex<Vec<ObservedLegacyEvent>>,
        on_progress: Option<Arc<dyn Fn() + Send + Sync>>,
        on_complete: Option<Arc<dyn Fn() + Send + Sync>>,
        on_failed: Option<Arc<dyn Fn() + Send + Sync>>,
    }

    impl CompatibilityEventSink {
        fn new(on_progress: Option<Arc<dyn Fn() + Send + Sync>>) -> Self {
            Self {
                events: Mutex::new(Vec::new()),
                on_progress,
                on_complete: None,
                on_failed: None,
            }
        }

        fn with_complete(mut self, on_complete: Arc<dyn Fn() + Send + Sync>) -> Self {
            self.on_complete = Some(on_complete);
            self
        }

        fn with_failed(mut self, on_failed: Arc<dyn Fn() + Send + Sync>) -> Self {
            self.on_failed = Some(on_failed);
            self
        }

        fn events(&self) -> Vec<ObservedLegacyEvent> {
            self.events.lock().unwrap().clone()
        }
    }

    impl LegacyGenerationEventSink for CompatibilityEventSink {
        fn progress(&self, generation_id: &str) {
            self.events
                .lock()
                .unwrap()
                .push(ObservedLegacyEvent::Progress(generation_id.to_string()));
            if let Some(on_progress) = &self.on_progress {
                on_progress();
            }
        }

        fn complete(&self, generation_id: &str) {
            self.events
                .lock()
                .unwrap()
                .push(ObservedLegacyEvent::Complete(generation_id.to_string()));
            if let Some(on_complete) = &self.on_complete {
                on_complete();
            }
        }

        fn failed(&self, generation_id: &str, sanitized_message: &str) {
            self.events
                .lock()
                .unwrap()
                .push(ObservedLegacyEvent::Failed(
                    generation_id.to_string(),
                    sanitized_message.to_string(),
                ));
            if let Some(on_failed) = &self.on_failed {
                on_failed();
            }
        }
    }

    struct CompatibilityObservationEngine {
        calls: AtomicUsize,
        observed_api_key: Mutex<Option<String>>,
        observed_endpoint: Mutex<Option<String>>,
    }

    impl CompatibilityObservationEngine {
        fn new() -> Self {
            Self {
                calls: AtomicUsize::new(0),
                observed_api_key: Mutex::new(None),
                observed_endpoint: Mutex::new(None),
            }
        }

        fn respond(
            &self,
            api_key: &str,
            endpoint_url: &str,
            options: &GptImageRequestOptions,
        ) -> Result<ProviderAttemptBody, EngineCallError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            *self.observed_api_key.lock().unwrap() = Some(api_key.to_string());
            *self.observed_endpoint.lock().unwrap() = Some(endpoint_url.to_string());
            Ok(ProviderAttemptBody {
                body_text: r#"{"data":[{"b64_json":"decoded-by-fake"}]}"#.to_string(),
                requested_image_count: options.image_count,
            })
        }
    }

    #[async_trait::async_trait]
    impl ImageEngine for CompatibilityObservationEngine {
        async fn generate(
            &self,
            _model: &str,
            api_key: &str,
            endpoint_url: &str,
            _prompt: &str,
            options: &GptImageRequestOptions,
        ) -> Result<ProviderAttemptBody, EngineCallError> {
            self.respond(api_key, endpoint_url, options)
        }

        async fn edit(
            &self,
            _model: &str,
            api_key: &str,
            endpoint_url: &str,
            _prompt: &str,
            _source_images: &[PreparedEditImage],
            options: &GptImageRequestOptions,
        ) -> Result<ProviderAttemptBody, EngineCallError> {
            self.respond(api_key, endpoint_url, options)
        }
    }

    struct FailingCompatibilityDecoder {
        calls: AtomicUsize,
    }

    #[async_trait::async_trait]
    impl ImageResponseDecoder for FailingCompatibilityDecoder {
        async fn decode_and_download(
            &self,
            _response: &ProviderAttemptResponse,
            _cancellation: &CancellationProbe,
        ) -> Result<Vec<Vec<u8>>, GenerationExecutionError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Err(compatibility_execution_error(
                "response_decode_failed",
                "The provider response could not be decoded",
                "response_decode",
            ))
        }
    }

    #[tokio::test]
    async fn compatibility_executor_claims_exact_job_and_uses_current_key_for_snapshotted_profile()
    {
        let fixture = CompatibilityPreparationFixture::new();
        let old_endpoint = "https://provider-a.example.test/images/generations";
        fixture.save_profiles(
            ENGINE_GPT_IMAGE_2,
            ModelProviderProfilesState {
                active_provider_id: "provider-a".to_string(),
                profiles: vec![
                    ModelProviderProfile {
                        id: "provider-a".to_string(),
                        name: "Provider A".to_string(),
                        api_key: "sk-a-enqueue".to_string(),
                        endpoint_settings: EndpointSettings {
                            mode: ENDPOINT_MODE_FULL_URL.to_string(),
                            base_url: "https://unused-a.example.test/v1".to_string(),
                            generation_url: old_endpoint.to_string(),
                            edit_url: "https://provider-a.example.test/images/edits".to_string(),
                        },
                    },
                    ModelProviderProfile {
                        id: "provider-b".to_string(),
                        name: "Provider B".to_string(),
                        api_key: "sk-b-current".to_string(),
                        endpoint_settings: default_endpoint_settings_for_model(ENGINE_GPT_IMAGE_2),
                    },
                ],
            },
        );
        let older =
            prepare_compatibility_job(&fixture.db, &compatibility_request(ENGINE_GPT_IMAGE_2))
                .unwrap();
        {
            let mut conn = fixture.db.conn.lock().unwrap();
            enqueue_job(&mut conn, &older).expect("enqueue older FIFO job");
        }
        let exact =
            prepare_compatibility_job(&fixture.db, &compatibility_request(ENGINE_GPT_IMAGE_2))
                .unwrap();
        let exact_job_id = exact.job_id.clone();
        let exact_generation_id = exact.generation_id.clone();
        let older_job_id = older.job_id.clone();
        let db_for_switch = Arc::clone(&fixture.db);
        let running_job_id = exact_job_id.clone();
        let running_generation_id = exact_generation_id.clone();
        let queued_job_id = older_job_id.clone();
        let switch_calls = Arc::new(AtomicUsize::new(0));
        let observed_switch_calls = Arc::clone(&switch_calls);
        let progress_sink = CompatibilityEventSink::new(Some(Arc::new(move || {
            observed_switch_calls.fetch_add(1, Ordering::SeqCst);
            {
                let conn = db_for_switch.conn.lock().unwrap();
                assert_eq!(
                    get_job(&conn, &running_job_id).unwrap().status,
                    GenerationJobStatus::Running
                );
                assert_eq!(
                    get_job(&conn, &queued_job_id).unwrap().status,
                    GenerationJobStatus::Queued
                );
                let generation_status: String = conn
                    .query_row(
                        "SELECT status FROM generations WHERE id = ?1",
                        params![running_generation_id],
                        |row| row.get(0),
                    )
                    .unwrap();
                assert_eq!(generation_status, "running");
            }
            let mut state = settings::read_model_provider_profiles_state(
                db_for_switch.as_ref(),
                ENGINE_GPT_IMAGE_2,
            )
            .unwrap();
            state.active_provider_id = "provider-b".to_string();
            let profile_a = state
                .profiles
                .iter_mut()
                .find(|profile| profile.id == "provider-a")
                .unwrap();
            profile_a.api_key = "sk-a-current".to_string();
            profile_a.endpoint_settings.generation_url =
                "https://provider-a.example.test/changed".to_string();
            settings::save_model_provider_profiles_state(
                db_for_switch.as_ref(),
                ENGINE_GPT_IMAGE_2,
                state,
            )
            .unwrap();
        })));
        let db_for_complete = Arc::clone(&fixture.db);
        let completed_job_id = exact_job_id.clone();
        let completed_generation_id = exact_generation_id.clone();
        let sink = progress_sink.with_complete(Arc::new(move || {
            let conn = db_for_complete.conn.lock().unwrap();
            assert_eq!(
                get_job(&conn, &completed_job_id).unwrap().status,
                GenerationJobStatus::Completed
            );
            let generation_status: String = conn
                .query_row(
                    "SELECT status FROM generations WHERE id = ?1",
                    params![completed_generation_id],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(generation_status, "completed");
            let (started_logs, completed_logs): (i64, i64) = conn
                .query_row(
                    "SELECT
                        SUM(CASE WHEN message LIKE 'Started —%' THEN 1 ELSE 0 END),
                        SUM(CASE WHEN message LIKE 'Completed —%' THEN 1 ELSE 0 END)
                     FROM logs
                     WHERE generation_id = ?1 AND log_type = 'generation' AND level = 'info'",
                    params![completed_generation_id],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .unwrap();
            assert_eq!(started_logs, 1);
            assert_eq!(completed_logs, 1);
        }));
        let engine = CompatibilityObservationEngine::new();
        let decoder = FakeLocalDecoder {
            calls: Arc::new(AtomicUsize::new(0)),
            images: vec![jpeg_bytes()],
        };
        let artifact_store = FileResponseArtifactStore::new(fixture.root.join("responses"));
        let file_store = LocalGenerationFileStore::new(fixture.root.join("app-data"));

        let result = execute_compatibility_job(
            fixture.db.as_ref(),
            &engine,
            &artifact_store,
            &decoder,
            &file_store,
            &sink,
            exact,
        )
        .await
        .expect("execute exact compatibility job");

        assert_eq!(result.generation_id, exact_generation_id);
        assert_eq!(engine.calls.load(Ordering::SeqCst), 1);
        assert_eq!(
            engine.observed_api_key.lock().unwrap().as_deref(),
            Some("sk-a-current")
        );
        assert_eq!(
            engine.observed_endpoint.lock().unwrap().as_deref(),
            Some(old_endpoint)
        );
        assert_eq!(switch_calls.load(Ordering::SeqCst), 1);
        assert_eq!(
            sink.events(),
            vec![
                ObservedLegacyEvent::Progress(exact_generation_id.clone()),
                ObservedLegacyEvent::Complete(exact_generation_id.clone()),
            ]
        );
        {
            let conn = fixture.db.conn.lock().unwrap();
            assert_eq!(
                get_job(&conn, &exact_job_id).unwrap().status,
                GenerationJobStatus::Completed
            );
            assert_eq!(
                get_job(&conn, &older_job_id).unwrap().status,
                GenerationJobStatus::Queued
            );
            let persisted_execution_projection: String = conn
                .query_row(
                    "SELECT j.request_json || j.source_ref_json || j.provider_kind ||
                            j.provider_profile_id || j.endpoint_snapshot ||
                            COALESCE(j.error_message, '') || g.source_image_paths ||
                            g.request_metadata || COALESCE(g.error_message, '') ||
                            COALESCE((SELECT GROUP_CONCAT(message || COALESCE(metadata, ''))
                                      FROM logs WHERE generation_id = g.id), '') ||
                            COALESCE((SELECT GROUP_CONCAT(file_path || thumbnail_path)
                                      FROM images WHERE generation_id = g.id), '')
                     FROM generation_jobs j
                     JOIN generations g ON g.id = j.generation_id
                     WHERE j.id = ?1",
                    params![exact_job_id],
                    |row| row.get(0),
                )
                .unwrap();
            for secret in ["sk-a-enqueue", "sk-a-current", "sk-b-current"] {
                assert!(!persisted_execution_projection.contains(secret));
            }
        }
        drop(sink);
        fixture.cleanup();
    }

    #[tokio::test]
    async fn compatibility_executor_terminalizes_deleted_profile_before_failed_event() {
        let fixture = CompatibilityPreparationFixture::new();
        fixture.save_profiles(
            ENGINE_GPT_IMAGE_2,
            ModelProviderProfilesState {
                active_provider_id: "provider-a".to_string(),
                profiles: vec![ModelProviderProfile {
                    id: "provider-a".to_string(),
                    name: "Provider A".to_string(),
                    api_key: "sk-a-current".to_string(),
                    endpoint_settings: default_endpoint_settings_for_model(ENGINE_GPT_IMAGE_2),
                }],
            },
        );
        let exact =
            prepare_compatibility_job(&fixture.db, &compatibility_request(ENGINE_GPT_IMAGE_2))
                .unwrap();
        let job_id = exact.job_id.clone();
        let generation_id = exact.generation_id.clone();
        let db_for_delete = Arc::clone(&fixture.db);
        let progress_sink = CompatibilityEventSink::new(Some(Arc::new(move || {
            settings::save_model_provider_profiles_state(
                db_for_delete.as_ref(),
                ENGINE_GPT_IMAGE_2,
                ModelProviderProfilesState {
                    active_provider_id: String::new(),
                    profiles: Vec::new(),
                },
            )
            .unwrap();
        })));
        let db_for_failed = Arc::clone(&fixture.db);
        let failed_job_id = job_id.clone();
        let failed_generation_id = generation_id.clone();
        let sink = progress_sink.with_failed(Arc::new(move || {
            let conn = db_for_failed.conn.lock().unwrap();
            let job = get_job(&conn, &failed_job_id).unwrap();
            assert_eq!(job.status, GenerationJobStatus::Failed);
            assert_eq!(job.error_code.as_deref(), Some("provider_profile_missing"));
            let state: String = conn
                .query_row(
                    "SELECT status FROM generations WHERE id = ?1",
                    params![failed_generation_id],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(state, "failed");
            let recovery_count: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM generation_recoveries WHERE generation_id = ?1",
                    params![failed_generation_id],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(recovery_count, 0);
            let error_logs: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM logs
                     WHERE generation_id = ?1 AND log_type = 'generation' AND level = 'error'",
                    params![failed_generation_id],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(error_logs, 1);
        }));
        let engine = CompatibilityObservationEngine::new();
        let decoder = FakeLocalDecoder {
            calls: Arc::new(AtomicUsize::new(0)),
            images: vec![jpeg_bytes()],
        };
        let artifact_store = FileResponseArtifactStore::new(fixture.root.join("responses"));
        let file_store = LocalGenerationFileStore::new(fixture.root.join("app-data"));

        let error = execute_compatibility_job(
            fixture.db.as_ref(),
            &engine,
            &artifact_store,
            &decoder,
            &file_store,
            &sink,
            exact,
        )
        .await
        .expect_err("deleted stored profile must terminalize without provider call");

        assert!(matches!(error, AppError::ProviderProfileNotFound { .. }));
        assert_eq!(engine.calls.load(Ordering::SeqCst), 0);
        assert_eq!(
            sink.events(),
            vec![
                ObservedLegacyEvent::Progress(generation_id.clone()),
                ObservedLegacyEvent::Failed(
                    generation_id.clone(),
                    "The selected provider profile is unavailable".to_string(),
                ),
            ]
        );
        drop(sink);
        fixture.cleanup();
    }

    #[tokio::test]
    async fn compatibility_executor_persists_initial_failure_before_legacy_failed_event() {
        let fixture = CompatibilityPreparationFixture::new();
        fixture.save_profiles(
            ENGINE_GPT_IMAGE_2,
            ModelProviderProfilesState {
                active_provider_id: String::new(),
                profiles: Vec::new(),
            },
        );
        let failed =
            prepare_compatibility_job(&fixture.db, &compatibility_request(ENGINE_GPT_IMAGE_2))
                .unwrap();
        let job_id = failed.job_id.clone();
        let generation_id = failed.generation_id.clone();
        let db_for_failed = Arc::clone(&fixture.db);
        let failed_job_id = job_id.clone();
        let failed_generation_id = generation_id.clone();
        let sink = CompatibilityEventSink::new(None).with_failed(Arc::new(move || {
            let conn = db_for_failed.conn.lock().unwrap();
            let job = get_job(&conn, &failed_job_id).unwrap();
            assert_eq!(job.status, GenerationJobStatus::Failed);
            assert!(job.started_at.is_none());
            let recovery_count: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM generation_recoveries WHERE generation_id = ?1",
                    params![failed_generation_id],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(recovery_count, 0);
        }));
        let engine = CompatibilityObservationEngine::new();
        let decoder = FakeLocalDecoder {
            calls: Arc::new(AtomicUsize::new(0)),
            images: vec![jpeg_bytes()],
        };
        let artifact_store = FileResponseArtifactStore::new(fixture.root.join("responses"));
        let file_store = LocalGenerationFileStore::new(fixture.root.join("app-data"));

        let error = execute_compatibility_job(
            fixture.db.as_ref(),
            &engine,
            &artifact_store,
            &decoder,
            &file_store,
            &sink,
            failed,
        )
        .await
        .expect_err("initial missing profile remains visible and fails synchronously");

        assert!(matches!(error, AppError::ProviderProfileNotFound { .. }));
        assert_eq!(engine.calls.load(Ordering::SeqCst), 0);
        assert_eq!(
            sink.events(),
            vec![ObservedLegacyEvent::Failed(
                generation_id,
                "The selected provider profile is unavailable".to_string(),
            )]
        );
        drop(sink);
        fixture.cleanup();
    }

    #[tokio::test]
    async fn compatibility_executor_persists_empty_api_key_as_initial_failed_pair() {
        let fixture = CompatibilityPreparationFixture::new();
        fixture.save_profiles(
            ENGINE_GPT_IMAGE_2,
            ModelProviderProfilesState {
                active_provider_id: "provider-a".to_string(),
                profiles: vec![ModelProviderProfile {
                    id: "provider-a".to_string(),
                    name: "Provider A".to_string(),
                    api_key: "   ".to_string(),
                    endpoint_settings: default_endpoint_settings_for_model(ENGINE_GPT_IMAGE_2),
                }],
            },
        );
        let failed =
            prepare_compatibility_job(&fixture.db, &compatibility_request(ENGINE_GPT_IMAGE_2))
                .unwrap();
        assert_eq!(failed.status, GenerationJobStatus::Failed);
        assert_eq!(failed.provider_kind, "openai");
        assert_eq!(failed.provider_profile_id, "provider-a");
        assert!(executable_provider_snapshot_is_valid(
            &failed.provider_kind,
            &failed.provider_profile_id,
            &failed.endpoint_snapshot,
        ));
        assert_eq!(
            failed.error_code.as_deref(),
            Some("provider_configuration_invalid")
        );

        let job_id = failed.job_id.clone();
        let generation_id = failed.generation_id.clone();
        let db_for_failed = Arc::clone(&fixture.db);
        let failed_job_id = job_id.clone();
        let failed_generation_id = generation_id.clone();
        let sink = CompatibilityEventSink::new(None).with_failed(Arc::new(move || {
            let conn = db_for_failed.conn.lock().unwrap();
            let job = get_job(&conn, &failed_job_id).unwrap();
            assert_eq!(job.status, GenerationJobStatus::Failed);
            assert_eq!(
                job.error_code.as_deref(),
                Some("provider_configuration_invalid")
            );
            assert!(job.started_at.is_none());
            let (generation_status, recovery_count): (String, i64) = conn
                .query_row(
                    "SELECT g.status,
                            (SELECT COUNT(*) FROM generation_recoveries r
                             WHERE r.generation_id = g.id)
                     FROM generations g WHERE g.id = ?1",
                    params![failed_generation_id],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .unwrap();
            assert_eq!(generation_status, "failed");
            assert_eq!(recovery_count, 0);
        }));
        let engine = CompatibilityObservationEngine::new();
        let decoder = FakeLocalDecoder {
            calls: Arc::new(AtomicUsize::new(0)),
            images: vec![jpeg_bytes()],
        };
        let artifact_store = FileResponseArtifactStore::new(fixture.root.join("responses"));
        let file_store = LocalGenerationFileStore::new(fixture.root.join("app-data"));

        let error = execute_compatibility_job(
            fixture.db.as_ref(),
            &engine,
            &artifact_store,
            &decoder,
            &file_store,
            &sink,
            failed,
        )
        .await
        .expect_err("empty API key must be visible as an initial failure");

        assert!(matches!(error, AppError::ApiKeyNotSet { .. }));
        assert_eq!(engine.calls.load(Ordering::SeqCst), 0);
        assert_eq!(
            sink.events(),
            vec![ObservedLegacyEvent::Failed(
                generation_id,
                "The image provider configuration is invalid".to_string(),
            )]
        );
        drop(sink);
        fixture.cleanup();
    }

    #[tokio::test]
    async fn compatibility_executor_terminalizes_invalidated_edit_source_without_provider_call() {
        let fixture = CompatibilityPreparationFixture::new();
        fixture.save_profiles(
            ENGINE_GPT_IMAGE_2,
            ModelProviderProfilesState {
                active_provider_id: "provider-a".to_string(),
                profiles: vec![ModelProviderProfile {
                    id: "provider-a".to_string(),
                    name: "Provider A".to_string(),
                    api_key: "sk-a-current".to_string(),
                    endpoint_settings: default_endpoint_settings_for_model(ENGINE_GPT_IMAGE_2),
                }],
            },
        );
        let source_path = fixture.root.join("source.jpg");
        std::fs::write(&source_path, jpeg_bytes()).unwrap();
        let canonical_source = source_path.canonicalize().unwrap();
        let mut request = compatibility_request(ENGINE_GPT_IMAGE_2);
        request.kind = GenerationLifecycleKind::Edit;
        request.source_image_paths = vec![canonical_source.to_string_lossy().to_string()];
        let exact = prepare_compatibility_job(&fixture.db, &request).unwrap();
        let job_id = exact.job_id.clone();
        let generation_id = exact.generation_id.clone();
        let delete_source = canonical_source.clone();
        let progress_sink = CompatibilityEventSink::new(Some(Arc::new(move || {
            std::fs::remove_file(&delete_source).expect("invalidate edit source after claim");
        })));
        let db_for_failed = Arc::clone(&fixture.db);
        let failed_job_id = job_id.clone();
        let failed_generation_id = generation_id.clone();
        let sink = progress_sink.with_failed(Arc::new(move || {
            let conn = db_for_failed.conn.lock().unwrap();
            let job = get_job(&conn, &failed_job_id).unwrap();
            assert_eq!(job.status, GenerationJobStatus::Failed);
            assert_eq!(job.error_code.as_deref(), Some("source_image_invalid"));
            let (generation_status, recovery_count): (String, i64) = conn
                .query_row(
                    "SELECT g.status,
                            (SELECT COUNT(*) FROM generation_recoveries r
                             WHERE r.generation_id = g.id)
                     FROM generations g WHERE g.id = ?1",
                    params![failed_generation_id],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .unwrap();
            assert_eq!(generation_status, "failed");
            assert_eq!(recovery_count, 0);
        }));
        let engine = CompatibilityObservationEngine::new();
        let decoder_calls = Arc::new(AtomicUsize::new(0));
        let decoder = FakeLocalDecoder {
            calls: Arc::clone(&decoder_calls),
            images: vec![jpeg_bytes()],
        };
        let artifact_store = FileResponseArtifactStore::new(fixture.root.join("responses"));
        let file_store = LocalGenerationFileStore::new(fixture.root.join("app-data"));

        let error = execute_compatibility_job(
            fixture.db.as_ref(),
            &engine,
            &artifact_store,
            &decoder,
            &file_store,
            &sink,
            exact,
        )
        .await
        .expect_err("invalidated edit source must fail before provider HTTP");

        assert!(matches!(error, AppError::Validation { .. }));
        assert_eq!(engine.calls.load(Ordering::SeqCst), 0);
        assert_eq!(decoder_calls.load(Ordering::SeqCst), 0);
        assert_eq!(
            sink.events(),
            vec![
                ObservedLegacyEvent::Progress(generation_id.clone()),
                ObservedLegacyEvent::Failed(
                    generation_id,
                    "A source image is unavailable or invalid".to_string(),
                ),
            ]
        );
        drop(sink);
        fixture.cleanup();
    }

    #[tokio::test]
    async fn compatibility_executor_preserves_response_ready_recovery_on_local_failure() {
        let fixture = CompatibilityPreparationFixture::new();
        fixture.save_profiles(
            ENGINE_GPT_IMAGE_2,
            ModelProviderProfilesState {
                active_provider_id: "provider-a".to_string(),
                profiles: vec![ModelProviderProfile {
                    id: "provider-a".to_string(),
                    name: "Provider A".to_string(),
                    api_key: "sk-a-current".to_string(),
                    endpoint_settings: default_endpoint_settings_for_model(ENGINE_GPT_IMAGE_2),
                }],
            },
        );
        let exact =
            prepare_compatibility_job(&fixture.db, &compatibility_request(ENGINE_GPT_IMAGE_2))
                .unwrap();
        let job_id = exact.job_id.clone();
        let generation_id = exact.generation_id.clone();
        let sink_without_progress_hook = CompatibilityEventSink::new(None);
        let db_for_failed = Arc::clone(&fixture.db);
        let failed_job_id = job_id.clone();
        let failed_generation_id = generation_id.clone();
        let sink = sink_without_progress_hook.with_failed(Arc::new(move || {
            let conn = db_for_failed.conn.lock().unwrap();
            let job = get_job(&conn, &failed_job_id).unwrap();
            assert_eq!(job.status, GenerationJobStatus::Failed);
            assert_eq!(job.error_code.as_deref(), Some("response_decode_failed"));
            let (generation_status, request_state, response_file): (
                String,
                String,
                Option<String>,
            ) = conn
                .query_row(
                    "SELECT g.status, r.request_state, r.response_file
                     FROM generations g
                     JOIN generation_recoveries r ON r.generation_id = g.id
                     WHERE g.id = ?1",
                    params![failed_generation_id],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )
                .unwrap();
            assert_eq!(generation_status, "failed");
            assert_eq!(request_state, RECOVERY_STATE_RESPONSE_READY);
            let response_file = response_file.expect("verified recovery path");
            assert!(Path::new(&response_file).is_file());
        }));
        let engine = CompatibilityObservationEngine::new();
        let decoder = FailingCompatibilityDecoder {
            calls: AtomicUsize::new(0),
        };
        let artifact_store = FileResponseArtifactStore::new(fixture.root.join("responses"));
        let file_store = LocalGenerationFileStore::new(fixture.root.join("app-data"));

        let error = execute_compatibility_job(
            fixture.db.as_ref(),
            &engine,
            &artifact_store,
            &decoder,
            &file_store,
            &sink,
            exact,
        )
        .await
        .expect_err("local decode failure must retain response-ready recovery");

        assert!(matches!(error, AppError::Validation { .. }));
        assert_eq!(engine.calls.load(Ordering::SeqCst), 1);
        assert_eq!(decoder.calls.load(Ordering::SeqCst), 1);
        assert_eq!(
            sink.events(),
            vec![
                ObservedLegacyEvent::Progress(generation_id.clone()),
                ObservedLegacyEvent::Failed(
                    generation_id,
                    "The provider response could not be decoded".to_string(),
                ),
            ]
        );
        drop(sink);
        fixture.cleanup();
    }

    #[tokio::test]
    async fn compatibility_executor_marks_ambiguous_provider_outcome_interrupted() {
        let fixture = CompatibilityPreparationFixture::new();
        fixture.save_profiles(
            ENGINE_GPT_IMAGE_2,
            ModelProviderProfilesState {
                active_provider_id: "provider-a".to_string(),
                profiles: vec![ModelProviderProfile {
                    id: "provider-a".to_string(),
                    name: "Provider A".to_string(),
                    api_key: "sk-a-current".to_string(),
                    endpoint_settings: default_endpoint_settings_for_model(ENGINE_GPT_IMAGE_2),
                }],
            },
        );
        let exact =
            prepare_compatibility_job(&fixture.db, &compatibility_request(ENGINE_GPT_IMAGE_2))
                .unwrap();
        let job_id = exact.job_id.clone();
        let generation_id = exact.generation_id.clone();
        let db_for_failed = Arc::clone(&fixture.db);
        let interrupted_job_id = job_id.clone();
        let interrupted_generation_id = generation_id.clone();
        let sink = CompatibilityEventSink::new(None).with_failed(Arc::new(move || {
            let conn = db_for_failed.conn.lock().unwrap();
            let job = get_job(&conn, &interrupted_job_id).unwrap();
            assert_eq!(job.status, GenerationJobStatus::Interrupted);
            assert_eq!(job.error_code.as_deref(), Some("provider_outcome_unknown"));
            assert!(job.retryable);
            let (generation_status, recovery_count): (String, i64) = conn
                .query_row(
                    "SELECT g.status,
                            (SELECT COUNT(*) FROM generation_recoveries r
                             WHERE r.generation_id = g.id)
                     FROM generations g WHERE g.id = ?1",
                    params![interrupted_generation_id],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .unwrap();
            assert_eq!(generation_status, "interrupted");
            assert_eq!(recovery_count, 0);
        }));
        let engine_calls = Arc::new(AtomicUsize::new(0));
        let engine = FakeCoreEngine {
            calls: Arc::clone(&engine_calls),
            outcome: CoreEngineOutcome::Error(EngineCallError::provider_outcome_unknown(
                "connection closed after send",
            )),
        };
        let decoder = FakeLocalDecoder {
            calls: Arc::new(AtomicUsize::new(0)),
            images: vec![jpeg_bytes()],
        };
        let artifact_store = FileResponseArtifactStore::new(fixture.root.join("responses"));
        let file_store = LocalGenerationFileStore::new(fixture.root.join("app-data"));

        let error = execute_compatibility_job(
            fixture.db.as_ref(),
            &engine,
            &artifact_store,
            &decoder,
            &file_store,
            &sink,
            exact,
        )
        .await
        .expect_err("ambiguous provider outcome must be interrupted");

        assert!(matches!(error, AppError::Validation { .. }));
        assert_eq!(engine_calls.load(Ordering::SeqCst), 1);
        assert_eq!(
            sink.events(),
            vec![
                ObservedLegacyEvent::Progress(generation_id.clone()),
                ObservedLegacyEvent::Failed(
                    generation_id,
                    "The image provider outcome could not be confirmed".to_string(),
                ),
            ]
        );
        drop(sink);
        fixture.cleanup();
    }

    #[tokio::test]
    async fn compatibility_executor_preserves_manual_retryability_when_retry_after_is_invalid() {
        let fixture = CompatibilityPreparationFixture::new();
        fixture.save_profiles(
            ENGINE_GPT_IMAGE_2,
            ModelProviderProfilesState {
                active_provider_id: "provider-a".to_string(),
                profiles: vec![ModelProviderProfile {
                    id: "provider-a".to_string(),
                    name: "Provider A".to_string(),
                    api_key: "sk-a-current".to_string(),
                    endpoint_settings: default_endpoint_settings_for_model(ENGINE_GPT_IMAGE_2),
                }],
            },
        );
        let exact =
            prepare_compatibility_job(&fixture.db, &compatibility_request(ENGINE_GPT_IMAGE_2))
                .unwrap();
        let job_id = exact.job_id.clone();
        let generation_id = exact.generation_id.clone();
        let sink = CompatibilityEventSink::new(None);
        let engine_calls = Arc::new(AtomicUsize::new(0));
        let rate_limit_error =
            EngineCallError::from_http_status(429, Some(RetryAfterHint::Invalid));
        assert!(!rate_limit_error.safe_to_retry);
        let engine = FakeCoreEngine {
            calls: Arc::clone(&engine_calls),
            outcome: CoreEngineOutcome::Error(rate_limit_error),
        };
        let decoder = FakeLocalDecoder {
            calls: Arc::new(AtomicUsize::new(0)),
            images: vec![jpeg_bytes()],
        };
        let artifact_store = FileResponseArtifactStore::new(fixture.root.join("responses"));
        let file_store = LocalGenerationFileStore::new(fixture.root.join("app-data"));

        execute_compatibility_job(
            fixture.db.as_ref(),
            &engine,
            &artifact_store,
            &decoder,
            &file_store,
            &sink,
            exact,
        )
        .await
        .expect_err("rate limit remains a single failed compatibility attempt");

        assert_eq!(engine_calls.load(Ordering::SeqCst), 1);
        {
            let conn = fixture.db.conn.lock().unwrap();
            let job = get_job(&conn, &job_id).unwrap();
            assert_eq!(job.status, GenerationJobStatus::Failed);
            assert_eq!(job.error_code.as_deref(), Some("rate_limited"));
            assert!(job.retryable);
        }
        assert_eq!(
            sink.events(),
            vec![
                ObservedLegacyEvent::Progress(generation_id.clone()),
                ObservedLegacyEvent::Failed(
                    generation_id,
                    "The image provider rate limit was reached".to_string(),
                ),
            ]
        );
        fixture.cleanup();
    }

    async fn perform_prepared_test_attempt(
        engine: &dyn ImageEngine,
        snapshot: &GenerationExecutionSnapshot,
        credentials: &ProviderExecutionCredentials,
    ) -> Result<ProviderAttemptBody, GenerationExecutionError> {
        let prepared = prepare_provider_attempt(snapshot).await?;
        perform_provider_http_attempt(engine, snapshot, credentials, &prepared).await
    }

    struct FakeSingleAttemptEngine {
        calls: Arc<AtomicUsize>,
    }

    #[async_trait::async_trait]
    impl ImageEngine for FakeSingleAttemptEngine {
        async fn generate(
            &self,
            _model: &str,
            api_key: &str,
            endpoint_url: &str,
            _prompt: &str,
            options: &GptImageRequestOptions,
        ) -> Result<ProviderAttemptBody, EngineCallError> {
            assert_eq!(api_key, "ephemeral-key");
            assert_eq!(endpoint_url, "https://example.test/images/generations");
            self.calls.fetch_add(1, Ordering::SeqCst);
            Ok(ProviderAttemptBody {
                body_text: r#"{"data":[]}"#.to_string(),
                requested_image_count: options.image_count,
            })
        }

        async fn edit(
            &self,
            _model: &str,
            _api_key: &str,
            _endpoint_url: &str,
            _prompt: &str,
            _source_images: &[PreparedEditImage],
            _options: &GptImageRequestOptions,
        ) -> Result<ProviderAttemptBody, EngineCallError> {
            panic!("generate fixture must not call edit")
        }
    }

    struct FakeArtifactStore {
        calls: Arc<AtomicUsize>,
        root: PathBuf,
    }

    #[async_trait::async_trait]
    impl ResponseArtifactStore for FakeArtifactStore {
        fn expected_response_path(
            &self,
            context: &GenerationExecutionContext,
        ) -> Result<PathBuf, GenerationExecutionError> {
            FileResponseArtifactStore::new(self.root.clone()).expected_response_path(context)
        }

        async fn persist_verified_response(
            &self,
            context: &GenerationExecutionContext,
            body: ProviderAttemptBody,
        ) -> Result<ProviderAttemptResponse, GenerationExecutionError> {
            assert_eq!(context.job_id, "job-1");
            self.calls.fetch_add(1, Ordering::SeqCst);
            FileResponseArtifactStore::new(self.root.clone())
                .persist_verified_response(context, body)
                .await
        }

        async fn load_verified_response(
            &self,
            context: &GenerationExecutionContext,
            path: &Path,
        ) -> Result<ProviderAttemptResponse, GenerationExecutionError> {
            FileResponseArtifactStore::new(self.root.clone())
                .load_verified_response(context, path)
                .await
        }
    }

    struct MissingFileArtifactStore {
        path: PathBuf,
    }

    #[async_trait::async_trait]
    impl ResponseArtifactStore for MissingFileArtifactStore {
        fn expected_response_path(
            &self,
            _context: &GenerationExecutionContext,
        ) -> Result<PathBuf, GenerationExecutionError> {
            Ok(self.path.clone())
        }

        async fn persist_verified_response(
            &self,
            _context: &GenerationExecutionContext,
            body: ProviderAttemptBody,
        ) -> Result<ProviderAttemptResponse, GenerationExecutionError> {
            Ok(ProviderAttemptResponse {
                response_size: body.body_text.len() as u64,
                response_sha256: FileResponseArtifactStore::response_hash(&body.body_text),
                response_file: self.path.to_string_lossy().to_string(),
                body_text: body.body_text,
                requested_image_count: body.requested_image_count,
            })
        }

        async fn load_verified_response(
            &self,
            _context: &GenerationExecutionContext,
            _path: &Path,
        ) -> Result<ProviderAttemptResponse, GenerationExecutionError> {
            Err(GenerationExecutionError::response_artifact())
        }
    }

    struct CanonicalAliasArtifactStore {
        root: PathBuf,
    }

    #[async_trait::async_trait]
    impl ResponseArtifactStore for CanonicalAliasArtifactStore {
        fn expected_response_path(
            &self,
            _context: &GenerationExecutionContext,
        ) -> Result<PathBuf, GenerationExecutionError> {
            Ok(self
                .root
                .join("alias-component")
                .join("..")
                .join("artifact.response.json"))
        }

        async fn persist_verified_response(
            &self,
            _context: &GenerationExecutionContext,
            body: ProviderAttemptBody,
        ) -> Result<ProviderAttemptResponse, GenerationExecutionError> {
            std::fs::create_dir_all(self.root.join("alias-component"))
                .map_err(|_| GenerationExecutionError::response_artifact())?;
            let actual_path = self.root.join("artifact.response.json");
            std::fs::write(&actual_path, body.body_text.as_bytes())
                .map_err(|_| GenerationExecutionError::response_artifact())?;
            Ok(ProviderAttemptResponse {
                response_size: body.body_text.len() as u64,
                response_sha256: FileResponseArtifactStore::response_hash(&body.body_text),
                response_file: actual_path.to_string_lossy().to_string(),
                body_text: body.body_text,
                requested_image_count: body.requested_image_count,
            })
        }

        async fn load_verified_response(
            &self,
            _context: &GenerationExecutionContext,
            _path: &Path,
        ) -> Result<ProviderAttemptResponse, GenerationExecutionError> {
            Err(GenerationExecutionError::response_artifact())
        }
    }

    struct WrongOnDiskArtifactStore {
        root: PathBuf,
        load_calls: Arc<AtomicUsize>,
    }

    #[async_trait::async_trait]
    impl ResponseArtifactStore for WrongOnDiskArtifactStore {
        fn expected_response_path(
            &self,
            context: &GenerationExecutionContext,
        ) -> Result<PathBuf, GenerationExecutionError> {
            FileResponseArtifactStore::new(self.root.clone()).expected_response_path(context)
        }

        async fn persist_verified_response(
            &self,
            context: &GenerationExecutionContext,
            body: ProviderAttemptBody,
        ) -> Result<ProviderAttemptResponse, GenerationExecutionError> {
            let response_path = self.expected_response_path(context)?;
            std::fs::create_dir_all(
                response_path
                    .parent()
                    .ok_or_else(GenerationExecutionError::response_artifact)?,
            )
            .map_err(|_| GenerationExecutionError::response_artifact())?;
            std::fs::write(&response_path, b"{}")
                .map_err(|_| GenerationExecutionError::response_artifact())?;
            Ok(ProviderAttemptResponse {
                response_size: body.body_text.len() as u64,
                response_sha256: FileResponseArtifactStore::response_hash(&body.body_text),
                response_file: response_path.to_string_lossy().to_string(),
                body_text: body.body_text,
                requested_image_count: body.requested_image_count,
            })
        }

        async fn load_verified_response(
            &self,
            context: &GenerationExecutionContext,
            path: &Path,
        ) -> Result<ProviderAttemptResponse, GenerationExecutionError> {
            self.load_calls.fetch_add(1, Ordering::SeqCst);
            FileResponseArtifactStore::new(self.root.clone())
                .load_verified_response(context, path)
                .await
        }
    }

    struct FakeLocalDecoder {
        calls: Arc<AtomicUsize>,
        images: Vec<Vec<u8>>,
    }

    struct PrecreatedTestFixture {
        db: Arc<Database>,
        db_path: PathBuf,
        snapshot: GenerationExecutionSnapshot,
        root: PathBuf,
    }

    #[derive(Clone, Debug, Eq, PartialEq)]
    struct FencedResponseProjection {
        request_state: String,
        expected_response_file: Option<String>,
        response_file: Option<String>,
        response_size: Option<i64>,
        response_sha256: Option<String>,
        recovery_updated_at: String,
        job_status: String,
        job_stage: String,
        last_heartbeat_at: Option<String>,
        generation_status: String,
        image_count: i64,
    }

    impl PrecreatedTestFixture {
        fn new(requested_image_count: u8) -> Self {
            Self::new_with_request(
                requested_image_count,
                GenerationJobRequestKind::Generate,
                Vec::new(),
            )
        }

        fn new_with_request(
            requested_image_count: u8,
            kind: GenerationJobRequestKind,
            source_image_paths: Vec<String>,
        ) -> Self {
            let root = std::env::temp_dir().join(format!(
                "astro-studio-precreated-lifecycle-test-{}",
                uuid::Uuid::new_v4()
            ));
            std::fs::create_dir_all(&root).expect("create lifecycle fixture root");
            let db_path = root.join("astro_studio.db");
            let db = Arc::new(Database::open(&db_path).expect("open test db"));
            db.run_migrations().expect("migrate test db");
            let request_kind = kind.as_str().to_string();
            let source_kind = request_kind.clone();
            let prepared = PreparedGenerationJob {
                job_id: uuid::Uuid::new_v4().to_string(),
                client_request_id: uuid::Uuid::new_v4().to_string(),
                generation_id: uuid::Uuid::new_v4().to_string(),
                requested_conversation_id: None,
                requested_project_id: Some("default".to_string()),
                prompt: "draw a nebula".to_string(),
                model: "gpt-image-2".to_string(),
                request_kind,
                size: "1024x1024".to_string(),
                quality: "high".to_string(),
                background: "auto".to_string(),
                output_format: "png".to_string(),
                output_compression: 100,
                moderation: "auto".to_string(),
                input_fidelity: "high".to_string(),
                image_count: i32::from(requested_image_count),
                stream: false,
                partial_images: 0,
                source_image_paths,
                request_options: GenerationJobOptions {
                    size: Some("1024x1024".to_string()),
                    quality: Some("high".to_string()),
                    background: Some("auto".to_string()),
                    output_format: Some("png".to_string()),
                    output_compression: Some(100),
                    moderation: Some("auto".to_string()),
                    input_fidelity: Some("high".to_string()),
                    stream: Some(false),
                    partial_images: Some(0),
                    image_count: Some(requested_image_count),
                },
                parent_job_id: None,
                source_kind,
                source_ref: serde_json::json!({ "id": "precreated-test" }),
                provider_kind: "openai".to_string(),
                provider_profile_id: "profile-1".to_string(),
                endpoint_snapshot: "https://example.test/images/generations".to_string(),
                status: GenerationJobStatus::Queued,
                chain_attempt: 1,
                auto_attempt: 0,
                max_auto_attempts: 2,
                queued_at: "2026-07-10T00:00:00Z".to_string(),
                finished_at: None,
                error_code: None,
                error_message: None,
                retryable: false,
            };
            let enqueue = {
                let mut conn = db.conn.lock().expect("lock fixture db");
                enqueue_job(&mut conn, &prepared).expect("enqueue fixture job")
            };
            {
                let mut conn = db.conn.lock().expect("lock fixture db");
                let tx =
                    begin_generation_job_write_transaction(&mut conn).expect("begin fixture claim");
                claim_job_in_transaction(&tx, &enqueue.job_id).expect("claim fixture job");
                tx.commit().expect("commit fixture claim");
            }
            let snapshot = {
                let conn = db.conn.lock().expect("lock fixture db");
                load_generation_execution_snapshot(&conn, &enqueue.job_id)
                    .expect("load fixture execution snapshot")
            };
            Self {
                db,
                db_path,
                snapshot,
                root,
            }
        }

        fn response_store(&self) -> FileResponseArtifactStore {
            FileResponseArtifactStore::new(self.root.join("responses"))
        }

        fn enter_provider_request(
            &self,
            owner_id: &str,
            now_ms: i64,
            expected_response_file: PathBuf,
        ) -> WorkerTransitionAuthority {
            let conn = self.db.conn.lock().expect("lock fixture worker state");
            let authority = Self::acquire_worker(&conn, owner_id, now_ms);
            transition_running_job_stage_with_event(
                &conn,
                &self.snapshot.context.job_id,
                GenerationJobStage::Preparing,
                WorkerStageTransition::BeginProviderRequest {
                    expected_response_file,
                },
                &authority,
                now_ms,
            )
            .expect("enter fixture provider request stage");
            authority
        }

        fn acquire_worker(
            conn: &rusqlite::Connection,
            owner_id: &str,
            now_ms: i64,
        ) -> WorkerTransitionAuthority {
            match acquire_worker_lease(conn, owner_id, now_ms, Duration::from_millis(100))
                .expect("acquire fixture worker lease")
            {
                WorkerLeaseAcquireOutcome::Acquired { authority, .. } => authority,
                WorkerLeaseAcquireOutcome::Held { .. } => panic!("fixture lease unexpectedly held"),
            }
        }

        fn fenced_response_projection(&self) -> FencedResponseProjection {
            let conn = self.db.conn.lock().expect("read fenced response state");
            conn.query_row(
                "SELECT r.request_state, r.expected_response_file, r.response_file,
                        r.response_size, r.response_sha256, r.updated_at,
                        j.status, j.stage, j.last_heartbeat_at, g.status,
                        (SELECT COUNT(*) FROM images)
                   FROM generation_recoveries r
                   JOIN generation_jobs j ON j.generation_id = r.generation_id
                   JOIN generations g ON g.id = r.generation_id
                  WHERE r.generation_id = ?1 AND j.id = ?2",
                params![
                    self.snapshot.context.generation_id,
                    self.snapshot.context.job_id
                ],
                |row| {
                    Ok(FencedResponseProjection {
                        request_state: row.get(0)?,
                        expected_response_file: row.get(1)?,
                        response_file: row.get(2)?,
                        response_size: row.get(3)?,
                        response_sha256: row.get(4)?,
                        recovery_updated_at: row.get(5)?,
                        job_status: row.get(6)?,
                        job_stage: row.get(7)?,
                        last_heartbeat_at: row.get(8)?,
                        generation_status: row.get(9)?,
                        image_count: row.get(10)?,
                    })
                },
            )
            .expect("read fenced response projection")
        }

        async fn prepare_fenced_response(
            &self,
            owner_id: &str,
            now_ms: i64,
        ) -> (
            FileResponseArtifactStore,
            PathBuf,
            WorkerTransitionAuthority,
            ProviderAttemptResponse,
        ) {
            let response_store = self.response_store();
            let expected_path = response_store
                .expected_response_path(&self.snapshot.context)
                .expect("derive fenced response path");
            let authority = self.enter_provider_request(owner_id, now_ms, expected_path.clone());
            let response = response_store
                .persist_verified_response(
                    &self.snapshot.context,
                    ProviderAttemptBody {
                        body_text: r#"{"data":[]}"#.to_string(),
                        requested_image_count: 1,
                    },
                )
                .await
                .expect("persist fenced response");
            (response_store, expected_path, authority, response)
        }

        fn assert_pristine_provider_request(&self, expected_path: &Path) {
            let projection = self.fenced_response_projection();
            assert_eq!(projection.request_state, "requesting");
            assert_eq!(
                projection.expected_response_file.as_deref(),
                expected_path.to_str()
            );
            assert!(projection.response_file.is_none());
            assert!(projection.response_size.is_none());
            assert!(projection.response_sha256.is_none());
            assert_eq!(projection.job_status, "running");
            assert_eq!(projection.job_stage, "provider_request");
            assert_eq!(projection.generation_status, "running");
            assert_eq!(projection.image_count, 0);
        }

        fn file_root(&self) -> PathBuf {
            self.root.join("app-data")
        }

        fn row_count(&self, table: &str) -> i64 {
            assert!(matches!(
                table,
                "conversations"
                    | "generations"
                    | "generation_jobs"
                    | "generation_recoveries"
                    | "images"
            ));
            let conn = self.db.conn.lock().expect("lock fixture db");
            conn.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                row.get(0)
            })
            .expect("count lifecycle fixture rows")
        }

        fn cleanup(self) {
            let root = self.root.clone();
            drop(self);
            std::fs::remove_dir_all(root).ok();
        }
    }

    enum CoreEngineOutcome {
        Success,
        Error(EngineCallError),
    }

    struct FakeCoreEngine {
        calls: Arc<AtomicUsize>,
        outcome: CoreEngineOutcome,
    }

    impl FakeCoreEngine {
        fn respond(
            &self,
            options: &GptImageRequestOptions,
        ) -> Result<ProviderAttemptBody, EngineCallError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            match &self.outcome {
                CoreEngineOutcome::Success => Ok(ProviderAttemptBody {
                    body_text: r#"{"data":[{"b64_json":"unused"}]}"#.to_string(),
                    requested_image_count: options.image_count,
                }),
                CoreEngineOutcome::Error(error) => Err(error.clone()),
            }
        }
    }

    #[async_trait::async_trait]
    impl ImageEngine for FakeCoreEngine {
        async fn generate(
            &self,
            _model: &str,
            _api_key: &str,
            _endpoint_url: &str,
            _prompt: &str,
            options: &GptImageRequestOptions,
        ) -> Result<ProviderAttemptBody, EngineCallError> {
            self.respond(options)
        }

        async fn edit(
            &self,
            _model: &str,
            _api_key: &str,
            _endpoint_url: &str,
            _prompt: &str,
            _source_images: &[PreparedEditImage],
            options: &GptImageRequestOptions,
        ) -> Result<ProviderAttemptBody, EngineCallError> {
            self.respond(options)
        }
    }

    enum EditPathMutationAtEngineEntry {
        Delete,
        ReplaceWithSameLength(Vec<u8>),
        #[cfg(unix)]
        ReplaceWithSymlink(PathBuf),
    }

    struct PreparedBytesObservationEngine {
        calls: Arc<AtomicUsize>,
        source_path: PathBuf,
        expected_bytes: Vec<u8>,
        mutation: EditPathMutationAtEngineEntry,
    }

    #[async_trait::async_trait]
    impl ImageEngine for PreparedBytesObservationEngine {
        async fn generate(
            &self,
            _model: &str,
            _api_key: &str,
            _endpoint_url: &str,
            _prompt: &str,
            _options: &GptImageRequestOptions,
        ) -> Result<ProviderAttemptBody, EngineCallError> {
            panic!("edit bytes fixture must not call generate")
        }

        async fn edit(
            &self,
            _model: &str,
            _api_key: &str,
            _endpoint_url: &str,
            _prompt: &str,
            source_images: &[PreparedEditImage],
            options: &GptImageRequestOptions,
        ) -> Result<ProviderAttemptBody, EngineCallError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            match &self.mutation {
                EditPathMutationAtEngineEntry::Delete => {
                    std::fs::remove_file(&self.source_path).expect("delete source at engine entry");
                }
                EditPathMutationAtEngineEntry::ReplaceWithSameLength(bytes) => {
                    assert_eq!(bytes.len(), self.expected_bytes.len());
                    std::fs::write(&self.source_path, bytes)
                        .expect("replace source at engine entry");
                }
                #[cfg(unix)]
                EditPathMutationAtEngineEntry::ReplaceWithSymlink(target) => {
                    use std::os::unix::fs::symlink;
                    let original = self.source_path.with_extension("original.jpg");
                    std::fs::rename(&self.source_path, original)
                        .expect("move source at engine entry");
                    symlink(target, &self.source_path).expect("symlink source at engine entry");
                }
            }
            assert_eq!(source_images.len(), 1);
            assert_eq!(source_images[0].bytes(), self.expected_bytes.as_slice());
            assert_eq!(source_images[0].mime_type(), "image/jpeg");
            assert!(source_images[0].file_name().ends_with(".jpg"));
            Ok(ProviderAttemptBody {
                body_text: r#"{"data":[]}"#.to_string(),
                requested_image_count: options.image_count,
            })
        }
    }

    struct OrderedDatabaseDecoder {
        db: Arc<Database>,
        db_path: PathBuf,
        job_id: String,
        generation_id: String,
        engine_calls: Arc<AtomicUsize>,
        decoder_calls: Arc<AtomicUsize>,
        images: Vec<Vec<u8>>,
        request_cancel: bool,
    }

    #[async_trait::async_trait]
    impl ImageResponseDecoder for OrderedDatabaseDecoder {
        async fn decode_and_download(
            &self,
            response: &ProviderAttemptResponse,
            cancellation: &CancellationProbe,
        ) -> Result<Vec<Vec<u8>>, GenerationExecutionError> {
            cancellation.checkpoint("response_decode")?;
            assert!(self.db.conn.try_lock().is_ok(), "decoder must run unlocked");
            let conn = rusqlite::Connection::open(&self.db_path)
                .expect("open independent decoder connection");
            let (state, response_file, job_status, generation_status): (
                String,
                Option<String>,
                String,
                String,
            ) = conn
                .query_row(
                    "SELECT r.request_state, r.response_file, j.status, g.status
                     FROM generation_recoveries r
                     JOIN generation_jobs j ON j.generation_id = r.generation_id
                     JOIN generations g ON g.id = r.generation_id
                     WHERE r.generation_id = ?1 AND j.id = ?2",
                    params![self.generation_id, self.job_id],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
                )
                .expect("read committed response-ready projection");
            assert_eq!(state, "response_ready");
            assert_eq!(
                response_file.as_deref(),
                Some(response.response_file.as_str())
            );
            assert_eq!(job_status, "running");
            assert_eq!(generation_status, "running");
            if self.request_cancel {
                crate::generation_jobs::request_cancel(&conn, &self.job_id)
                    .expect("commit cancel before success");
            }
            assert_eq!(self.engine_calls.load(Ordering::SeqCst), 1);
            self.decoder_calls.fetch_add(1, Ordering::SeqCst);
            Ok(self.images.clone())
        }
    }

    struct DbLockFileObserver {
        db: Arc<Database>,
        promote_calls: AtomicUsize,
        disarm_calls: AtomicUsize,
        cleanup_calls: AtomicUsize,
        all_unlocked: AtomicBool,
        expected_final_paths: std::sync::OnceLock<Vec<PathBuf>>,
        cleanup_saw_all_promoted_paths: AtomicBool,
    }

    impl DbLockFileObserver {
        fn new(db: Arc<Database>) -> Self {
            Self {
                db,
                promote_calls: AtomicUsize::new(0),
                disarm_calls: AtomicUsize::new(0),
                cleanup_calls: AtomicUsize::new(0),
                all_unlocked: AtomicBool::new(true),
                expected_final_paths: std::sync::OnceLock::new(),
                cleanup_saw_all_promoted_paths: AtomicBool::new(true),
            }
        }

        fn set_expected_final_paths(&self, paths: Vec<PathBuf>) {
            let _ = self.expected_final_paths.set(paths);
        }

        fn record(&self, counter: &AtomicUsize) {
            let unlocked = self.db.conn.try_lock().is_ok();
            if !unlocked {
                self.all_unlocked.store(false, Ordering::SeqCst);
            }
            counter.fetch_add(1, Ordering::SeqCst);
        }
    }

    struct DbLockResponseVerificationObserver {
        db: Arc<Database>,
        hash_calls: AtomicUsize,
        metadata_calls: AtomicUsize,
        all_unlocked: AtomicBool,
    }

    impl DbLockResponseVerificationObserver {
        fn new(db: Arc<Database>) -> Self {
            Self {
                db,
                hash_calls: AtomicUsize::new(0),
                metadata_calls: AtomicUsize::new(0),
                all_unlocked: AtomicBool::new(true),
            }
        }

        fn record(&self, counter: &AtomicUsize) {
            let unlocked = self.db.conn.try_lock().is_ok();
            if !unlocked {
                self.all_unlocked.store(false, Ordering::SeqCst);
            }
            counter.fetch_add(1, Ordering::SeqCst);
        }
    }

    impl ResponseVerificationObserver for DbLockResponseVerificationObserver {
        fn observe(&self, event: ResponseVerificationEvent) {
            match event {
                ResponseVerificationEvent::BeforeBodyHash => self.record(&self.hash_calls),
                ResponseVerificationEvent::BeforeFileMetadata => self.record(&self.metadata_calls),
            }
        }
    }

    impl GenerationFileLifecycleObserver for DbLockFileObserver {
        fn before_promote_io(&self) {
            self.record(&self.promote_calls);
        }

        fn before_disarm_io(&self) {
            self.record(&self.disarm_calls);
        }

        fn before_cleanup_io(&self) {
            if let Some(paths) = self.expected_final_paths.get() {
                if !paths.is_empty() && !paths.iter().all(|path| path.is_file()) {
                    self.cleanup_saw_all_promoted_paths
                        .store(false, Ordering::SeqCst);
                }
            }
            self.record(&self.cleanup_calls);
        }
    }

    struct ObservedLocalFileStore {
        root: PathBuf,
        observer: Arc<dyn GenerationFileLifecycleObserver>,
        path_observer: Arc<DbLockFileObserver>,
    }

    #[async_trait::async_trait]
    impl GenerationFileStore for ObservedLocalFileStore {
        async fn stage_images(
            &self,
            snapshot: &GenerationExecutionSnapshot,
            images: Vec<Vec<u8>>,
            cancellation: &CancellationProbe,
        ) -> Result<StagedGenerationFiles, GenerationExecutionError> {
            cancellation.checkpoint("image_staging")?;
            let root = self.root.clone();
            let generation_id = snapshot.context.generation_id.clone();
            let output_format = snapshot.output_format.clone();
            let created_at = snapshot.created_at.clone();
            let observer = Arc::clone(&self.observer);
            let staged = tokio::task::spawn_blocking(move || {
                file_manager::FileManager::new(root).stage_generation_images_with_observer(
                    &generation_id,
                    &images,
                    &output_format,
                    &created_at,
                    observer,
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
            self.path_observer
                .set_expected_final_paths(staged.final_paths());
            Ok(staged)
        }
    }

    #[tokio::test]
    async fn precreated_local_continuation_commits_ready_before_decoder_and_short_success() {
        let fixture = PrecreatedTestFixture::new(3);
        let identity_counts_before = [
            fixture.row_count("conversations"),
            fixture.row_count("generations"),
            fixture.row_count("generation_jobs"),
        ];
        let engine_calls = Arc::new(AtomicUsize::new(0));
        let decoder_calls = Arc::new(AtomicUsize::new(0));
        let engine = FakeCoreEngine {
            calls: Arc::clone(&engine_calls),
            outcome: CoreEngineOutcome::Success,
        };
        let body = perform_prepared_test_attempt(
            &engine,
            &fixture.snapshot,
            &ProviderExecutionCredentials::new("ephemeral-secret".to_string()),
        )
        .await
        .expect("one paid provider phase");
        let response_store = fixture.response_store();
        let file_observer = Arc::new(DbLockFileObserver::new(Arc::clone(&fixture.db)));
        let file_observer_trait: Arc<dyn GenerationFileLifecycleObserver> = file_observer.clone();
        let outcome = continue_precreated_generation_after_provider(
            fixture.db.as_ref(),
            &response_store,
            &OrderedDatabaseDecoder {
                db: Arc::clone(&fixture.db),
                db_path: fixture.db_path.clone(),
                job_id: fixture.snapshot.context.job_id.clone(),
                generation_id: fixture.snapshot.context.generation_id.clone(),
                engine_calls: Arc::clone(&engine_calls),
                decoder_calls: Arc::clone(&decoder_calls),
                images: vec![jpeg_bytes()],
                request_cancel: false,
            },
            &ObservedLocalFileStore {
                root: fixture.file_root(),
                observer: file_observer_trait,
                path_observer: Arc::clone(&file_observer),
            },
            &fixture.snapshot,
            body,
            &CancellationProbe::new(),
        )
        .await
        .expect("complete precreated local continuation");
        let result = match outcome {
            PrecreatedLocalOutcome::Completed(result) => result,
            PrecreatedLocalOutcome::CancelRequested => panic!("success must not cancel"),
        };

        assert_eq!(engine_calls.load(Ordering::SeqCst), 1);
        assert_eq!(decoder_calls.load(Ordering::SeqCst), 1);
        assert_eq!(result.generation_id, fixture.snapshot.context.generation_id);
        assert_eq!(
            result.conversation_id,
            fixture.snapshot.context.conversation_id
        );
        assert_eq!(result.images.len(), 1, "short response must remain short");
        assert!(result
            .images
            .iter()
            .all(|image| Path::new(&image.file_path).is_file()));
        assert_eq!(file_observer.promote_calls.load(Ordering::SeqCst), 1);
        assert_eq!(file_observer.disarm_calls.load(Ordering::SeqCst), 1);
        assert_eq!(file_observer.cleanup_calls.load(Ordering::SeqCst), 0);
        assert!(file_observer.all_unlocked.load(Ordering::SeqCst));
        assert_eq!(
            identity_counts_before,
            [
                fixture.row_count("conversations"),
                fixture.row_count("generations"),
                fixture.row_count("generation_jobs"),
            ]
        );
        assert_eq!(fixture.row_count("generation_recoveries"), 0);
        assert_eq!(fixture.row_count("images"), 1);
        let conn = fixture.db.conn.lock().expect("lock completed fixture");
        let (status, requested, metadata): (String, i32, String) = conn
            .query_row(
                "SELECT status, image_count, request_metadata FROM generations WHERE id = ?1",
                params![fixture.snapshot.context.generation_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .expect("read completed generation");
        assert_eq!(status, "completed");
        assert_eq!(requested, 3);
        let metadata: serde_json::Value = serde_json::from_str(&metadata).unwrap();
        assert_eq!(metadata["image_count"], serde_json::json!(3));
        assert_eq!(metadata["actual_image_count"], serde_json::json!(1));
        drop(conn);
        let completed_status = {
            let conn = fixture.db.conn.lock().unwrap();
            get_job(&conn, &fixture.snapshot.context.job_id)
                .unwrap()
                .status
        };
        assert_eq!(completed_status, GenerationJobStatus::Completed);
        let late_cancel = {
            let conn = fixture.db.conn.lock().unwrap();
            crate::generation_jobs::request_cancel(&conn, &fixture.snapshot.context.job_id)
                .expect_err("completion-first must reject a late cancel")
        };
        assert_eq!(
            late_cancel.stable_code(),
            "generation_job_invalid_transition"
        );
        fixture.cleanup();
    }

    #[tokio::test]
    async fn provider_error_stays_running_until_explicit_finalizer() {
        let fixture = PrecreatedTestFixture::new(1);
        let engine_calls = Arc::new(AtomicUsize::new(0));
        let raw_secret = "sk-provider-secret-must-not-persist";
        let error = match perform_prepared_test_attempt(
            &FakeCoreEngine {
                calls: Arc::clone(&engine_calls),
                outcome: CoreEngineOutcome::Error(EngineCallError {
                    code: "rate_limited".to_string(),
                    sanitized_message: raw_secret.to_string(),
                    retry_after: Some(RetryAfterHint::DelaySeconds(3)),
                    safe_to_retry: true,
                    outcome_ambiguous: false,
                }),
            },
            &fixture.snapshot,
            &ProviderExecutionCredentials::new("ephemeral-secret".to_string()),
        )
        .await
        {
            Err(error) => error,
            Ok(_) => panic!("provider error must remain structured"),
        };
        assert_eq!(engine_calls.load(Ordering::SeqCst), 1);
        assert_eq!(error.code(), "rate_limited");
        assert!(matches!(
            error,
            GenerationExecutionError::Engine(EngineCallError {
                safe_to_retry: true,
                retry_after: Some(RetryAfterHint::DelaySeconds(3)),
                ..
            })
        ));
        let running_status = {
            let conn = fixture.db.conn.lock().unwrap();
            get_job(&conn, &fixture.snapshot.context.job_id)
                .unwrap()
                .status
        };
        assert_eq!(running_status, GenerationJobStatus::Running);
        assert_eq!(fixture.row_count("generation_recoveries"), 1);

        finalize_generation_failure(
            fixture.db.as_ref(),
            &fixture.snapshot.context,
            &error,
            &GenerationTerminalDisposition {
                status: GenerationJobStatus::Failed,
                error_code: "rate_limited".to_string(),
                retryable: true,
                preserve_response_ready: false,
            },
        )
        .expect("explicit policy finalizer");
        let conn = fixture.db.conn.lock().expect("lock failed fixture");
        let (job_status, job_message, generation_status, generation_message): (
            String,
            Option<String>,
            String,
            Option<String>,
        ) = conn
            .query_row(
                "SELECT j.status, j.error_message, g.status, g.error_message
                 FROM generation_jobs j JOIN generations g ON g.id = j.generation_id
                 WHERE j.id = ?1",
                params![fixture.snapshot.context.job_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .unwrap();
        assert_eq!(job_status, "failed");
        assert_eq!(generation_status, "failed");
        assert_eq!(
            job_message.as_deref(),
            Some("The provider rate limit was reached")
        );
        assert_eq!(generation_message, job_message);
        let persisted = format!("{job_message:?}{generation_message:?}");
        assert!(!persisted.contains(raw_secret));
        drop(conn);
        assert_eq!(fixture.row_count("generation_recoveries"), 0);
        fixture.cleanup();
    }

    #[tokio::test]
    async fn failure_disposition_matrix_preserves_or_deletes_recovery_without_secret() {
        for (initial_ready, preserve, should_succeed, recovery_after) in [
            (false, false, true, 0),
            (true, true, true, 1),
            (true, false, true, 0),
            (false, true, false, 1),
        ] {
            let fixture = PrecreatedTestFixture::new(1);
            let response_store = fixture.response_store();
            let response = if initial_ready {
                let response = response_store
                    .persist_verified_response(
                        &fixture.snapshot.context,
                        ProviderAttemptBody {
                            body_text: r#"{"data":[]}"#.to_string(),
                            requested_image_count: 1,
                        },
                    )
                    .await
                    .expect("persist matrix response");
                promote_verified_response(
                    fixture.db.as_ref(),
                    &fixture.snapshot.context,
                    &response_store
                        .expected_response_path(&fixture.snapshot.context)
                        .expect("derive matrix response path"),
                    &response,
                )
                .expect("promote matrix response");
                Some(response)
            } else {
                None
            };
            let raw_secret = "sk-local-secret-must-not-persist";
            let error = GenerationExecutionError::Local {
                code: "response_decode_failed".to_string(),
                sanitized_message: raw_secret.to_string(),
                stage: "response_decode".to_string(),
            };
            let result = finalize_generation_failure(
                fixture.db.as_ref(),
                &fixture.snapshot.context,
                &error,
                &GenerationTerminalDisposition {
                    status: GenerationJobStatus::Failed,
                    error_code: "response_decode_failed".to_string(),
                    retryable: false,
                    preserve_response_ready: preserve,
                },
            );
            assert_eq!(result.is_ok(), should_succeed);
            {
                let conn = fixture.db.conn.lock().expect("lock matrix fixture");
                let job = get_job(&conn, &fixture.snapshot.context.job_id).unwrap();
                assert_eq!(
                    job.status,
                    if should_succeed {
                        GenerationJobStatus::Failed
                    } else {
                        GenerationJobStatus::Running
                    }
                );
                assert!(!format!("{job:?}").contains(raw_secret));
                let recovery_count: i64 = conn
                    .query_row(
                        "SELECT COUNT(*) FROM generation_recoveries WHERE generation_id = ?1",
                        params![fixture.snapshot.context.generation_id],
                        |row| row.get(0),
                    )
                    .unwrap();
                assert_eq!(recovery_count, recovery_after);
                if !should_succeed {
                    let state: String = conn
                        .query_row(
                            "SELECT request_state FROM generation_recoveries WHERE generation_id = ?1",
                            params![fixture.snapshot.context.generation_id],
                            |row| row.get(0),
                        )
                        .unwrap();
                    assert_eq!(state, "requesting");
                }
            }
            if let Some(response) = response {
                assert!(Path::new(&response.response_file).is_file());
                response_store
                    .load_verified_response(
                        &fixture.snapshot.context,
                        Path::new(&response.response_file),
                    )
                    .await
                    .expect("failure finalizer never deletes artifact");
            }
            fixture.cleanup();
        }
    }

    #[tokio::test]
    async fn response_promotion_failure_keeps_verified_artifact_and_skips_decoder() {
        let fixture = PrecreatedTestFixture::new(1);
        {
            let conn = fixture
                .db
                .conn
                .lock()
                .expect("lock promotion failure fixture");
            conn.execute_batch(
                "CREATE TEMP TRIGGER fail_response_ready
                 BEFORE UPDATE OF request_state ON generation_recoveries
                 WHEN NEW.request_state = 'response_ready'
                 BEGIN SELECT RAISE(ABORT, 'injected response-ready failure'); END;",
            )
            .expect("install response-ready fault");
        }
        let engine_calls = Arc::new(AtomicUsize::new(0));
        let decoder_calls = Arc::new(AtomicUsize::new(0));
        let body = perform_prepared_test_attempt(
            &FakeCoreEngine {
                calls: Arc::clone(&engine_calls),
                outcome: CoreEngineOutcome::Success,
            },
            &fixture.snapshot,
            &ProviderExecutionCredentials::new("ephemeral-secret".to_string()),
        )
        .await
        .unwrap();
        let response_store = fixture.response_store();
        let error = continue_precreated_generation_after_provider(
            fixture.db.as_ref(),
            &response_store,
            &FakeLocalDecoder {
                calls: Arc::clone(&decoder_calls),
                images: vec![jpeg_bytes()],
            },
            &LocalGenerationFileStore::new(fixture.file_root()),
            &fixture.snapshot,
            body,
            &CancellationProbe::new(),
        )
        .await
        .expect_err("response-ready SQL fault must stop local continuation");
        assert!(matches!(
            &error,
            GenerationExecutionError::Local { code, stage, .. }
                if code == "database_error" && stage == "response_ready_commit"
        ));
        assert_eq!(engine_calls.load(Ordering::SeqCst), 1);
        assert_eq!(decoder_calls.load(Ordering::SeqCst), 0);
        let response_path = response_store
            .expected_response_path(&fixture.snapshot.context)
            .expect("expected persisted response path");
        assert!(response_path.is_file());
        response_store
            .load_verified_response(&fixture.snapshot.context, &response_path)
            .await
            .expect("verified artifact survives DB promotion failure");
        let conn = fixture
            .db
            .conn
            .lock()
            .expect("lock promotion failure state");
        let (state, response_file, job_status, generation_status): (
            String,
            Option<String>,
            String,
            String,
        ) = conn
            .query_row(
                "SELECT r.request_state, r.response_file, j.status, g.status
                 FROM generation_recoveries r
                 JOIN generation_jobs j ON j.generation_id = r.generation_id
                 JOIN generations g ON g.id = r.generation_id
                 WHERE j.id = ?1",
                params![fixture.snapshot.context.job_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .unwrap();
        assert_eq!((state.as_str(), response_file), ("requesting", None));
        assert_eq!(
            (job_status.as_str(), generation_status.as_str()),
            ("running", "running")
        );
        drop(conn);
        fixture.cleanup();
    }

    #[tokio::test]
    async fn response_promotion_verifies_body_and_file_before_database_lock() {
        let fixture = PrecreatedTestFixture::new(1);
        let response_store = fixture.response_store();
        let response = response_store
            .persist_verified_response(
                &fixture.snapshot.context,
                ProviderAttemptBody {
                    body_text: r#"{"data":[]}"#.to_string(),
                    requested_image_count: 1,
                },
            )
            .await
            .expect("persist response for lock oracle");
        let expected_path = response_store
            .expected_response_path(&fixture.snapshot.context)
            .expect("derive expected response path");
        let observer = DbLockResponseVerificationObserver::new(Arc::clone(&fixture.db));

        promote_verified_response_with_observer(
            fixture.db.as_ref(),
            &fixture.snapshot.context,
            &expected_path,
            &response,
            &observer,
        )
        .expect("promote response after unlocked verification");

        assert_eq!(observer.hash_calls.load(Ordering::SeqCst), 1);
        assert_eq!(observer.metadata_calls.load(Ordering::SeqCst), 1);
        assert!(observer.all_unlocked.load(Ordering::SeqCst));
        {
            let conn = fixture.db.conn.lock().expect("read promoted recovery");
            let (state, response_file): (String, Option<String>) = conn
                .query_row(
                    "SELECT request_state, response_file FROM generation_recoveries
                     WHERE generation_id = ?1",
                    params![fixture.snapshot.context.generation_id],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .expect("read promoted response state");
            assert_eq!(state, "response_ready");
            assert_eq!(
                response_file.as_deref(),
                Some(response.response_file.as_str())
            );
        }
        fixture.cleanup();
    }

    #[tokio::test]
    async fn fenced_response_promotion_verifies_artifact_before_database_lock() {
        let fixture = PrecreatedTestFixture::new(1);
        let now_ms = chrono::Utc::now().timestamp_millis();
        let (response_store, _, authority, response) = fixture
            .prepare_fenced_response("fenced-response-lock-oracle", now_ms)
            .await;
        let observer = DbLockResponseVerificationObserver::new(Arc::clone(&fixture.db));

        let event = promote_verified_response_fenced_with_optional_observer(
            fixture.db.as_ref(),
            &response_store,
            &fixture.snapshot.context,
            &response,
            &authority,
            now_ms + 1,
            Some(&observer),
        )
        .expect("promote fenced response after unlocked verification");

        assert_eq!(event.stage, GenerationJobStage::ResponseReady);
        assert_eq!(observer.hash_calls.load(Ordering::SeqCst), 1);
        assert_eq!(observer.metadata_calls.load(Ordering::SeqCst), 1);
        assert!(observer.all_unlocked.load(Ordering::SeqCst));
        fixture.cleanup();
    }

    #[tokio::test]
    async fn fenced_response_promotion_commits_verified_metadata_and_event_despite_late_cancel() {
        let fixture = PrecreatedTestFixture::new(1);
        let now_ms = chrono::Utc::now().timestamp_millis();
        let (response_store, expected_path, authority, response) = fixture
            .prepare_fenced_response("fenced-response-worker", now_ms)
            .await;
        {
            let conn = fixture.db.conn.lock().expect("request late cancellation");
            request_cancel(&conn, &fixture.snapshot.context.job_id)
                .expect("persist late cancellation");
        }

        let event = promote_verified_response_fenced(
            fixture.db.as_ref(),
            &response_store,
            &fixture.snapshot.context,
            &response,
            &authority,
            now_ms + 1,
        )
        .expect("promote verified response under worker fence");

        assert_eq!(event.job_id, fixture.snapshot.context.job_id);
        assert_eq!(event.generation_id, fixture.snapshot.context.generation_id);
        assert_eq!(event.status, GenerationJobStatus::Running);
        assert_eq!(event.stage, GenerationJobStage::ResponseReady);
        assert!(event.cancel_requested_at.is_some());
        let projection = fixture.fenced_response_projection();
        assert_eq!(projection.request_state, "response_ready");
        assert_eq!(
            projection.expected_response_file.as_deref(),
            expected_path.to_str()
        );
        assert_eq!(projection.response_file.as_deref(), expected_path.to_str());
        assert_eq!(
            projection.response_size,
            Some(response.response_size as i64)
        );
        assert_eq!(
            projection.response_sha256.as_deref(),
            Some(response.response_sha256.as_str())
        );
        assert_eq!(projection.job_stage, "response_ready");
        let expected_timestamp = lifecycle_worker_timestamp(now_ms + 1).unwrap();
        assert_eq!(projection.recovery_updated_at, expected_timestamp);
        assert_eq!(
            projection.last_heartbeat_at.as_deref(),
            Some(expected_timestamp.as_str())
        );
        let conn = fixture.db.conn.lock().expect("reread fenced event");
        let tx = conn.unchecked_transaction().expect("begin event reread");
        let persisted_event = get_job_event_in_transaction(&tx, &event.job_id)
            .expect("reread committed fenced event");
        tx.commit().expect("commit event reread");
        assert_eq!(event, persisted_event);
        drop(conn);
        fixture.cleanup();
    }

    #[tokio::test]
    async fn fenced_response_promotion_commits_verified_empty_raw_body() {
        let fixture = PrecreatedTestFixture::new(1);
        let response_store = fixture.response_store();
        let expected_path = response_store
            .expected_response_path(&fixture.snapshot.context)
            .expect("derive empty-response path");
        let now_ms = chrono::Utc::now().timestamp_millis();
        let authority = fixture.enter_provider_request(
            "fenced-response-empty-body",
            now_ms,
            expected_path.clone(),
        );
        let response = response_store
            .persist_verified_response(
                &fixture.snapshot.context,
                ProviderAttemptBody {
                    body_text: String::new(),
                    requested_image_count: 1,
                },
            )
            .await
            .expect("persist empty raw response envelope");
        let empty_hash = FileResponseArtifactStore::response_hash("");
        assert_eq!(response.response_size, 0);
        assert_eq!(response.response_sha256, empty_hash);

        let event = promote_verified_response_fenced(
            fixture.db.as_ref(),
            &response_store,
            &fixture.snapshot.context,
            &response,
            &authority,
            now_ms + 1,
        )
        .expect("promote verified empty raw response");

        assert_eq!(event.job_id, fixture.snapshot.context.job_id);
        assert_eq!(event.status, GenerationJobStatus::Running);
        assert_eq!(event.stage, GenerationJobStage::ResponseReady);
        let projection = fixture.fenced_response_projection();
        assert_eq!(projection.request_state, "response_ready");
        assert_eq!(
            projection.expected_response_file.as_deref(),
            expected_path.to_str()
        );
        assert_eq!(projection.response_file.as_deref(), expected_path.to_str());
        assert_eq!(projection.response_size, Some(0));
        assert_eq!(
            projection.response_sha256.as_deref(),
            Some(empty_hash.as_str())
        );
        fixture.cleanup();
    }

    #[tokio::test]
    async fn fenced_response_promotion_rejects_artifact_changed_after_verified_load() {
        let fixture = PrecreatedTestFixture::new(1);
        let now_ms = chrono::Utc::now().timestamp_millis();
        let (response_store, expected_path, authority, response) = fixture
            .prepare_fenced_response("fenced-response-file-check", now_ms)
            .await;
        std::fs::write(
            &expected_path,
            br#"{"version":1,"body_text":"changed","body_size":7,"body_sha256":"d67e2e944994496c8d5f83c7e8fb8a29d4db6d9be4d9a0a0f16b1eaf39f2f318","requested_image_count":1}"#,
        )
        .expect("replace response after verified load");

        let error = promote_verified_response_fenced(
            fixture.db.as_ref(),
            &response_store,
            &fixture.snapshot.context,
            &response,
            &authority,
            now_ms + 1,
        )
        .expect_err("changed artifact must not reach response-ready state");
        assert!(matches!(
            error,
            WorkerTransitionError::Repository(AppError::GenerationJobInvalidSnapshot)
        ));
        fixture.assert_pristine_provider_request(&expected_path);
        fixture.cleanup();
    }

    #[tokio::test]
    async fn fenced_response_promotion_rejects_response_from_an_unconfigured_store() {
        let fixture = PrecreatedTestFixture::new(1);
        let configured_store = fixture.response_store();
        let configured_path = configured_store
            .expected_response_path(&fixture.snapshot.context)
            .expect("derive configured response path");
        let alternate_store = FileResponseArtifactStore::new(fixture.root.join("alternate"));
        let now_ms = chrono::Utc::now().timestamp_millis();
        let authority = fixture.enter_provider_request(
            "fenced-response-store-capability",
            now_ms,
            configured_path.clone(),
        );
        let alternate_response = alternate_store
            .persist_verified_response(
                &fixture.snapshot.context,
                ProviderAttemptBody {
                    body_text: r#"{"data":[]}"#.to_string(),
                    requested_image_count: 1,
                },
            )
            .await
            .expect("persist response in alternate store");

        let error = promote_verified_response_fenced(
            fixture.db.as_ref(),
            &configured_store,
            &fixture.snapshot.context,
            &alternate_response,
            &authority,
            now_ms + 1,
        )
        .expect_err("unconfigured store response must not be promoted");
        assert!(matches!(
            error,
            WorkerTransitionError::Repository(AppError::GenerationJobInvalidSnapshot)
        ));
        fixture.assert_pristine_provider_request(&configured_path);
        fixture.cleanup();
    }

    #[tokio::test]
    async fn fenced_response_promotion_rejects_stale_owner_without_side_effects_then_allows_takeover(
    ) {
        let fixture = PrecreatedTestFixture::new(1);
        let now_ms = chrono::Utc::now().timestamp_millis();
        let (response_store, _, authority_a, response) = fixture
            .prepare_fenced_response("fenced-response-worker-a", now_ms)
            .await;
        let authority_b = {
            let conn = fixture.db.conn.lock().expect("acquire takeover lease");
            PrecreatedTestFixture::acquire_worker(&conn, "fenced-response-worker-b", now_ms + 100)
        };
        let before = fixture.fenced_response_projection();

        let stale_error = promote_verified_response_fenced(
            fixture.db.as_ref(),
            &response_store,
            &fixture.snapshot.context,
            &response,
            &authority_a,
            now_ms + 101,
        )
        .expect_err("stale worker A must lose fencing race");
        assert!(matches!(
            stale_error,
            WorkerTransitionError::Lease(WorkerLeaseError::LeaseLost)
        ));
        assert_eq!(fixture.fenced_response_projection(), before);

        let event = promote_verified_response_fenced(
            fixture.db.as_ref(),
            &response_store,
            &fixture.snapshot.context,
            &response,
            &authority_b,
            now_ms + 101,
        )
        .expect("takeover worker B promotes response");
        assert_eq!(event.stage, GenerationJobStage::ResponseReady);
        fixture.cleanup();
    }

    #[tokio::test]
    async fn fenced_response_promotion_rejects_authority_at_exact_expiry_without_mutation() {
        let fixture = PrecreatedTestFixture::new(1);
        let now_ms = chrono::Utc::now().timestamp_millis();
        let (response_store, expected_path, authority, response) = fixture
            .prepare_fenced_response("fenced-response-expiry", now_ms)
            .await;

        let error = promote_verified_response_fenced(
            fixture.db.as_ref(),
            &response_store,
            &fixture.snapshot.context,
            &response,
            &authority,
            now_ms + 100,
        )
        .expect_err("authority is stale at its exact expiry");
        assert!(matches!(
            error,
            WorkerTransitionError::Lease(WorkerLeaseError::LeaseLost)
        ));
        fixture.assert_pristine_provider_request(&expected_path);
        fixture.cleanup();
    }

    #[tokio::test]
    async fn fenced_response_promotion_never_moves_job_heartbeat_backwards() {
        let fixture = PrecreatedTestFixture::new(1);
        let now_ms = chrono::Utc::now().timestamp_millis();
        let (response_store, _, authority, response) = fixture
            .prepare_fenced_response("fenced-response-monotonic", now_ms)
            .await;
        let future_heartbeat = lifecycle_worker_timestamp(now_ms + 10_000).unwrap();
        {
            let conn = fixture.db.conn.lock().expect("install future heartbeat");
            conn.execute(
                "UPDATE generation_jobs SET last_heartbeat_at = ?1 WHERE id = ?2",
                params![future_heartbeat, fixture.snapshot.context.job_id],
            )
            .expect("install future heartbeat");
        }
        let baseline = fixture.fenced_response_projection();

        let error = promote_verified_response_fenced(
            fixture.db.as_ref(),
            &response_store,
            &fixture.snapshot.context,
            &response,
            &authority,
            now_ms + 1,
        )
        .expect_err("response promotion must not move heartbeat backwards");
        assert!(matches!(
            error,
            WorkerTransitionError::Repository(AppError::GenerationJobInvalidTransition)
        ));
        assert_eq!(fixture.fenced_response_projection(), baseline);
        fixture.cleanup();
    }

    #[tokio::test]
    async fn fenced_response_promotion_requires_provider_request_stage() {
        let fixture = PrecreatedTestFixture::new(1);
        let now_ms = chrono::Utc::now().timestamp_millis();
        let (response_store, _, authority, response) = fixture
            .prepare_fenced_response("fenced-response-wrong-stage", now_ms)
            .await;
        {
            let conn = fixture.db.conn.lock().expect("enter retry backoff");
            transition_running_job_stage_with_event(
                &conn,
                &fixture.snapshot.context.job_id,
                GenerationJobStage::ProviderRequest,
                WorkerStageTransition::EnterRetryBackoff,
                &authority,
                now_ms + 1,
            )
            .expect("enter retry backoff before promotion");
        }

        let error = promote_verified_response_fenced(
            fixture.db.as_ref(),
            &response_store,
            &fixture.snapshot.context,
            &response,
            &authority,
            now_ms + 2,
        )
        .expect_err("retry-backoff job cannot promote a provider response");
        assert!(matches!(
            error,
            WorkerTransitionError::Repository(AppError::GenerationJobInvalidTransition)
        ));
        let projection = fixture.fenced_response_projection();
        assert_eq!(projection.request_state, "requesting");
        assert!(projection.response_file.is_none());
        assert!(projection.response_size.is_none());
        assert!(projection.response_sha256.is_none());
        assert_eq!(projection.job_stage, "retry_backoff");
        fixture.cleanup();
    }

    #[tokio::test]
    async fn fenced_response_promotion_requires_pristine_recovery_metadata() {
        let fixture = PrecreatedTestFixture::new(1);
        let now_ms = chrono::Utc::now().timestamp_millis();
        let (response_store, _, authority, response) = fixture
            .prepare_fenced_response("fenced-response-partial-metadata", now_ms)
            .await;
        {
            let conn = fixture.db.conn.lock().expect("install partial metadata");
            conn.execute(
                "UPDATE generation_recoveries SET response_size = 1 WHERE generation_id = ?1",
                params![fixture.snapshot.context.generation_id],
            )
            .expect("install partial response metadata");
        }
        let baseline = fixture.fenced_response_projection();

        let error = promote_verified_response_fenced(
            fixture.db.as_ref(),
            &response_store,
            &fixture.snapshot.context,
            &response,
            &authority,
            now_ms + 1,
        )
        .expect_err("partial recovery metadata must not be overwritten");
        assert!(matches!(
            error,
            WorkerTransitionError::Repository(AppError::GenerationJobInvalidTransition)
        ));
        assert_eq!(fixture.fenced_response_projection(), baseline);
        fixture.cleanup();
    }

    #[tokio::test]
    async fn fenced_response_promotion_requires_persisted_expected_path_to_match_store() {
        let fixture = PrecreatedTestFixture::new(1);
        let now_ms = chrono::Utc::now().timestamp_millis();
        let (response_store, _, authority, response) = fixture
            .prepare_fenced_response("fenced-response-expected-path", now_ms)
            .await;
        let mismatched_path = fixture.root.join("unexpected-store").join(format!(
            "{}.response.json",
            fixture.snapshot.context.generation_id
        ));
        let mismatched_path = mismatched_path.to_string_lossy().to_string();
        {
            let conn = fixture.db.conn.lock().expect("install mismatched path");
            conn.execute(
                "UPDATE generation_recoveries SET expected_response_file = ?1
                  WHERE generation_id = ?2",
                params![mismatched_path, fixture.snapshot.context.generation_id],
            )
            .expect("install mismatched expected path");
        }
        let baseline = fixture.fenced_response_projection();

        let error = promote_verified_response_fenced(
            fixture.db.as_ref(),
            &response_store,
            &fixture.snapshot.context,
            &response,
            &authority,
            now_ms + 1,
        )
        .expect_err("persisted expected path must match configured store");
        assert!(matches!(
            error,
            WorkerTransitionError::Repository(AppError::GenerationJobInvalidTransition)
        ));
        assert_eq!(fixture.fenced_response_projection(), baseline);
        fixture.cleanup();
    }

    #[tokio::test]
    async fn fenced_response_promotion_rejects_duplicate_without_rebroadcast_or_mutation() {
        let fixture = PrecreatedTestFixture::new(1);
        let now_ms = chrono::Utc::now().timestamp_millis();
        let (response_store, _, authority, response) = fixture
            .prepare_fenced_response("fenced-response-duplicate", now_ms)
            .await;
        promote_verified_response_fenced(
            fixture.db.as_ref(),
            &response_store,
            &fixture.snapshot.context,
            &response,
            &authority,
            now_ms + 1,
        )
        .expect("first response promotion commits");
        let baseline = fixture.fenced_response_projection();

        let duplicate = promote_verified_response_fenced(
            fixture.db.as_ref(),
            &response_store,
            &fixture.snapshot.context,
            &response,
            &authority,
            now_ms + 2,
        );
        assert!(matches!(
            duplicate,
            Err(WorkerTransitionError::Repository(
                AppError::GenerationJobInvalidTransition
            ))
        ));
        assert_eq!(fixture.fenced_response_projection(), baseline);
        fixture.cleanup();
    }

    #[tokio::test]
    async fn fenced_response_promotion_rolls_back_if_persisted_hash_is_changed_after_update() {
        let fixture = PrecreatedTestFixture::new(1);
        let now_ms = chrono::Utc::now().timestamp_millis();
        let (response_store, expected_path, authority, response) = fixture
            .prepare_fenced_response("fenced-response-postcheck", now_ms)
            .await;
        {
            let conn = fixture
                .db
                .conn
                .lock()
                .expect("install hash mutation trigger");
            conn.execute_batch(
                "CREATE TEMP TRIGGER mutate_fenced_response_hash
                 AFTER UPDATE OF request_state ON generation_recoveries
                 WHEN NEW.request_state = 'response_ready'
                 BEGIN
                     UPDATE generation_recoveries
                        SET response_sha256 = '0000000000000000000000000000000000000000000000000000000000000000'
                      WHERE generation_id = NEW.generation_id;
                 END;",
            )
            .expect("install hash mutation trigger");
        }

        let error = promote_verified_response_fenced(
            fixture.db.as_ref(),
            &response_store,
            &fixture.snapshot.context,
            &response,
            &authority,
            now_ms + 1,
        )
        .expect_err("post-update metadata corruption must roll back");
        assert!(matches!(
            error,
            WorkerTransitionError::Repository(AppError::GenerationJobCorruptPersistedData)
        ));
        fixture.assert_pristine_provider_request(&expected_path);
        fixture.cleanup();
    }

    #[tokio::test]
    async fn fenced_response_promotion_commit_failure_returns_no_event_and_rolls_back_both_rows() {
        let fixture = PrecreatedTestFixture::new(1);
        let now_ms = chrono::Utc::now().timestamp_millis();
        let (response_store, _, authority, response) = fixture
            .prepare_fenced_response("fenced-response-commit-failure", now_ms)
            .await;
        {
            let conn = fixture.db.conn.lock().expect("install deferred failure");
            conn.execute_batch(
                "CREATE TEMP TRIGGER fail_fenced_response_at_commit
                 AFTER UPDATE OF stage ON generation_jobs
                 WHEN NEW.stage = 'response_ready'
                 BEGIN
                     INSERT INTO images (id, generation_id, file_path)
                     VALUES ('fenced-response-invalid-image', 'missing-generation', '/tmp/rollback.png');
                 END;
                 PRAGMA defer_foreign_keys=ON;",
            )
            .expect("install deferred commit failure");
        }
        let baseline = fixture.fenced_response_projection();

        let result = promote_verified_response_fenced(
            fixture.db.as_ref(),
            &response_store,
            &fixture.snapshot.context,
            &response,
            &authority,
            now_ms + 1,
        );
        assert!(matches!(
            result,
            Err(WorkerTransitionError::Repository(AppError::Database { .. }))
        ));
        assert_eq!(fixture.fenced_response_projection(), baseline);
        fixture.cleanup();
    }

    #[tokio::test]
    async fn response_promotion_rejects_wrong_count_and_exact_path_without_mutation() {
        let wrong_count_fixture = PrecreatedTestFixture::new(1);
        let wrong_count_store = wrong_count_fixture.response_store();
        let wrong_count_response = wrong_count_store
            .persist_verified_response(
                &wrong_count_fixture.snapshot.context,
                ProviderAttemptBody {
                    body_text: r#"{"data":[]}"#.to_string(),
                    requested_image_count: 2,
                },
            )
            .await
            .expect("persist structurally valid wrong-count response");
        let wrong_count_path = wrong_count_store
            .expected_response_path(&wrong_count_fixture.snapshot.context)
            .expect("derive wrong-count response path");
        let wrong_count_error = promote_verified_response(
            wrong_count_fixture.db.as_ref(),
            &wrong_count_fixture.snapshot.context,
            &wrong_count_path,
            &wrong_count_response,
        )
        .expect_err("snapshot count mismatch must not promote recovery");
        assert!(matches!(
            wrong_count_error,
            AppError::GenerationJobInvalidSnapshot
        ));
        {
            let conn = wrong_count_fixture.db.conn.lock().unwrap();
            let (state, response_file): (String, Option<String>) = conn
                .query_row(
                    "SELECT request_state, response_file FROM generation_recoveries
                     WHERE generation_id = ?1",
                    params![wrong_count_fixture.snapshot.context.generation_id],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .unwrap();
            assert_eq!((state.as_str(), response_file), ("requesting", None));
        }
        wrong_count_fixture.cleanup();

        let wrong_path_fixture = PrecreatedTestFixture::new(1);
        let expected_store = wrong_path_fixture.response_store();
        let other_store = FileResponseArtifactStore::new(wrong_path_fixture.root.join("other"));
        let wrong_path_response = other_store
            .persist_verified_response(
                &wrong_path_fixture.snapshot.context,
                ProviderAttemptBody {
                    body_text: r#"{"data":[]}"#.to_string(),
                    requested_image_count: 1,
                },
            )
            .await
            .expect("persist response at a different valid path");
        let expected_path = expected_store
            .expected_response_path(&wrong_path_fixture.snapshot.context)
            .expect("derive exact expected response path");
        let wrong_path_error = promote_verified_response(
            wrong_path_fixture.db.as_ref(),
            &wrong_path_fixture.snapshot.context,
            &expected_path,
            &wrong_path_response,
        )
        .expect_err("a different regular response path must not promote recovery");
        assert!(matches!(
            wrong_path_error,
            AppError::GenerationJobInvalidSnapshot
        ));
        {
            let conn = wrong_path_fixture.db.conn.lock().unwrap();
            let (state, response_file): (String, Option<String>) = conn
                .query_row(
                    "SELECT request_state, response_file FROM generation_recoveries
                     WHERE generation_id = ?1",
                    params![wrong_path_fixture.snapshot.context.generation_id],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .unwrap();
            assert_eq!((state.as_str(), response_file), ("requesting", None));
        }
        wrong_path_fixture.cleanup();
    }

    #[tokio::test]
    async fn verified_response_commit_cannot_be_cross_wired_to_another_context() {
        let owner = PrecreatedTestFixture::new(1);
        let owner_store = owner.response_store();
        let response = owner_store
            .persist_verified_response(
                &owner.snapshot.context,
                ProviderAttemptBody {
                    body_text: r#"{"data":[]}"#.to_string(),
                    requested_image_count: 1,
                },
            )
            .await
            .expect("persist owner response");
        let expected_path = owner_store
            .expected_response_path(&owner.snapshot.context)
            .expect("derive owner response path");
        let response = VerifiedResponseCommit::verify(
            &owner.snapshot.context,
            &expected_path,
            &response,
            None,
        )
        .expect("construct verified owner commit");
        let other = PrecreatedTestFixture::new(1);

        {
            let mut conn = other.db.conn.lock().unwrap();
            let tx = begin_generation_job_write_transaction(&mut conn).unwrap();
            let error =
                promote_verified_response_in_transaction(&tx, &other.snapshot.context, &response)
                    .expect_err("verified response identity must not cross generation contexts");
            assert!(matches!(error, AppError::GenerationJobInvalidSnapshot));
            tx.rollback().unwrap();
        }
        {
            let conn = other.db.conn.lock().unwrap();
            let (state, response_file): (String, Option<String>) = conn
                .query_row(
                    "SELECT request_state, response_file FROM generation_recoveries
                     WHERE generation_id = ?1",
                    params![other.snapshot.context.generation_id],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .unwrap();
            assert_eq!((state.as_str(), response_file), ("requesting", None));
        }
        owner.cleanup();
        other.cleanup();
    }

    #[tokio::test]
    async fn response_artifact_identity_cannot_be_rebound_to_another_context() {
        let owner = PrecreatedTestFixture::new(1);
        let owner_store = owner.response_store();
        let response = owner_store
            .persist_verified_response(
                &owner.snapshot.context,
                ProviderAttemptBody {
                    body_text: r#"{"data":[]}"#.to_string(),
                    requested_image_count: 1,
                },
            )
            .await
            .expect("persist owner response");
        let owner_path = owner_store
            .expected_response_path(&owner.snapshot.context)
            .expect("derive owner response path");
        let other = PrecreatedTestFixture::new(1);

        let result = promote_verified_response(
            other.db.as_ref(),
            &other.snapshot.context,
            &owner_path,
            &response,
        );
        let error = result.expect_err("job A's artifact cannot be rebound to job B");
        assert!(matches!(error, AppError::GenerationJobInvalidSnapshot));
        {
            let conn = other.db.conn.lock().unwrap();
            let (state, response_file): (String, Option<String>) = conn
                .query_row(
                    "SELECT request_state, response_file FROM generation_recoveries
                     WHERE generation_id = ?1",
                    params![other.snapshot.context.generation_id],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .unwrap();
            assert_eq!((state.as_str(), response_file), ("requesting", None));
        }
        owner.cleanup();
        other.cleanup();
    }

    #[tokio::test]
    async fn missing_expected_response_artifact_never_promotes_or_calls_decoder() {
        let fixture = PrecreatedTestFixture::new(1);
        let engine_calls = Arc::new(AtomicUsize::new(0));
        let decoder_calls = Arc::new(AtomicUsize::new(0));
        let body = perform_prepared_test_attempt(
            &FakeCoreEngine {
                calls: Arc::clone(&engine_calls),
                outcome: CoreEngineOutcome::Success,
            },
            &fixture.snapshot,
            &ProviderExecutionCredentials::new("ephemeral-secret".to_string()),
        )
        .await
        .unwrap();
        let missing_path = fixture.root.join("missing.response.json");
        let error = continue_precreated_generation_after_provider(
            fixture.db.as_ref(),
            &MissingFileArtifactStore {
                path: missing_path.clone(),
            },
            &FakeLocalDecoder {
                calls: Arc::clone(&decoder_calls),
                images: vec![jpeg_bytes()],
            },
            &LocalGenerationFileStore::new(fixture.file_root()),
            &fixture.snapshot,
            body,
            &CancellationProbe::new(),
        )
        .await
        .expect_err("a trait claim cannot replace a real verified artifact");
        assert_eq!(error.code(), "recovery_failed");
        assert_eq!(engine_calls.load(Ordering::SeqCst), 1);
        assert_eq!(decoder_calls.load(Ordering::SeqCst), 0);
        assert!(!missing_path.exists());
        let conn = fixture.db.conn.lock().unwrap();
        let (state, response_file, job_status, generation_status): (
            String,
            Option<String>,
            String,
            String,
        ) = conn
            .query_row(
                "SELECT r.request_state, r.response_file, j.status, g.status
                 FROM generation_recoveries r
                 JOIN generation_jobs j ON j.generation_id = r.generation_id
                 JOIN generations g ON g.id = r.generation_id
                 WHERE j.id = ?1",
                params![fixture.snapshot.context.job_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .unwrap();
        assert_eq!((state.as_str(), response_file), ("requesting", None));
        assert_eq!(
            (job_status.as_str(), generation_status.as_str()),
            ("running", "running")
        );
        drop(conn);
        fixture.cleanup();
    }

    #[tokio::test]
    async fn canonical_alias_response_path_is_not_the_exact_expected_artifact_identity() {
        let fixture = PrecreatedTestFixture::new(1);
        let decoder_calls = Arc::new(AtomicUsize::new(0));
        let alias_root = fixture.root.join("malicious-alias-store");
        let error = continue_precreated_generation_after_provider(
            fixture.db.as_ref(),
            &CanonicalAliasArtifactStore {
                root: alias_root.clone(),
            },
            &FakeLocalDecoder {
                calls: Arc::clone(&decoder_calls),
                images: vec![jpeg_bytes()],
            },
            &LocalGenerationFileStore::new(fixture.file_root()),
            &fixture.snapshot,
            ProviderAttemptBody {
                body_text: r#"{"data":[]}"#.to_string(),
                requested_image_count: 1,
            },
            &CancellationProbe::new(),
        )
        .await
        .expect_err("canonical aliases must not replace exact artifact identity");
        assert_eq!(error.code(), "recovery_failed");
        assert_eq!(decoder_calls.load(Ordering::SeqCst), 0);
        assert!(alias_root.join("artifact.response.json").is_file());
        let conn = fixture.db.conn.lock().unwrap();
        let (state, response_file): (String, Option<String>) = conn
            .query_row(
                "SELECT request_state, response_file FROM generation_recoveries
                 WHERE generation_id = ?1",
                params![fixture.snapshot.context.generation_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!((state.as_str(), response_file), ("requesting", None));
        drop(conn);
        fixture.cleanup();
    }

    #[tokio::test]
    async fn success_sql_failure_rolls_back_projection_then_cleans_promoted_files_unlocked() {
        let fixture = PrecreatedTestFixture::new(2);
        {
            let conn = fixture.db.conn.lock().unwrap();
            conn.execute_batch(
                "CREATE TEMP TRIGGER fail_late_success_projection
                 BEFORE UPDATE OF status ON generations
                 WHEN NEW.status = 'completed'
                 BEGIN SELECT RAISE(ABORT, 'injected late success failure'); END;",
            )
            .unwrap();
        }
        let engine_calls = Arc::new(AtomicUsize::new(0));
        let decoder_calls = Arc::new(AtomicUsize::new(0));
        let body = perform_prepared_test_attempt(
            &FakeCoreEngine {
                calls: Arc::clone(&engine_calls),
                outcome: CoreEngineOutcome::Success,
            },
            &fixture.snapshot,
            &ProviderExecutionCredentials::new("ephemeral-secret".to_string()),
        )
        .await
        .unwrap();
        let response_store = fixture.response_store();
        let file_observer = Arc::new(DbLockFileObserver::new(Arc::clone(&fixture.db)));
        let file_observer_trait: Arc<dyn GenerationFileLifecycleObserver> = file_observer.clone();
        let error = continue_precreated_generation_after_provider(
            fixture.db.as_ref(),
            &response_store,
            &FakeLocalDecoder {
                calls: Arc::clone(&decoder_calls),
                images: vec![jpeg_bytes(), jpeg_bytes()],
            },
            &ObservedLocalFileStore {
                root: fixture.file_root(),
                observer: file_observer_trait,
                path_observer: Arc::clone(&file_observer),
            },
            &fixture.snapshot,
            body,
            &CancellationProbe::new(),
        )
        .await
        .expect_err("injected SQL fault must roll back the whole success projection");
        assert!(matches!(
            &error,
            GenerationExecutionError::Local { code, stage, .. }
                if code == "database_error" && stage == "success_commit"
        ));
        assert_eq!(engine_calls.load(Ordering::SeqCst), 1);
        assert_eq!(decoder_calls.load(Ordering::SeqCst), 1);
        assert_eq!(file_observer.promote_calls.load(Ordering::SeqCst), 1);
        assert_eq!(file_observer.disarm_calls.load(Ordering::SeqCst), 0);
        assert_eq!(file_observer.cleanup_calls.load(Ordering::SeqCst), 1);
        assert!(file_observer.all_unlocked.load(Ordering::SeqCst));
        assert!(
            file_observer
                .cleanup_saw_all_promoted_paths
                .load(Ordering::SeqCst),
            "cleanup hook must see files that were promoted before SQL"
        );
        let promoted_paths = file_observer
            .expected_final_paths
            .get()
            .expect("staging records immutable final paths")
            .clone();
        assert_eq!(promoted_paths.len(), 4);
        assert!(promoted_paths.iter().all(|path| !path.exists()));

        {
            let conn = fixture.db.conn.lock().unwrap();
            let (job_status, generation_status, state, response_file, metadata): (
                String,
                String,
                String,
                Option<String>,
                String,
            ) = conn
                .query_row(
                    "SELECT j.status, g.status, r.request_state, r.response_file, g.request_metadata
                     FROM generation_jobs j
                     JOIN generations g ON g.id = j.generation_id
                     JOIN generation_recoveries r ON r.generation_id = g.id
                     WHERE j.id = ?1",
                    params![fixture.snapshot.context.job_id],
                    |row| {
                        Ok((
                            row.get(0)?,
                            row.get(1)?,
                            row.get(2)?,
                            row.get(3)?,
                            row.get(4)?,
                        ))
                    },
                )
                .unwrap();
            let image_count: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM images WHERE generation_id = ?1",
                    params![fixture.snapshot.context.generation_id],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(
                (job_status.as_str(), generation_status.as_str()),
                ("running", "running")
            );
            assert_eq!(state, "response_ready");
            assert!(response_file.is_some());
            assert_eq!(image_count, 0);
            let metadata: serde_json::Value = serde_json::from_str(&metadata).unwrap();
            assert!(metadata.get("actual_image_count").is_none());
        }
        let response_path = response_store
            .expected_response_path(&fixture.snapshot.context)
            .unwrap();
        response_store
            .load_verified_response(&fixture.snapshot.context, &response_path)
            .await
            .expect("paid response remains available for local recovery");
        fixture.cleanup();
    }

    #[tokio::test]
    async fn finalizer_sql_failure_rolls_back_recovery_and_both_terminal_rows() {
        let fixture = PrecreatedTestFixture::new(1);
        let response_store = fixture.response_store();
        let response = response_store
            .persist_verified_response(
                &fixture.snapshot.context,
                ProviderAttemptBody {
                    body_text: r#"{"data":[]}"#.to_string(),
                    requested_image_count: 1,
                },
            )
            .await
            .unwrap();
        let expected_path = response_store
            .expected_response_path(&fixture.snapshot.context)
            .unwrap();
        promote_verified_response(
            fixture.db.as_ref(),
            &fixture.snapshot.context,
            &expected_path,
            &response,
        )
        .unwrap();
        {
            let conn = fixture.db.conn.lock().unwrap();
            conn.execute_batch(
                "CREATE TEMP TRIGGER fail_terminal_generation
                 BEFORE UPDATE OF status ON generations
                 WHEN NEW.status = 'failed'
                 BEGIN SELECT RAISE(ABORT, 'injected terminal failure'); END;",
            )
            .unwrap();
        }
        let raw_secret = "sk-finalizer-rollback-secret";
        let execution_error = GenerationExecutionError::Local {
            code: "response_decode_failed".to_string(),
            sanitized_message: raw_secret.to_string(),
            stage: "response_decode".to_string(),
        };
        let error = finalize_generation_failure(
            fixture.db.as_ref(),
            &fixture.snapshot.context,
            &execution_error,
            &GenerationTerminalDisposition {
                status: GenerationJobStatus::Failed,
                error_code: "response_decode_failed".to_string(),
                retryable: false,
                preserve_response_ready: false,
            },
        )
        .expect_err("terminal SQL fault must roll back recovery deletion and job update");
        assert_eq!(error.stable_code(), "database_error");

        {
            let conn = fixture.db.conn.lock().unwrap();
            let (job_status, job_error, generation_status, generation_error, state, path): (
                String,
                Option<String>,
                String,
                Option<String>,
                String,
                Option<String>,
            ) = conn
                .query_row(
                    "SELECT j.status, j.error_message, g.status, g.error_message,
                            r.request_state, r.response_file
                     FROM generation_jobs j
                     JOIN generations g ON g.id = j.generation_id
                     JOIN generation_recoveries r ON r.generation_id = g.id
                     WHERE j.id = ?1",
                    params![fixture.snapshot.context.job_id],
                    |row| {
                        Ok((
                            row.get(0)?,
                            row.get(1)?,
                            row.get(2)?,
                            row.get(3)?,
                            row.get(4)?,
                            row.get(5)?,
                        ))
                    },
                )
                .unwrap();
            assert_eq!(
                (job_status.as_str(), generation_status.as_str()),
                ("running", "running")
            );
            assert_eq!((job_error, generation_error), (None, None));
            assert_eq!(state, "response_ready");
            assert_eq!(path.as_deref(), Some(response.response_file.as_str()));
            assert!(!format!("{path:?}").contains(raw_secret));
        }
        response_store
            .load_verified_response(
                &fixture.snapshot.context,
                Path::new(&response.response_file),
            )
            .await
            .expect("finalizer rollback never deletes the response artifact");
        fixture.cleanup();
    }

    #[tokio::test]
    async fn cancel_first_rejects_success_cleans_files_then_explicitly_acknowledges() {
        let fixture = PrecreatedTestFixture::new(1);
        let engine_calls = Arc::new(AtomicUsize::new(0));
        let decoder_calls = Arc::new(AtomicUsize::new(0));
        let body = perform_prepared_test_attempt(
            &FakeCoreEngine {
                calls: Arc::clone(&engine_calls),
                outcome: CoreEngineOutcome::Success,
            },
            &fixture.snapshot,
            &ProviderExecutionCredentials::new("ephemeral-secret".to_string()),
        )
        .await
        .unwrap();
        let file_observer = Arc::new(DbLockFileObserver::new(Arc::clone(&fixture.db)));
        let file_observer_trait: Arc<dyn GenerationFileLifecycleObserver> = file_observer.clone();
        let outcome = continue_precreated_generation_after_provider(
            fixture.db.as_ref(),
            &fixture.response_store(),
            &OrderedDatabaseDecoder {
                db: Arc::clone(&fixture.db),
                db_path: fixture.db_path.clone(),
                job_id: fixture.snapshot.context.job_id.clone(),
                generation_id: fixture.snapshot.context.generation_id.clone(),
                engine_calls: Arc::clone(&engine_calls),
                decoder_calls: Arc::clone(&decoder_calls),
                images: vec![jpeg_bytes()],
                request_cancel: true,
            },
            &ObservedLocalFileStore {
                root: fixture.file_root(),
                observer: file_observer_trait,
                path_observer: Arc::clone(&file_observer),
            },
            &fixture.snapshot,
            body,
            &CancellationProbe::new(),
        )
        .await
        .expect("cancel-first is a typed local outcome");
        assert!(matches!(outcome, PrecreatedLocalOutcome::CancelRequested));
        assert_eq!(engine_calls.load(Ordering::SeqCst), 1);
        assert_eq!(decoder_calls.load(Ordering::SeqCst), 1);
        assert_eq!(file_observer.promote_calls.load(Ordering::SeqCst), 1);
        assert_eq!(file_observer.disarm_calls.load(Ordering::SeqCst), 0);
        assert_eq!(file_observer.cleanup_calls.load(Ordering::SeqCst), 1);
        assert!(file_observer.all_unlocked.load(Ordering::SeqCst));
        assert_eq!(fixture.row_count("images"), 0);
        let cancel_error = GenerationExecutionError::Local {
            code: "cancelled_by_user".to_string(),
            sanitized_message: "The generation was cancelled".to_string(),
            stage: "cancellation".to_string(),
        };
        finalize_generation_failure(
            fixture.db.as_ref(),
            &fixture.snapshot.context,
            &cancel_error,
            &GenerationTerminalDisposition {
                status: GenerationJobStatus::Cancelled,
                error_code: "cancelled_by_user".to_string(),
                retryable: false,
                preserve_response_ready: false,
            },
        )
        .expect("acknowledge cancel after file cleanup");
        let conn = fixture.db.conn.lock().unwrap();
        let job = get_job(&conn, &fixture.snapshot.context.job_id).unwrap();
        assert_eq!(job.status, GenerationJobStatus::Cancelled);
        drop(conn);
        assert_eq!(fixture.row_count("generation_recoveries"), 0);
        fixture.cleanup();
    }

    #[tokio::test]
    async fn edit_source_revalidation_rejects_deleted_replaced_and_symlinked_inputs_before_engine()
    {
        enum Mutation {
            Delete,
            Replace,
            #[cfg(unix)]
            Symlink,
        }

        let mutations = [
            Mutation::Delete,
            Mutation::Replace,
            #[cfg(unix)]
            Mutation::Symlink,
        ];
        for mutation in mutations {
            let source_root = std::env::temp_dir().join(format!(
                "astro-studio-edit-revalidation-{}",
                uuid::Uuid::new_v4()
            ));
            std::fs::create_dir_all(&source_root).unwrap();
            let source = source_root.join("source.jpg");
            std::fs::write(&source, jpeg_bytes()).unwrap();
            let persisted_path = source.canonicalize().unwrap().to_string_lossy().to_string();
            let fixture = PrecreatedTestFixture::new_with_request(
                1,
                GenerationJobRequestKind::Edit,
                vec![persisted_path],
            );
            match mutation {
                Mutation::Delete => std::fs::remove_file(&source).unwrap(),
                Mutation::Replace => std::fs::write(&source, b"not an image").unwrap(),
                #[cfg(unix)]
                Mutation::Symlink => {
                    use std::os::unix::fs::symlink;
                    let original = source_root.join("original.jpg");
                    std::fs::rename(&source, &original).unwrap();
                    symlink(&original, &source).unwrap();
                }
            }
            let calls = Arc::new(AtomicUsize::new(0));
            let error = match perform_prepared_test_attempt(
                &FakeCoreEngine {
                    calls: Arc::clone(&calls),
                    outcome: CoreEngineOutcome::Success,
                },
                &fixture.snapshot,
                &ProviderExecutionCredentials::new("ephemeral-secret".to_string()),
            )
            .await
            {
                Err(error) => error,
                Ok(_) => panic!("invalid persisted edit source must fail before provider"),
            };
            assert_eq!(error.code(), "source_image_invalid");
            assert_eq!(calls.load(Ordering::SeqCst), 0);
            fixture.cleanup();
            std::fs::remove_dir_all(source_root).ok();
        }
    }

    #[tokio::test]
    async fn edit_attempt_uses_prepared_bytes_when_path_changes_at_engine_entry() {
        enum MutationKind {
            Delete,
            SameLengthReplacement,
            #[cfg(unix)]
            SymlinkReplacement,
        }

        for mutation_kind in [
            MutationKind::Delete,
            MutationKind::SameLengthReplacement,
            #[cfg(unix)]
            MutationKind::SymlinkReplacement,
        ] {
            let source_root = std::env::temp_dir().join(format!(
                "astro-studio-edit-prepared-bytes-{}",
                uuid::Uuid::new_v4()
            ));
            std::fs::create_dir_all(&source_root).unwrap();
            let source_path = source_root.join("source.jpg");
            let original = b"\xff\xd8\xfforiginal-source-payload".to_vec();
            let replacement = b"\xff\xd8\xffreplacement-source-data".to_vec();
            assert_eq!(original.len(), replacement.len());
            std::fs::write(&source_path, &original).unwrap();
            let canonical_source = source_path.canonicalize().unwrap();
            let fixture = PrecreatedTestFixture::new_with_request(
                1,
                GenerationJobRequestKind::Edit,
                vec![canonical_source.to_string_lossy().to_string()],
            );
            let mutation = match mutation_kind {
                MutationKind::Delete => EditPathMutationAtEngineEntry::Delete,
                MutationKind::SameLengthReplacement => {
                    EditPathMutationAtEngineEntry::ReplaceWithSameLength(replacement.clone())
                }
                #[cfg(unix)]
                MutationKind::SymlinkReplacement => {
                    let target = source_root.join("replacement.jpg");
                    std::fs::write(&target, &replacement).unwrap();
                    EditPathMutationAtEngineEntry::ReplaceWithSymlink(target)
                }
            };
            let calls = Arc::new(AtomicUsize::new(0));
            let prepared = prepare_provider_attempt(&fixture.snapshot)
                .await
                .expect("prepare edit bytes before provider future");
            let body = perform_provider_http_attempt(
                &PreparedBytesObservationEngine {
                    calls: Arc::clone(&calls),
                    source_path: canonical_source,
                    expected_bytes: original,
                    mutation,
                },
                &fixture.snapshot,
                &ProviderExecutionCredentials::new("ephemeral-secret".to_string()),
                &prepared,
            )
            .await
            .expect("prepared bytes remain authoritative after pathname mutation");
            assert_eq!(body.requested_image_count, 1);
            assert_eq!(calls.load(Ordering::SeqCst), 1);
            fixture.cleanup();
            std::fs::remove_dir_all(source_root).ok();
        }
    }

    #[test]
    fn edit_source_parent_swap_between_identity_and_cap_open_is_rejected() {
        let root = std::env::temp_dir().join(format!(
            "astro-studio-edit-parent-swap-{}",
            uuid::Uuid::new_v4()
        ));
        let parent = root.join("source-parent");
        let replacement_parent = root.join("replacement-parent");
        std::fs::create_dir_all(&parent).unwrap();
        std::fs::create_dir_all(&replacement_parent).unwrap();
        let source_path = parent.join("source.jpg");
        std::fs::write(&source_path, b"\xff\xd8\xfforiginal-source").unwrap();
        std::fs::write(
            replacement_parent.join("source.jpg"),
            b"\xff\xd8\xffreplacement-data",
        )
        .unwrap();
        let canonical_source = source_path.canonicalize().unwrap();
        let original_parent = root.join("original-parent");
        let mut hook_ran = false;
        let result = prepare_persisted_edit_image_with_hook(
            canonical_source.to_string_lossy().as_ref(),
            || {
                std::fs::rename(&parent, &original_parent).unwrap();
                std::fs::rename(&replacement_parent, &parent).unwrap();
                hook_ran = true;
            },
        );
        let error = match result {
            Err(error) => error,
            Ok(_) => panic!("replacement parent identity must be rejected"),
        };
        assert!(hook_ran);
        assert_eq!(error.code(), "source_image_invalid");
        assert_eq!(
            error.sanitized_message(),
            "A source image is unavailable or invalid"
        );
        std::fs::remove_dir_all(root).ok();
    }

    #[cfg(unix)]
    #[test]
    fn edit_source_parent_symlink_swap_after_canonicalize_is_rejected() {
        use std::os::unix::fs::symlink;

        let root = std::env::temp_dir().join(format!(
            "astro-studio-edit-parent-symlink-swap-{}",
            uuid::Uuid::new_v4()
        ));
        let parent = root.join("source-parent");
        let external_parent = root.join("external-parent");
        std::fs::create_dir_all(&parent).unwrap();
        std::fs::create_dir_all(&external_parent).unwrap();
        let source_path = parent.join("source.jpg");
        std::fs::write(&source_path, b"\xff\xd8\xfforiginal-source").unwrap();
        std::fs::write(
            external_parent.join("source.jpg"),
            b"\xff\xd8\xffexternal-source",
        )
        .unwrap();
        let canonical_source = source_path.canonicalize().unwrap();
        let original_parent = root.join("original-parent");
        let mut hook_ran = false;
        let result = prepare_persisted_edit_image_with_hook(
            canonical_source.to_string_lossy().as_ref(),
            || {
                std::fs::rename(&parent, &original_parent).unwrap();
                symlink(&external_parent, &parent).unwrap();
                hook_ran = true;
            },
        );

        let error = match result {
            Err(error) => error,
            Ok(_) => panic!("symlinked replacement parent must not become the identity anchor"),
        };
        assert!(hook_ran);
        assert_eq!(error.code(), "source_image_invalid");
        std::fs::remove_dir_all(root).ok();
    }

    #[tokio::test]
    async fn cross_wired_success_images_are_rejected_without_mutation() {
        let fixture = PrecreatedTestFixture::new(1);
        let other = PrecreatedTestFixture::new(1);
        let response_store = fixture.response_store();
        let response = response_store
            .persist_verified_response(
                &fixture.snapshot.context,
                ProviderAttemptBody {
                    body_text: r#"{"data":[]}"#.to_string(),
                    requested_image_count: 1,
                },
            )
            .await
            .unwrap();
        let expected_path = response_store
            .expected_response_path(&fixture.snapshot.context)
            .unwrap();
        promote_verified_response(
            fixture.db.as_ref(),
            &fixture.snapshot.context,
            &expected_path,
            &response,
        )
        .unwrap();
        let promoted = file_manager::FileManager::new(other.file_root())
            .stage_generation_images(
                &other.snapshot.context.generation_id,
                &[jpeg_bytes()],
                &other.snapshot.output_format,
                &other.snapshot.created_at,
            )
            .unwrap()
            .promote()
            .unwrap();
        let promoted_commit =
            PromotedImageCommit::from_promoted(&promoted, &other.snapshot.context.generation_id)
                .unwrap();
        let error = {
            let mut conn = fixture.db.conn.lock().unwrap();
            let tx = begin_generation_job_write_transaction(&mut conn).unwrap();
            let error = commit_generation_success_in_transaction(
                &tx,
                &fixture.snapshot.context,
                &fixture.snapshot.request,
                &promoted_commit,
            )
            .expect_err("cross-wired image projection must fail");
            tx.rollback().unwrap();
            error
        };
        drop(promoted_commit);
        let promoted_paths = promoted.final_paths();
        drop(promoted);
        assert!(promoted_paths.iter().all(|path| !path.exists()));
        assert_eq!(error.stable_code(), "generation_job_invalid_snapshot");
        assert_eq!(fixture.row_count("images"), 0);
        assert_eq!(fixture.row_count("generation_recoveries"), 1);
        {
            let conn = fixture.db.conn.lock().unwrap();
            assert_eq!(
                get_job(&conn, &fixture.snapshot.context.job_id)
                    .unwrap()
                    .status,
                GenerationJobStatus::Running
            );
        }
        fixture.cleanup();
        other.cleanup();
    }

    #[tokio::test]
    async fn success_transaction_uses_only_guard_derived_image_paths() {
        let fixture = PrecreatedTestFixture::new(1);
        let response_store = fixture.response_store();
        let response = response_store
            .persist_verified_response(
                &fixture.snapshot.context,
                ProviderAttemptBody {
                    body_text: r#"{"data":[]}"#.to_string(),
                    requested_image_count: 1,
                },
            )
            .await
            .expect("persist response before forged success attempt");
        let expected_path = response_store
            .expected_response_path(&fixture.snapshot.context)
            .expect("derive response path before forged success attempt");
        promote_verified_response(
            fixture.db.as_ref(),
            &fixture.snapshot.context,
            &expected_path,
            &response,
        )
        .expect("promote verified response");
        let promoted = file_manager::FileManager::new(fixture.file_root())
            .stage_generation_images(
                &fixture.snapshot.context.generation_id,
                &[jpeg_bytes()],
                &fixture.snapshot.output_format,
                &fixture.snapshot.created_at,
            )
            .unwrap()
            .promote()
            .unwrap();
        let promoted_paths = promoted.final_paths();
        let promoted_commit =
            PromotedImageCommit::from_promoted(&promoted, &fixture.snapshot.context.generation_id)
                .expect("derive sealed success input from the live promoted guard");
        assert_eq!(promoted_commit.images().len(), 1);
        assert_eq!(
            promoted_commit.images()[0].file_path,
            promoted_paths[0].to_string_lossy()
        );
        assert_eq!(
            promoted_commit.images()[0].thumbnail_path,
            promoted_paths[1].to_string_lossy()
        );

        let result = {
            let mut conn = fixture.db.conn.lock().unwrap();
            let tx = begin_generation_job_write_transaction(&mut conn).unwrap();
            let result = commit_generation_success_in_transaction(
                &tx,
                &fixture.snapshot.context,
                &fixture.snapshot.request,
                &promoted_commit,
            );
            tx.rollback().unwrap();
            result
        };
        assert!(matches!(
            result,
            Ok(GenerationSuccessTransition::Completed(_))
        ));
        drop(promoted_commit);
        drop(promoted);
        assert!(promoted_paths.iter().all(|path| !path.exists()));
        assert_eq!(fixture.row_count("images"), 0);
        assert_eq!(fixture.row_count("generation_recoveries"), 1);
        fixture.cleanup();
    }

    #[async_trait::async_trait]
    impl ImageResponseDecoder for FakeLocalDecoder {
        async fn decode_and_download(
            &self,
            _response: &ProviderAttemptResponse,
            cancellation: &CancellationProbe,
        ) -> Result<Vec<Vec<u8>>, GenerationExecutionError> {
            cancellation.checkpoint("response_decode")?;
            self.calls.fetch_add(1, Ordering::SeqCst);
            Ok(self.images.clone())
        }
    }

    #[test]
    fn lifecycle_request_kind_names_generate_and_edit_for_persistence() {
        assert_eq!(GenerationLifecycleKind::Generate.as_str(), "generate");
        assert_eq!(GenerationLifecycleKind::Edit.as_str(), "edit");
    }

    #[tokio::test]
    async fn response_artifact_path_is_deterministic_and_verified_after_atomic_write() {
        let root = std::env::temp_dir().join(format!(
            "astro-studio-response-artifact-test-{}",
            uuid::Uuid::new_v4()
        ));
        let store = FileResponseArtifactStore::new(root.clone());
        let context = fixture_execution_context();
        let response = store
            .persist_verified_response(
                &context,
                ProviderAttemptBody {
                    body_text: r#"{"data":[{"b64_json":"aW1hZ2U="}]}"#.to_string(),
                    requested_image_count: 2,
                },
            )
            .await
            .expect("persist verified response");

        assert_eq!(
            Path::new(&response.response_file),
            root.join("generation-jobs")
                .join("job-1")
                .join("generation-1.response.json")
        );
        assert_eq!(response.requested_image_count, 2);
        assert_eq!(response.response_size, response.body_text.len() as u64);
        assert_eq!(response.response_sha256.len(), 64);
        assert!(Path::new(&response.response_file).is_file());
        assert!(std::fs::read_dir(
            Path::new(&response.response_file)
                .parent()
                .expect("response parent")
        )
        .expect("read response directory")
        .all(|entry| !entry
            .expect("directory entry")
            .file_name()
            .to_string_lossy()
            .ends_with(".tmp")));

        let loaded = store
            .load_verified_response(&context, Path::new(&response.response_file))
            .await
            .expect("load verified response");
        assert_eq!(loaded.body_text, response.body_text);
        assert_eq!(loaded.response_sha256, response.response_sha256);

        std::fs::remove_dir_all(root).ok();
    }

    #[tokio::test]
    async fn oversized_encoded_response_envelope_creates_no_final_or_temporary_file() {
        let root = std::env::temp_dir().join(format!(
            "astro-studio-response-envelope-limit-{}",
            uuid::Uuid::new_v4()
        ));
        let store = FileResponseArtifactStore::new(root.clone());
        let context = fixture_execution_context();
        let expected_path = store
            .expected_response_path(&context)
            .expect("derive oversized response path");
        let control_body_length =
            usize::try_from(FileResponseArtifactStore::MAX_RESPONSE_BODY_BYTES / 3)
                .expect("response limit fits usize");
        let result = store
            .persist_verified_response(
                &context,
                ProviderAttemptBody {
                    body_text: "\0".repeat(control_body_length),
                    requested_image_count: 1,
                },
            )
            .await;

        let error = match result {
            Err(error) => error,
            Ok(_) => panic!("an envelope beyond the loader bound must not be published"),
        };
        assert_eq!(error.code(), "recovery_failed");
        assert!(!expected_path.exists());
        assert!(
            !root.exists(),
            "envelope size validation must happen before directory or temp-file creation"
        );
    }

    #[tokio::test]
    async fn response_artifact_rejects_tampered_body_or_metadata() {
        let root = std::env::temp_dir().join(format!(
            "astro-studio-response-artifact-tamper-test-{}",
            uuid::Uuid::new_v4()
        ));
        let store = FileResponseArtifactStore::new(root.clone());
        let context = fixture_execution_context();
        let response = store
            .persist_verified_response(
                &context,
                ProviderAttemptBody {
                    body_text: r#"{"data":[]}"#.to_string(),
                    requested_image_count: 1,
                },
            )
            .await
            .expect("persist response");
        let path = Path::new(&response.response_file);
        let mut envelope: serde_json::Value =
            serde_json::from_slice(&std::fs::read(path).expect("read envelope"))
                .expect("parse envelope");
        envelope["body_text"] = serde_json::json!("tampered");
        std::fs::write(
            path,
            serde_json::to_vec(&envelope).expect("encode envelope"),
        )
        .expect("tamper envelope");

        let error = match store.load_verified_response(&context, path).await {
            Err(error) => error,
            Ok(_) => panic!("tampered response must fail verification"),
        };
        assert_eq!(error.code(), "recovery_failed");
        assert!(!error.sanitized_message().contains("tampered"));

        std::fs::remove_dir_all(root).ok();
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn response_artifact_rejects_symlinked_job_parent_without_external_leak() {
        use std::os::unix::fs::symlink;

        let root = std::env::temp_dir().join(format!(
            "astro-studio-response-symlink-test-{}",
            uuid::Uuid::new_v4()
        ));
        let outside = std::env::temp_dir().join(format!(
            "astro-studio-response-symlink-outside-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(root.join("generation-jobs")).expect("create response root");
        std::fs::create_dir_all(&outside).expect("create outside directory");
        symlink(&outside, root.join("generation-jobs").join("job-1")).expect("symlink job parent");

        let store = FileResponseArtifactStore::new(root.clone());
        let result = store
            .persist_verified_response(
                &fixture_execution_context(),
                ProviderAttemptBody {
                    body_text: r#"{"data":[]}"#.to_string(),
                    requested_image_count: 1,
                },
            )
            .await;
        assert!(result.is_err());
        assert!(!outside.join("generation-1.response.json").exists());

        std::fs::remove_dir_all(root).ok();
        std::fs::remove_dir_all(outside).ok();
    }

    #[cfg(unix)]
    #[test]
    fn response_writer_rejects_job_directory_swapped_after_validation() {
        use std::os::unix::fs::symlink;

        let root = std::env::temp_dir().join(format!(
            "astro-studio-response-write-swap-test-{}",
            uuid::Uuid::new_v4()
        ));
        let job_dir = root.join("generation-jobs").join("job-1");
        let original_job_dir = root.join("generation-jobs").join("job-1-original");
        let other_job_dir = root.join("generation-jobs").join("job-other");
        std::fs::create_dir_all(&job_dir).expect("create job directory");
        std::fs::create_dir_all(&other_job_dir).expect("create other job directory");
        let context = fixture_execution_context();
        let mut swapped = false;

        let result = FileResponseArtifactStore::prepare_response_directory_with_hook(
            &root,
            &context,
            &mut |stage| {
                if stage == ResponseDirectoryOpenStage::Job && !swapped {
                    std::fs::rename(&job_dir, &original_job_dir)
                        .expect("move validated job directory");
                    symlink("job-other", &job_dir)
                        .expect("replace job directory with sibling symlink");
                    swapped = true;
                }
            },
        );
        if let Ok(prepared) = &result {
            prepared
                .directory
                .write("escaped", b"escaped")
                .expect("demonstrate followed directory handle");
        }

        assert!(result.is_err());
        assert!(!other_job_dir.join("escaped").exists());
        std::fs::remove_dir_all(root).ok();
    }

    #[tokio::test]
    async fn concurrent_response_install_never_overwrites_or_deletes_the_winner() {
        let root = std::env::temp_dir().join(format!(
            "astro-studio-response-install-race-test-{}",
            uuid::Uuid::new_v4()
        ));
        let store = FileResponseArtifactStore::new(root.clone());
        let context = fixture_execution_context();
        let first = store.persist_verified_response(
            &context,
            ProviderAttemptBody {
                body_text: r#"{"winner":"first"}"#.to_string(),
                requested_image_count: 1,
            },
        );
        let second = store.persist_verified_response(
            &context,
            ProviderAttemptBody {
                body_text: r#"{"winner":"second"}"#.to_string(),
                requested_image_count: 1,
            },
        );
        let (first, second) = tokio::join!(first, second);
        assert_eq!(usize::from(first.is_ok()) + usize::from(second.is_ok()), 1);

        let path = store
            .expected_response_path(&context)
            .expect("expected response path");
        let loaded = store
            .load_verified_response(&context, &path)
            .await
            .expect("winner remains verified");
        assert!(matches!(
            loaded.body_text.as_str(),
            r#"{"winner":"first"}"# | r#"{"winner":"second"}"#
        ));

        let mut same_context = context.clone();
        same_context.job_id = "job-2".to_string();
        same_context.generation_id = "generation-2".to_string();
        let same_body = || ProviderAttemptBody {
            body_text: r#"{"same":true}"#.to_string(),
            requested_image_count: 1,
        };
        let (same_first, same_second) = tokio::join!(
            store.persist_verified_response(&same_context, same_body()),
            store.persist_verified_response(&same_context, same_body()),
        );
        assert!(same_first.is_ok());
        assert!(same_second.is_ok());

        std::fs::remove_dir_all(root).ok();
    }

    #[tokio::test]
    async fn response_loader_rejects_another_jobs_valid_artifact() {
        let root = std::env::temp_dir().join(format!(
            "astro-studio-response-context-test-{}",
            uuid::Uuid::new_v4()
        ));
        let store = FileResponseArtifactStore::new(root.clone());
        let expected_context = fixture_execution_context();
        let mut other_context = expected_context.clone();
        other_context.job_id = "job-other".to_string();
        other_context.generation_id = "generation-other".to_string();
        let other = store
            .persist_verified_response(
                &other_context,
                ProviderAttemptBody {
                    body_text: r#"{"other":true}"#.to_string(),
                    requested_image_count: 1,
                },
            )
            .await
            .expect("persist other response");
        assert!(store
            .load_verified_response(&expected_context, Path::new(&other.response_file))
            .await
            .is_err());
        std::fs::remove_dir_all(root).ok();
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn response_loader_rejects_a_symlinked_response_root() {
        use std::os::unix::fs::symlink;

        let parent = std::env::temp_dir().join(format!(
            "astro-studio-response-root-symlink-test-{}",
            uuid::Uuid::new_v4()
        ));
        let real_root = parent.join("real-root");
        let alias_root = parent.join("alias-root");
        let real_store = FileResponseArtifactStore::new(real_root.clone());
        let context = fixture_execution_context();
        real_store
            .persist_verified_response(
                &context,
                ProviderAttemptBody {
                    body_text: r#"{"data":[]}"#.to_string(),
                    requested_image_count: 1,
                },
            )
            .await
            .expect("persist response under real root");
        symlink(&real_root, &alias_root).expect("create response-root symlink");
        let alias_store = FileResponseArtifactStore::new(alias_root.clone());
        let alias_path = alias_store
            .expected_response_path(&context)
            .expect("derive alias response path");

        let result = alias_store
            .load_verified_response(&context, &alias_path)
            .await;
        let error = match result {
            Err(error) => error,
            Ok(_) => panic!("loader must reject the response root before canonicalizing it"),
        };
        assert_eq!(error.code(), "recovery_failed");
        std::fs::remove_file(alias_root).ok();
        std::fs::remove_dir_all(parent).ok();
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn response_loader_rejects_final_component_symlink() {
        use std::os::unix::fs::symlink;

        let root = std::env::temp_dir().join(format!(
            "astro-studio-response-final-symlink-test-{}",
            uuid::Uuid::new_v4()
        ));
        let store = FileResponseArtifactStore::new(root.clone());
        let context = fixture_execution_context();
        let response = store
            .persist_verified_response(
                &context,
                ProviderAttemptBody {
                    body_text: r#"{"data":[]}"#.to_string(),
                    requested_image_count: 1,
                },
            )
            .await
            .expect("persist response");
        let response_path = Path::new(&response.response_file);
        let target = response_path.with_file_name("symlink-target.response.json");
        std::fs::write(
            &target,
            std::fs::read(response_path).expect("read response"),
        )
        .expect("write symlink target");
        std::fs::remove_file(response_path).expect("remove response before symlink");
        symlink(&target, response_path).expect("create final symlink");

        assert!(store
            .load_verified_response(&context, response_path)
            .await
            .is_err());
        std::fs::remove_dir_all(root).ok();
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn response_loader_rejects_job_directory_swapped_after_validation() {
        use std::os::unix::fs::symlink;

        let root = std::env::temp_dir().join(format!(
            "astro-studio-response-read-swap-test-{}",
            uuid::Uuid::new_v4()
        ));
        let store = FileResponseArtifactStore::new(root.clone());
        let context = fixture_execution_context();
        let response = store
            .persist_verified_response(
                &context,
                ProviderAttemptBody {
                    body_text: r#"{"trusted":true}"#.to_string(),
                    requested_image_count: 1,
                },
            )
            .await
            .expect("persist trusted response");
        let response_path = PathBuf::from(&response.response_file);
        let job_dir = root.join("generation-jobs").join("job-1");
        let original_job_dir = root.join("generation-jobs").join("job-1-original");
        let other_job_dir = root.join("generation-jobs").join("job-other");
        std::fs::create_dir_all(&other_job_dir).expect("create other job directory");
        std::fs::copy(
            &response_path,
            other_job_dir.join("generation-1.response.json"),
        )
        .expect("copy valid envelope into other job");
        let mut swapped = false;

        let loaded = FileResponseArtifactStore::load_verified_response_sync_with_hook(
            &root,
            &context,
            &response_path,
            &mut |stage| {
                if stage == ResponseDirectoryOpenStage::Job && !swapped {
                    std::fs::rename(&job_dir, &original_job_dir)
                        .expect("move validated job directory");
                    symlink("job-other", &job_dir)
                        .expect("replace job directory with sibling symlink");
                    swapped = true;
                }
            },
        );

        assert!(loaded.is_err());
        std::fs::remove_dir_all(root).ok();
    }

    #[tokio::test]
    async fn provider_attempt_uses_snapshot_once_then_hands_raw_body_to_artifact_store() {
        let engine_calls = Arc::new(AtomicUsize::new(0));
        let artifact_calls = Arc::new(AtomicUsize::new(0));
        let artifact_root = std::env::temp_dir().join(format!(
            "astro-studio-fake-artifact-store-{}",
            uuid::Uuid::new_v4()
        ));
        let body = perform_prepared_test_attempt(
            &FakeSingleAttemptEngine {
                calls: Arc::clone(&engine_calls),
            },
            &fixture_execution_snapshot(),
            &ProviderExecutionCredentials::new("ephemeral-key".to_string()),
        )
        .await
        .expect("perform provider HTTP attempt");
        assert_eq!(engine_calls.load(Ordering::SeqCst), 1);
        assert_eq!(artifact_calls.load(Ordering::SeqCst), 0);

        let response = persist_provider_attempt_response(
            &FakeArtifactStore {
                calls: Arc::clone(&artifact_calls),
                root: artifact_root.clone(),
            },
            &fixture_execution_snapshot(),
            body,
        )
        .await
        .expect("persist provider response");

        assert_eq!(artifact_calls.load(Ordering::SeqCst), 1);
        assert_eq!(response.requested_image_count, 2);
        std::fs::remove_dir_all(artifact_root).ok();
    }

    #[tokio::test]
    async fn sealed_prepared_attempt_rejects_snapshot_and_variant_crosswire_before_engine() {
        let snapshot = fixture_execution_snapshot();
        let calls = Arc::new(AtomicUsize::new(0));
        let credentials = ProviderExecutionCredentials::new("ephemeral-key".to_string());

        let prepared = prepare_provider_attempt(&snapshot).await.unwrap();
        let mut other_snapshot = snapshot.clone();
        other_snapshot.context.job_id = "job-other".to_string();
        let context_error = match perform_provider_http_attempt(
            &FakeSingleAttemptEngine {
                calls: Arc::clone(&calls),
            },
            &other_snapshot,
            &credentials,
            &prepared,
        )
        .await
        {
            Err(error) => error,
            Ok(_) => panic!("prepared context cannot be rebound to another snapshot"),
        };
        assert_eq!(context_error.code(), "provider_configuration_invalid");

        let wrong_variant = PreparedProviderAttempt {
            context: snapshot.context.clone(),
            request: snapshot.request.clone(),
            payload: PreparedProviderPayload::Edit(Vec::new()),
        };
        let variant_error = match perform_provider_http_attempt(
            &FakeSingleAttemptEngine {
                calls: Arc::clone(&calls),
            },
            &snapshot,
            &credentials,
            &wrong_variant,
        )
        .await
        {
            Err(error) => error,
            Ok(_) => panic!("generate snapshot cannot consume edit preparation"),
        };
        assert_eq!(variant_error.code(), "provider_configuration_invalid");
        assert_eq!(calls.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn persisted_response_is_reread_before_it_can_become_verified() {
        let root = std::env::temp_dir().join(format!(
            "astro-studio-wrong-on-disk-response-{}",
            uuid::Uuid::new_v4()
        ));
        let load_calls = Arc::new(AtomicUsize::new(0));
        let snapshot = fixture_execution_snapshot();
        let result = persist_provider_attempt_response(
            &WrongOnDiskArtifactStore {
                root: root.clone(),
                load_calls: Arc::clone(&load_calls),
            },
            &snapshot,
            ProviderAttemptBody {
                body_text: r#"{"data":[]}"#.to_string(),
                requested_image_count: snapshot.runtime_options.image_count,
            },
        )
        .await;

        let error = match result {
            Err(error) => error,
            Ok(_) => panic!("a correct return value cannot substitute for the on-disk envelope"),
        };
        assert_eq!(error.code(), "recovery_failed");
        assert_eq!(load_calls.load(Ordering::SeqCst), 1);
        std::fs::remove_dir_all(root).ok();
    }

    #[tokio::test]
    async fn verified_response_decodes_then_stages_without_provider_submission() {
        let root = std::env::temp_dir().join(format!(
            "astro-studio-local-resume-test-{}",
            uuid::Uuid::new_v4()
        ));
        let decoder_calls = Arc::new(AtomicUsize::new(0));
        let response = ProviderAttemptResponse {
            body_text: r#"{"data":[]}"#.to_string(),
            response_file: root.join("response.json").to_string_lossy().to_string(),
            response_size: 11,
            response_sha256: "0".repeat(64),
            requested_image_count: 2,
        };
        let staged = resume_verified_response(
            &FakeLocalDecoder {
                calls: Arc::clone(&decoder_calls),
                images: vec![jpeg_bytes()],
            },
            &LocalGenerationFileStore::new(root.clone()),
            &fixture_execution_snapshot(),
            &response,
            &CancellationProbe::new(),
        )
        .await
        .expect("resume response locally");

        assert_eq!(decoder_calls.load(Ordering::SeqCst), 1);
        assert_eq!(staged.len(), 1);
        assert!(staged.final_paths().iter().all(|path| !path.exists()));
        drop(staged);
        std::fs::remove_dir_all(root).ok();
    }

    #[tokio::test]
    async fn cancelled_local_resume_stops_before_decoder_or_staging() {
        let decoder_calls = Arc::new(AtomicUsize::new(0));
        let cancellation = CancellationProbe::new();
        cancellation.cancel();
        let response = ProviderAttemptResponse {
            body_text: r#"{"data":[]}"#.to_string(),
            response_file: "/safe/response.json".to_string(),
            response_size: 11,
            response_sha256: "0".repeat(64),
            requested_image_count: 2,
        };
        let root = std::env::temp_dir().join(format!(
            "astro-studio-cancelled-resume-test-{}",
            uuid::Uuid::new_v4()
        ));
        let result = resume_verified_response(
            &FakeLocalDecoder {
                calls: Arc::clone(&decoder_calls),
                images: vec![jpeg_bytes()],
            },
            &LocalGenerationFileStore::new(root.clone()),
            &fixture_execution_snapshot(),
            &response,
            &cancellation,
        )
        .await;
        let error = match result {
            Err(error) => error,
            Ok(_) => panic!("cancelled resume must fail"),
        };
        assert_eq!(error.code(), "cancelled_by_user");
        assert_eq!(decoder_calls.load(Ordering::SeqCst), 0);
        assert!(!root.exists());
    }

    #[test]
    fn retry_after_metadata_remains_available_to_worker_policy() {
        let error = GenerationExecutionError::Engine(EngineCallError::from_http_status(
            429,
            Some(RetryAfterHint::DelaySeconds(5)),
        ));
        assert_eq!(error.code(), "rate_limited");
        assert!(matches!(
            error,
            GenerationExecutionError::Engine(EngineCallError {
                retry_after: Some(RetryAfterHint::DelaySeconds(5)),
                safe_to_retry: true,
                ..
            })
        ));
    }
}
