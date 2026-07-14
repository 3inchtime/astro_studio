use crate::commands::conversations;
use crate::error::AppError;
use crate::generation_lifecycle::{GenerationExecutionContext, GenerationExecutionSnapshot};
use crate::generation_worker_lease::{
    assert_worker_transition_authority_in_transaction, WorkerLeaseError, WorkerTransitionAuthority,
};
use crate::models::{
    EnqueueGenerationResult, GenerationJob, GenerationJobEvent, GenerationJobFilter,
    GenerationJobPage, GenerationJobStage, GenerationJobStatus, GptImageRequestOptions,
    DEFAULT_GENERATION_JOB_PAGE_LIMIT, MAX_GENERATION_JOB_PAGE_LIMIT,
};
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use rusqlite::types::Value as SqlValue;
use rusqlite::{
    params, params_from_iter, Connection, OptionalExtension, Row, Transaction, TransactionBehavior,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::path::{Component, Path, PathBuf};

const JOB_COLUMNS: &str = "id, client_request_id, generation_id, parent_job_id, source_kind,
    source_ref_json, status, stage, request_json, provider_kind, provider_profile_id,
    endpoint_snapshot, chain_attempt, auto_attempt, max_auto_attempts, queued_at, started_at,
    finished_at, cancel_requested_at, last_heartbeat_at, error_code, error_message, retryable";

const ALIASED_JOB_COLUMNS: &str = "g.id, g.client_request_id, g.generation_id, g.parent_job_id,
    g.source_kind, g.source_ref_json, g.status, g.stage, g.request_json, g.provider_kind,
    g.provider_profile_id, g.endpoint_snapshot, g.chain_attempt, g.auto_attempt,
    g.max_auto_attempts, g.queued_at, g.started_at, g.finished_at, g.cancel_requested_at,
    g.last_heartbeat_at, g.error_code, g.error_message, g.retryable";

const GENERATION_JOB_WRITE_BUSY_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);
// Keep this repository-side recovery bound aligned with
// `FileResponseArtifactStore::MAX_RESPONSE_BODY_BYTES` without widening the
// current two-file worker-transition slice into lifecycle ownership.
const MAX_WORKER_RESPONSE_BODY_BYTES: i64 = 64 * 1024 * 1024;
const MAX_PERSISTED_RESPONSE_PATH_BYTES: usize = 32_768;

