use crate::commands::conversations;
use crate::error::AppError;
use crate::models::{
    EnqueueGenerationResult, GenerationJob, GenerationJobFilter, GenerationJobPage,
    GenerationJobStatus, DEFAULT_GENERATION_JOB_PAGE_LIMIT, MAX_GENERATION_JOB_PAGE_LIMIT,
};
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use rusqlite::types::Value as SqlValue;
use rusqlite::{params, params_from_iter, Connection, OptionalExtension, Row, Transaction};
use serde::{Deserialize, Serialize};
use serde_json::Value;

const JOB_COLUMNS: &str = "id, client_request_id, generation_id, parent_job_id, source_kind,
    source_ref_json, status, request_json, provider_kind, provider_profile_id, endpoint_snapshot,
    chain_attempt, auto_attempt, max_auto_attempts, queued_at, started_at, finished_at,
    cancel_requested_at, last_heartbeat_at, error_code, error_message, retryable";

const ALIASED_JOB_COLUMNS: &str = "g.id, g.client_request_id, g.generation_id, g.parent_job_id,
    g.source_kind, g.source_ref_json, g.status, g.request_json, g.provider_kind,
    g.provider_profile_id, g.endpoint_snapshot, g.chain_attempt, g.auto_attempt,
    g.max_auto_attempts, g.queued_at, g.started_at, g.finished_at, g.cancel_requested_at,
    g.last_heartbeat_at, g.error_code, g.error_message, g.retryable";

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
struct StoredGenerationJob {
    id: String,
    client_request_id: String,
    generation_id: String,
    parent_job_id: Option<String>,
    source_kind: String,
    source_ref_json: String,
    status: String,
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
        request_json: row.get(7)?,
        provider_kind: row.get(8)?,
        provider_profile_id: row.get(9)?,
        endpoint_snapshot: row.get(10)?,
        chain_attempt: row.get(11)?,
        auto_attempt: row.get(12)?,
        max_auto_attempts: row.get(13)?,
        queued_at: row.get(14)?,
        started_at: row.get(15)?,
        finished_at: row.get(16)?,
        cancel_requested_at: row.get(17)?,
        last_heartbeat_at: row.get(18)?,
        error_code: row.get(19)?,
        error_message: row.get(20)?,
        retryable: row.get(21)?,
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
    let executable_provider = stored.provider_kind != "unresolved"
        && stored.provider_profile_id != "unresolved"
        && nonempty(&stored.provider_kind)
        && nonempty(&stored.provider_profile_id)
        && nonempty(&stored.endpoint_snapshot)
        && endpoint_snapshot_is_public(&stored.endpoint_snapshot);
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
            || (stored.provider_kind != "unresolved"
                && stored.provider_profile_id != "unresolved"
                && nonempty(&stored.provider_kind)
                && nonempty(&stored.provider_profile_id)
                && nonempty(&stored.endpoint_snapshot)
                && endpoint_snapshot_is_public(&stored.endpoint_snapshot)));
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

fn query_job(
    conn: &Connection,
    predicate: &str,
    value: &str,
) -> Result<Option<GenerationJob>, AppError> {
    let sql = format!("SELECT {JOB_COLUMNS} FROM generation_jobs WHERE {predicate} = ?1");
    let stored = conn
        .query_row(&sql, params![value], stored_job_from_row)
        .optional()
        .map_err(|error| persisted_row_error("Read generation job failed", error))?;
    let job = stored.map(decode_stored_job).transpose()?;
    if let Some(job) = job.as_ref() {
        validate_job_projection(conn, job)?;
    }
    Ok(job)
}

pub(crate) fn get_job(conn: &Connection, id: &str) -> Result<GenerationJob, AppError> {
    query_job(conn, "id", id)?.ok_or(AppError::GenerationJobNotFound)
}

