use crate::api_gateway::{EngineCallError, ImageEngine, ProviderAttemptBody};
use crate::commands::{conversations, settings};
use crate::current_timestamp;
use crate::db::Database;
use crate::error::AppError;
use crate::file_manager::{self, StagedGenerationFiles};
use crate::generation_jobs::{GenerationJobRequest, GenerationJobRequestKind};
use crate::image_engines::{provider_for_model, ImageProvider};
use crate::model_registry::{
    image_endpoint_url_for_model_settings, normalize_image_model,
    sanitize_request_options_for_model, ImageEndpointKind,
};
use crate::models::*;
use cap_fs_ext::DirExt;
use rusqlite::params;
use sha2::{Digest, Sha256};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::{Emitter, Manager};

const RECOVERY_STATE_REQUESTING: &str = "requesting";

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

/// Verified response envelope. Raw bodies are deliberately not serializable or
/// debuggable outside the app-owned response store.
pub(crate) struct ProviderAttemptResponse {
    pub(crate) body_text: String,
    pub(crate) response_file: String,
    pub(crate) response_size: u64,
    pub(crate) response_sha256: String,
    pub(crate) requested_image_count: u8,
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
        let maximum_envelope_size = Self::MAX_RESPONSE_BODY_BYTES.saturating_mul(2);
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
        Self::decode_verified_envelope(&canonical_expected_path, &bytes)
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
        let relative_path = Self::response_relative_path(context)?;
        if response_path != root_dir.join(&relative_path) {
            return Err(GenerationExecutionError::response_artifact());
        }
        let file_name = relative_path
            .file_name()
            .and_then(|value| value.to_str())
            .ok_or_else(GenerationExecutionError::response_artifact)?;
        let prepared = Self::prepare_response_directory(root_dir, context)?;
        let response_path = prepared.absolute_directory.join(file_name);
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
                let existing =
                    Self::load_verified_response_sync(root_dir, context, &response_path)?;
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