#[derive(Debug, thiserror::Error)]
pub(crate) enum WorkerTransitionError {
    #[error(transparent)]
    Lease(#[from] WorkerLeaseError),
    #[error(transparent)]
    Repository(#[from] AppError),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum WorkerStageTransition {
    BeginProviderRequest { expected_response_file: PathBuf },
    EnterRetryBackoff,
    EnterLocalProcessing,
}

#[derive(Clone)]
pub(crate) struct PreparedGenerationJob {
    pub job_id: String,
    pub client_request_id: String,
    pub generation_id: String,
    pub requested_conversation_id: Option<String>,
    pub requested_project_id: Option<String>,
    pub prompt: String,
    pub model: String,
    pub request_kind: String,
    pub size: String,
    pub quality: String,
    pub background: String,
    pub output_format: String,
    pub output_compression: i32,
    pub moderation: String,
    pub input_fidelity: String,
    pub image_count: i32,
    pub stream: bool,
    pub partial_images: u8,
    pub source_image_paths: Vec<String>,
    pub request_options: GenerationJobOptions,
    pub parent_job_id: Option<String>,
    pub source_kind: String,
    pub source_ref: Value,
    pub provider_kind: String,
    pub provider_profile_id: String,
    pub endpoint_snapshot: String,
    pub status: GenerationJobStatus,
    pub chain_attempt: i32,
    pub auto_attempt: i32,
    pub max_auto_attempts: i32,
    pub queued_at: String,
    pub finished_at: Option<String>,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
    pub retryable: bool,
}

#[derive(Clone)]
pub(crate) struct GenerationJobTerminalUpdate {
    pub job_id: String,
    pub expected_status: GenerationJobStatus,
    pub status: GenerationJobStatus,
    pub finished_at: String,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
    pub retryable: bool,
}

#[derive(Debug)]
pub(crate) struct GenerationJobTransition<T> {
    pub(crate) value: T,
    /// Present only when this call committed a new durable state transition.
    /// Idempotent enqueue/retry acknowledgements intentionally carry no event
    /// so a future command adapter cannot rebroadcast an old transition.
    pub(crate) event: Option<GenerationJobEvent>,
}

impl<T> GenerationJobTransition<T> {
    fn into_value(self) -> T {
        let Self { value, event } = self;
        drop(event);
        value
    }
}

#[derive(Debug)]
struct StoredGenerationJob {
    id: String,
    client_request_id: String,
    generation_id: String,
    parent_job_id: Option<String>,
    source_kind: String,
    source_ref_json: String,
    status: String,
    stage: String,
    request_json: String,
    provider_kind: String,
    provider_profile_id: String,
    endpoint_snapshot: String,
    chain_attempt: i32,
    auto_attempt: i32,
    max_auto_attempts: i32,
    queued_at: String,
    started_at: Option<String>,
    finished_at: Option<String>,
    cancel_requested_at: Option<String>,
    last_heartbeat_at: Option<String>,
    error_code: Option<String>,
    error_message: Option<String>,
    retryable: i64,
}

#[derive(Debug)]
struct RetryGenerationSnapshot {
    prompt: String,
    model: String,
    request_kind: String,
    size: String,
    quality: String,
    background: String,
    output_format: String,
    output_compression: i32,
    moderation: String,
    input_fidelity: String,
    image_count: i32,
    stream: bool,
    partial_images: u8,
    source_image_paths: Vec<String>,
    status: String,
    conversation_id: String,
    project_id: String,
}

#[derive(Debug)]
struct LinkedGenerationSnapshot {
    prompt: String,
    model: String,
    request_kind: String,
    size: String,
    quality: String,
    background: String,
    output_format: String,
    output_compression: i32,
    moderation: String,
    input_fidelity: String,
    image_count: i32,
    source_image_count: i32,
    source_image_paths_json: String,
    request_metadata_json: Option<String>,
    status: String,
    error_message: Option<String>,
    conversation_id: Option<String>,
    created_at: String,
    deleted_at: Option<String>,
}

#[derive(Debug)]
struct LinkedRecoverySnapshot {
    request_kind: String,
    request_state: String,
    output_format: String,
    response_file: Option<String>,
    created_at: String,
    updated_at: String,
}

#[derive(Debug)]
struct LinkedImageProjection {
    id: String,
    file_path: String,
    thumbnail_path: Option<String>,
    width: i32,
    height: i32,
    file_size: i64,
    created_at: String,
}

/// All linked rows needed to validate a page. Keeping validation detached from
/// `Connection` prevents per-job projection queries from creeping back in.
#[derive(Debug, Default)]
struct JobProjectionBatch {
    generations: HashMap<String, LinkedGenerationSnapshot>,
    live_conversation_ids: HashSet<String>,
    images: HashMap<String, Vec<LinkedImageProjection>>,
    recoveries: HashMap<String, LinkedRecoverySnapshot>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum GenerationJobRequestKind {
    Generate,
    Edit,
}

impl GenerationJobRequestKind {
    fn parse(value: &str) -> Result<Self, AppError> {
        match value {
            "generate" => Ok(Self::Generate),
            "edit" => Ok(Self::Edit),
            _ => Err(AppError::GenerationJobInvalidSnapshot),
        }
    }

    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Generate => "generate",
            Self::Edit => "edit",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub(crate) struct GenerationJobOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) size: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) quality: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) background: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) output_format: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) output_compression: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) moderation: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) input_fidelity: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) stream: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) partial_images: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) image_count: Option<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub(crate) struct GenerationJobRequest {
    pub(crate) kind: GenerationJobRequestKind,
    pub(crate) prompt: String,
    pub(crate) model: String,
    pub(crate) source_image_paths: Vec<String>,
    pub(crate) options: GenerationJobOptions,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) requested_conversation_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) requested_project_id: Option<String>,
    pub(crate) conversation_id: String,
    pub(crate) project_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ResolvedConversationIdentity {
    conversation_id: String,
    project_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
struct GenerateEditSourceRef {
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    conversation_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    project_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
struct CanvasSourceRef {
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    round_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    canvas_round_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    document_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    canvas_document_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    revision_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    source_revision_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    parent_round_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    conversation_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    project_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum GenerationJobSourceRef {
    GenerateEdit(GenerateEditSourceRef),
    Canvas(CanvasSourceRef),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
struct CanonicalGenerationMetadata {
    request_kind: GenerationJobRequestKind,
    conversation_id: String,
    project_id: String,
    model: String,
    size: String,
    quality: String,
    background: String,
    output_format: String,
    output_compression: u8,
    moderation: String,
    input_fidelity: String,
    stream: bool,
    partial_images: u8,
    image_count: u8,
    source_image_count: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    actual_image_count: Option<u8>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct JobCursor {
    version: u8,
    queued_at: String,
    rowid: i64,
}

fn database_error(context: &str, error: rusqlite::Error) -> AppError {
    AppError::Database {
        message: format!("{context}: {error}"),
    }
}

fn persisted_row_error(context: &str, error: rusqlite::Error) -> AppError {
    if matches!(
        error,
        rusqlite::Error::FromSqlConversionFailure(..)
            | rusqlite::Error::IntegralValueOutOfRange(..)
            | rusqlite::Error::InvalidColumnType(..)
    ) {
        AppError::GenerationJobCorruptPersistedData
    } else {
        database_error(context, error)
    }
}

fn status_as_str(status: &GenerationJobStatus) -> &'static str {
    match status {
        GenerationJobStatus::Queued => "queued",
        GenerationJobStatus::Running => "running",
        GenerationJobStatus::Completed => "completed",
        GenerationJobStatus::Failed => "failed",
        GenerationJobStatus::Cancelled => "cancelled",
        GenerationJobStatus::Interrupted => "interrupted",
    }
}

fn parse_status(value: &str) -> Result<GenerationJobStatus, AppError> {
    match value {
        "queued" => Ok(GenerationJobStatus::Queued),
        "running" => Ok(GenerationJobStatus::Running),
        "completed" => Ok(GenerationJobStatus::Completed),
        "failed" => Ok(GenerationJobStatus::Failed),
        "cancelled" => Ok(GenerationJobStatus::Cancelled),
        "interrupted" => Ok(GenerationJobStatus::Interrupted),
        _ => Err(AppError::GenerationJobCorruptPersistedData),
    }
}

fn stage_as_str(stage: GenerationJobStage) -> &'static str {
    match stage {
        GenerationJobStage::MigrationUnknown => "migration_unknown",
        GenerationJobStage::Queued => "queued",
        GenerationJobStage::Preparing => "preparing",
        GenerationJobStage::ProviderRequest => "provider_request",
        GenerationJobStage::RetryBackoff => "retry_backoff",
        GenerationJobStage::ResponseReady => "response_ready",
        GenerationJobStage::LocalProcessing => "local_processing",
        GenerationJobStage::StartupReconciliation => "startup_reconciliation",
        GenerationJobStage::LegacyResponseRecovery => "legacy_response_recovery",
        GenerationJobStage::Terminal => "terminal",
    }
}

fn parse_stage(value: &str) -> Result<GenerationJobStage, AppError> {
    match value {
        "migration_unknown" => Ok(GenerationJobStage::MigrationUnknown),
        "queued" => Ok(GenerationJobStage::Queued),
        "preparing" => Ok(GenerationJobStage::Preparing),
        "provider_request" => Ok(GenerationJobStage::ProviderRequest),
        "retry_backoff" => Ok(GenerationJobStage::RetryBackoff),
        "response_ready" => Ok(GenerationJobStage::ResponseReady),
        "local_processing" => Ok(GenerationJobStage::LocalProcessing),
        "startup_reconciliation" => Ok(GenerationJobStage::StartupReconciliation),
        "legacy_response_recovery" => Ok(GenerationJobStage::LegacyResponseRecovery),
        "terminal" => Ok(GenerationJobStage::Terminal),
        _ => Err(AppError::GenerationJobCorruptPersistedData),
    }
}

fn status_stage_valid(status: &GenerationJobStatus, stage: GenerationJobStage) -> bool {
    match status {
        GenerationJobStatus::Queued => stage == GenerationJobStage::Queued,
        GenerationJobStatus::Running => matches!(
            stage,
            GenerationJobStage::Preparing
                | GenerationJobStage::ProviderRequest
                | GenerationJobStage::RetryBackoff
                | GenerationJobStage::ResponseReady
                | GenerationJobStage::LocalProcessing
                | GenerationJobStage::StartupReconciliation
        ),
        GenerationJobStatus::Completed
        | GenerationJobStatus::Failed
        | GenerationJobStatus::Cancelled
        | GenerationJobStatus::Interrupted => stage == GenerationJobStage::Terminal,
    }
}

fn is_terminal(status: &GenerationJobStatus) -> bool {
    matches!(
        status,
        GenerationJobStatus::Completed
            | GenerationJobStatus::Failed
            | GenerationJobStatus::Cancelled
            | GenerationJobStatus::Interrupted
    )
}

pub(crate) fn can_transition(from: GenerationJobStatus, to: GenerationJobStatus) -> bool {
    matches!(
        (from, to),
        (GenerationJobStatus::Queued, GenerationJobStatus::Running)
            | (GenerationJobStatus::Queued, GenerationJobStatus::Cancelled)
            | (GenerationJobStatus::Running, GenerationJobStatus::Completed)
            | (GenerationJobStatus::Running, GenerationJobStatus::Failed)
            | (GenerationJobStatus::Running, GenerationJobStatus::Cancelled)
            | (
                GenerationJobStatus::Running,
                GenerationJobStatus::Interrupted
            )
    )
}

fn stored_job_from_row(row: &Row<'_>) -> rusqlite::Result<StoredGenerationJob> {
    Ok(StoredGenerationJob {
        id: row.get(0)?,
        client_request_id: row.get(1)?,
        generation_id: row.get(2)?,
        parent_job_id: row.get(3)?,
        source_kind: row.get(4)?,
        source_ref_json: row.get(5)?,
        status: row.get(6)?,
        stage: row.get(7)?,
        request_json: row.get(8)?,
        provider_kind: row.get(9)?,
        provider_profile_id: row.get(10)?,
        endpoint_snapshot: row.get(11)?,
        chain_attempt: row.get(12)?,
        auto_attempt: row.get(13)?,
        max_auto_attempts: row.get(14)?,
        queued_at: row.get(15)?,
        started_at: row.get(16)?,
        finished_at: row.get(17)?,
        cancel_requested_at: row.get(18)?,
        last_heartbeat_at: row.get(19)?,
        error_code: row.get(20)?,
        error_message: row.get(21)?,
        retryable: row.get(22)?,
    })
}

fn decode_stored_job(stored: StoredGenerationJob) -> Result<GenerationJob, AppError> {
    if !matches!(stored.retryable, 0 | 1) {
        return Err(AppError::GenerationJobCorruptPersistedData);
    }
    let raw_source_ref: Value = serde_json::from_str(&stored.source_ref_json)
        .map_err(|_| AppError::GenerationJobCorruptPersistedData)?;
    let canonical_request: GenerationJobRequest = serde_json::from_str(&stored.request_json)
        .map_err(|_| AppError::GenerationJobCorruptPersistedData)?;
    let request = serde_json::to_value(&canonical_request)
        .map_err(|_| AppError::GenerationJobCorruptPersistedData)?;
    let request_identity = ResolvedConversationIdentity {
        conversation_id: canonical_request.conversation_id.clone(),
        project_id: canonical_request.project_id.clone(),
    };
    let source_ref = canonical_source_ref(
        &stored.source_kind,
        &raw_source_ref,
        Some(&request_identity),
    )
    .map_err(|_| AppError::GenerationJobCorruptPersistedData)?;
    if source_ref != raw_source_ref {
        return Err(AppError::GenerationJobCorruptPersistedData);
    }
    let request_kind_matches_source = matches!(
        (stored.source_kind.as_str(), canonical_request.kind),
        ("generate", GenerationJobRequestKind::Generate)
            | ("edit", GenerationJobRequestKind::Edit)
            | (
                "canvas",
                GenerationJobRequestKind::Generate | GenerationJobRequestKind::Edit
            )
    );
    let request_fields_valid = request_kind_matches_source
        && nonempty(&canonical_request.prompt)
        && nonempty(&canonical_request.model)
        && nonempty(&canonical_request.conversation_id)
        && nonempty(&canonical_request.project_id)
        && [
            canonical_request.prompt.as_str(),
            canonical_request.model.as_str(),
            canonical_request.conversation_id.as_str(),
        ]
        .into_iter()
        .all(public_string_is_safe)
        && public_string_is_safe(&canonical_request.project_id)
        && canonical_request
            .source_image_paths
            .iter()
            .all(|path| nonempty(path) && public_string_is_safe(path))
        && [
            canonical_request.requested_conversation_id.as_deref(),
            canonical_request.requested_project_id.as_deref(),
        ]
        .into_iter()
        .flatten()
        .all(|value| nonempty(value) && public_string_is_safe(value))
        && request_options_shape_valid(&canonical_request.options);
    let status = parse_status(&stored.status)?;
    let stage = parse_stage(&stored.stage)?;
    let timestamps_valid = canonical_timestamp(&stored.queued_at)
        && [
            stored.started_at.as_deref(),
            stored.finished_at.as_deref(),
            stored.cancel_requested_at.as_deref(),
            stored.last_heartbeat_at.as_deref(),
        ]
        .into_iter()
        .flatten()
        .all(canonical_timestamp);
    let error_fields_valid = match stored.error_code.as_deref() {
        Some(code) if safe_error_code(code) => {
            stored.error_message.as_deref() == Some(terminal_message_for_code(code))
        }
        None => stored.error_message.is_none(),
        _ => false,
    };
    let executable_provider = executable_provider_snapshot_is_valid(
        &stored.provider_kind,
        &stored.provider_profile_id,
        &stored.endpoint_snapshot,
    );
    let stored_public_fields_valid = [
        stored.id.as_str(),
        stored.client_request_id.as_str(),
        stored.generation_id.as_str(),
        stored.source_kind.as_str(),
        stored.provider_kind.as_str(),
        stored.provider_profile_id.as_str(),
    ]
    .into_iter()
    .all(public_string_is_safe)
        && stored
            .parent_job_id
            .as_deref()
            .is_none_or(public_string_is_safe)
        && stored
            .error_code
            .as_deref()
            .is_none_or(public_string_is_safe)
        && stored
            .error_message
            .as_deref()
            .is_none_or(public_string_is_safe);
    let initial_configuration_failure = status == GenerationJobStatus::Failed
        && stored.started_at.is_none()
        && stored.last_heartbeat_at.is_none()
        && stored.cancel_requested_at.is_none()
        && stored.finished_at.is_some()
        && matches!(
            stored.error_code.as_deref(),
            Some("provider_profile_missing" | "provider_configuration_invalid")
        )
        && stored.retryable == 0
        && ((stored.provider_kind == "unresolved"
            && stored.provider_profile_id == "unresolved"
            && stored.endpoint_snapshot.is_empty())
            || executable_provider);
    let state_fields_valid = match status {
        GenerationJobStatus::Queued => {
            stored.started_at.is_none()
                && stored.last_heartbeat_at.is_none()
                && stored.finished_at.is_none()
                && stored.cancel_requested_at.is_none()
                && stored.error_code.is_none()
                && stored.error_message.is_none()
                && stored.retryable == 0
                && stored.auto_attempt == 0
                && executable_provider
        }
        GenerationJobStatus::Running => {
            stored.started_at.is_some()
                && stored.last_heartbeat_at.is_some()
                && stored.finished_at.is_none()
                && stored.error_code.is_none()
                && stored.error_message.is_none()
                && stored.retryable == 0
                && executable_provider
        }
        GenerationJobStatus::Completed => {
            stored.started_at.is_some()
                && stored.last_heartbeat_at.is_some()
                && stored.finished_at.is_some()
                && stored.cancel_requested_at.is_none()
                && stored.error_code.is_none()
                && stored.error_message.is_none()
                && stored.retryable == 0
                && executable_provider
        }
        GenerationJobStatus::Failed => {
            initial_configuration_failure
                || (stored.started_at.is_some()
                    && stored.last_heartbeat_at.is_some()
                    && stored.finished_at.is_some()
                    && stored.cancel_requested_at.is_none()
                    && stored.error_code.is_some()
                    && executable_provider)
        }
        GenerationJobStatus::Interrupted => {
            stored.started_at.is_some()
                && stored.last_heartbeat_at.is_some()
                && stored.finished_at.is_some()
                && stored.cancel_requested_at.is_none()
                && stored.error_code.is_some()
                && executable_provider
        }
        GenerationJobStatus::Cancelled => {
            stored.finished_at.is_some()
                && stored.cancel_requested_at.is_some()
                && stored.error_code.as_deref() == Some("cancelled_by_user")
                && stored.retryable == 0
                && executable_provider
                && ((stored.started_at.is_none() && stored.last_heartbeat_at.is_none())
                    || (stored.started_at.is_some() && stored.last_heartbeat_at.is_some()))
        }
    };
    if !timestamps_valid
        || !request_fields_valid
        || !stored_public_fields_valid
        || !error_fields_valid
        || !state_fields_valid
        || stored.chain_attempt < 1
        || stored.auto_attempt < 0
        || stored.max_auto_attempts < 0
        || stored.auto_attempt > stored.max_auto_attempts
        || stage == GenerationJobStage::MigrationUnknown
        || !status_stage_valid(&status, stage)
    {
        return Err(AppError::GenerationJobCorruptPersistedData);
    }
    Ok(GenerationJob {
        id: stored.id,
        client_request_id: stored.client_request_id,
        generation_id: stored.generation_id,
        parent_job_id: stored.parent_job_id,
        source_kind: stored.source_kind,
        source_ref,
        status,
        stage,
        request,
        provider_kind: stored.provider_kind,
        provider_profile_id: stored.provider_profile_id,
        endpoint_snapshot: stored.endpoint_snapshot,
        chain_attempt: stored.chain_attempt,
        auto_attempt: stored.auto_attempt,
        max_auto_attempts: stored.max_auto_attempts,
        queued_at: stored.queued_at,
        started_at: stored.started_at,
        finished_at: stored.finished_at,
        cancel_requested_at: stored.cancel_requested_at,
        last_heartbeat_at: stored.last_heartbeat_at,
        error_code: stored.error_code,
        error_message: stored.error_message,
        retryable: stored.retryable == 1,
    })
}

fn query_job_in_transaction(
    tx: &Transaction<'_>,
    predicate: &str,
    value: &str,
) -> Result<Option<GenerationJob>, AppError> {
    let sql = format!("SELECT {JOB_COLUMNS} FROM generation_jobs WHERE {predicate} = ?1");
    let stored = tx
        .query_row(&sql, params![value], stored_job_from_row)
        .optional()
        .map_err(|error| persisted_row_error("Read generation job failed", error))?;
    let job = stored.map(decode_stored_job).transpose()?;
    if let Some(job) = job.as_ref() {
        let generation_ids = vec![job.generation_id.clone()];
        let mut no_query_observer = || {};
        let projections = load_job_projection_batch_with_query_observer(
            tx,
            &generation_ids,
            &mut no_query_observer,
        )?;
        validate_job_projection(job, &projections)?;
    }
    Ok(job)
}

fn with_generation_job_read_transaction<T>(
    conn: &Connection,
    operation: &str,
    read: impl FnOnce(&Transaction<'_>) -> Result<T, AppError>,
) -> Result<T, AppError> {
    let tx = conn.unchecked_transaction().map_err(|error| {
        database_error(
            &format!("Begin {operation} generation job read transaction failed"),
            error,
        )
    })?;
    let result = read(&tx)?;
    tx.commit().map_err(|error| {
        database_error(
            &format!("Commit {operation} generation job read transaction failed"),
            error,
        )
    })?;
    Ok(result)
}

pub(crate) fn get_job(conn: &Connection, id: &str) -> Result<GenerationJob, AppError> {
    with_generation_job_read_transaction(conn, "get", |tx| get_job_in_transaction(tx, id))
}

pub(crate) fn get_job_in_transaction(
    tx: &Transaction<'_>,
    id: &str,
) -> Result<GenerationJob, AppError> {
    query_job_in_transaction(tx, "id", id)?.ok_or(AppError::GenerationJobNotFound)
}

pub(crate) fn get_job_event_in_transaction(
    tx: &Transaction<'_>,
    id: &str,
) -> Result<GenerationJobEvent, AppError> {
    let job = get_job_in_transaction(tx, id)?;
    let (conversation_id, queue_position) = tx
        .query_row(
            "SELECT generation.conversation_id,
                    CASE
                        WHEN current.status = 'queued' AND current.stage = 'queued' THEN (
                            SELECT COUNT(*)
                              FROM generation_jobs queued
                             WHERE queued.status = 'queued'
                               AND queued.stage = 'queued'
                               AND (
                                   queued.queued_at < current.queued_at
                                   OR (
                                       queued.queued_at = current.queued_at
                                       AND queued.rowid <= current.rowid
                                   )
                               )
                        )
                        ELSE NULL
                    END
               FROM generation_jobs current
               JOIN generations generation ON generation.id = current.generation_id
              WHERE current.id = ?1",
            params![id],
            |row| {
                Ok((
                    row.get::<_, Option<String>>(0)?,
                    row.get::<_, Option<i64>>(1)?,
                ))
            },
        )
        .optional()
        .map_err(|error| persisted_row_error("Read generation job event failed", error))?
        .ok_or(AppError::GenerationJobNotFound)?;
    let conversation_id = conversation_id
        .filter(|value| source_reference_id_valid(value))
        .ok_or(AppError::GenerationJobCorruptPersistedData)?;

    Ok(GenerationJobEvent {
        job_id: job.id,
        generation_id: job.generation_id,
        conversation_id,
        source_kind: job.source_kind,
        source_ref: job.source_ref,
        status: job.status,
        stage: job.stage,
        queue_position,
        chain_attempt: job.chain_attempt,
        auto_attempt: job.auto_attempt,
        max_auto_attempts: job.max_auto_attempts,
        cancel_requested_at: job.cancel_requested_at,
        error_code: job.error_code,
        error_message: job.error_message,
        retryable: job.retryable,
        queued_at: job.queued_at,
        started_at: job.started_at,
        finished_at: job.finished_at,
    })
}

pub(crate) fn get_job_event(conn: &Connection, id: &str) -> Result<GenerationJobEvent, AppError> {
    with_generation_job_read_transaction(conn, "event", |tx| get_job_event_in_transaction(tx, id))
}

pub(crate) fn find_job_by_client_request_id(
    conn: &Connection,
    client_request_id: &str,
) -> Result<Option<GenerationJob>, AppError> {
    with_generation_job_read_transaction(conn, "find", |tx| {
        find_job_by_client_request_id_in_transaction(tx, client_request_id)
    })
}

pub(crate) fn find_job_by_client_request_id_in_transaction(
    tx: &Transaction<'_>,
    client_request_id: &str,
) -> Result<Option<GenerationJob>, AppError> {
    query_job_in_transaction(tx, "client_request_id", client_request_id)
}

fn resolved_conversation_identity(
    conn: &Connection,
    conversation_id: &str,
) -> Result<ResolvedConversationIdentity, AppError> {
    let identity = conn
        .query_row(
            "SELECT c.id, p.id, typeof(c.id), typeof(p.id)
             FROM conversations c
             JOIN projects p ON p.id = c.project_id AND p.deleted_at IS NULL
             WHERE c.id = ?1 AND c.deleted_at IS NULL",
            params![conversation_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                ))
            },
        )
        .optional()
        .map_err(|error| persisted_row_error("Read resolved conversation failed", error))?
        .ok_or(AppError::GenerationJobCorruptPersistedData)?;
    if identity.2 != "text"
        || identity.3 != "text"
        || !source_reference_id_valid(&identity.0)
        || !source_reference_id_valid(&identity.1)
    {
        return Err(AppError::GenerationJobCorruptPersistedData);
    }
    Ok(ResolvedConversationIdentity {
        conversation_id: identity.0,
        project_id: identity.1,
    })
}

fn live_conversation_id(conn: &Connection, conversation_id: &str) -> Result<String, AppError> {
    let conversation = conn
        .query_row(
            "SELECT id, typeof(id) FROM conversations
             WHERE id = ?1 AND deleted_at IS NULL",
            params![conversation_id],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()
        .map_err(|error| persisted_row_error("Read live conversation failed", error))?
        .ok_or(AppError::GenerationJobCorruptPersistedData)?;
    if conversation.1 != "text" || !source_reference_id_valid(&conversation.0) {
        return Err(AppError::GenerationJobCorruptPersistedData);
    }
    Ok(conversation.0)
}

fn load_linked_generation(
    conn: &Connection,
    generation_id: &str,
) -> Result<LinkedGenerationSnapshot, AppError> {
    conn.query_row(
        "SELECT prompt, engine, request_kind, size, quality, background, output_format,
                output_compression, moderation, input_fidelity, image_count,
                source_image_count, source_image_paths, request_metadata, status,
                error_message, conversation_id, created_at, deleted_at
         FROM generations WHERE id = ?1",
        params![generation_id],
        |row| linked_generation_from_row_offset(row, 0),
    )
    .optional()
    .map_err(|error| persisted_row_error("Read linked generation failed", error))?
    .ok_or(AppError::GenerationJobCorruptPersistedData)
}

fn linked_generation_from_row_offset(
    row: &Row<'_>,
    offset: usize,
) -> rusqlite::Result<LinkedGenerationSnapshot> {
    Ok(LinkedGenerationSnapshot {
        prompt: row.get(offset)?,
        model: row.get(offset + 1)?,
        request_kind: row.get(offset + 2)?,
        size: row.get(offset + 3)?,
        quality: row.get(offset + 4)?,
        background: row.get(offset + 5)?,
        output_format: row.get(offset + 6)?,
        output_compression: row.get(offset + 7)?,
        moderation: row.get(offset + 8)?,
        input_fidelity: row.get(offset + 9)?,
        image_count: row.get(offset + 10)?,
        source_image_count: row.get(offset + 11)?,
        source_image_paths_json: row.get(offset + 12)?,
        request_metadata_json: row.get(offset + 13)?,
        status: row.get(offset + 14)?,
        error_message: row.get(offset + 15)?,
        conversation_id: row.get(offset + 16)?,
        created_at: row.get(offset + 17)?,
        deleted_at: row.get(offset + 18)?,
    })
}

fn linked_recovery_from_row_offset(
    row: &Row<'_>,
    offset: usize,
) -> rusqlite::Result<LinkedRecoverySnapshot> {
    Ok(LinkedRecoverySnapshot {
        request_kind: row.get(offset)?,
        request_state: row.get(offset + 1)?,
        output_format: row.get(offset + 2)?,
        response_file: row.get(offset + 3)?,
        created_at: row.get(offset + 4)?,
        updated_at: row.get(offset + 5)?,
    })
}

fn recovery_matches_job(
    recovery: &LinkedRecoverySnapshot,
    job: &GenerationJob,
    request: &GenerationJobRequest,
    generation: &LinkedGenerationSnapshot,
) -> bool {
    let state_and_file_match = match recovery.request_state.as_str() {
        "requesting" => recovery.response_file.is_none(),
        "response_ready" => recovery
            .response_file
            .as_deref()
            .is_some_and(|path| nonempty(path) && public_string_is_safe(path)),
        _ => false,
    };
    recovery.request_kind == request.kind.as_str()
        && recovery.output_format == generation.output_format
        && recovery.created_at == job.queued_at
        && canonical_timestamp(&recovery.created_at)
        && canonical_timestamp(&recovery.updated_at)
        && recovery.updated_at >= recovery.created_at
        && [
            recovery.request_kind.as_str(),
            recovery.request_state.as_str(),
            recovery.output_format.as_str(),
            recovery.created_at.as_str(),
            recovery.updated_at.as_str(),
        ]
        .into_iter()
        .all(public_string_is_safe)
        && state_and_file_match
}

fn linked_image_from_row_offset(
    row: &Row<'_>,
    offset: usize,
) -> rusqlite::Result<LinkedImageProjection> {
    Ok(LinkedImageProjection {
        id: row.get(offset)?,
        file_path: row.get(offset + 1)?,
        thumbnail_path: row.get(offset + 2)?,
        width: row.get(offset + 3)?,
        height: row.get(offset + 4)?,
        file_size: row.get(offset + 5)?,
        created_at: row.get(offset + 6)?,
    })
}

fn batch_predicate(column: &str, value_count: usize) -> String {
    if value_count == 0 {
        "0".to_string()
    } else {
        format!(
            "{column} IN ({})",
            std::iter::repeat_n("?", value_count)
                .collect::<Vec<_>>()
                .join(", ")
        )
    }
}

fn unique_strings(values: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut seen = HashSet::new();
    values
        .into_iter()
        .filter(|value| seen.insert(value.clone()))
        .collect()
}

fn load_linked_generations_batch(
    tx: &Transaction<'_>,
    generation_ids: &[String],
    observe_query: &mut dyn FnMut(),
) -> Result<HashMap<String, LinkedGenerationSnapshot>, AppError> {
    observe_query();
    let sql = format!(
        "SELECT id, prompt, engine, request_kind, size, quality, background, output_format,
                output_compression, moderation, input_fidelity, image_count,
                source_image_count, source_image_paths, request_metadata, status,
                error_message, conversation_id, created_at, deleted_at
         FROM generations WHERE {}",
        batch_predicate("id", generation_ids.len())
    );
    let mut statement = tx
        .prepare(&sql)
        .map_err(|error| database_error("Prepare linked generation batch failed", error))?;
    let rows = statement
        .query_map(params_from_iter(generation_ids.iter()), |row| {
            Ok((
                row.get::<_, String>(0)?,
                linked_generation_from_row_offset(row, 1)?,
            ))
        })
        .map_err(|error| database_error("Query linked generation batch failed", error))?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|error| persisted_row_error("Read linked generation batch failed", error))?;
    let mut generations = HashMap::with_capacity(rows.len());
    for (generation_id, generation) in rows {
        if generations.insert(generation_id, generation).is_some() {
            return Err(AppError::GenerationJobCorruptPersistedData);
        }
    }
    Ok(generations)
}

fn load_live_conversation_ids_batch(
    tx: &Transaction<'_>,
    conversation_ids: &[String],
    observe_query: &mut dyn FnMut(),
) -> Result<HashSet<String>, AppError> {
    observe_query();
    let sql = format!(
        "SELECT id, typeof(id) FROM conversations
         WHERE deleted_at IS NULL AND {}",
        batch_predicate("id", conversation_ids.len())
    );
    let mut statement = tx
        .prepare(&sql)
        .map_err(|error| database_error("Prepare live conversation batch failed", error))?;
    let rows = statement
        .query_map(params_from_iter(conversation_ids.iter()), |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|error| database_error("Query live conversation batch failed", error))?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|error| persisted_row_error("Read live conversation batch failed", error))?;
    let mut live_conversation_ids = HashSet::with_capacity(rows.len());
    for (conversation_id, sql_type) in rows {
        if sql_type != "text" || !live_conversation_ids.insert(conversation_id) {
            return Err(AppError::GenerationJobCorruptPersistedData);
        }
    }
    Ok(live_conversation_ids)
}

fn load_linked_images_batch(
    tx: &Transaction<'_>,
    generation_ids: &[String],
    observe_query: &mut dyn FnMut(),
) -> Result<HashMap<String, Vec<LinkedImageProjection>>, AppError> {
    observe_query();
    let sql = format!(
        "SELECT generation_id, id, file_path, thumbnail_path, width, height, file_size, created_at
         FROM images WHERE {} ORDER BY generation_id ASC, id ASC",
        batch_predicate("generation_id", generation_ids.len())
    );
    let mut statement = tx
        .prepare(&sql)
        .map_err(|error| database_error("Prepare linked image batch failed", error))?;
    let rows = statement
        .query_map(params_from_iter(generation_ids.iter()), |row| {
            Ok((
                row.get::<_, String>(0)?,
                linked_image_from_row_offset(row, 1)?,
            ))
        })
        .map_err(|error| database_error("Query linked image batch failed", error))?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|error| persisted_row_error("Read linked image batch failed", error))?;
    let mut images = HashMap::<String, Vec<LinkedImageProjection>>::new();
    let mut image_ids = HashSet::with_capacity(rows.len());
    for (generation_id, image) in rows {
        if !image_ids.insert(image.id.clone()) {
            return Err(AppError::GenerationJobCorruptPersistedData);
        }
        images.entry(generation_id).or_default().push(image);
    }
    Ok(images)
}