pub(crate) fn find_job_by_client_request_id(
    conn: &Connection,
    client_request_id: &str,
) -> Result<Option<GenerationJob>, AppError> {
    query_job(conn, "client_request_id", client_request_id)
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

fn conversation_id_for_generation(
    conn: &Connection,
    generation_id: &str,
) -> Result<String, AppError> {
    let conversation = conn
        .query_row(
            "SELECT conversation_id, typeof(conversation_id) FROM generations WHERE id = ?1",
            params![generation_id],
            |row| Ok((row.get::<_, Option<String>>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()
        .map_err(|error| persisted_row_error("Read job conversation failed", error))?
        .ok_or(AppError::GenerationJobCorruptPersistedData)?;
    if conversation.1 != "text" {
        return Err(AppError::GenerationJobCorruptPersistedData);
    }
    let conversation_id = conversation
        .0
        .filter(|conversation_id| nonempty(conversation_id))
        .ok_or(AppError::GenerationJobCorruptPersistedData)?;
    live_conversation_id(conn, &conversation_id)
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
        |row| {
            Ok(LinkedGenerationSnapshot {
                prompt: row.get(0)?,
                model: row.get(1)?,
                request_kind: row.get(2)?,
                size: row.get(3)?,
                quality: row.get(4)?,
                background: row.get(5)?,
                output_format: row.get(6)?,
                output_compression: row.get(7)?,
                moderation: row.get(8)?,
                input_fidelity: row.get(9)?,
                image_count: row.get(10)?,
                source_image_count: row.get(11)?,
                source_image_paths_json: row.get(12)?,
                request_metadata_json: row.get(13)?,
                status: row.get(14)?,
                error_message: row.get(15)?,
                conversation_id: row.get(16)?,
                created_at: row.get(17)?,
                deleted_at: row.get(18)?,
            })
        },
    )
    .optional()
    .map_err(|error| persisted_row_error("Read linked generation failed", error))?
    .ok_or(AppError::GenerationJobCorruptPersistedData)
}

fn load_linked_recovery(
    conn: &Connection,
    generation_id: &str,
) -> Result<Option<LinkedRecoverySnapshot>, AppError> {
    conn.query_row(
        "SELECT request_kind, request_state, output_format, response_file, created_at, updated_at
         FROM generation_recoveries WHERE generation_id = ?1",
        params![generation_id],
        |row| {
            Ok(LinkedRecoverySnapshot {
                request_kind: row.get(0)?,
                request_state: row.get(1)?,
                output_format: row.get(2)?,
                response_file: row.get(3)?,
                created_at: row.get(4)?,
                updated_at: row.get(5)?,
            })
        },
    )
    .optional()
    .map_err(|error| persisted_row_error("Read linked generation recovery failed", error))
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

fn validate_job_projection(conn: &Connection, job: &GenerationJob) -> Result<(), AppError> {
    let request: GenerationJobRequest = serde_json::from_value(job.request.clone())
        .map_err(|_| AppError::GenerationJobCorruptPersistedData)?;
    let generation = load_linked_generation(conn, &job.generation_id)?;
    let conversation_id = generation
        .conversation_id
        .as_deref()
        .filter(|value| nonempty(value) && public_string_is_safe(value))
        .ok_or(AppError::GenerationJobCorruptPersistedData)?;
    let linked_conversation_id = live_conversation_id(conn, conversation_id)?;
    let source_image_paths: Vec<String> = serde_json::from_str(&generation.source_image_paths_json)
        .map_err(|_| AppError::GenerationJobCorruptPersistedData)?;
    let metadata: CanonicalGenerationMetadata = generation
        .request_metadata_json
        .as_deref()
        .ok_or(AppError::GenerationJobCorruptPersistedData)
        .and_then(|value| {
            serde_json::from_str(value).map_err(|_| AppError::GenerationJobCorruptPersistedData)
        })?;
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
        && metadata.conversation_id == linked_conversation_id
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
        && request
            .options
            .stream
            .is_none_or(|value| metadata.stream == value)
        && request
            .options
            .partial_images
            .is_none_or(|value| metadata.partial_images == value);
    let request_matches = request.conversation_id == linked_conversation_id
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
    if !public_generation_fields || !metadata_matches || !request_matches || !generation_matches {
        return Err(AppError::GenerationJobCorruptPersistedData);
    }

    let recovery = load_linked_recovery(conn, &job.generation_id)?;
    let recovery_valid = match job.status {
        GenerationJobStatus::Queued => recovery.as_ref().is_some_and(|recovery| {
            recovery.request_state == "requesting"
                && recovery.updated_at == job.queued_at
                && recovery_matches_job(recovery, job, &request, &generation)
        }),
        GenerationJobStatus::Running => recovery
            .as_ref()
            .is_some_and(|recovery| recovery_matches_job(recovery, job, &request, &generation)),
        GenerationJobStatus::Failed if job.started_at.is_none() => recovery.is_none(),
        GenerationJobStatus::Cancelled if job.started_at.is_none() => recovery.is_none(),
        GenerationJobStatus::Completed
        | GenerationJobStatus::Failed
        | GenerationJobStatus::Cancelled
        | GenerationJobStatus::Interrupted => recovery
            .as_ref()
            .is_none_or(|recovery| recovery_matches_job(recovery, job, &request, &generation)),
    };
    if recovery_valid {
        Ok(())
    } else {
        Err(AppError::GenerationJobCorruptPersistedData)
    }
}

fn enqueue_result_for_job(
    conn: &Connection,
    job: &GenerationJob,
) -> Result<EnqueueGenerationResult, AppError> {
    let request: GenerationJobRequest = serde_json::from_value(job.request.clone())
        .map_err(|_| AppError::GenerationJobCorruptPersistedData)?;
    let conversation_id = conversation_id_for_generation(conn, &job.generation_id)?;
    if request.conversation_id != conversation_id {
        return Err(AppError::GenerationJobCorruptPersistedData);
    }
    Ok(EnqueueGenerationResult {
        job_id: job.id.clone(),
        generation_id: job.generation_id.clone(),
        conversation_id,
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
    find_job_by_client_request_id(conn, client_request_id)?
        .as_ref()
        .map(|job| enqueue_result_for_job(conn, job))
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

fn has_bearer_token(value: &str) -> bool {
    let words = value.split_whitespace().collect::<Vec<_>>();
    words.windows(2).any(|pair| {
        let marker = pair[0].trim_matches(|character: char| !character.is_ascii_alphanumeric());
        if !marker.eq_ignore_ascii_case("bearer") {
            return false;
        }
        let candidate = pair[1].trim_matches(|character: char| {
            !character.is_ascii_alphanumeric() && !matches!(character, '-' | '_' | '.')
        });
        contains_credential_token(candidate) || candidate.len() >= 8
    })
}

fn contains_credential_token(value: &str) -> bool {
    has_prefixed_token(value, "sk-", 6)
        || has_prefixed_token(value, "sk_", 6)
        || has_prefixed_token(value, "ghp_", 8)
        || has_prefixed_token(value, "github_pat_", 12)
        || has_prefixed_token(value, "aiza", 8)
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

fn terminal_message_for_code(code: &str) -> &'static str {
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
    let resolved_provider = request.provider_kind != "unresolved"
        && request.provider_profile_id != "unresolved"
        && nonempty(&request.provider_kind)
        && nonempty(&request.provider_profile_id)
        && nonempty(&request.endpoint_snapshot)
        && snapshot_is_public;
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
            status, request_json, provider_kind, provider_profile_id, endpoint_snapshot,
            chain_attempt, auto_attempt, max_auto_attempts, queued_at, started_at, finished_at,
            cancel_requested_at, last_heartbeat_at, error_code, error_message, retryable
         ) VALUES (
            ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15,
            NULL, ?16, NULL, NULL, ?17, ?18, ?19
         )",
        params![
            request.job_id,
            request.client_request_id,
            request.generation_id,
            request.parent_job_id,
            request.source_kind,
            source_ref_json,
            status,
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

    let job = get_job(tx, &request.job_id)?;
    enqueue_result_for_job(tx, &job)
}

pub(crate) fn insert_job_in_transaction(
    tx: &Transaction<'_>,
    request: &PreparedGenerationJob,
) -> Result<EnqueueGenerationResult, AppError> {
    if let Some(existing) = find_job_by_client_request_id(tx, &request.client_request_id)? {
        if existing_matches_prepared(&existing, request) {
            return enqueue_result_for_job(tx, &existing);
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

pub(crate) fn enqueue_job(
    conn: &mut Connection,
    request: &PreparedGenerationJob,
) -> Result<EnqueueGenerationResult, AppError> {
    if let Some(existing) = find_job_by_client_request_id(conn, &request.client_request_id)? {
        if existing_matches_prepared(&existing, request) {
            return enqueue_result_for_job(conn, &existing);
        }
        return Err(AppError::GenerationJobIdempotencyConflict);
    }

    let tx = conn
        .transaction()
        .map_err(|error| database_error("Begin generation job transaction failed", error))?;
    let result = insert_job_in_transaction(&tx, request)?;
    tx.commit()
        .map_err(|error| database_error("Commit generation job transaction failed", error))?;
    Ok(result)
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

pub(crate) fn list_jobs(
    conn: &Connection,
    filter: &GenerationJobFilter,
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
    let mut statement = conn
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
            validate_job_projection(conn, &job)?;
            Ok((rowid, job))
        })
        .collect::<Result<Vec<_>, AppError>>()?;
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
        request_json: row.get(offset + 7)?,
        provider_kind: row.get(offset + 8)?,
        provider_profile_id: row.get(offset + 9)?,
        endpoint_snapshot: row.get(offset + 10)?,
        chain_attempt: row.get(offset + 11)?,
        auto_attempt: row.get(offset + 12)?,
        max_auto_attempts: row.get(offset + 13)?,
        queued_at: row.get(offset + 14)?,
        started_at: row.get(offset + 15)?,
        finished_at: row.get(offset + 16)?,
        cancel_requested_at: row.get(offset + 17)?,
        last_heartbeat_at: row.get(offset + 18)?,
        error_code: row.get(offset + 19)?,
        error_message: row.get(offset + 20)?,
        retryable: row.get(offset + 21)?,
    })
}

pub(crate) fn claim_next_job(conn: &mut Connection) -> Result<Option<GenerationJob>, AppError> {
    let tx = conn
        .transaction()
        .map_err(|error| database_error("Begin job claim transaction failed", error))?;
    let candidate = tx
        .query_row(
            &format!(
                "SELECT {JOB_COLUMNS}
             FROM generation_jobs
             WHERE status = 'queued'
             ORDER BY queued_at ASC, rowid ASC
             LIMIT 1"
            ),
            [],
            stored_job_from_row,
        )
        .optional()
        .map_err(|error| persisted_row_error("Select next generation job failed", error))?
        .map(decode_stored_job)
        .transpose()?;
    let Some(candidate) = candidate else {
        tx.commit()
            .map_err(|error| database_error("Commit empty job claim failed", error))?;
        return Ok(None);
    };
    validate_job_projection(&tx, &candidate)?;
    let job_id = candidate.id;
    let generation_id = candidate.generation_id;
    let timestamp = crate::current_timestamp();
    let updated_job = tx
        .execute(
            "UPDATE generation_jobs
             SET status = 'running', started_at = ?1, last_heartbeat_at = ?1
             WHERE id = ?2 AND status = 'queued'",
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
    let job = get_job(&tx, &job_id)?;
    tx.commit()
        .map_err(|error| database_error("Commit job claim failed", error))?;
    Ok(Some(job))
}

pub(crate) fn request_cancel(conn: &Connection, id: &str) -> Result<GenerationJob, AppError> {
    let tx = conn
        .unchecked_transaction()
        .map_err(|error| database_error("Begin job cancellation transaction failed", error))?;
    let current = get_job(&tx, id)?;
    let timestamp = crate::current_timestamp();
    match current.status {
        GenerationJobStatus::Queued => {
            let updated_job = tx
                .execute(
                    "UPDATE generation_jobs
                     SET status = 'cancelled', cancel_requested_at = ?1, finished_at = ?1,
                         error_code = 'cancelled_by_user', error_message = ?2, retryable = 0
                     WHERE id = ?3 AND status = 'queued'",
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
        }
        GenerationJobStatus::Running => {
            let updated = tx
                .execute(
                    "UPDATE generation_jobs
                     SET cancel_requested_at = COALESCE(cancel_requested_at, ?1)
                     WHERE id = ?2 AND status = 'running'",
                    params![timestamp, id],
                )
                .map_err(|error| {
                    database_error("Request generation job cancellation failed", error)
                })?;
            if updated != 1 {
                return Err(AppError::GenerationJobInvalidTransition);
            }
        }
        _ => return Err(AppError::GenerationJobInvalidTransition),
    }
    let job = get_job(&tx, id)?;
    tx.commit()
        .map_err(|error| database_error("Commit job cancellation failed", error))?;
    Ok(job)
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
            "SELECT generation_id, status FROM generation_jobs WHERE id = ?1",
            params![update.job_id],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()
        .map_err(|error| persisted_row_error("Read terminal generation job failed", error))?
        .ok_or(AppError::GenerationJobNotFound)?;
    if parse_status(&current.1)? != update.expected_status {
        return Err(AppError::GenerationJobInvalidTransition);
    }
    if !source_reference_id_valid(&current.0) {
        return Err(AppError::GenerationJobCorruptPersistedData);
    }
    let generation_id = current.0;

    let updated = tx
        .execute(
            "UPDATE generation_jobs
             SET status = ?1, finished_at = ?2, error_code = ?3, error_message = ?4,
                 retryable = ?5
             WHERE id = ?6 AND status = ?7
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
    get_job(tx, &update.job_id)
}

pub(crate) fn finish_job(
    conn: &Connection,
    update: &GenerationJobTerminalUpdate,
) -> Result<GenerationJob, AppError> {
    let tx = conn
        .unchecked_transaction()
        .map_err(|error| database_error("Begin job finish transaction failed", error))?;
    let job = finish_job_in_transaction(&tx, update)?;
    tx.commit()
        .map_err(|error| database_error("Commit job finish failed", error))?;
    Ok(job)
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

fn retry_existing_result(
    conn: &Connection,
    parent_id: &str,
    existing: &GenerationJob,
    source_ref_override: Option<&Value>,
) -> Result<EnqueueGenerationResult, AppError> {
    if existing.parent_job_id.as_deref() != Some(parent_id) {
        return Err(AppError::GenerationJobIdempotencyConflict);
    }
    let parent = get_job(conn, parent_id)?;
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
    enqueue_result_for_job(conn, existing)
}

pub(crate) fn insert_retry_job_in_transaction(
    tx: &Transaction<'_>,
    parent_id: &str,
    client_request_id: &str,
    source_ref_override: Option<&Value>,
) -> Result<EnqueueGenerationResult, AppError> {
    if let Some(existing) = find_job_by_client_request_id(tx, client_request_id)? {
        return retry_existing_result(tx, parent_id, &existing, source_ref_override);
    }
    let parent = get_job(tx, parent_id)?;
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

pub(crate) fn create_retry_job(
    conn: &mut Connection,
    parent_id: &str,
    client_request_id: &str,
) -> Result<EnqueueGenerationResult, AppError> {
    if let Some(existing) = find_job_by_client_request_id(conn, client_request_id)? {
        return retry_existing_result(conn, parent_id, &existing, None);
    }
    let tx = conn
        .transaction()
        .map_err(|error| database_error("Begin retry transaction failed", error))?;
    let result = insert_retry_job_in_transaction(&tx, parent_id, client_request_id, None)?;
    tx.commit()
        .map_err(|error| database_error("Commit retry transaction failed", error))?;
    Ok(result)
}

#[cfg(test)]
#[path = "generation_jobs_tests.rs"]
mod tests;