        let verified = match Self::load_verified_response_sync(root_dir, context, &response_path) {
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

pub(crate) async fn perform_provider_http_attempt(
    engine: &dyn ImageEngine,
    snapshot: &GenerationExecutionSnapshot,
    credentials: &ProviderExecutionCredentials,
) -> Result<ProviderAttemptBody, GenerationExecutionError> {
    let expected_provider_kind = match provider_for_model(&snapshot.context.model) {
        ImageProvider::OpenAi => "openai",
        ImageProvider::Gemini => "gemini",
    };
    if snapshot.context.provider_kind != expected_provider_kind
        || snapshot.request.model != snapshot.context.model
        || snapshot.request.conversation_id != snapshot.context.conversation_id
        || snapshot.output_format != snapshot.runtime_options.output_format
        || snapshot.context.endpoint_url.is_empty()
        || snapshot.context.provider_profile_id.is_empty()
        || credentials.expose_for_engine().is_empty()
    {
        return Err(GenerationExecutionError::Engine(
            EngineCallError::provider_configuration_invalid(),
        ));
    }

    let body = match snapshot.request.kind {
        GenerationJobRequestKind::Generate => {
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
            engine
                .edit(
                    &snapshot.context.model,
                    credentials.expose_for_engine(),
                    &snapshot.context.endpoint_url,
                    &snapshot.request.prompt,
                    &snapshot.request.source_image_paths,
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
    let response = artifact_store
        .persist_verified_response(&snapshot.context, body)
        .await?;
    if response.requested_image_count != snapshot.runtime_options.image_count {
        return Err(GenerationExecutionError::response_artifact());
    }
    Ok(response)
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

pub(crate) fn source_image_paths_json(source_image_paths: &[String]) -> Result<String, AppError> {
    serde_json::to_string(source_image_paths).map_err(|e| AppError::Database {
        message: format!("Serialize source image paths failed: {}", e),
    })
}

pub(crate) fn generation_request_metadata_json(
    request_kind: GenerationLifecycleKind,
    conversation_id: &str,
    model: &str,
    options: &GptImageRequestOptions,
    source_image_paths: &[String],
) -> Result<String, AppError> {
    serde_json::to_string(&serde_json::json!({
        "request_kind": request_kind.as_str(),
        "conversation_id": conversation_id,
        "model": model,
        "size": &options.size,
        "quality": &options.quality,
        "background": &options.background,
        "output_format": &options.output_format,
        "output_compression": options.output_compression,
        "moderation": &options.moderation,
        "input_fidelity": &options.input_fidelity,
        "stream": options.stream,
        "partial_images": options.partial_images,
        "image_count": options.image_count,
        "source_image_count": source_image_paths.len(),
    }))
    .map_err(|e| AppError::Database {
        message: format!("Serialize generation metadata failed: {}", e),
    })
}

fn resolve_image_endpoint_url_for_model(
    db: &Database,
    model: &str,
    kind: ImageEndpointKind,
) -> Result<String, AppError> {
    let settings = settings::read_model_endpoint_settings(db, model)?;
    Ok(image_endpoint_url_for_model_settings(
        model, &settings, kind,
    ))
}

fn create_processing_generation(
    conn: &rusqlite::Connection,
    generation_id: &str,
    prompt: &str,
    model: &str,
    options: &GptImageRequestOptions,
    conversation_id: &str,
    created_at: &str,
    request_kind: GenerationLifecycleKind,
    source_image_paths: &[String],
) -> Result<(), AppError> {
    let source_image_paths_json = source_image_paths_json(source_image_paths)?;
    let request_metadata = generation_request_metadata_json(
        request_kind,
        conversation_id,
        model,
        options,
        source_image_paths,
    )?;
    let tx = conn
        .unchecked_transaction()
        .map_err(|e| AppError::Database {
            message: format!("Begin transaction failed: {}", e),
        })?;
    tx.execute(
        "INSERT INTO generations (
            id, prompt, engine, request_kind, size, quality, background, output_format,
            output_compression, moderation, input_fidelity, image_count, source_image_count,
            source_image_paths, request_metadata, status, error_message, conversation_id, created_at
         ) VALUES (
            ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15,
            'processing', NULL, ?16, ?17
         )",
        params![
            generation_id,
            prompt,
            model,
            request_kind.as_str(),
            &options.size,
            &options.quality,
            &options.background,
            &options.output_format,
            options.output_compression,
            &options.moderation,
            &options.input_fidelity,
            options.image_count,
            source_image_paths.len() as i64,
            source_image_paths_json,
            request_metadata,
            conversation_id,
            created_at
        ],
    )
    .map_err(|e| AppError::Database {
        message: format!("Insert processing generation failed: {}", e),
    })?;
    tx.execute(
        "INSERT INTO generation_recoveries (generation_id, request_kind, request_state, output_format, response_file, created_at, updated_at) \
         VALUES (?1, ?2, ?3, ?4, NULL, ?5, ?5)",
        params![
            generation_id,
            request_kind.as_str(),
            RECOVERY_STATE_REQUESTING,
            &options.output_format,
            created_at
        ],
    )
    .map_err(|e| AppError::Database {
        message: format!("Insert generation recovery failed: {}", e),
    })?;
    tx.commit().map_err(|e| AppError::Database {
        message: format!("Commit transaction failed: {}", e),
    })
}

fn set_generation_failed(
    conn: &rusqlite::Connection,
    generation_id: &str,
    error_message: &str,
    clear_recovery: bool,
) -> Result<(), AppError> {
    let tx = conn
        .unchecked_transaction()
        .map_err(|e| AppError::Database {
            message: format!("Begin transaction failed: {}", e),
        })?;
    tx.execute(
        "UPDATE generations SET status = 'failed', error_message = ?1 WHERE id = ?2",
        params![error_message, generation_id],
    )
    .map_err(|e| AppError::Database {
        message: format!("Update generation status failed: {}", e),
    })?;
    if clear_recovery {
        tx.execute(
            "DELETE FROM generation_recoveries WHERE generation_id = ?1",
            params![generation_id],
        )
        .map_err(|e| AppError::Database {
            message: format!("Clear generation recovery failed: {}", e),
        })?;
    }
    tx.commit().map_err(|e| AppError::Database {
        message: format!("Commit transaction failed: {}", e),
    })
}

fn save_generation_images(
    app: &tauri::AppHandle,
    db: &Database,
    generation_id: &str,
    created_at: &str,
    output_format: &str,
    images_data: &[Vec<u8>],
) -> Result<Vec<GeneratedImage>, AppError> {
    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| AppError::FileSystem {
            message: format!("Get app data dir failed: {}", e),
        })?;
    let fm = file_manager::FileManager::new(app_data_dir);
    let conn = db.conn.lock().map_err(|e| AppError::Database {
        message: format!("Lock failed: {}", e),
    })?;
    let tx = conn
        .unchecked_transaction()
        .map_err(|e| AppError::Database {
            message: format!("Begin transaction failed: {}", e),
        })?;

    tx.execute(
        "DELETE FROM images WHERE generation_id = ?1",
        params![generation_id],
    )
    .map_err(|e| AppError::Database {
        message: format!("Clear existing images failed: {}", e),
    })?;

    let mut saved_images = Vec::with_capacity(images_data.len());
    for (i, data) in images_data.iter().enumerate() {
        let img_id = format!("{}_{}", generation_id, i);
        let saved = fm
            .save_image_at(&img_id, data, output_format, Some(created_at))
            .map_err(|e| AppError::FileSystem { message: e })?;

        tx.execute(
            "INSERT INTO images (id, generation_id, file_path, thumbnail_path, width, height, file_size, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![img_id, generation_id, saved.file_path, saved.thumbnail_path, saved.width, saved.height, saved.file_size, created_at],
        ).map_err(|e| AppError::Database {
            message: format!("Insert image record failed: {}", e),
        })?;

        saved_images.push(GeneratedImage {
            id: img_id,
            generation_id: generation_id.to_string(),
            file_path: saved.file_path,
            thumbnail_path: saved.thumbnail_path,
            width: saved.width,
            height: saved.height,
            file_size: saved.file_size,
        });
    }

    tx.execute(
        "UPDATE generations SET status = 'completed', error_message = NULL WHERE id = ?1",
        params![generation_id],
    )
    .map_err(|e| AppError::Database {
        message: format!("Update generation status failed: {}", e),
    })?;
    tx.execute(
        "DELETE FROM generation_recoveries WHERE generation_id = ?1",
        params![generation_id],
    )
    .map_err(|e| AppError::Database {
        message: format!("Clear generation recovery failed: {}", e),
    })?;
    tx.commit().map_err(|e| AppError::Database {
        message: format!("Commit transaction failed: {}", e),
    })?;

    Ok(saved_images)
}

pub(crate) async fn run_generation_lifecycle(
    app: &tauri::AppHandle,
    db: &Database,
    engine: &crate::api_gateway::GptImageEngine,
    request: GenerationLifecycleRequest,
) -> Result<GenerateResult, AppError> {
    let mut options = image_request_options(
        request.size,
        request.quality,
        request.background,
        request.output_format,
        request.output_compression,
        request.moderation,
        request.input_fidelity,
        request.image_count,
    );
    let generation_id = uuid::Uuid::new_v4().to_string();
    let created_at = current_timestamp();

    let conversation_id = {
        let conn = db.conn.lock().map_err(|e| AppError::Database {
            message: format!("Lock failed: {}", e),
        })?;
        conversations::resolve_conversation_id_for_generation(
            &conn,
            request.conversation_id.as_deref(),
            request.project_id.as_deref(),
            &request.prompt,
        )?
    };
    let model = normalize_image_model(
        request
            .model
            .as_deref()
            .or(db.get_setting(SETTING_IMAGE_MODEL)?.as_deref())
            .unwrap_or(DEFAULT_IMAGE_MODEL),
    )
    .to_string();
    options = sanitize_request_options_for_model(&model, options);

    if request.kind == GenerationLifecycleKind::Generate {
        let _ = db.insert_log(
            "generation",
            "info",
            &format!(
                "Started — size: {}, quality: {}, background: {}, output_format: {}, image_count: {}",
                options.size,
                options.quality,
                options.background,
                options.output_format,
                options.image_count
            ),
            Some(&generation_id),
            Some(
                &serde_json::json!({
                    "model": &model,
                    "size": &options.size, "quality": &options.quality,
                    "background": &options.background, "output_format": &options.output_format,
                    "output_compression": options.output_compression, "moderation": &options.moderation,
                    "stream": options.stream, "partial_images": options.partial_images,
                    "image_count": options.image_count, "conversation_id": conversation_id
                })
                .to_string(),
            ),
            None,
        )?;
    }

    let api_key =
        settings::read_model_api_key(db, &model)?.ok_or_else(|| AppError::ApiKeyNotSet {
            model: model.clone(),
        })?;
    let endpoint_url =
        resolve_image_endpoint_url_for_model(db, &model, request.kind.endpoint_kind())?;
    {
        let conn = db.conn.lock().map_err(|e| AppError::Database {
            message: format!("Lock failed: {}", e),
        })?;
        create_processing_generation(
            &conn,
            &generation_id,
            &request.prompt,
            &model,
            &options,
            &conversation_id,
            &created_at,
            request.kind,
            &request.source_image_paths,
        )?;
    }

    let _ = app.emit(
        "generation:progress",
        serde_json::json!({ "generation_id": generation_id, "status": "processing" }),
    );

    let result = match request.kind {
        GenerationLifecycleKind::Generate => {
            engine
                .generate(&model, &api_key, &endpoint_url, &request.prompt, &options)
                .await
        }
        GenerationLifecycleKind::Edit => {
            engine
                .edit(
                    &model,
                    &api_key,
                    &endpoint_url,
                    &request.prompt,
                    &request.source_image_paths,
                    &options,
                )
                .await
        }
    };

    match result {
        Ok(engine_response) => {
            if engine_response.requested_image_count != options.image_count {
                return Err(AppError::Validation {
                    message: "Provider attempt request count did not match the persisted request"
                        .to_string(),
                });
            }
            let images = engine
                .decode_images_from_response(&engine_response.body_text, &|| false)
                .await
                .map_err(|message| AppError::Validation { message })?;

            let saved_images = save_generation_images(
                app,
                db,
                &generation_id,
                &created_at,
                &options.output_format,
                &images,
            )?;

            let _ = db.insert_log(
                "generation",
                "info",
                &request.kind.completed_log_message(saved_images.len()),
                Some(&generation_id),
                Some(&serde_json::json!({"image_count": saved_images.len()}).to_string()),
                None,
            );

            let _ = app.emit(
                "generation:complete",
                serde_json::json!({ "generation_id": generation_id, "status": "completed" }),
            );

            Ok(GenerateResult {
                generation_id,
                conversation_id,
                images: saved_images,
            })
        }
        Err(e) => {
            let message = e.sanitized_message;
            let _ = db.insert_log(
                "generation",
                "error",
                &request.kind.failed_log_message(&message),
                Some(&generation_id),
                None,
                None,
            );

            let conn = db.conn.lock().map_err(|e| AppError::Database {
                message: format!("Lock failed: {}", e),
            })?;
            set_generation_failed(&conn, &generation_id, &message, true)?;

            let _ = app.emit(
                "generation:failed",
                serde_json::json!({ "generation_id": generation_id, "error": &message }),
            );

            Err(AppError::Validation { message })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api_gateway::{ProviderAttemptBody, RetryAfterHint};
    use crate::generation_jobs::{
        GenerationJobOptions, GenerationJobRequest, GenerationJobRequestKind,
    };
    use crate::models::DEFAULT_IMAGE_COUNT;
    use image::{DynamicImage, ImageBuffer, ImageFormat, Rgb};
    use std::io::Cursor;
    use std::path::Path;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

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
            _source_image_paths: &[String],
            _options: &GptImageRequestOptions,
        ) -> Result<ProviderAttemptBody, EngineCallError> {
            panic!("generate fixture must not call edit")
        }
    }

    struct FakeArtifactStore {
        calls: Arc<AtomicUsize>,
    }

    #[async_trait::async_trait]
    impl ResponseArtifactStore for FakeArtifactStore {
        fn expected_response_path(
            &self,
            context: &GenerationExecutionContext,
        ) -> Result<PathBuf, GenerationExecutionError> {
            Ok(PathBuf::from(format!(
                "/safe/{}.response.json",
                context.job_id
            )))
        }

        async fn persist_verified_response(
            &self,
            context: &GenerationExecutionContext,
            body: ProviderAttemptBody,
        ) -> Result<ProviderAttemptResponse, GenerationExecutionError> {
            assert_eq!(context.job_id, "job-1");
            self.calls.fetch_add(1, Ordering::SeqCst);
            Ok(ProviderAttemptResponse {
                response_size: body.body_text.len() as u64,
                response_sha256: FileResponseArtifactStore::response_hash(&body.body_text),
                response_file: "/safe/job-1.response.json".to_string(),
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

    struct FakeLocalDecoder {
        calls: Arc<AtomicUsize>,
        images: Vec<Vec<u8>>,
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

    #[test]
    fn lifecycle_metadata_counts_source_images_without_storing_paths() {
        let options = image_request_options(
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            Some(DEFAULT_IMAGE_COUNT),
        );
        let metadata = generation_request_metadata_json(
            GenerationLifecycleKind::Edit,
            "conversation-1",
            "gpt-image-2",
            &options,
            &["/Users/example/private.png".to_string()],
        )
        .expect("serialize metadata");

        assert!(metadata.contains("\"request_kind\":\"edit\""));
        assert!(metadata.contains("\"source_image_count\":1"));
        assert!(!metadata.contains("private.png"));
        assert!(!metadata.contains("source_image_paths"));
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
            root.canonicalize()
                .expect("canonical response root")
                .join("generation-jobs")
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
        let body = perform_provider_http_attempt(
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
            },
            &fixture_execution_snapshot(),
            body,
        )
        .await
        .expect("persist provider response");

        assert_eq!(artifact_calls.load(Ordering::SeqCst), 1);
        assert_eq!(response.requested_image_count, 2);
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