fn load_linked_recoveries_batch(
    tx: &Transaction<'_>,
    generation_ids: &[String],
    observe_query: &mut dyn FnMut(),
) -> Result<HashMap<String, LinkedRecoverySnapshot>, AppError> {
    observe_query();
    let sql = format!(
        "SELECT generation_id, request_kind, request_state, output_format, response_file,
                created_at, updated_at
         FROM generation_recoveries WHERE {}",
        batch_predicate("generation_id", generation_ids.len())
    );
    let mut statement = tx
        .prepare(&sql)
        .map_err(|error| database_error("Prepare linked recovery batch failed", error))?;
    let rows = statement
        .query_map(params_from_iter(generation_ids.iter()), |row| {
            Ok((
                row.get::<_, String>(0)?,
                linked_recovery_from_row_offset(row, 1)?,
            ))
        })
        .map_err(|error| database_error("Query linked recovery batch failed", error))?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|error| persisted_row_error("Read linked recovery batch failed", error))?;
    let mut recoveries = HashMap::with_capacity(rows.len());
    for (generation_id, recovery) in rows {
        if recoveries.insert(generation_id, recovery).is_some() {
            return Err(AppError::GenerationJobCorruptPersistedData);
        }
    }
    Ok(recoveries)
}

fn load_job_projection_batch_with_query_observer(
    tx: &Transaction<'_>,
    generation_ids: &[String],
    observe_query: &mut dyn FnMut(),
) -> Result<JobProjectionBatch, AppError> {
    // These four calls intentionally execute even for an empty page so list
    // reads retain a fixed main-plus-four statement plan.
    let generations = load_linked_generations_batch(tx, generation_ids, observe_query)?;
    let conversation_ids = unique_strings(
        generations
            .values()
            .filter_map(|generation| generation.conversation_id.clone()),
    );
    let live_conversation_ids =
        load_live_conversation_ids_batch(tx, &conversation_ids, observe_query)?;
    let images = load_linked_images_batch(tx, generation_ids, observe_query)?;
    let recoveries = load_linked_recoveries_batch(tx, generation_ids, observe_query)?;
    Ok(JobProjectionBatch {
        generations,
        live_conversation_ids,
        images,
        recoveries,
    })
}

fn image_projection_path_valid(path: &str) -> bool {
    nonempty(path)
        && path.len() <= 32_768
        && !path.chars().any(char::is_control)
        && public_string_is_safe(path)
}

fn validate_job_projection(
    job: &GenerationJob,
    projections: &JobProjectionBatch,
) -> Result<(), AppError> {
    let request: GenerationJobRequest = serde_json::from_value(job.request.clone())
        .map_err(|_| AppError::GenerationJobCorruptPersistedData)?;
    let generation = projections
        .generations
        .get(&job.generation_id)
        .ok_or(AppError::GenerationJobCorruptPersistedData)?;
    let conversation_id = generation
        .conversation_id
        .as_deref()
        .filter(|value| nonempty(value) && public_string_is_safe(value))
        .ok_or(AppError::GenerationJobCorruptPersistedData)?;
    let linked_conversation_id = projections
        .live_conversation_ids
        .get(conversation_id)
        .ok_or(AppError::GenerationJobCorruptPersistedData)?;
    let source_image_paths: Vec<String> = serde_json::from_str(&generation.source_image_paths_json)
        .map_err(|_| AppError::GenerationJobCorruptPersistedData)?;
    let metadata: CanonicalGenerationMetadata = generation
        .request_metadata_json
        .as_deref()
        .ok_or(AppError::GenerationJobCorruptPersistedData)
        .and_then(|value| {
            serde_json::from_str(value).map_err(|_| AppError::GenerationJobCorruptPersistedData)
        })?;
    let images = projections
        .images
        .get(&job.generation_id)
        .map(Vec::as_slice)
        .unwrap_or_default();
    let public_generation_fields = [
        generation.prompt.as_str(),
        generation.model.as_str(),
        generation.request_kind.as_str(),
        generation.size.as_str(),
        generation.quality.as_str(),
        generation.background.as_str(),
        generation.output_format.as_str(),
        generation.moderation.as_str(),
        generation.input_fidelity.as_str(),
        generation.status.as_str(),
        generation.created_at.as_str(),
    ]
    .into_iter()
    .all(|value| nonempty(value) && public_string_is_safe(value))
        && generation
            .error_message
            .as_deref()
            .is_none_or(public_string_is_safe)
        && source_image_paths
            .iter()
            .all(|path| nonempty(path) && public_string_is_safe(path));
    let metadata_matches = metadata.request_kind == request.kind
        && metadata.conversation_id == linked_conversation_id.as_str()
        && metadata.project_id == request.project_id
        && metadata.model == generation.model
        && metadata.size == generation.size
        && metadata.quality == generation.quality
        && metadata.background == generation.background
        && metadata.output_format == generation.output_format
        && i32::from(metadata.output_compression) == generation.output_compression
        && metadata.moderation == generation.moderation
        && metadata.input_fidelity == generation.input_fidelity
        && i32::from(metadata.image_count) == generation.image_count
        && metadata.source_image_count == source_image_paths.len()
        && metadata.output_compression <= 100
        && metadata.partial_images <= 3
        && (1..=4).contains(&metadata.image_count)
        && request
            .options
            .stream
            .is_none_or(|value| metadata.stream == value)
        && request
            .options
            .partial_images
            .is_none_or(|value| metadata.partial_images == value);
    let request_matches = request.conversation_id == linked_conversation_id.as_str()
        && request.prompt == generation.prompt
        && request.model == generation.model
        && request.kind.as_str() == generation.request_kind
        && request.source_image_paths == source_image_paths
        && request
            .options
            .size
            .as_deref()
            .is_none_or(|value| value == generation.size)
        && request
            .options
            .quality
            .as_deref()
            .is_none_or(|value| value == generation.quality)
        && request
            .options
            .background
            .as_deref()
            .is_none_or(|value| value == generation.background)
        && request
            .options
            .output_format
            .as_deref()
            .is_none_or(|value| value == generation.output_format)
        && request
            .options
            .output_compression
            .is_none_or(|value| i32::from(value) == generation.output_compression)
        && request
            .options
            .moderation
            .as_deref()
            .is_none_or(|value| value == generation.moderation)
        && request
            .options
            .input_fidelity
            .as_deref()
            .is_none_or(|value| value == generation.input_fidelity)
        && request
            .options
            .image_count
            .is_none_or(|value| i32::from(value) == generation.image_count);
    let generation_matches = generation.source_image_count
        == i32::try_from(source_image_paths.len())
            .map_err(|_| AppError::GenerationJobCorruptPersistedData)?
        && generation.status == status_as_str(&job.status)
        && generation.error_message == job.error_message
        && generation.created_at == job.queued_at
        && generation.deleted_at.is_none();
    let image_rows_valid = images.iter().all(|image| {
        source_reference_id_valid(&image.id)
            && image_projection_path_valid(&image.file_path)
            && image
                .thumbnail_path
                .as_deref()
                .is_some_and(image_projection_path_valid)
            && image.width > 0
            && image.height > 0
            && image.file_size > 0
            && image.created_at == generation.created_at
            && canonical_timestamp(&image.created_at)
    });
    let actual_image_count = u8::try_from(images.len()).ok();
    let image_projection_matches = match job.status {
        GenerationJobStatus::Completed => metadata.actual_image_count.is_some_and(|actual| {
            actual_image_count == Some(actual)
                && (1..=metadata.image_count).contains(&actual)
                && image_rows_valid
        }),
        GenerationJobStatus::Queued
        | GenerationJobStatus::Running
        | GenerationJobStatus::Failed
        | GenerationJobStatus::Cancelled
        | GenerationJobStatus::Interrupted => {
            metadata.actual_image_count.is_none() && images.is_empty()
        }
    };
    if !public_generation_fields
        || !metadata_matches
        || !request_matches
        || !generation_matches
        || !image_projection_matches
    {
        return Err(AppError::GenerationJobCorruptPersistedData);
    }

    let recovery = projections.recoveries.get(&job.generation_id);
    let recovery_valid = match job.status {
        GenerationJobStatus::Queued => recovery.is_some_and(|recovery| {
            recovery.request_state == "requesting"
                && recovery.updated_at == job.queued_at
                && recovery_matches_job(recovery, job, &request, generation)
        }),
        GenerationJobStatus::Running => recovery
            .is_some_and(|recovery| recovery_matches_job(recovery, job, &request, generation)),
        GenerationJobStatus::Failed if job.started_at.is_none() => recovery.is_none(),
        GenerationJobStatus::Cancelled if job.started_at.is_none() => recovery.is_none(),
        GenerationJobStatus::Completed => recovery.is_none(),
        GenerationJobStatus::Failed
        | GenerationJobStatus::Cancelled
        | GenerationJobStatus::Interrupted => recovery
            .is_none_or(|recovery| recovery_matches_job(recovery, job, &request, generation)),
    };
    if recovery_valid {
        Ok(())
    } else {
        Err(AppError::GenerationJobCorruptPersistedData)
    }
}

fn enqueue_result_for_validated_job(
    job: &GenerationJob,
) -> Result<EnqueueGenerationResult, AppError> {
    let request: GenerationJobRequest = serde_json::from_value(job.request.clone())
        .map_err(|_| AppError::GenerationJobCorruptPersistedData)?;
    Ok(EnqueueGenerationResult {
        job_id: job.id.clone(),
        generation_id: job.generation_id.clone(),
        conversation_id: request.conversation_id,
        status: job.status.clone(),
        retryable: job.retryable,
        cancel_requested_at: job.cancel_requested_at.clone(),
        error_code: job.error_code.clone(),
        error_message: job.error_message.clone(),
        queued_at: job.queued_at.clone(),
        finished_at: job.finished_at.clone(),
    })
}

pub(crate) fn find_enqueue_result_by_client_request_id(
    conn: &Connection,
    client_request_id: &str,
) -> Result<Option<EnqueueGenerationResult>, AppError> {
    with_generation_job_read_transaction(conn, "find enqueue result", |tx| {
        find_enqueue_result_by_client_request_id_in_transaction(tx, client_request_id)
    })
}

pub(crate) fn find_enqueue_result_by_client_request_id_in_transaction(
    tx: &Transaction<'_>,
    client_request_id: &str,
) -> Result<Option<EnqueueGenerationResult>, AppError> {
    find_job_by_client_request_id_in_transaction(tx, client_request_id)?
        .as_ref()
        .map(enqueue_result_for_validated_job)
        .transpose()
}

fn normalized_field_name(key: &str) -> String {
    key.chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

fn sensitive_query_key(key: &str) -> bool {
    let key = normalized_field_name(key);
    matches!(
        key.as_str(),
        "key"
            | "auth"
            | "authorization"
            | "sig"
            | "signature"
            | "password"
            | "secret"
            | "clientsecret"
            | "privatekey"
            | "credential"
            | "credentials"
            | "token"
            | "accesstoken"
            | "refreshtoken"
    ) || key.ends_with("apikey")
        || key.ends_with("accesstoken")
        || key.ends_with("refreshtoken")
        || key.ends_with("clientsecret")
        || key.ends_with("privatekey")
        || key.ends_with("signature")
}

fn ascii_token_end(value: &str, start: usize) -> usize {
    value[start..]
        .char_indices()
        .find_map(|(offset, character)| {
            (!character.is_ascii_alphanumeric() && !matches!(character, '-' | '_' | '.'))
                .then_some(start + offset)
        })
        .unwrap_or(value.len())
}

fn bearer_token_end(value: &str, start: usize) -> usize {
    value[start..]
        .char_indices()
        .find_map(|(offset, character)| {
            (!character.is_ascii_alphanumeric()
                && !matches!(character, '-' | '.' | '_' | '~' | '+' | '/' | '='))
            .then_some(start + offset)
        })
        .unwrap_or(value.len())
}

fn bearer_token_shape_valid(candidate: &str) -> bool {
    let mut padding_started = false;
    !candidate.is_empty()
        && candidate.bytes().all(|byte| {
            if byte == b'=' {
                padding_started = true;
                true
            } else {
                !padding_started
                    && (byte.is_ascii_alphanumeric()
                        || matches!(byte, b'-' | b'.' | b'_' | b'~' | b'+' | b'/'))
            }
        })
}

fn has_prefixed_token(value: &str, prefix: &str, minimum_length: usize) -> bool {
    let lowercase = value.to_ascii_lowercase();
    lowercase.match_indices(prefix).any(|(start, _)| {
        let boundary_is_public = start == 0
            || !lowercase[..start]
                .chars()
                .next_back()
                .is_some_and(|character| character.is_ascii_alphanumeric());
        let end = ascii_token_end(&lowercase, start);
        boundary_is_public && end.saturating_sub(start) >= minimum_length
    })
}

fn has_slack_token(value: &str) -> bool {
    let lowercase = value.to_ascii_lowercase();
    lowercase.match_indices("xox").any(|(start, _)| {
        let boundary_is_public = start == 0
            || !lowercase[..start]
                .chars()
                .next_back()
                .is_some_and(|character| character.is_ascii_alphanumeric());
        let candidate = &lowercase[start..ascii_token_end(&lowercase, start)];
        let bytes = candidate.as_bytes();
        boundary_is_public
            && bytes.len() >= 6
            && bytes[3].is_ascii_alphabetic()
            && matches!(bytes[4], b'-' | b'_')
    })
}

fn has_jwt_token(value: &str) -> bool {
    let lowercase = value.to_ascii_lowercase();
    lowercase.match_indices("eyj").any(|(start, _)| {
        let boundary_is_public = start == 0
            || !lowercase[..start]
                .chars()
                .next_back()
                .is_some_and(|character| character.is_ascii_alphanumeric());
        let candidate = &lowercase[start..ascii_token_end(&lowercase, start)];
        let segments = candidate.split('.').collect::<Vec<_>>();
        boundary_is_public
            && segments.len() == 3
            && segments.iter().all(|segment| {
                !segment.is_empty()
                    && segment
                        .bytes()
                        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
            })
    })
}

fn has_google_api_key(value: &str) -> bool {
    value.match_indices("AIza").any(|(start, _)| {
        let boundary_is_public = start == 0
            || !value[..start]
                .chars()
                .next_back()
                .is_some_and(|character| character.is_ascii_alphanumeric());
        let candidate_end = value[start..]
            .char_indices()
            .find_map(|(offset, character)| {
                (!character.is_ascii_alphanumeric() && !matches!(character, '-' | '_'))
                    .then_some(start + offset)
            })
            .unwrap_or(value.len());
        boundary_is_public && candidate_end.saturating_sub(start) == 39
    })
}

fn has_bearer_token(value: &str) -> bool {
    let lowercase = value.to_ascii_lowercase();
    lowercase.match_indices("bearer").any(|(start, marker)| {
        let marker_end = start + marker.len();
        let boundary_before = start == 0
            || !lowercase[..start]
                .chars()
                .next_back()
                .is_some_and(|character| character.is_ascii_alphanumeric());
        let Some(first_after) = lowercase[marker_end..].chars().next() else {
            return false;
        };
        if !boundary_before || !first_after.is_whitespace() {
            return false;
        }
        let candidate_start = marker_end
            + lowercase[marker_end..]
                .find(|character: char| !character.is_whitespace())
                .unwrap_or(lowercase.len() - marker_end);
        if candidate_start >= lowercase.len() {
            return false;
        }
        let candidate_end = bearer_token_end(&lowercase, candidate_start);
        let candidate = &lowercase[candidate_start..candidate_end];
        let original_candidate = &value[candidate_start..candidate_end];
        let explicit_authorization = lowercase[..start].trim_end().ends_with("authorization:")
            || lowercase[..start].trim_end().ends_with("authorization=");
        let has_digit = candidate.bytes().any(|byte| byte.is_ascii_digit());
        let has_token_symbol = candidate
            .bytes()
            .any(|byte| matches!(byte, b'-' | b'_' | b'.'));
        explicit_authorization && candidate.len() >= 8 && bearer_token_shape_valid(candidate)
            || has_prefixed_token(candidate, "sk-", 6)
            || has_prefixed_token(candidate, "sk_", 6)
            || has_prefixed_token(candidate, "ghp_", 8)
            || has_prefixed_token(candidate, "github_pat_", 12)
            || has_google_api_key(original_candidate)
            || has_slack_token(candidate)
            || has_jwt_token(candidate)
            || (candidate.len() >= 16 && (has_digit || has_token_symbol))
            || candidate.len() >= 32
    })
}

fn contains_credential_token(value: &str) -> bool {
    has_prefixed_token(value, "sk-", 6)
        || has_prefixed_token(value, "sk_", 6)
        || has_prefixed_token(value, "ghp_", 8)
        || has_prefixed_token(value, "github_pat_", 12)
        || has_google_api_key(value)
        || has_slack_token(value)
        || has_jwt_token(value)
        || has_bearer_token(value)
}

fn public_string_is_safe(value: &str) -> bool {
    !contains_credential_token(value)
}

fn credential_like_query_value(value: &str) -> bool {
    contains_credential_token(value)
}

fn endpoint_snapshot_is_public(endpoint: &str) -> bool {
    if endpoint.is_empty() {
        return true;
    }
    let Ok(url) = reqwest::Url::parse(endpoint) else {
        return false;
    };
    if !public_string_is_safe(endpoint)
        || !matches!(url.scheme(), "http" | "https")
        || !url.username().is_empty()
        || url.password().is_some()
        || url.fragment().is_some()
    {
        return false;
    }
    url.query_pairs()
        .all(|(key, value)| !sensitive_query_key(&key) && !credential_like_query_value(&value))
}

pub(crate) fn executable_provider_snapshot_is_valid(
    provider_kind: &str,
    provider_profile_id: &str,
    endpoint_snapshot: &str,
) -> bool {
    provider_kind != "unresolved"
        && provider_profile_id != "unresolved"
        && nonempty(provider_kind)
        && nonempty(provider_profile_id)
        && public_string_is_safe(provider_kind)
        && public_string_is_safe(provider_profile_id)
        && nonempty(endpoint_snapshot)
        && endpoint_snapshot_is_public(endpoint_snapshot)
}

fn nonempty(value: &str) -> bool {
    !value.trim().is_empty()
}

fn request_options_shape_valid(options: &GenerationJobOptions) -> bool {
    [
        options.size.as_deref(),
        options.quality.as_deref(),
        options.background.as_deref(),
        options.output_format.as_deref(),
        options.moderation.as_deref(),
        options.input_fidelity.as_deref(),
    ]
    .into_iter()
    .flatten()
    .all(|value| nonempty(value) && public_string_is_safe(value))
        && options.output_compression.is_none_or(|value| value <= 100)
        && options
            .image_count
            .is_none_or(|value| (1..=4).contains(&value))
        && options.partial_images.is_none_or(|value| value <= 3)
}

fn request_options_match_prepared(request: &PreparedGenerationJob) -> bool {
    let options = &request.request_options;
    request_options_shape_valid(options)
        && options
            .size
            .as_deref()
            .is_none_or(|value| value == request.size)
        && options
            .quality
            .as_deref()
            .is_none_or(|value| value == request.quality)
        && options
            .background
            .as_deref()
            .is_none_or(|value| value == request.background)
        && options
            .output_format
            .as_deref()
            .is_none_or(|value| value == request.output_format)
        && options
            .output_compression
            .is_none_or(|value| i32::from(value) == request.output_compression)
        && options
            .moderation
            .as_deref()
            .is_none_or(|value| value == request.moderation)
        && options
            .input_fidelity
            .as_deref()
            .is_none_or(|value| value == request.input_fidelity)
        && options.stream.is_none_or(|value| value == request.stream)
        && options
            .partial_images
            .is_none_or(|value| value == request.partial_images)
        && options
            .image_count
            .is_none_or(|value| i32::from(value) == request.image_count)
}

fn safe_error_code(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
        && public_string_is_safe(value)
}

pub(crate) fn terminal_message_for_code(code: &str) -> &'static str {
    match code {
        "provider_profile_missing" => "The selected provider profile is unavailable",
        "provider_configuration_invalid" => "The provider configuration is invalid",
        "source_image_invalid" => "A source image is unavailable or invalid",
        "request_rejected" | "invalid_request" => "The generation request was rejected",
        "rate_limited" => "The provider rate limit was reached",
        "provider_unavailable" => "The provider is temporarily unavailable",
        "network_before_response" => "The provider could not be reached",
        "provider_outcome_unknown" => "The provider outcome could not be confirmed",
        "response_decode_failed" => "The provider response could not be decoded",
        "image_save_failed" => "Generated images could not be saved",
        "cancelled_by_user" => "The operation was cancelled",
        "recovery_failed" => "The interrupted operation could not be recovered",
        "app_interrupted" => "The operation was interrupted",
        "canvas_error" => "Canvas generation failed",
        _ => "The generation job failed",
    }
}

fn canonical_timestamp(value: &str) -> bool {
    chrono::DateTime::parse_from_rfc3339(value).is_ok_and(|timestamp| {
        timestamp
            .with_timezone(&chrono::Utc)
            .to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
            == value
    })
}

fn timestamp_is_not_before(value: &str, floor: &str) -> bool {
    canonical_timestamp(value) && canonical_timestamp(floor) && value >= floor
}

fn canonical_worker_timestamp(now_ms: i64) -> Result<String, WorkerTransitionError> {
    if now_ms < 0 {
        return Err(WorkerLeaseError::InvalidTiming.into());
    }
    chrono::DateTime::<chrono::Utc>::from_timestamp_millis(now_ms)
        .map(|timestamp| timestamp.to_rfc3339_opts(chrono::SecondsFormat::Secs, true))
        .ok_or_else(|| WorkerLeaseError::TimeOverflow.into())
}

fn expected_response_path_is_valid(path: &Path, generation_id: &str) -> bool {
    let Some(path_text) = path.to_str() else {
        return false;
    };
    path.is_absolute()
        && path_text.len() <= MAX_PERSISTED_RESPONSE_PATH_BYTES
        && !path_text.chars().any(char::is_control)
        && !path_text
            .split(['/', '\\'])
            .any(|component| matches!(component, "." | ".."))
        && path
            .components()
            .all(|component| !matches!(component, Component::CurDir | Component::ParentDir))
        && path.file_name().and_then(|name| name.to_str())
            == Some(format!("{generation_id}.response.json").as_str())
}

pub(crate) fn validate_worker_recovery_for_stage(
    tx: &Transaction<'_>,
    generation_id: &str,
    stage: GenerationJobStage,
) -> Result<(), AppError> {
    let recovery = tx
        .query_row(
            "SELECT request_state, expected_response_file, response_file,
                    response_size, response_sha256
               FROM generation_recoveries WHERE generation_id = ?1",
            params![generation_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, Option<i64>>(3)?,
                    row.get::<_, Option<String>>(4)?,
                ))
            },
        )
        .optional()
        .map_err(|error| persisted_row_error("Read worker generation recovery failed", error))?
        .ok_or(AppError::GenerationJobCorruptPersistedData)?;
    let (request_state, expected_file, response_file, response_size, response_sha256) = recovery;
    let requesting_empty = request_state == "requesting"
        && expected_file.is_none()
        && response_file.is_none()
        && response_size.is_none()
        && response_sha256.is_none();
    let requesting_bound = request_state == "requesting"
        && expected_file
            .as_deref()
            .is_some_and(|path| expected_response_path_is_valid(Path::new(path), generation_id))
        && response_file.is_none()
        && response_size.is_none()
        && response_sha256.is_none();
    let response_ready_bound = request_state == "response_ready"
        && expected_file.as_deref().is_some_and(|expected| {
            response_file.as_deref() == Some(expected)
                && expected_response_path_is_valid(Path::new(expected), generation_id)
        })
        && response_size.is_some_and(|size| (0..=MAX_WORKER_RESPONSE_BODY_BYTES).contains(&size))
        && response_sha256.as_deref().is_some_and(|sha256| {
            sha256.len() == 64
                && sha256
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
        });
    let valid = match stage {
        GenerationJobStage::Preparing => requesting_empty,
        GenerationJobStage::ProviderRequest | GenerationJobStage::RetryBackoff => requesting_bound,
        GenerationJobStage::ResponseReady | GenerationJobStage::LocalProcessing => {
            response_ready_bound
        }
        _ => false,
    };
    if valid {
        Ok(())
    } else {
        Err(AppError::GenerationJobCorruptPersistedData)
    }
}

fn source_reference_id_valid(value: &str) -> bool {
    nonempty(value)
        && value.len() <= 256
        && !value.chars().any(char::is_control)
        && public_string_is_safe(value)
}

fn source_reference_ids_valid<'a>(values: impl IntoIterator<Item = &'a Option<String>>) -> bool {
    values
        .into_iter()
        .flatten()
        .all(|value| source_reference_id_valid(value))
}

impl GenerationJobSourceRef {
    fn parse(source_kind: &str, value: &Value) -> Result<Self, AppError> {
        let source_ref = match source_kind {
            "generate" | "edit" => Self::GenerateEdit(
                serde_json::from_value(value.clone())
                    .map_err(|_| AppError::GenerationJobInvalidSnapshot)?,
            ),
            "canvas" => Self::Canvas(
                serde_json::from_value(value.clone())
                    .map_err(|_| AppError::GenerationJobInvalidSnapshot)?,
            ),
            _ => return Err(AppError::GenerationJobInvalidSnapshot),
        };
        let ids_valid = match &source_ref {
            Self::GenerateEdit(source_ref) => source_reference_ids_valid([
                &source_ref.id,
                &source_ref.conversation_id,
                &source_ref.project_id,
            ]),
            Self::Canvas(source_ref) => source_reference_ids_valid([
                &source_ref.id,
                &source_ref.round_id,
                &source_ref.canvas_round_id,
                &source_ref.document_id,
                &source_ref.canvas_document_id,
                &source_ref.revision_id,
                &source_ref.source_revision_id,
                &source_ref.parent_round_id,
                &source_ref.conversation_id,
                &source_ref.project_id,
            ]),
        };
        if ids_valid {
            Ok(source_ref)
        } else {
            Err(AppError::GenerationJobInvalidSnapshot)
        }
    }

    fn into_value(
        self,
        identity: Option<&ResolvedConversationIdentity>,
    ) -> Result<Value, AppError> {
        match self {
            Self::GenerateEdit(mut source_ref) => {
                if let Some(identity) = identity {
                    source_ref.conversation_id = Some(identity.conversation_id.clone());
                    source_ref.project_id = Some(identity.project_id.clone());
                }
                serde_json::to_value(source_ref).map_err(|_| AppError::GenerationJobInvalidSnapshot)
            }
            Self::Canvas(mut source_ref) => {
                if let Some(identity) = identity {
                    source_ref.conversation_id = Some(identity.conversation_id.clone());
                    source_ref.project_id = Some(identity.project_id.clone());
                }
                serde_json::to_value(source_ref).map_err(|_| AppError::GenerationJobInvalidSnapshot)
            }
        }
    }
}

fn canonical_source_ref(
    source_kind: &str,
    value: &Value,
    identity: Option<&ResolvedConversationIdentity>,
) -> Result<Value, AppError> {
    GenerationJobSourceRef::parse(source_kind, value)?.into_value(identity)
}

fn canonical_request(
    request: &PreparedGenerationJob,
    identity: &ResolvedConversationIdentity,
) -> Result<GenerationJobRequest, AppError> {
    Ok(GenerationJobRequest {
        kind: GenerationJobRequestKind::parse(&request.request_kind)?,
        prompt: request.prompt.clone(),
        model: request.model.clone(),
        source_image_paths: request.source_image_paths.clone(),
        options: request.request_options.clone(),
        requested_conversation_id: request.requested_conversation_id.clone(),
        requested_project_id: request.requested_project_id.clone(),
        conversation_id: identity.conversation_id.clone(),
        project_id: identity.project_id.clone(),
    })
}

fn canonical_metadata(
    request: &PreparedGenerationJob,
    identity: &ResolvedConversationIdentity,
) -> Result<CanonicalGenerationMetadata, AppError> {
    Ok(CanonicalGenerationMetadata {
        request_kind: GenerationJobRequestKind::parse(&request.request_kind)?,
        conversation_id: identity.conversation_id.clone(),
        project_id: identity.project_id.clone(),
        model: request.model.clone(),
        size: request.size.clone(),
        quality: request.quality.clone(),
        background: request.background.clone(),
        output_format: request.output_format.clone(),
        output_compression: u8::try_from(request.output_compression)
            .map_err(|_| AppError::GenerationJobInvalidSnapshot)?,
        moderation: request.moderation.clone(),
        input_fidelity: request.input_fidelity.clone(),
        stream: request.stream,
        partial_images: request.partial_images,
        image_count: u8::try_from(request.image_count)
            .map_err(|_| AppError::GenerationJobInvalidSnapshot)?,
        source_image_count: request.source_image_paths.len(),
        actual_image_count: None,
    })
}

fn validate_prepared_job(request: &PreparedGenerationJob) -> Result<(), AppError> {
    let common_fields_valid = [
        request.job_id.as_str(),
        request.client_request_id.as_str(),
        request.generation_id.as_str(),
        request.prompt.as_str(),
        request.model.as_str(),
        request.request_kind.as_str(),
        request.size.as_str(),
        request.quality.as_str(),
        request.background.as_str(),
        request.output_format.as_str(),
        request.moderation.as_str(),
        request.input_fidelity.as_str(),
        request.source_kind.as_str(),
        request.provider_kind.as_str(),
        request.provider_profile_id.as_str(),
        request.queued_at.as_str(),
    ]
    .iter()
    .all(|value| nonempty(value) && public_string_is_safe(value));
    let timestamps_valid = canonical_timestamp(&request.queued_at)
        && request
            .finished_at
            .as_deref()
            .is_none_or(canonical_timestamp);
    let canonical_fields_valid = GenerationJobRequestKind::parse(&request.request_kind).is_ok()
        && request_options_match_prepared(request)
        && canonical_source_ref(&request.source_kind, &request.source_ref, None).is_ok();
    let optional_public_fields_valid = [
        request.requested_conversation_id.as_deref(),
        request.requested_project_id.as_deref(),
        request.parent_job_id.as_deref(),
        request.finished_at.as_deref(),
        request.error_code.as_deref(),
    ]
    .into_iter()
    .flatten()
    .all(|value| nonempty(value) && public_string_is_safe(value));
    let paths_are_public = request
        .source_image_paths
        .iter()
        .all(|path| nonempty(path) && public_string_is_safe(path));
    let counters_valid = request.chain_attempt >= 1
        && request.auto_attempt >= 0
        && request.max_auto_attempts >= 0
        && request.auto_attempt <= request.max_auto_attempts
        && (1..=4).contains(&request.image_count)
        && request.partial_images <= 3
        && (0..=100).contains(&request.output_compression);
    let snapshot_is_public = endpoint_snapshot_is_public(&request.endpoint_snapshot);
    let unresolved_provider = request.provider_kind == "unresolved"
        && request.provider_profile_id == "unresolved"
        && request.endpoint_snapshot.is_empty();
    let resolved_provider = executable_provider_snapshot_is_valid(
        &request.provider_kind,
        &request.provider_profile_id,
        &request.endpoint_snapshot,
    );
    let status_fields_valid = match request.status {
        GenerationJobStatus::Queued => {
            request.finished_at.is_none()
                && request.error_code.is_none()
                && request.error_message.is_none()
                && !request.retryable
                && resolved_provider
        }
        GenerationJobStatus::Failed => {
            request.finished_at.is_some()
                && matches!(
                    request.error_code.as_deref(),
                    Some("provider_profile_missing" | "provider_configuration_invalid")
                )
                && !request.retryable
                && (unresolved_provider || resolved_provider)
        }
        _ => false,
    };

    if common_fields_valid
        && timestamps_valid
        && canonical_fields_valid
        && optional_public_fields_valid
        && paths_are_public
        && counters_valid
        && snapshot_is_public
        && status_fields_valid
    {
        Ok(())
    } else {
        Err(AppError::GenerationJobInvalidSnapshot)
    }
}

fn existing_matches_prepared(existing: &GenerationJob, request: &PreparedGenerationJob) -> bool {
    let Ok(existing_request) =
        serde_json::from_value::<GenerationJobRequest>(existing.request.clone())
    else {
        return false;
    };
    if existing_request.requested_conversation_id != request.requested_conversation_id
        || existing_request.requested_project_id != request.requested_project_id
    {
        return false;
    }
    let identity = ResolvedConversationIdentity {
        conversation_id: existing_request.conversation_id.clone(),
        project_id: existing_request.project_id.clone(),
    };
    let Ok(expected_request) = canonical_request(request, &identity) else {
        return false;
    };
    let Ok(expected_source_ref) =
        canonical_source_ref(&request.source_kind, &request.source_ref, Some(&identity))
    else {
        return false;
    };
    existing.parent_job_id == request.parent_job_id
        && existing.source_kind == request.source_kind
        && existing.source_ref == expected_source_ref
        && existing_request == expected_request
        && existing.provider_kind == request.provider_kind
        && existing.provider_profile_id == request.provider_profile_id
        && existing.endpoint_snapshot == request.endpoint_snapshot
        && existing.chain_attempt == request.chain_attempt
        && existing.max_auto_attempts == request.max_auto_attempts
}

fn insert_prepared_rows_in_transaction(
    tx: &Transaction<'_>,
    request: &PreparedGenerationJob,
    identity: &ResolvedConversationIdentity,
) -> Result<EnqueueGenerationResult, AppError> {
    let canonical_request = canonical_request(request, identity)?;
    let request_metadata = canonical_metadata(request, identity)?;
    let canonical_source_ref =
        canonical_source_ref(&request.source_kind, &request.source_ref, Some(identity))?;
    let source_image_paths_json = serde_json::to_string(&request.source_image_paths)
        .map_err(|_| AppError::GenerationJobInvalidSnapshot)?;
    let request_metadata_json = serde_json::to_string(&request_metadata)
        .map_err(|_| AppError::GenerationJobInvalidSnapshot)?;
    let source_ref_json = serde_json::to_string(&canonical_source_ref)
        .map_err(|_| AppError::GenerationJobInvalidSnapshot)?;
    let request_json = serde_json::to_string(&canonical_request)
        .map_err(|_| AppError::GenerationJobInvalidSnapshot)?;
    let status = status_as_str(&request.status);
    let stage = match request.status {
        GenerationJobStatus::Queued => GenerationJobStage::Queued,
        GenerationJobStatus::Failed => GenerationJobStage::Terminal,
        _ => return Err(AppError::GenerationJobInvalidSnapshot),
    };
    let persisted_error_message = request.error_code.as_deref().map(terminal_message_for_code);

    tx.execute(
        "INSERT INTO generations (
            id, prompt, engine, request_kind, size, quality, background, output_format,
            output_compression, moderation, input_fidelity, image_count, source_image_count,
            source_image_paths, request_metadata, status, error_message, conversation_id, created_at
         ) VALUES (
            ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15,
            ?16, ?17, ?18, ?19
         )",
        params![
            request.generation_id,
            request.prompt,
            request.model,
            request.request_kind,
            request.size,
            request.quality,
            request.background,
            request.output_format,
            request.output_compression,
            request.moderation,
            request.input_fidelity,
            request.image_count,
            request.source_image_paths.len() as i32,
            source_image_paths_json,
            request_metadata_json,
            status,
            persisted_error_message,
            identity.conversation_id,
            request.queued_at,
        ],
    )
    .map_err(|error| database_error("Insert queued generation failed", error))?;

    if request.status == GenerationJobStatus::Queued {
        tx.execute(
            "INSERT INTO generation_recoveries (
                generation_id, request_kind, request_state, output_format, response_file,
                created_at, updated_at
             ) VALUES (?1, ?2, 'requesting', ?3, NULL, ?4, ?4)",
            params![
                request.generation_id,
                request.request_kind,
                request.output_format,
                request.queued_at,
            ],
        )
        .map_err(|error| database_error("Insert generation recovery failed", error))?;
    }

    tx.execute(
        "INSERT INTO generation_jobs (
            id, client_request_id, generation_id, parent_job_id, source_kind, source_ref_json,
            status, stage, request_json, provider_kind, provider_profile_id, endpoint_snapshot,
            chain_attempt, auto_attempt, max_auto_attempts, queued_at, started_at, finished_at,
            cancel_requested_at, last_heartbeat_at, error_code, error_message, retryable
         ) VALUES (
            ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15,
            ?16, NULL, ?17, NULL, NULL, ?18, ?19, ?20
         )",
        params![
            request.job_id,
            request.client_request_id,
            request.generation_id,
            request.parent_job_id,
            request.source_kind,
            source_ref_json,
            status,
            stage_as_str(stage),
            request_json,
            request.provider_kind,
            request.provider_profile_id,
            request.endpoint_snapshot,
            request.chain_attempt,
            request.auto_attempt,
            request.max_auto_attempts,
            request.queued_at,
            request.finished_at,
            request.error_code,
            persisted_error_message,
            i64::from(request.retryable),
        ],
    )
    .map_err(|error| database_error("Insert generation job failed", error))?;

    let job = get_job_in_transaction(tx, &request.job_id)?;
    enqueue_result_for_validated_job(&job)
}

pub(crate) fn insert_job_in_transaction(
    tx: &Transaction<'_>,
    request: &PreparedGenerationJob,
) -> Result<EnqueueGenerationResult, AppError> {
    if let Some(existing) =
        find_job_by_client_request_id_in_transaction(tx, &request.client_request_id)?
    {
        if existing_matches_prepared(&existing, request) {
            return enqueue_result_for_validated_job(&existing);
        }
        return Err(AppError::GenerationJobIdempotencyConflict);
    }

    validate_prepared_job(request)?;
    if request.parent_job_id.is_some() || request.chain_attempt != 1 || request.auto_attempt != 0 {
        return Err(AppError::GenerationJobInvalidSnapshot);
    }
    let conversation_id = conversations::resolve_conversation_id_for_generation(
        tx,
        request.requested_conversation_id.as_deref(),
        request.requested_project_id.as_deref(),
        &request.prompt,
    )?;
    let identity = resolved_conversation_identity(tx, &conversation_id)?;
    insert_prepared_rows_in_transaction(tx, request, &identity)
}

pub(crate) fn enqueue_job_with_event(
    conn: &mut Connection,
    request: &PreparedGenerationJob,
) -> Result<GenerationJobTransition<EnqueueGenerationResult>, AppError> {
    if let Some(existing) = find_job_by_client_request_id(conn, &request.client_request_id)? {
        if existing_matches_prepared(&existing, request) {
            return Ok(GenerationJobTransition {
                value: enqueue_result_for_validated_job(&existing)?,
                event: None,
            });
        }
        return Err(AppError::GenerationJobIdempotencyConflict);
    }

    let tx = begin_generation_job_write_transaction(conn)?;
    let (value, event) = if let Some(existing) =
        find_job_by_client_request_id_in_transaction(&tx, &request.client_request_id)?
    {
        if !existing_matches_prepared(&existing, request) {
            return Err(AppError::GenerationJobIdempotencyConflict);
        }
        (enqueue_result_for_validated_job(&existing)?, None)
    } else {
        let value = insert_job_in_transaction(&tx, request)?;
        let event = get_job_event_in_transaction(&tx, &value.job_id)?;
        (value, Some(event))
    };
    tx.commit()
        .map_err(|error| database_error("Commit generation job transaction failed", error))?;
    Ok(GenerationJobTransition { value, event })
}

pub(crate) fn enqueue_job(
    conn: &mut Connection,
    request: &PreparedGenerationJob,
) -> Result<EnqueueGenerationResult, AppError> {
    enqueue_job_with_event(conn, request).map(GenerationJobTransition::into_value)
}

pub(crate) fn begin_generation_job_write_transaction(
    conn: &mut Connection,
) -> Result<Transaction<'_>, AppError> {
    begin_generation_job_write_transaction_unchecked(conn)
}

fn begin_generation_job_write_transaction_unchecked(
    conn: &Connection,
) -> Result<Transaction<'_>, AppError> {
    conn.busy_timeout(GENERATION_JOB_WRITE_BUSY_TIMEOUT)
        .map_err(|error| database_error("Configure generation job writer wait failed", error))?;
    Transaction::new_unchecked(conn, TransactionBehavior::Immediate)
        .map_err(|error| database_error("Begin generation job write transaction failed", error))
}

/// Inserts, claims, and snapshots one caller-selected root job in a single
/// immediate transaction. Unlike `claim_next_job`, this compatibility path
/// must never claim an older queued job or silently reuse another idempotent
/// identity.
pub(crate) fn insert_and_claim_exact_job(
    conn: &mut Connection,
    request: &PreparedGenerationJob,
) -> Result<GenerationExecutionSnapshot, AppError> {
    let tx = begin_generation_job_write_transaction(conn)?;
    let inserted = insert_job_in_transaction(&tx, request)?;
    if inserted.job_id != request.job_id || inserted.generation_id != request.generation_id {
        return Err(AppError::GenerationJobIdempotencyConflict);
    }

    let claimed = claim_job_in_transaction(&tx, &request.job_id)?;
    if claimed.id != request.job_id || claimed.generation_id != request.generation_id {
        return Err(AppError::GenerationJobCorruptPersistedData);
    }

    let snapshot = load_generation_execution_snapshot_in_transaction(&tx, &request.job_id)?;
    if snapshot.context.job_id != request.job_id
        || snapshot.context.generation_id != request.generation_id
        || snapshot.context.conversation_id != inserted.conversation_id
        || snapshot.request.conversation_id != inserted.conversation_id
    {
        return Err(AppError::GenerationJobCorruptPersistedData);
    }

    tx.commit().map_err(|error| {
        database_error(
            "Commit exact generation job insert-and-claim transaction failed",
            error,
        )
    })?;
    Ok(snapshot)
}

fn decode_cursor(value: &str) -> Result<JobCursor, AppError> {
    let bytes = URL_SAFE_NO_PAD
        .decode(value)
        .map_err(|_| AppError::GenerationJobCorruptCursor)?;
    let cursor: JobCursor =
        serde_json::from_slice(&bytes).map_err(|_| AppError::GenerationJobCorruptCursor)?;
    if cursor.version != 1 || cursor.rowid <= 0 || !canonical_timestamp(&cursor.queued_at) {
        return Err(AppError::GenerationJobCorruptCursor);
    }
    Ok(cursor)
}

fn encode_cursor(queued_at: &str, rowid: i64) -> Result<String, AppError> {
    if rowid <= 0 || !canonical_timestamp(queued_at) {
        return Err(AppError::GenerationJobCorruptPersistedData);
    }
    let bytes = serde_json::to_vec(&JobCursor {
        version: 1,
        queued_at: queued_at.to_string(),
        rowid,
    })
    .map_err(|_| AppError::GenerationJobCorruptPersistedData)?;
    Ok(URL_SAFE_NO_PAD.encode(bytes))
}

fn list_jobs_in_transaction_with_query_observer(
    tx: &Transaction<'_>,
    filter: &GenerationJobFilter,
    observe_query: &mut dyn FnMut(),
) -> Result<GenerationJobPage, AppError> {
    let limit = filter
        .limit
        .unwrap_or(DEFAULT_GENERATION_JOB_PAGE_LIMIT)
        .clamp(1, MAX_GENERATION_JOB_PAGE_LIMIT);
    let cursor = filter.cursor.as_deref().map(decode_cursor).transpose()?;
    let mut clauses = Vec::new();
    let mut values: Vec<SqlValue> = Vec::new();

    if let Some(statuses) = filter
        .statuses
        .as_ref()
        .filter(|statuses| !statuses.is_empty())
    {
        let placeholders = statuses
            .iter()
            .map(|status| {
                values.push(SqlValue::Text(status_as_str(status).to_string()));
                format!("?{}", values.len())
            })
            .collect::<Vec<_>>()
            .join(", ");
        clauses.push(format!("g.status IN ({placeholders})"));
    }
    if let Some(source_kind) = filter.source_kind.as_deref() {
        values.push(SqlValue::Text(source_kind.to_string()));
        clauses.push(format!("g.source_kind = ?{}", values.len()));
    }
    if let Some(source_ref_id) = filter.source_ref_id.as_deref() {
        values.push(SqlValue::Text(source_ref_id.to_string()));
        clauses.push(format!(
            "EXISTS (
                SELECT 1 FROM json_each(
                    CASE WHEN json_valid(g.source_ref_json) THEN g.source_ref_json ELSE '{{}}' END
                ) source_value
                WHERE CAST(source_value.value AS TEXT) = ?{}
            )",
            values.len()
        ));
    }
    if let Some(generation_id) = filter.generation_id.as_deref() {
        values.push(SqlValue::Text(generation_id.to_string()));
        clauses.push(format!("g.generation_id = ?{}", values.len()));
    }
    if let Some(cursor) = cursor {
        values.push(SqlValue::Text(cursor.queued_at.clone()));
        let queued_index = values.len();
        values.push(SqlValue::Integer(cursor.rowid));
        let rowid_index = values.len();
        clauses.push(format!(
            "(g.queued_at > ?{queued_index}
                OR (g.queued_at = ?{queued_index} AND g.rowid > ?{rowid_index}))"
        ));
    }

    let where_clause = if clauses.is_empty() {
        String::new()
    } else {
        format!(" WHERE {}", clauses.join(" AND "))
    };
    values.push(SqlValue::Integer(i64::from(limit) + 1));
    let limit_index = values.len();
    let sql = format!(
        "SELECT g.rowid, {ALIASED_JOB_COLUMNS}
         FROM generation_jobs g{where_clause}
         ORDER BY g.queued_at ASC, g.rowid ASC
         LIMIT ?{limit_index}"
    );
    observe_query();
    let mut statement = tx
        .prepare(&sql)
        .map_err(|error| database_error("Prepare generation job list failed", error))?;
    let stored = statement
        .query_map(params_from_iter(values.iter()), |row| {
            Ok((row.get::<_, i64>(0)?, stored_job_from_row_offset(row, 1)?))
        })
        .map_err(|error| database_error("Query generation job list failed", error))?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|error| persisted_row_error("Read generation job list failed", error))?;
    drop(statement);

    let has_more = stored.len() > limit as usize;
    let mut items_with_order = stored
        .into_iter()
        .take(limit as usize)
        .map(|(rowid, stored)| {
            let job = decode_stored_job(stored)?;
            Ok((rowid, job))
        })
        .collect::<Result<Vec<_>, AppError>>()?;
    let generation_ids = unique_strings(
        items_with_order
            .iter()
            .map(|(_, job)| job.generation_id.clone()),
    );
    let projections =
        load_job_projection_batch_with_query_observer(tx, &generation_ids, observe_query)?;
    for (_, job) in &items_with_order {
        validate_job_projection(job, &projections)?;
    }
    let next_cursor = if has_more {
        items_with_order
            .last()
            .map(|(rowid, job)| encode_cursor(&job.queued_at, *rowid))
            .transpose()?
    } else {
        None
    };
    let items = items_with_order.drain(..).map(|(_, job)| job).collect();
    Ok(GenerationJobPage { items, next_cursor })
}

pub(crate) fn list_jobs_in_transaction(
    tx: &Transaction<'_>,
    filter: &GenerationJobFilter,
) -> Result<GenerationJobPage, AppError> {
    let mut no_query_observer = || {};
    list_jobs_in_transaction_with_query_observer(tx, filter, &mut no_query_observer)
}

pub(crate) fn list_jobs(
    conn: &Connection,
    filter: &GenerationJobFilter,
) -> Result<GenerationJobPage, AppError> {
    with_generation_job_read_transaction(conn, "list", |tx| list_jobs_in_transaction(tx, filter))
}

fn stored_job_from_row_offset(
    row: &Row<'_>,
    offset: usize,
) -> rusqlite::Result<StoredGenerationJob> {
    Ok(StoredGenerationJob {
        id: row.get(offset)?,
        client_request_id: row.get(offset + 1)?,
        generation_id: row.get(offset + 2)?,
        parent_job_id: row.get(offset + 3)?,
        source_kind: row.get(offset + 4)?,
        source_ref_json: row.get(offset + 5)?,
        status: row.get(offset + 6)?,
        stage: row.get(offset + 7)?,
        request_json: row.get(offset + 8)?,
        provider_kind: row.get(offset + 9)?,
        provider_profile_id: row.get(offset + 10)?,
        endpoint_snapshot: row.get(offset + 11)?,
        chain_attempt: row.get(offset + 12)?,
        auto_attempt: row.get(offset + 13)?,
        max_auto_attempts: row.get(offset + 14)?,
        queued_at: row.get(offset + 15)?,
        started_at: row.get(offset + 16)?,
        finished_at: row.get(offset + 17)?,
        cancel_requested_at: row.get(offset + 18)?,
        last_heartbeat_at: row.get(offset + 19)?,
        error_code: row.get(offset + 20)?,
        error_message: row.get(offset + 21)?,
        retryable: row.get(offset + 22)?,
    })
}

pub(crate) fn claim_next_job_with_event(
    conn: &mut Connection,
) -> Result<Option<GenerationJobTransition<GenerationJob>>, AppError> {
    let tx = begin_generation_job_write_transaction(conn)?;
    let candidate_id = tx
        .query_row(
            "SELECT id
             FROM generation_jobs
             WHERE status = 'queued' AND stage = 'queued'
             ORDER BY queued_at ASC, rowid ASC
             LIMIT 1",
            [],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|error| persisted_row_error("Select next generation job failed", error))?;
    let Some(candidate_id) = candidate_id else {
        tx.commit()
            .map_err(|error| database_error("Commit empty job claim failed", error))?;
        return Ok(None);
    };
    let job = claim_job_in_transaction(&tx, &candidate_id)?;
    let event = get_job_event_in_transaction(&tx, &candidate_id)?;
    tx.commit()
        .map_err(|error| database_error("Commit job claim failed", error))?;
    Ok(Some(GenerationJobTransition {
        value: job,
        event: Some(event),
    }))
}

pub(crate) fn claim_next_job_fenced_with_event(
    conn: &Connection,
    authority: &WorkerTransitionAuthority,
    now_ms: i64,
) -> Result<Option<GenerationJobTransition<GenerationJob>>, WorkerTransitionError> {
    claim_next_job_fenced_with_event_with_transaction_time(conn, authority, || now_ms)
}

/// Samples worker time exactly once after the immediate transaction is held.
/// `now` must be side-effect free so the authority assertion remains the
/// first database read in the worker-owned transaction.
pub(crate) fn claim_next_job_fenced_with_event_with_transaction_time<Now>(
    conn: &Connection,
    authority: &WorkerTransitionAuthority,
    now: Now,
) -> Result<Option<GenerationJobTransition<GenerationJob>>, WorkerTransitionError>
where
    Now: FnOnce() -> i64,
{
    let tx = begin_generation_job_write_transaction_unchecked(conn)?;
    let now_ms = now();
    let timestamp = canonical_worker_timestamp(now_ms)?;
    assert_worker_transition_authority_in_transaction(&tx, authority, now_ms)?;
    let candidate_id = tx
        .query_row(
            "SELECT id
             FROM generation_jobs
             WHERE status = 'queued' AND stage = 'queued'
             ORDER BY queued_at ASC, rowid ASC
             LIMIT 1",
            [],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|error| persisted_row_error("Select next fenced generation job failed", error))?;
    let Some(candidate_id) = candidate_id else {
        tx.commit()
            .map_err(|error| database_error("Commit empty fenced job claim failed", error))?;
        return Ok(None);
    };
    let candidate = get_job_in_transaction(&tx, &candidate_id)?;
    validate_worker_recovery_for_stage(
        &tx,
        &candidate.generation_id,
        GenerationJobStage::Preparing,
    )?;
    let job = claim_job_at_in_transaction(&tx, &candidate_id, &timestamp)?;
    let event = get_job_event_in_transaction(&tx, &candidate_id)?;
    tx.commit()
        .map_err(|error| database_error("Commit fenced job claim failed", error))?;
    Ok(Some(GenerationJobTransition {
        value: job,
        event: Some(event),
    }))
}

pub(crate) fn transition_running_job_stage_with_event(
    conn: &Connection,
    job_id: &str,
    expected_stage: GenerationJobStage,
    transition: WorkerStageTransition,
    authority: &WorkerTransitionAuthority,
    now_ms: i64,
) -> Result<GenerationJobTransition<GenerationJob>, WorkerTransitionError> {
    transition_running_job_stage_with_event_with_transaction_time(
        conn,
        job_id,
        expected_stage,
        transition,
        authority,
        || now_ms,
    )
}

/// Samples worker time exactly once after the immediate transaction is held.
/// `now` must not access the database.
pub(crate) fn transition_running_job_stage_with_event_with_transaction_time<Now>(
    conn: &Connection,
    job_id: &str,
    expected_stage: GenerationJobStage,
    transition: WorkerStageTransition,
    authority: &WorkerTransitionAuthority,
    now: Now,
) -> Result<GenerationJobTransition<GenerationJob>, WorkerTransitionError>
where
    Now: FnOnce() -> i64,
{
    let tx = begin_generation_job_write_transaction_unchecked(conn)?;
    let now_ms = now();
    let timestamp = canonical_worker_timestamp(now_ms)?;
    assert_worker_transition_authority_in_transaction(&tx, authority, now_ms)?;
    let current = get_job_in_transaction(&tx, job_id)?;
    if current.status != GenerationJobStatus::Running || current.stage != expected_stage {
        return Err(AppError::GenerationJobInvalidTransition.into());
    }
    if !current
        .started_at
        .as_deref()
        .is_some_and(|started_at| timestamp_is_not_before(&timestamp, started_at))
        || !current
            .last_heartbeat_at
            .as_deref()
            .is_some_and(|heartbeat_at| timestamp_is_not_before(&timestamp, heartbeat_at))
    {
        return Err(AppError::GenerationJobInvalidTransition.into());
    }
    validate_worker_recovery_for_stage(&tx, &current.generation_id, current.stage)?;

    let (next_stage, requires_no_cancel) = match transition {
        WorkerStageTransition::BeginProviderRequest {
            expected_response_file,
        } if expected_stage == GenerationJobStage::Preparing => {
            if !expected_response_path_is_valid(&expected_response_file, &current.generation_id) {
                return Err(AppError::GenerationJobInvalidSnapshot.into());
            }
            let expected_response_file = expected_response_file
                .to_str()
                .ok_or(AppError::GenerationJobInvalidSnapshot)?;
            let updated_recovery = tx
                .execute(
                    "UPDATE generation_recoveries
                        SET expected_response_file = ?1, updated_at = ?2
                      WHERE generation_id = ?3
                        AND request_state = 'requesting'
                        AND expected_response_file IS NULL
                        AND response_file IS NULL
                        AND response_size IS NULL
                        AND response_sha256 IS NULL",
                    params![expected_response_file, timestamp, current.generation_id],
                )
                .map_err(|error| {
                    database_error("Record expected generation response path failed", error)
                })?;
            if updated_recovery != 1 {
                return Err(AppError::GenerationJobInvalidTransition.into());
            }
            (GenerationJobStage::ProviderRequest, true)
        }
        WorkerStageTransition::EnterRetryBackoff
            if expected_stage == GenerationJobStage::ProviderRequest =>
        {
            (GenerationJobStage::RetryBackoff, true)
        }
        WorkerStageTransition::EnterLocalProcessing
            if expected_stage == GenerationJobStage::ResponseReady =>
        {
            (GenerationJobStage::LocalProcessing, true)
        }
        _ => return Err(AppError::GenerationJobInvalidTransition.into()),
    };

    let updated = tx
        .execute(
            "UPDATE generation_jobs
                SET stage = ?1, last_heartbeat_at = ?2
              WHERE id = ?3 AND status = 'running' AND stage = ?4
                AND (?5 = 0 OR cancel_requested_at IS NULL)
                AND started_at IS NOT NULL AND started_at <= ?2
                AND last_heartbeat_at IS NOT NULL AND last_heartbeat_at <= ?2",
            params![
                stage_as_str(next_stage),
                timestamp,
                job_id,
                stage_as_str(expected_stage),
                i64::from(requires_no_cancel),
            ],
        )
        .map_err(|error| database_error("Transition generation job stage failed", error))?;
    if updated != 1 {
        return Err(AppError::GenerationJobInvalidTransition.into());
    }
    validate_worker_recovery_for_stage(&tx, &current.generation_id, next_stage)?;
    let job = get_job_in_transaction(&tx, job_id)?;
    let event = get_job_event_in_transaction(&tx, job_id)?;
    tx.commit()
        .map_err(|error| database_error("Commit generation job stage failed", error))?;
    Ok(GenerationJobTransition {
        value: job,
        event: Some(event),
    })
}

pub(crate) fn heartbeat_running_job(
    conn: &Connection,
    job_id: &str,
    expected_stage: GenerationJobStage,
    authority: &WorkerTransitionAuthority,
    now_ms: i64,
) -> Result<GenerationJob, WorkerTransitionError> {
    heartbeat_running_job_with_transaction_time(conn, job_id, expected_stage, authority, || now_ms)
}

/// Samples worker time exactly once after the immediate transaction is held.
/// `now` must not access the database.
pub(crate) fn heartbeat_running_job_with_transaction_time<Now>(
    conn: &Connection,
    job_id: &str,
    expected_stage: GenerationJobStage,
    authority: &WorkerTransitionAuthority,
    now: Now,
) -> Result<GenerationJob, WorkerTransitionError>
where
    Now: FnOnce() -> i64,
{
    let tx = begin_generation_job_write_transaction_unchecked(conn)?;
    let now_ms = now();
    let timestamp = canonical_worker_timestamp(now_ms)?;
    assert_worker_transition_authority_in_transaction(&tx, authority, now_ms)?;
    let updated = tx
        .execute(
            "UPDATE generation_jobs
                SET last_heartbeat_at = ?1
              WHERE id = ?2 AND status = 'running' AND stage = ?3
                AND started_at IS NOT NULL AND started_at <= ?1
                AND last_heartbeat_at IS NOT NULL
                AND last_heartbeat_at <= ?1",
            params![timestamp, job_id, stage_as_str(expected_stage)],
        )
        .map_err(|error| database_error("Heartbeat generation job failed", error))?;
    if updated != 1 {
        return Err(AppError::GenerationJobInvalidTransition.into());
    }
    let job = get_job_in_transaction(&tx, job_id)?;
    tx.commit()
        .map_err(|error| database_error("Commit generation job heartbeat failed", error))?;
    Ok(job)
}

pub(crate) fn heartbeat_running_job_current_stage_fenced(
    conn: &Connection,
    job_id: &str,
    authority: &WorkerTransitionAuthority,
    now_ms: i64,
) -> Result<GenerationJob, WorkerTransitionError> {
    heartbeat_running_job_current_stage_fenced_with_transaction_time(
        conn,
        job_id,
        authority,
        || now_ms,
    )
}

/// Samples worker time exactly once after the immediate transaction is held.
/// `now` must not access the database.
pub(crate) fn heartbeat_running_job_current_stage_fenced_with_transaction_time<Now>(
    conn: &Connection,
    job_id: &str,
    authority: &WorkerTransitionAuthority,
    now: Now,
) -> Result<GenerationJob, WorkerTransitionError>
where
    Now: FnOnce() -> i64,
{
    let tx = begin_generation_job_write_transaction_unchecked(conn)?;
    let now_ms = now();
    let timestamp = canonical_worker_timestamp(now_ms)?;
    // Keep the authority assertion as the first database operation after
    // BEGIN IMMEDIATE and the transaction-time sample; the stage read and
    // heartbeat then share one snapshot.
    assert_worker_transition_authority_in_transaction(&tx, authority, now_ms)?;
    let current = get_job_in_transaction(&tx, job_id)?;
    if current.status != GenerationJobStatus::Running
        || !matches!(
            current.stage,
            GenerationJobStage::Preparing
                | GenerationJobStage::ProviderRequest
                | GenerationJobStage::RetryBackoff
                | GenerationJobStage::ResponseReady
                | GenerationJobStage::LocalProcessing
        )
    {
        return Err(AppError::GenerationJobInvalidTransition.into());
    }
    let updated = tx
        .execute(
            "UPDATE generation_jobs
                SET last_heartbeat_at = ?1
              WHERE id = ?2 AND status = 'running' AND stage = ?3
                AND started_at IS NOT NULL AND started_at <= ?1
                AND last_heartbeat_at IS NOT NULL
                AND last_heartbeat_at <= ?1",
            params![timestamp, job_id, stage_as_str(current.stage)],
        )
        .map_err(|error| database_error("Heartbeat current generation job stage failed", error))?;
    if updated != 1 {
        return Err(AppError::GenerationJobInvalidTransition.into());
    }
    let job = get_job_in_transaction(&tx, job_id)?;
    tx.commit()
        .map_err(|error| database_error("Commit current generation job heartbeat failed", error))?;
    Ok(job)
}

pub(crate) fn reread_running_job_cancel_requested_fenced(
    conn: &Connection,
    job_id: &str,
    authority: &WorkerTransitionAuthority,
    now_ms: i64,
) -> Result<bool, WorkerTransitionError> {
    reread_running_job_cancel_requested_fenced_with_transaction_time(
        conn,
        job_id,
        authority,
        || now_ms,
    )
}

/// Samples worker time exactly once after the immediate transaction is held.
/// `now` must not access the database.
pub(crate) fn reread_running_job_cancel_requested_fenced_with_transaction_time<Now>(
    conn: &Connection,
    job_id: &str,
    authority: &WorkerTransitionAuthority,
    now: Now,
) -> Result<bool, WorkerTransitionError>
where
    Now: FnOnce() -> i64,
{
    let tx = begin_generation_job_write_transaction_unchecked(conn)?;
    let now_ms = now();
    // Keep the authority assertion as the first database operation after
    // BEGIN IMMEDIATE and the transaction-time sample so a stale process
    // cannot inspect worker-owned state.
    assert_worker_transition_authority_in_transaction(&tx, authority, now_ms)?;
    let job = get_job_in_transaction(&tx, job_id)?;
    if job.status != GenerationJobStatus::Running
        || !matches!(
            job.stage,
            GenerationJobStage::Preparing
                | GenerationJobStage::ProviderRequest
                | GenerationJobStage::RetryBackoff
                | GenerationJobStage::ResponseReady
                | GenerationJobStage::LocalProcessing
        )
    {
        return Err(AppError::GenerationJobInvalidTransition.into());
    }
    let cancel_requested = job.cancel_requested_at.is_some();
    tx.commit()
        .map_err(|error| database_error("Commit generation cancellation reread failed", error))?;
    Ok(cancel_requested)
}

pub(crate) fn reserve_automatic_retry_with_event(
    conn: &Connection,
    job_id: &str,
    expected_auto_attempt: i32,
    authority: &WorkerTransitionAuthority,
    now_ms: i64,
) -> Result<GenerationJobTransition<GenerationJob>, WorkerTransitionError> {
    reserve_automatic_retry_with_event_with_transaction_time(
        conn,
        job_id,
        expected_auto_attempt,
        authority,
        || now_ms,
    )
}

/// Samples worker time exactly once after the immediate transaction is held.
/// `now` must not access the database.
pub(crate) fn reserve_automatic_retry_with_event_with_transaction_time<Now>(
    conn: &Connection,
    job_id: &str,
    expected_auto_attempt: i32,
    authority: &WorkerTransitionAuthority,
    now: Now,
) -> Result<GenerationJobTransition<GenerationJob>, WorkerTransitionError>
where
    Now: FnOnce() -> i64,
{
    let tx = begin_generation_job_write_transaction_unchecked(conn)?;
    let now_ms = now();
    let timestamp = canonical_worker_timestamp(now_ms)?;
    assert_worker_transition_authority_in_transaction(&tx, authority, now_ms)?;
    let current = get_job_in_transaction(&tx, job_id)?;
    if current.status != GenerationJobStatus::Running
        || current.stage != GenerationJobStage::RetryBackoff
        || current.cancel_requested_at.is_some()
        || current.auto_attempt != expected_auto_attempt
    {
        return Err(AppError::GenerationJobInvalidTransition.into());
    }
    if !current
        .started_at
        .as_deref()
        .is_some_and(|started_at| timestamp_is_not_before(&timestamp, started_at))
        || !current
            .last_heartbeat_at
            .as_deref()
            .is_some_and(|heartbeat_at| timestamp_is_not_before(&timestamp, heartbeat_at))
    {
        return Err(AppError::GenerationJobInvalidTransition.into());
    }
    validate_worker_recovery_for_stage(&tx, &current.generation_id, current.stage)?;
    let next_auto_attempt = current
        .auto_attempt
        .checked_add(1)
        .ok_or(AppError::GenerationJobInvalidTransition)?;
    if next_auto_attempt > current.max_auto_attempts {
        return Err(AppError::GenerationJobInvalidTransition.into());
    }

    let updated = tx
        .execute(
            "UPDATE generation_jobs
                SET auto_attempt = ?1, stage = 'provider_request', last_heartbeat_at = ?2
              WHERE id = ?3 AND status = 'running' AND stage = 'retry_backoff'
                AND auto_attempt = ?4 AND max_auto_attempts >= ?1
                AND cancel_requested_at IS NULL
                AND started_at IS NOT NULL AND started_at <= ?2
                AND last_heartbeat_at IS NOT NULL
                AND last_heartbeat_at <= ?2",
            params![next_auto_attempt, timestamp, job_id, expected_auto_attempt],
        )
        .map_err(|error| database_error("Reserve automatic generation retry failed", error))?;
    if updated != 1 {
        return Err(AppError::GenerationJobInvalidTransition.into());
    }
    validate_worker_recovery_for_stage(
        &tx,
        &current.generation_id,
        GenerationJobStage::ProviderRequest,
    )?;
    let job = get_job_in_transaction(&tx, job_id)?;
    let event = get_job_event_in_transaction(&tx, job_id)?;
    tx.commit()
        .map_err(|error| database_error("Commit automatic generation retry failed", error))?;
    Ok(GenerationJobTransition {
        value: job,
        event: Some(event),
    })
}

pub(crate) fn claim_next_job(conn: &mut Connection) -> Result<Option<GenerationJob>, AppError> {
    claim_next_job_with_event(conn)
        .map(|transition| transition.map(GenerationJobTransition::into_value))
}

pub(crate) fn claim_job_in_transaction(
    tx: &Transaction<'_>,
    job_id: &str,
) -> Result<GenerationJob, AppError> {
    let timestamp = crate::current_timestamp();
    claim_job_at_in_transaction(tx, job_id, &timestamp)
}

fn claim_job_at_in_transaction(
    tx: &Transaction<'_>,
    job_id: &str,
    timestamp: &str,
) -> Result<GenerationJob, AppError> {
    if !canonical_timestamp(timestamp) {
        return Err(AppError::GenerationJobInvalidSnapshot);
    }
    let candidate = get_job_in_transaction(tx, job_id)?;
    if candidate.status != GenerationJobStatus::Queued
        || candidate.stage != GenerationJobStage::Queued
        || !timestamp_is_not_before(timestamp, &candidate.queued_at)
    {
        return Err(AppError::GenerationJobInvalidTransition);
    }
    let generation_id = candidate.generation_id;
    let updated_job = tx
        .execute(
            "UPDATE generation_jobs
             SET status = 'running', stage = 'preparing', started_at = ?1,
                 last_heartbeat_at = ?1
             WHERE id = ?2 AND status = 'queued' AND stage = 'queued'
               AND queued_at <= ?1",
            params![timestamp, job_id],
        )
        .map_err(|error| database_error("Claim generation job failed", error))?;
    if updated_job != 1 {
        return Err(AppError::GenerationJobInvalidTransition);
    }
    let updated_generation = tx
        .execute(
            "UPDATE generations SET status = 'running', error_message = NULL
             WHERE id = ?1 AND status = 'queued'",
            params![generation_id],
        )
        .map_err(|error| database_error("Claim generation record failed", error))?;
    if updated_generation != 1 {
        return Err(AppError::GenerationJobCorruptPersistedData);
    }
    get_job_in_transaction(tx, job_id)
}

pub(crate) fn load_generation_execution_snapshot(
    conn: &Connection,
    job_id: &str,
) -> Result<GenerationExecutionSnapshot, AppError> {
    let tx = conn.unchecked_transaction().map_err(|error| {
        database_error(
            "Begin generation execution snapshot transaction failed",
            error,
        )
    })?;
    let snapshot = load_generation_execution_snapshot_in_transaction(&tx, job_id)?;
    tx.commit().map_err(|error| {
        database_error(
            "Commit generation execution snapshot transaction failed",
            error,
        )
    })?;
    Ok(snapshot)
}

pub(crate) fn load_generation_execution_snapshot_in_transaction(
    tx: &Transaction<'_>,
    job_id: &str,
) -> Result<GenerationExecutionSnapshot, AppError> {
    load_generation_execution_snapshot_for_stage_in_transaction(
        tx,
        job_id,
        GenerationJobStage::Preparing,
    )
}

pub(crate) fn load_generation_execution_snapshot_for_stage_in_transaction(
    tx: &Transaction<'_>,
    job_id: &str,
    expected_stage: GenerationJobStage,
) -> Result<GenerationExecutionSnapshot, AppError> {
    let job = get_job_in_transaction(tx, job_id)?;
    if job.status != GenerationJobStatus::Running || job.stage != expected_stage {
        return Err(AppError::GenerationJobInvalidTransition);
    }
    let request: GenerationJobRequest = serde_json::from_value(job.request.clone())
        .map_err(|_| AppError::GenerationJobCorruptPersistedData)?;
    let generation = load_linked_generation(tx, &job.generation_id)?;
    let metadata: CanonicalGenerationMetadata = generation
        .request_metadata_json
        .as_deref()
        .ok_or(AppError::GenerationJobCorruptPersistedData)
        .and_then(|value| {
            serde_json::from_str(value).map_err(|_| AppError::GenerationJobCorruptPersistedData)
        })?;
    let runtime_options = GptImageRequestOptions {
        size: generation.size.clone(),
        quality: generation.quality.clone(),
        background: generation.background.clone(),
        output_format: generation.output_format.clone(),
        output_compression: u8::try_from(generation.output_compression)
            .map_err(|_| AppError::GenerationJobCorruptPersistedData)?,
        moderation: generation.moderation.clone(),
        input_fidelity: generation.input_fidelity.clone(),
        stream: metadata.stream,
        partial_images: metadata.partial_images,
        image_count: u8::try_from(generation.image_count)
            .map_err(|_| AppError::GenerationJobCorruptPersistedData)?,
    };
    Ok(GenerationExecutionSnapshot {
        context: GenerationExecutionContext {
            generation_id: job.generation_id,
            job_id: job.id,
            conversation_id: request.conversation_id.clone(),
            provider_kind: job.provider_kind,
            model: generation.model,
            endpoint_url: job.endpoint_snapshot,
            provider_profile_id: job.provider_profile_id,
        },
        request,
        runtime_options,
        created_at: generation.created_at,
        output_format: generation.output_format,
    })
}

/// Records the actual number of persisted images without weakening the typed
/// canonical metadata contract. Callers must insert every image row first and
/// keep the surrounding transaction open until the terminal job transition
/// validates the completed projection.
pub(crate) fn set_actual_image_count_in_transaction(
    tx: &Transaction<'_>,
    generation_id: &str,
    actual_image_count: u8,
) -> Result<(), AppError> {
    let (status, requested_image_count, raw_metadata): (String, i32, String) = tx
        .query_row(
            "SELECT status, image_count, request_metadata
             FROM generations WHERE id = ?1",
            params![generation_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()
        .map_err(|error| persisted_row_error("Read generation metadata failed", error))?
        .ok_or(AppError::GenerationJobCorruptPersistedData)?;
    let mut metadata: CanonicalGenerationMetadata = serde_json::from_str(&raw_metadata)
        .map_err(|_| AppError::GenerationJobCorruptPersistedData)?;
    let requested_image_count = u8::try_from(requested_image_count)
        .map_err(|_| AppError::GenerationJobCorruptPersistedData)?;
    if status != "running"
        || metadata.image_count != requested_image_count
        || metadata.actual_image_count.is_some()
        || !(1..=requested_image_count).contains(&actual_image_count)
    {
        return Err(AppError::GenerationJobInvalidSnapshot);
    }
    let stored_image_count: i64 = tx
        .query_row(
            "SELECT COUNT(*) FROM images WHERE generation_id = ?1",
            params![generation_id],
            |row| row.get(0),
        )
        .map_err(|error| database_error("Count completed generation images failed", error))?;
    if stored_image_count != i64::from(actual_image_count) {
        return Err(AppError::GenerationJobInvalidSnapshot);
    }

    metadata.actual_image_count = Some(actual_image_count);
    let encoded_metadata =
        serde_json::to_string(&metadata).map_err(|_| AppError::GenerationJobInvalidSnapshot)?;
    let updated = tx
        .execute(
            "UPDATE generations SET request_metadata = ?1
             WHERE id = ?2 AND status = 'running' AND request_metadata = ?3",
            params![encoded_metadata, generation_id, raw_metadata],
        )
        .map_err(|error| database_error("Record actual generation image count failed", error))?;
    if updated != 1 {
        return Err(AppError::GenerationJobInvalidTransition);
    }
    Ok(())
}

pub(crate) fn request_cancel_with_event(
    conn: &Connection,
    id: &str,
) -> Result<GenerationJobTransition<GenerationJob>, AppError> {
    let tx = begin_generation_job_write_transaction_unchecked(conn)?;
    let current = get_job_in_transaction(&tx, id)?;
    let timestamp = crate::current_timestamp();
    let changed = match current.status {
        GenerationJobStatus::Queued => {
            let updated_job = tx
                .execute(
                    "UPDATE generation_jobs
                     SET status = 'cancelled', stage = 'terminal', cancel_requested_at = ?1,
                         finished_at = ?1, error_code = 'cancelled_by_user', error_message = ?2,
                         retryable = 0
                     WHERE id = ?3 AND status = 'queued' AND stage = 'queued'",
                    params![
                        timestamp,
                        terminal_message_for_code("cancelled_by_user"),
                        id
                    ],
                )
                .map_err(|error| database_error("Cancel queued generation job failed", error))?;
            if updated_job != 1 {
                return Err(AppError::GenerationJobInvalidTransition);
            }
            let updated_generation = tx
                .execute(
                    "UPDATE generations SET status = 'cancelled', error_message = ?2
                     WHERE id = ?1 AND status = 'queued'",
                    params![
                        current.generation_id,
                        terminal_message_for_code("cancelled_by_user")
                    ],
                )
                .map_err(|error| database_error("Cancel queued generation failed", error))?;
            if updated_generation != 1 {
                return Err(AppError::GenerationJobCorruptPersistedData);
            }
            let deleted_recovery = tx
                .execute(
                    "DELETE FROM generation_recoveries
                     WHERE generation_id = ?1 AND request_state = 'requesting'
                       AND response_file IS NULL",
                    params![current.generation_id],
                )
                .map_err(|error| {
                    database_error("Delete cancelled generation recovery failed", error)
                })?;
            if deleted_recovery != 1 {
                return Err(AppError::GenerationJobCorruptPersistedData);
            }
            true
        }
        GenerationJobStatus::Running => {
            if current.cancel_requested_at.is_some() {
                false
            } else {
                let updated = tx
                    .execute(
                        "UPDATE generation_jobs
                         SET cancel_requested_at = ?1
                         WHERE id = ?2 AND status = 'running'
                           AND cancel_requested_at IS NULL",
                        params![timestamp, id],
                    )
                    .map_err(|error| {
                        database_error("Request generation job cancellation failed", error)
                    })?;
                if updated != 1 {
                    return Err(AppError::GenerationJobInvalidTransition);
                }
                true
            }
        }
        _ => return Err(AppError::GenerationJobInvalidTransition),
    };
    let job = get_job_in_transaction(&tx, id)?;
    let event = if changed {
        Some(get_job_event_in_transaction(&tx, id)?)
    } else {
        None
    };
    tx.commit()
        .map_err(|error| database_error("Commit job cancellation failed", error))?;
    Ok(GenerationJobTransition { value: job, event })
}

pub(crate) fn request_cancel(conn: &Connection, id: &str) -> Result<GenerationJob, AppError> {
    request_cancel_with_event(conn, id).map(GenerationJobTransition::into_value)
}

pub(crate) fn finish_job_in_transaction(
    tx: &Transaction<'_>,
    update: &GenerationJobTerminalUpdate,
) -> Result<GenerationJob, AppError> {
    if !is_terminal(&update.status)
        || !can_transition(update.expected_status.clone(), update.status.clone())
        || !canonical_timestamp(&update.finished_at)
    {
        return Err(AppError::GenerationJobInvalidTransition);
    }
    let terminal_fields_valid = match update.status {
        GenerationJobStatus::Completed => {
            update.error_code.is_none() && update.error_message.is_none() && !update.retryable
        }
        GenerationJobStatus::Failed | GenerationJobStatus::Interrupted => {
            update.error_code.as_deref().is_some_and(safe_error_code)
        }
        GenerationJobStatus::Cancelled => !update.retryable,
        _ => false,
    };
    if !terminal_fields_valid {
        return Err(AppError::GenerationJobInvalidSnapshot);
    }
    let persisted_error_code = match update.status {
        GenerationJobStatus::Completed => None,
        GenerationJobStatus::Cancelled => Some("cancelled_by_user"),
        GenerationJobStatus::Failed | GenerationJobStatus::Interrupted => {
            update.error_code.as_deref()
        }
        _ => None,
    };
    let persisted_error_message = persisted_error_code.map(terminal_message_for_code);
    let current = tx
        .query_row(
            "SELECT generation_id, status, stage FROM generation_jobs WHERE id = ?1",
            params![update.job_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            },
        )
        .optional()
        .map_err(|error| persisted_row_error("Read terminal generation job failed", error))?
        .ok_or(AppError::GenerationJobNotFound)?;
    if parse_status(&current.1)? != update.expected_status {
        return Err(AppError::GenerationJobInvalidTransition);
    }
    let current_stage = parse_stage(&current.2)?;
    if current_stage == GenerationJobStage::MigrationUnknown
        || !status_stage_valid(&update.expected_status, current_stage)
    {
        return Err(AppError::GenerationJobInvalidTransition);
    }
    if !source_reference_id_valid(&current.0) {
        return Err(AppError::GenerationJobCorruptPersistedData);
    }
    let generation_id = current.0;

    let updated = tx
        .execute(
            "UPDATE generation_jobs
             SET status = ?1, stage = 'terminal', finished_at = ?2, error_code = ?3,
                 error_message = ?4, retryable = ?5
             WHERE id = ?6 AND status = ?7
               AND stage = ?8
               AND ((?1 = 'cancelled' AND cancel_requested_at IS NOT NULL)
                    OR (?1 <> 'cancelled' AND cancel_requested_at IS NULL))",
            params![
                status_as_str(&update.status),
                update.finished_at,
                persisted_error_code,
                persisted_error_message,
                i64::from(update.retryable),
                update.job_id,
                status_as_str(&update.expected_status),
                stage_as_str(current_stage),
            ],
        )
        .map_err(|error| database_error("Finish generation job failed", error))?;
    if updated != 1 {
        let exists = tx
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM generation_jobs WHERE id = ?1)",
                params![update.job_id],
                |row| row.get::<_, i64>(0),
            )
            .map_err(|error| database_error("Check generation job failed", error))?;
        return Err(if exists == 0 {
            AppError::GenerationJobNotFound
        } else {
            AppError::GenerationJobInvalidTransition
        });
    }

    let updated_generation = tx
        .execute(
            "UPDATE generations
             SET status = ?1, error_message = ?2
             WHERE id = ?3 AND status = ?4",
            params![
                status_as_str(&update.status),
                persisted_error_message,
                generation_id,
                status_as_str(&update.expected_status),
            ],
        )
        .map_err(|error| database_error("Finish generation record failed", error))?;
    if updated_generation != 1 {
        return Err(AppError::GenerationJobCorruptPersistedData);
    }
    get_job_in_transaction(tx, &update.job_id)
}

pub(crate) fn finish_job_with_event(
    conn: &Connection,
    update: &GenerationJobTerminalUpdate,
) -> Result<GenerationJobTransition<GenerationJob>, AppError> {
    let tx = begin_generation_job_write_transaction_unchecked(conn)?;
    let job = finish_job_in_transaction(&tx, update)?;
    let event = get_job_event_in_transaction(&tx, &update.job_id)?;
    tx.commit()
        .map_err(|error| database_error("Commit job finish failed", error))?;
    Ok(GenerationJobTransition {
        value: job,
        event: Some(event),
    })
}

pub(crate) fn finish_job(
    conn: &Connection,
    update: &GenerationJobTerminalUpdate,
) -> Result<GenerationJob, AppError> {
    finish_job_with_event(conn, update).map(GenerationJobTransition::into_value)
}

fn load_retry_generation(
    conn: &Connection,
    generation_id: &str,
) -> Result<RetryGenerationSnapshot, AppError> {
    let raw = conn
        .query_row(
            "SELECT prompt, engine, request_kind, size, quality, background, output_format,
                    output_compression, moderation, input_fidelity, image_count,
                    source_image_paths, request_metadata, status, conversation_id
             FROM generations WHERE id = ?1",
            params![generation_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, i32>(7)?,
                    row.get::<_, String>(8)?,
                    row.get::<_, String>(9)?,
                    row.get::<_, i32>(10)?,
                    row.get::<_, String>(11)?,
                    row.get::<_, Option<String>>(12)?,
                    row.get::<_, String>(13)?,
                    row.get::<_, Option<String>>(14)?,
                ))
            },
        )
        .optional()
        .map_err(|error| persisted_row_error("Read retry generation failed", error))?
        .ok_or(AppError::GenerationJobCorruptPersistedData)?;
    let source_image_paths = serde_json::from_str::<Vec<String>>(&raw.11)
        .map_err(|_| AppError::GenerationJobCorruptPersistedData)?;
    let request_metadata = raw
        .12
        .as_deref()
        .ok_or(AppError::GenerationJobCorruptPersistedData)
        .and_then(|value| {
            serde_json::from_str::<CanonicalGenerationMetadata>(value)
                .map_err(|_| AppError::GenerationJobCorruptPersistedData)
        })?;
    let conversation_id = raw
        .14
        .filter(|conversation_id| nonempty(conversation_id))
        .ok_or(AppError::GenerationJobCorruptPersistedData)?;
    let linked_conversation_id = live_conversation_id(conn, &conversation_id)?;
    let metadata_matches_generation = request_metadata.request_kind.as_str() == raw.2
        && request_metadata.conversation_id == linked_conversation_id
        && source_reference_id_valid(&request_metadata.project_id)
        && request_metadata.model == raw.1
        && request_metadata.size == raw.3
        && request_metadata.quality == raw.4
        && request_metadata.background == raw.5
        && request_metadata.output_format == raw.6
        && i32::from(request_metadata.output_compression) == raw.7
        && request_metadata.moderation == raw.8
        && request_metadata.input_fidelity == raw.9
        && i32::from(request_metadata.image_count) == raw.10
        && request_metadata.source_image_count == source_image_paths.len();
    if !metadata_matches_generation {
        return Err(AppError::GenerationJobCorruptPersistedData);
    }
    Ok(RetryGenerationSnapshot {
        prompt: raw.0,
        model: raw.1,
        request_kind: raw.2,
        size: raw.3,
        quality: raw.4,
        background: raw.5,
        output_format: raw.6,
        output_compression: raw.7,
        moderation: raw.8,
        input_fidelity: raw.9,
        image_count: raw.10,
        stream: request_metadata.stream,
        partial_images: request_metadata.partial_images,
        source_image_paths,
        status: raw.13,
        conversation_id,
        project_id: request_metadata.project_id,
    })
}

fn retry_existing_result_in_transaction(
    tx: &Transaction<'_>,
    parent_id: &str,
    existing: &GenerationJob,
    source_ref_override: Option<&Value>,
) -> Result<EnqueueGenerationResult, AppError> {
    if existing.parent_job_id.as_deref() != Some(parent_id) {
        return Err(AppError::GenerationJobIdempotencyConflict);
    }
    let parent = get_job_in_transaction(tx, parent_id)?;
    if !matches!(parent.source_kind.as_str(), "generate" | "edit") && source_ref_override.is_none()
    {
        return Err(AppError::GenerationJobUnsupportedSource);
    }
    let parent_request: GenerationJobRequest = serde_json::from_value(parent.request.clone())
        .map_err(|_| AppError::GenerationJobCorruptPersistedData)?;
    let parent_identity = ResolvedConversationIdentity {
        conversation_id: parent_request.conversation_id.clone(),
        project_id: parent_request.project_id.clone(),
    };
    let expected_source_ref = if let Some(source_ref_override) = source_ref_override {
        canonical_source_ref(
            &parent.source_kind,
            source_ref_override,
            Some(&parent_identity),
        )
        .map_err(|_| AppError::GenerationJobIdempotencyConflict)?
    } else {
        parent.source_ref.clone()
    };
    if existing.source_ref != expected_source_ref {
        return Err(AppError::GenerationJobIdempotencyConflict);
    }
    let expected_chain_attempt = parent
        .chain_attempt
        .checked_add(1)
        .ok_or(AppError::GenerationJobCorruptPersistedData)?;
    if existing.source_kind != parent.source_kind
        || existing.request != parent.request
        || existing.provider_kind != parent.provider_kind
        || existing.provider_profile_id != parent.provider_profile_id
        || existing.endpoint_snapshot != parent.endpoint_snapshot
        || existing.chain_attempt != expected_chain_attempt
        || existing.max_auto_attempts != parent.max_auto_attempts
    {
        return Err(AppError::GenerationJobCorruptPersistedData);
    }
    enqueue_result_for_validated_job(existing)
}

pub(crate) fn insert_retry_job_in_transaction(
    tx: &Transaction<'_>,
    parent_id: &str,
    client_request_id: &str,
    source_ref_override: Option<&Value>,
) -> Result<EnqueueGenerationResult, AppError> {
    if let Some(existing) = find_job_by_client_request_id_in_transaction(tx, client_request_id)? {
        return retry_existing_result_in_transaction(tx, parent_id, &existing, source_ref_override);
    }
    let parent = get_job_in_transaction(tx, parent_id)?;
    if !matches!(parent.source_kind.as_str(), "generate" | "edit") && source_ref_override.is_none()
    {
        return Err(AppError::GenerationJobUnsupportedSource);
    }
    if !parent.retryable
        || !matches!(
            parent.status,
            GenerationJobStatus::Failed | GenerationJobStatus::Interrupted
        )
    {
        return Err(AppError::GenerationJobNotRetryable);
    }
    let generation = load_retry_generation(tx, &parent.generation_id)?;
    if generation.status != status_as_str(&parent.status) {
        return Err(AppError::GenerationJobCorruptPersistedData);
    }
    let chain_attempt = parent
        .chain_attempt
        .checked_add(1)
        .ok_or(AppError::GenerationJobCorruptPersistedData)?;
    let queued_at = crate::current_timestamp();
    let parent_request: GenerationJobRequest = serde_json::from_value(parent.request.clone())
        .map_err(|_| AppError::GenerationJobCorruptPersistedData)?;
    if parent_request.conversation_id != generation.conversation_id
        || parent_request.project_id != generation.project_id
        || parent_request.prompt != generation.prompt
        || parent_request.model != generation.model
        || parent_request.source_image_paths != generation.source_image_paths
        || parent_request.kind.as_str() != generation.request_kind
    {
        return Err(AppError::GenerationJobCorruptPersistedData);
    }
    let request = PreparedGenerationJob {
        job_id: uuid::Uuid::new_v4().to_string(),
        client_request_id: client_request_id.to_string(),
        generation_id: uuid::Uuid::new_v4().to_string(),
        requested_conversation_id: parent_request.requested_conversation_id.clone(),
        requested_project_id: parent_request.requested_project_id.clone(),
        prompt: generation.prompt,
        model: generation.model,
        request_kind: generation.request_kind,
        size: generation.size,
        quality: generation.quality,
        background: generation.background,
        output_format: generation.output_format,
        output_compression: generation.output_compression,
        moderation: generation.moderation,
        input_fidelity: generation.input_fidelity,
        image_count: generation.image_count,
        stream: generation.stream,
        partial_images: generation.partial_images,
        source_image_paths: generation.source_image_paths,
        request_options: parent_request.options.clone(),
        parent_job_id: Some(parent.id.clone()),
        source_kind: parent.source_kind,
        source_ref: source_ref_override
            .cloned()
            .unwrap_or_else(|| parent.source_ref.clone()),
        provider_kind: parent.provider_kind,
        provider_profile_id: parent.provider_profile_id,
        endpoint_snapshot: parent.endpoint_snapshot,
        status: GenerationJobStatus::Queued,
        chain_attempt,
        auto_attempt: 0,
        max_auto_attempts: parent.max_auto_attempts,
        queued_at,
        finished_at: None,
        error_code: None,
        error_message: None,
        retryable: false,
    };
    validate_prepared_job(&request)?;
    let identity = ResolvedConversationIdentity {
        conversation_id: generation.conversation_id,
        project_id: generation.project_id,
    };
    insert_prepared_rows_in_transaction(tx, &request, &identity)
}

pub(crate) fn create_retry_job_with_event(
    conn: &mut Connection,
    parent_id: &str,
    client_request_id: &str,
) -> Result<GenerationJobTransition<EnqueueGenerationResult>, AppError> {
    let existing_result = with_generation_job_read_transaction(conn, "find retry", |tx| {
        find_job_by_client_request_id_in_transaction(tx, client_request_id)?
            .as_ref()
            .map(|existing| retry_existing_result_in_transaction(tx, parent_id, existing, None))
            .transpose()
    })?;
    if let Some(existing_result) = existing_result {
        return Ok(GenerationJobTransition {
            value: existing_result,
            event: None,
        });
    }
    let tx = begin_generation_job_write_transaction(conn)?;
    let (value, event) = if let Some(existing) =
        find_job_by_client_request_id_in_transaction(&tx, client_request_id)?
    {
        (
            retry_existing_result_in_transaction(&tx, parent_id, &existing, None)?,
            None,
        )
    } else {
        let value = insert_retry_job_in_transaction(&tx, parent_id, client_request_id, None)?;
        let event = get_job_event_in_transaction(&tx, &value.job_id)?;
        (value, Some(event))
    };
    tx.commit()
        .map_err(|error| database_error("Commit retry transaction failed", error))?;
    Ok(GenerationJobTransition { value, event })
}

pub(crate) fn create_retry_job(
    conn: &mut Connection,
    parent_id: &str,
    client_request_id: &str,
) -> Result<EnqueueGenerationResult, AppError> {
    create_retry_job_with_event(conn, parent_id, client_request_id)
        .map(GenerationJobTransition::into_value)
}

#[cfg(test)]
#[path = "generation_jobs_tests.rs"]
mod tests;
