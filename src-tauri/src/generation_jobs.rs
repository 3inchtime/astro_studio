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
use serde_json::{Map, Value};

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
    pub request_extra_fields: Map<String, Value>,
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub(crate) struct GenerationJobOptions {
    pub(crate) size: String,
    pub(crate) quality: String,
    pub(crate) background: String,
    pub(crate) output_format: String,
    pub(crate) output_compression: u8,
    pub(crate) moderation: String,
    pub(crate) input_fidelity: String,
    pub(crate) stream: bool,
    pub(crate) partial_images: u8,
    pub(crate) image_count: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub(crate) struct GenerationJobRequest {
    pub(crate) kind: GenerationJobRequestKind,
    pub(crate) prompt: String,
    pub(crate) model: String,
    pub(crate) source_image_paths: Vec<String>,
    pub(crate) options: GenerationJobOptions,
    pub(crate) conversation_id: String,
    pub(crate) project_id: Option<String>,
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
    let source_ref = canonical_source_ref(
        &stored.source_kind,
        &raw_source_ref,
        Some(&canonical_request.conversation_id),
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
        && canonical_request.project_id.as_deref().is_none_or(nonempty)
        && [
            canonical_request.options.size.as_str(),
            canonical_request.options.quality.as_str(),
            canonical_request.options.background.as_str(),
            canonical_request.options.output_format.as_str(),
            canonical_request.options.moderation.as_str(),
            canonical_request.options.input_fidelity.as_str(),
        ]
        .into_iter()
        .all(nonempty)
        && canonical_request.options.output_compression <= 100
        && (1..=4).contains(&canonical_request.options.image_count)
        && canonical_request.options.partial_images <= 3;
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
    stored.map(decode_stored_job).transpose()
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
    conversation
        .0
        .filter(|conversation_id| nonempty(conversation_id))
        .ok_or(AppError::GenerationJobCorruptPersistedData)
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

fn credential_like_query_value(value: &str) -> bool {
    let value = value.trim().to_ascii_lowercase();
    value.starts_with("sk-")
        || value.starts_with("sk_")
        || value.starts_with("ghp_")
        || value.starts_with("github_pat_")
        || value.starts_with("aiza")
        || value.contains("bearer ")
        || (value.starts_with("eyj") && value.matches('.').count() >= 2)
}

fn endpoint_snapshot_is_public(endpoint: &str) -> bool {
    if endpoint.is_empty() {
        return true;
    }
    let Ok(url) = reqwest::Url::parse(endpoint) else {
        return false;
    };
    if !matches!(url.scheme(), "http" | "https")
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

fn safe_error_code(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
        && !credential_like_query_value(value)
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
    nonempty(value) && value.len() <= 256 && !value.chars().any(char::is_control)
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

    fn into_value(self, conversation_id: Option<&str>) -> Result<Value, AppError> {
        match self {
            Self::GenerateEdit(mut source_ref) => {
                if let Some(conversation_id) = conversation_id {
                    source_ref.conversation_id = Some(conversation_id.to_string());
                }
                serde_json::to_value(source_ref).map_err(|_| AppError::GenerationJobInvalidSnapshot)
            }
            Self::Canvas(source_ref) => {
                serde_json::to_value(source_ref).map_err(|_| AppError::GenerationJobInvalidSnapshot)
            }
        }
    }
}

fn canonical_source_ref(
    source_kind: &str,
    value: &Value,
    conversation_id: Option<&str>,
) -> Result<Value, AppError> {
    GenerationJobSourceRef::parse(source_kind, value)?.into_value(conversation_id)
}

fn canonical_request(
    request: &PreparedGenerationJob,
    conversation_id: &str,
) -> Result<GenerationJobRequest, AppError> {
    Ok(GenerationJobRequest {
        kind: GenerationJobRequestKind::parse(&request.request_kind)?,
        prompt: request.prompt.clone(),
        model: request.model.clone(),
        source_image_paths: request.source_image_paths.clone(),
        options: GenerationJobOptions {
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
        },
        conversation_id: conversation_id.to_string(),
        project_id: request.requested_project_id.clone(),
    })
}

fn canonical_metadata(
    request: &PreparedGenerationJob,
    conversation_id: &str,
) -> Result<CanonicalGenerationMetadata, AppError> {
    let canonical_request = canonical_request(request, conversation_id)?;
    Ok(CanonicalGenerationMetadata {
        request_kind: canonical_request.kind,
        conversation_id: canonical_request.conversation_id,
        model: canonical_request.model,
        size: canonical_request.options.size,
        quality: canonical_request.options.quality,
        background: canonical_request.options.background,
        output_format: canonical_request.options.output_format,
        output_compression: canonical_request.options.output_compression,
        moderation: canonical_request.options.moderation,
        input_fidelity: canonical_request.options.input_fidelity,
        stream: canonical_request.options.stream,
        partial_images: canonical_request.options.partial_images,
        image_count: canonical_request.options.image_count,
        source_image_count: canonical_request.source_image_paths.len(),
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
    .all(|value| nonempty(value));
    let timestamps_valid = canonical_timestamp(&request.queued_at)
        && request
            .finished_at
            .as_deref()
            .is_none_or(canonical_timestamp);
    let canonical_fields_valid = request.request_extra_fields.is_empty()
        && GenerationJobRequestKind::parse(&request.request_kind).is_ok()
        && canonical_source_ref(&request.source_kind, &request.source_ref, None).is_ok();
    let counters_valid = request.chain_attempt >= 1
        && request.auto_attempt >= 0
        && request.max_auto_attempts >= 0
        && request.auto_attempt <= request.max_auto_attempts
        && (1..=4).contains(&request.image_count)
        && request.partial_images <= 3
        && (0..=100).contains(&request.output_compression);
    let snapshot_is_public = endpoint_snapshot_is_public(&request.endpoint_snapshot);
    let unresolved_provider =
        request.provider_kind == "unresolved" || request.provider_profile_id == "unresolved";
    let status_fields_valid = match request.status {
        GenerationJobStatus::Queued => {
            request.finished_at.is_none()
                && request.error_code.is_none()
                && request.error_message.is_none()
                && !request.retryable
                && !unresolved_provider
                && nonempty(&request.endpoint_snapshot)
        }
        GenerationJobStatus::Failed => {
            request.finished_at.is_some()
                && matches!(
                    request.error_code.as_deref(),
                    Some("provider_profile_missing" | "provider_configuration_invalid")
                )
                && !request.retryable
                && (!unresolved_provider
                    || (request.provider_kind == "unresolved"
                        && request.provider_profile_id == "unresolved"
                        && request.endpoint_snapshot.is_empty()))
        }
        _ => false,
    };

    if common_fields_valid
        && timestamps_valid
        && canonical_fields_valid
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
    let Ok(expected_request) = canonical_request(request, &existing_request.conversation_id) else {
        return false;
    };
    let Ok(expected_source_ref) = canonical_source_ref(
        &request.source_kind,
        &request.source_ref,
        Some(&existing_request.conversation_id),
    ) else {
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
    conversation_id: &str,
) -> Result<EnqueueGenerationResult, AppError> {
    let canonical_request = canonical_request(request, conversation_id)?;
    let request_metadata = canonical_metadata(request, conversation_id)?;
    let canonical_source_ref = canonical_source_ref(
        &request.source_kind,
        &request.source_ref,
        Some(conversation_id),
    )?;
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
            conversation_id,
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
    insert_prepared_rows_in_transaction(tx, request, &conversation_id)
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

    let has_more = stored.len() > limit as usize;
    let mut items_with_order = stored
        .into_iter()
        .take(limit as usize)
        .map(|(rowid, stored)| Ok((rowid, decode_stored_job(stored)?)))
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

    let job = get_job(tx, &update.job_id)?;
    let updated_generation = tx
        .execute(
            "UPDATE generations
             SET status = ?1, error_message = ?2
             WHERE id = ?3 AND status = ?4",
            params![
                status_as_str(&update.status),
                persisted_error_message,
                job.generation_id,
                status_as_str(&update.expected_status),
            ],
        )
        .map_err(|error| database_error("Finish generation record failed", error))?;
    if updated_generation != 1 {
        return Err(AppError::GenerationJobCorruptPersistedData);
    }
    Ok(job)
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
    let metadata_matches_generation = request_metadata.request_kind.as_str() == raw.2
        && request_metadata.conversation_id == conversation_id
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
    let expected_source_ref = if let Some(source_ref_override) = source_ref_override {
        canonical_source_ref(
            &parent.source_kind,
            source_ref_override,
            Some(&parent_request.conversation_id),
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
        || parent_request.prompt != generation.prompt
        || parent_request.model != generation.model
        || parent_request.source_image_paths != generation.source_image_paths
        || parent_request.kind.as_str() != generation.request_kind
        || parent_request.options.size != generation.size
        || parent_request.options.quality != generation.quality
        || parent_request.options.background != generation.background
        || parent_request.options.output_format != generation.output_format
        || i32::from(parent_request.options.output_compression) != generation.output_compression
        || parent_request.options.moderation != generation.moderation
        || parent_request.options.input_fidelity != generation.input_fidelity
        || i32::from(parent_request.options.image_count) != generation.image_count
        || parent_request.options.stream != generation.stream
        || parent_request.options.partial_images != generation.partial_images
    {
        return Err(AppError::GenerationJobCorruptPersistedData);
    }
    let request = PreparedGenerationJob {
        job_id: uuid::Uuid::new_v4().to_string(),
        client_request_id: client_request_id.to_string(),
        generation_id: uuid::Uuid::new_v4().to_string(),
        requested_conversation_id: Some(generation.conversation_id.clone()),
        requested_project_id: parent_request.project_id,
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
        request_extra_fields: Map::new(),
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
    insert_prepared_rows_in_transaction(tx, &request, &generation.conversation_id)
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
mod tests {
    use super::*;
    use crate::db::Database;
    use crate::error::AppError;
    use crate::models::{GenerationJobFilter, GenerationJobStatus};
    use rusqlite::{params, Connection};
    use serde_json::json;
    use std::path::{Path, PathBuf};

    struct TestDatabaseDirectory(PathBuf);

    impl Drop for TestDatabaseDirectory {
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.0).ok();
        }
    }

    struct JobFixture {
        database: Database,
        path: PathBuf,
        _directory: TestDatabaseDirectory,
    }

    impl JobFixture {
        fn new() -> Self {
            let directory = std::env::temp_dir().join(format!(
                "astro-studio-generation-job-repository-test-{}",
                uuid::Uuid::new_v4()
            ));
            std::fs::create_dir_all(&directory).expect("create test database directory");
            let path = directory.join("astro_studio.db");
            let database = Database::open(&path).expect("open test database");
            database.run_migrations().expect("run v16 migrations");
            Self {
                database,
                path,
                _directory: TestDatabaseDirectory(directory),
            }
        }

        fn open_connection(&self) -> Connection {
            let conn = Connection::open(&self.path).expect("open fixture connection");
            conn.execute_batch("PRAGMA foreign_keys=ON;")
                .expect("enable foreign keys");
            conn
        }

        fn prepared(&self, client_request_id: &str, operation: &str) -> PreparedGenerationJob {
            self.prepared_at(client_request_id, operation, "2026-07-10T00:00:00Z")
        }

        fn prepared_at(
            &self,
            client_request_id: &str,
            operation: &str,
            queued_at: &str,
        ) -> PreparedGenerationJob {
            PreparedGenerationJob {
                job_id: uuid::Uuid::new_v4().to_string(),
                client_request_id: client_request_id.to_string(),
                generation_id: uuid::Uuid::new_v4().to_string(),
                requested_conversation_id: None,
                requested_project_id: Some("default".to_string()),
                prompt: format!("prompt for {operation}"),
                model: "gpt-image-2".to_string(),
                request_kind: "generate".to_string(),
                size: "1024x1024".to_string(),
                quality: "high".to_string(),
                background: "auto".to_string(),
                output_format: "png".to_string(),
                output_compression: 100,
                moderation: "auto".to_string(),
                input_fidelity: "high".to_string(),
                image_count: 1,
                stream: false,
                partial_images: 0,
                source_image_paths: Vec::new(),
                request_extra_fields: Map::new(),
                parent_job_id: None,
                source_kind: "generate".to_string(),
                source_ref: json!({ "id": operation }),
                provider_kind: "openai".to_string(),
                provider_profile_id: "profile-1".to_string(),
                endpoint_snapshot: "https://api.example.test/v1/images/generations".to_string(),
                status: GenerationJobStatus::Queued,
                chain_attempt: 1,
                auto_attempt: 0,
                max_auto_attempts: 2,
                queued_at: queued_at.to_string(),
                finished_at: None,
                error_code: None,
                error_message: None,
                retryable: false,
            }
        }

        fn enqueue(
            &self,
            client_request_id: &str,
            operation: &str,
        ) -> crate::models::GenerationJob {
            let request = self.prepared(client_request_id, operation);
            let result = {
                let mut conn = self.database.conn.lock().expect("lock database");
                enqueue_job(&mut conn, &request).expect("enqueue job")
            };
            self.get(&result.job_id)
        }

        fn enqueue_prepared(
            &self,
            request: &PreparedGenerationJob,
        ) -> Result<crate::models::EnqueueGenerationResult, AppError> {
            let mut conn = self.database.conn.lock().expect("lock database");
            enqueue_job(&mut conn, request)
        }

        fn get(&self, id: &str) -> crate::models::GenerationJob {
            let conn = self.database.conn.lock().expect("lock database");
            get_job(&conn, id).expect("get job")
        }

        fn get_result(
            &self,
            client_request_id: &str,
        ) -> Option<crate::models::EnqueueGenerationResult> {
            let conn = self.database.conn.lock().expect("lock database");
            find_enqueue_result_by_client_request_id(&conn, client_request_id)
                .expect("find enqueue result")
        }

        fn claim(&self) -> Option<crate::models::GenerationJob> {
            let mut conn = self.database.conn.lock().expect("lock database");
            claim_next_job(&mut conn).expect("claim next job")
        }

        fn fail_retryable(
            &self,
            client_request_id: &str,
            operation: &str,
        ) -> crate::models::GenerationJob {
            let queued = self.enqueue(client_request_id, operation);
            let claimed = self.claim().expect("claim queued job");
            assert_eq!(claimed.id, queued.id);
            let update = GenerationJobTerminalUpdate {
                job_id: queued.id.clone(),
                expected_status: GenerationJobStatus::Running,
                status: GenerationJobStatus::Failed,
                finished_at: "2026-07-10T00:00:02Z".to_string(),
                error_code: Some("provider_unavailable".to_string()),
                error_message: Some("The provider is temporarily unavailable".to_string()),
                retryable: true,
            };
            let conn = self.database.conn.lock().expect("lock database");
            finish_job(&conn, &update).expect("finish failed job")
        }

        fn count(&self, table: &str) -> i64 {
            let conn = self.database.conn.lock().expect("lock database");
            count_table(&conn, table)
        }

        fn generation_status(&self, generation_id: &str) -> String {
            let conn = self.database.conn.lock().expect("lock database");
            conn.query_row(
                "SELECT status FROM generations WHERE id = ?1",
                params![generation_id],
                |row| row.get(0),
            )
            .expect("read generation status")
        }

        fn list(&self, filter: &GenerationJobFilter) -> crate::models::GenerationJobPage {
            let conn = self.database.conn.lock().expect("lock database");
            list_jobs(&conn, filter).expect("list jobs")
        }
    }

    fn count_table(conn: &Connection, table: &str) -> i64 {
        assert!(matches!(
            table,
            "conversations" | "generations" | "generation_jobs" | "generation_recoveries" | "logs"
        ));
        conn.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
            row.get(0)
        })
        .expect("count fixture table")
    }

    fn stable_code(error: &AppError) -> &'static str {
        error.stable_code()
    }

    #[test]
    fn job_transition_matrix_allows_only_documented_edges() {
        use GenerationJobStatus::*;
        let statuses = [Queued, Running, Completed, Failed, Cancelled, Interrupted];

        for from in &statuses {
            for to in &statuses {
                let expected = matches!(
                    (from, to),
                    (Queued, Running)
                        | (Queued, Cancelled)
                        | (Running, Completed)
                        | (Running, Failed)
                        | (Running, Cancelled)
                        | (Running, Interrupted)
                );
                assert_eq!(
                    can_transition(from.clone(), to.clone()),
                    expected,
                    "unexpected transition decision for {from:?} -> {to:?}"
                );
            }
        }
    }

    #[test]
    fn repeated_client_request_returns_existing_before_any_duplicate_side_effect() {
        let fixture = JobFixture::new();
        let first_request = fixture.prepared("request-1", "same-operation");
        let first = fixture
            .enqueue_prepared(&first_request)
            .expect("first enqueue");
        let counts_after_first = [
            fixture.count("conversations"),
            fixture.count("generations"),
            fixture.count("generation_jobs"),
            fixture.count("generation_recoveries"),
            fixture.count("logs"),
        ];

        let repeated_request = fixture.prepared("request-1", "same-operation");
        let second = fixture
            .enqueue_prepared(&repeated_request)
            .expect("idempotent enqueue");

        assert_eq!(first.job_id, second.job_id);
        assert_eq!(first.generation_id, second.generation_id);
        assert_eq!(first.conversation_id, second.conversation_id);
        assert_eq!(
            counts_after_first,
            [
                fixture.count("conversations"),
                fixture.count("generations"),
                fixture.count("generation_jobs"),
                fixture.count("generation_recoveries"),
                fixture.count("logs"),
            ]
        );
    }

    #[test]
    fn duplicate_recheck_inside_outer_transaction_precedes_conversation_resolution() {
        let fixture = JobFixture::new();
        let first = fixture.enqueue("request-1", "same-operation");
        let mut repeated = fixture.prepared("request-1", "same-operation");
        repeated.requested_conversation_id = Some("missing-conversation".to_string());

        let mut conn = fixture.open_connection();
        let tx = conn.transaction().expect("begin outer transaction");
        let result = insert_job_in_transaction(&tx, &repeated).expect("idempotent insert");
        assert_eq!(result.job_id, first.id);
        assert_eq!(count_table(&tx, "conversations"), 1);
        assert_eq!(count_table(&tx, "generations"), 1);
        tx.commit().expect("commit duplicate transaction");
    }

    #[test]
    fn composable_enqueue_outer_rollback_removes_conversation_generation_recovery_and_job() {
        let fixture = JobFixture::new();
        let mut conn = fixture.open_connection();
        let tx = conn.transaction().expect("begin outer transaction");
        insert_job_in_transaction(&tx, &fixture.prepared("request-1", "rollback"))
            .expect("insert in outer transaction");
        assert_eq!(count_table(&tx, "conversations"), 1);
        assert_eq!(count_table(&tx, "generations"), 1);
        assert_eq!(count_table(&tx, "generation_recoveries"), 1);
        assert_eq!(count_table(&tx, "generation_jobs"), 1);
        tx.rollback().expect("rollback outer transaction");

        assert_eq!(fixture.count("conversations"), 0);
        assert_eq!(fixture.count("generations"), 0);
        assert_eq!(fixture.count("generation_recoveries"), 0);
        assert_eq!(fixture.count("generation_jobs"), 0);
    }

    #[test]
    fn composable_enqueue_outer_rollback_restores_updated_conversation() {
        let fixture = JobFixture::new();
        {
            let conn = fixture.database.conn.lock().expect("lock database");
            conn.execute(
                "INSERT INTO conversations (id, project_id, title) VALUES ('conversation-1', 'default', 'Original title')",
                [],
            )
            .expect("insert conversation");
        }
        let mut request = fixture.prepared("request-1", "updated-conversation");
        request.requested_conversation_id = Some("conversation-1".to_string());

        let mut conn = fixture.open_connection();
        let tx = conn.transaction().expect("begin outer transaction");
        let result = insert_job_in_transaction(&tx, &request).expect("insert job");
        assert_eq!(result.conversation_id, "conversation-1");
        let changed_title: String = tx
            .query_row(
                "SELECT title FROM conversations WHERE id = 'conversation-1'",
                [],
                |row| row.get(0),
            )
            .expect("read updated title");
        assert_ne!(changed_title, "Original title");
        tx.rollback().expect("rollback outer transaction");

        let conn = fixture.database.conn.lock().expect("lock database");
        let restored_title: String = conn
            .query_row(
                "SELECT title FROM conversations WHERE id = 'conversation-1'",
                [],
                |row| row.get(0),
            )
            .expect("read restored title");
        assert_eq!(restored_title, "Original title");
    }

    #[test]
    fn initial_provider_configuration_failure_is_inserted_atomically_as_terminal() {
        let fixture = JobFixture::new();
        let mut request = fixture.prepared("request-1", "missing-profile");
        request.status = GenerationJobStatus::Failed;
        request.provider_kind = "unresolved".to_string();
        request.provider_profile_id = "unresolved".to_string();
        request.endpoint_snapshot.clear();
        request.finished_at = Some(request.queued_at.clone());
        request.error_code = Some("provider_profile_missing".to_string());
        request.error_message = Some("Bearer sk-secret from a provider response".to_string());

        let result = fixture
            .enqueue_prepared(&request)
            .expect("persist failed enqueue");
        let job = fixture.get(&result.job_id);
        assert_eq!(result.status, GenerationJobStatus::Failed);
        assert_eq!(job.status, GenerationJobStatus::Failed);
        assert_eq!(job.finished_at.as_deref(), Some(request.queued_at.as_str()));
        assert_eq!(job.error_code.as_deref(), Some("provider_profile_missing"));
        assert_eq!(
            job.error_message.as_deref(),
            Some("The selected provider profile is unavailable")
        );
        assert!(!serde_json::to_string(&job)
            .expect("serialize failed job")
            .contains("sk-secret"));
        assert!(!job.retryable);
        assert!(!result.retryable);
        assert_eq!(result.error_code, job.error_code);
        assert_eq!(result.error_message, job.error_message);
        assert_eq!(result.queued_at, job.queued_at);
        assert_eq!(result.finished_at, job.finished_at);
        assert_eq!(fixture.generation_status(&job.generation_id), "failed");
        assert_eq!(fixture.count("generation_recoveries"), 0);
        assert!(fixture.claim().is_none());
    }

    #[test]
    fn secret_bearing_request_snapshot_is_rejected_without_persistence_or_leakage() {
        let fixture = JobFixture::new();
        for (index, private_request) in [
            json!({ "api_key": "secret-key" }),
            json!({ "nested": { "apiKey": "secret-key" } }),
            json!({ "headers": { "x-api-key": "secret-key" } }),
            json!({ "headers": { "authorization": "Bearer secret-key" } }),
        ]
        .into_iter()
        .enumerate()
        {
            let mut request = fixture.prepared(&format!("request-{index}"), "secret-snapshot");
            request.request_extra_fields = private_request
                .as_object()
                .expect("private request object")
                .clone();
            let error = fixture
                .enqueue_prepared(&request)
                .expect_err("secret-bearing snapshots must fail");
            assert_eq!(stable_code(&error), "generation_job_invalid_snapshot");
            assert!(!error.to_string().contains("secret-key"));
        }
        for (index, endpoint) in [
            "https://user:secret-key@example.test/images",
            "https://example.test/images?x%2Dapi%2Dkey=secret-key",
            "https://example.test/images?client_secret=secret-key",
            "https://example.test/images?routing=sk-secret",
            "https://example.test/images#access_token=secret-key",
        ]
        .into_iter()
        .enumerate()
        {
            let mut request =
                fixture.prepared(&format!("endpoint-request-{index}"), "secret-endpoint");
            request.endpoint_snapshot = endpoint.to_string();
            let error = fixture
                .enqueue_prepared(&request)
                .expect_err("secret-bearing endpoint snapshots must fail");
            assert_eq!(stable_code(&error), "generation_job_invalid_snapshot");
            assert!(!error.to_string().contains("secret-key"));
        }
        assert_eq!(fixture.count("conversations"), 0);
        assert_eq!(fixture.count("generations"), 0);
        assert_eq!(fixture.count("generation_jobs"), 0);
    }

    #[test]
    fn unknown_source_reference_keys_are_rejected_before_persistence() {
        let fixture = JobFixture::new();
        for (index, source_ref) in [
            json!({ "unknown_id": "value" }),
            json!({ "client_secret": "secret-key" }),
            json!({ "id": { "nested": "value" } }),
        ]
        .into_iter()
        .enumerate()
        {
            let mut request = fixture.prepared(&format!("request-{index}"), "bad-source-ref");
            request.source_ref = source_ref;
            let error = fixture
                .enqueue_prepared(&request)
                .expect_err("unknown source reference fields must fail");
            assert_eq!(stable_code(&error), "generation_job_invalid_snapshot");
            assert!(!error.to_string().contains("secret-key"));
        }
        assert_eq!(fixture.count("generation_jobs"), 0);
    }

    #[test]
    fn safe_custom_endpoint_queries_and_credential_like_prompt_text_are_preserved() {
        let fixture = JobFixture::new();
        for (index, endpoint) in [
            "https://api.example.test/images?api-version=2026-01-01",
            "https://api.example.test/images?routing=primary&region=west",
        ]
        .into_iter()
        .enumerate()
        {
            let mut request = fixture.prepared(&format!("request-{index}"), "safe-query");
            request.endpoint_snapshot = endpoint.to_string();
            request.prompt =
                "Paint the literal text Bearer sk-example as a warning label".to_string();
            let result = fixture.enqueue_prepared(&request).expect("safe snapshot");
            let job = fixture.get(&result.job_id);
            assert_eq!(job.endpoint_snapshot, endpoint);
            assert_eq!(job.request["prompt"], json!(request.prompt));
        }
    }

    #[test]
    fn persisted_snapshot_and_public_row_are_secret_free() {
        let fixture = JobFixture::new();
        let job = fixture.enqueue("request-1", "public-snapshot");
        let serialized = serde_json::to_string(&job).expect("serialize job");
        assert!(!serialized.contains("api_key"));
        assert!(!serialized.contains("secret-key"));
        assert!(job.request["conversation_id"].as_str().is_some());
        let mut request_keys = job
            .request
            .as_object()
            .expect("canonical request object")
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>();
        request_keys.sort_unstable();
        assert_eq!(
            request_keys,
            [
                "conversation_id",
                "kind",
                "model",
                "options",
                "project_id",
                "prompt",
                "source_image_paths",
            ]
        );
        let mut option_keys = job.request["options"]
            .as_object()
            .expect("canonical options object")
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>();
        option_keys.sort_unstable();
        assert_eq!(
            option_keys,
            [
                "background",
                "image_count",
                "input_fidelity",
                "moderation",
                "output_compression",
                "output_format",
                "partial_images",
                "quality",
                "size",
                "stream",
            ]
        );
        assert_eq!(
            job.source_ref["conversation_id"],
            job.request["conversation_id"]
        );
    }

    #[test]
    fn prepared_snapshot_rejects_inconsistent_initial_status_fields() {
        let fixture = JobFixture::new();

        let mut queued_with_terminal = fixture.prepared("request-1", "queued-terminal");
        queued_with_terminal.finished_at = Some(queued_with_terminal.queued_at.clone());
        queued_with_terminal.error_code = Some("unexpected".to_string());

        let mut queued_unresolved = fixture.prepared("request-2", "queued-unresolved");
        queued_unresolved.provider_kind = "unresolved".to_string();
        queued_unresolved.provider_profile_id = "unresolved".to_string();
        queued_unresolved.endpoint_snapshot.clear();

        let mut failed_without_terminal = fixture.prepared("request-3", "failed-incomplete");
        failed_without_terminal.status = GenerationJobStatus::Failed;
        failed_without_terminal.provider_kind = "unresolved".to_string();
        failed_without_terminal.provider_profile_id = "unresolved".to_string();
        failed_without_terminal.endpoint_snapshot.clear();

        for invalid in [
            queued_with_terminal,
            queued_unresolved,
            failed_without_terminal,
        ] {
            let error = fixture
                .enqueue_prepared(&invalid)
                .expect_err("inconsistent initial snapshot must fail");
            assert_eq!(stable_code(&error), "generation_job_invalid_snapshot");
        }
        assert_eq!(fixture.count("conversations"), 0);
        assert_eq!(fixture.count("generations"), 0);
        assert_eq!(fixture.count("generation_jobs"), 0);
    }

    #[test]
    fn root_enqueue_rejects_fabricated_retry_lineage_and_attempts() {
        let fixture = JobFixture::new();
        let mut request = fixture.prepared("request-1", "fabricated-lineage");
        request.parent_job_id = Some("fabricated-parent".to_string());
        request.chain_attempt = 2;
        request.auto_attempt = 1;

        let error = fixture
            .enqueue_prepared(&request)
            .expect_err("root enqueue cannot fabricate retry state");
        assert_eq!(stable_code(&error), "generation_job_invalid_snapshot");
        assert_eq!(fixture.count("conversations"), 0);
        assert_eq!(fixture.count("generations"), 0);
        assert_eq!(fixture.count("generation_jobs"), 0);
    }

    #[test]
    fn initial_failed_snapshot_accepts_only_configuration_error_codes() {
        let fixture = JobFixture::new();
        let mut request = fixture.prepared("request-1", "invalid-initial-failure");
        request.status = GenerationJobStatus::Failed;
        request.finished_at = Some(request.queued_at.clone());
        request.error_code = Some("provider_unavailable".to_string());
        request.error_message = Some("The provider is unavailable".to_string());

        let error = fixture
            .enqueue_prepared(&request)
            .expect_err("runtime failures cannot bypass the running state");
        assert_eq!(stable_code(&error), "generation_job_invalid_snapshot");
        assert_eq!(fixture.count("generations"), 0);
        assert_eq!(fixture.count("generation_jobs"), 0);
    }

    #[test]
    fn persisted_generation_matches_normalized_snapshot_and_job_timestamp() {
        let fixture = JobFixture::new();
        let request = fixture.prepared("request-1", "normalized-generation");
        let result = fixture
            .enqueue_prepared(&request)
            .expect("enqueue normalized job");
        let conn = fixture.database.conn.lock().expect("lock database");
        let persisted = conn
            .query_row(
                "SELECT prompt, engine, request_kind, size, quality, background, output_format,
                        output_compression, moderation, input_fidelity, image_count,
                        source_image_count, source_image_paths, request_metadata, status,
                        conversation_id, created_at
                 FROM generations WHERE id = ?1",
                params![result.generation_id],
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
                        row.get::<_, i32>(11)?,
                        row.get::<_, String>(12)?,
                        row.get::<_, Option<String>>(13)?,
                        row.get::<_, String>(14)?,
                        row.get::<_, Option<String>>(15)?,
                        row.get::<_, String>(16)?,
                    ))
                },
            )
            .expect("read persisted generation");

        assert_eq!(persisted.0, request.prompt);
        assert_eq!(persisted.1, request.model);
        assert_eq!(persisted.2, request.request_kind);
        assert_eq!(persisted.3, request.size);
        assert_eq!(persisted.4, request.quality);
        assert_eq!(persisted.5, request.background);
        assert_eq!(persisted.6, request.output_format);
        assert_eq!(persisted.7, request.output_compression);
        assert_eq!(persisted.8, request.moderation);
        assert_eq!(persisted.9, request.input_fidelity);
        assert_eq!(persisted.10, request.image_count);
        assert_eq!(persisted.11, request.source_image_paths.len() as i32);
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&persisted.12).expect("valid source paths"),
            json!(request.source_image_paths)
        );
        assert!(serde_json::from_str::<serde_json::Value>(
            persisted.13.as_deref().expect("request metadata")
        )
        .is_ok());
        assert_eq!(persisted.14, "queued");
        assert_eq!(
            persisted.15.as_deref(),
            Some(result.conversation_id.as_str())
        );
        assert_eq!(persisted.16, request.queued_at);
        assert_eq!(result.queued_at, request.queued_at);
    }

    #[test]
    fn claim_and_list_use_stable_same_second_fifo_and_update_generation() {
        let fixture = JobFixture::new();
        let first = fixture.enqueue("request-1", "first");
        let second = fixture.enqueue("request-2", "second");
        assert_eq!(first.queued_at, second.queued_at);

        let listed = fixture.list(&GenerationJobFilter::default());
        assert_eq!(
            listed
                .items
                .iter()
                .map(|job| job.id.as_str())
                .collect::<Vec<_>>(),
            [first.id.as_str(), second.id.as_str()]
        );

        let claimed = fixture.claim().expect("claim first job");
        assert_eq!(claimed.id, first.id);
        assert_eq!(claimed.status, GenerationJobStatus::Running);
        assert_eq!(fixture.generation_status(&first.generation_id), "running");

        let claimed_second = fixture.claim().expect("claim second job");
        assert_eq!(claimed_second.id, second.id);
    }

    #[test]
    fn claim_rejects_corrupt_unresolved_queued_snapshot_before_transition() {
        let fixture = JobFixture::new();
        let queued = fixture.enqueue("request-1", "corrupt-claim");
        {
            let conn = fixture.database.conn.lock().expect("lock database");
            conn.execute(
                "UPDATE generation_jobs
                 SET provider_kind = 'unresolved', provider_profile_id = 'unresolved',
                     endpoint_snapshot = ''
                 WHERE id = ?1",
                params![queued.id],
            )
            .expect("corrupt queued provider snapshot");
        }

        let error = {
            let mut conn = fixture.database.conn.lock().expect("lock database");
            claim_next_job(&mut conn).expect_err("corrupt queued row must not be claimed")
        };
        assert_eq!(stable_code(&error), "generation_job_corrupt_persisted_data");
        let conn = fixture.database.conn.lock().expect("lock database");
        let statuses: (String, String) = conn
            .query_row(
                "SELECT j.status, g.status
                 FROM generation_jobs j JOIN generations g ON g.id = j.generation_id
                 WHERE j.id = ?1",
                params![queued.id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("read rolled-back statuses");
        assert_eq!(statuses, ("queued".to_string(), "queued".to_string()));
    }

    #[test]
    fn claim_rejects_blank_provider_identity_or_nonzero_queued_attempt() {
        for (index, mutation) in [
            "provider_kind = '   ', provider_profile_id = ''",
            "auto_attempt = 1",
        ]
        .into_iter()
        .enumerate()
        {
            let fixture = JobFixture::new();
            let queued = fixture.enqueue(&format!("request-{index}"), "bad-queued-state");
            {
                let conn = fixture.database.conn.lock().expect("lock database");
                conn.execute(
                    &format!("UPDATE generation_jobs SET {mutation} WHERE id = ?1"),
                    params![queued.id],
                )
                .expect("corrupt queued state");
            }
            let error = {
                let mut conn = fixture.database.conn.lock().expect("lock database");
                claim_next_job(&mut conn).expect_err("malformed queued row must not be claimed")
            };
            assert_eq!(stable_code(&error), "generation_job_corrupt_persisted_data");
        }
    }

    #[test]
    fn stale_expected_status_cannot_finish_or_mutate_generation() {
        let fixture = JobFixture::new();
        let queued = fixture.enqueue("request-1", "stale-finish");
        let update = GenerationJobTerminalUpdate {
            job_id: queued.id.clone(),
            expected_status: GenerationJobStatus::Running,
            status: GenerationJobStatus::Failed,
            finished_at: "2026-07-10T00:00:01Z".to_string(),
            error_code: Some("provider_error".to_string()),
            error_message: Some("Provider request failed".to_string()),
            retryable: true,
        };

        let conn = fixture.database.conn.lock().expect("lock database");
        let error = finish_job(&conn, &update).expect_err("stale transition must fail");
        drop(conn);
        assert_eq!(stable_code(&error), "generation_job_invalid_transition");
        assert_eq!(fixture.get(&queued.id).status, GenerationJobStatus::Queued);
        assert_eq!(fixture.generation_status(&queued.generation_id), "queued");
    }

    #[test]
    fn queued_cancel_updates_job_and_generation_while_running_cancel_only_stamps_request() {
        let fixture = JobFixture::new();
        let queued = fixture.enqueue("request-1", "cancel-queued");
        let cancelled = {
            let conn = fixture.database.conn.lock().expect("lock database");
            request_cancel(&conn, &queued.id).expect("cancel queued job")
        };
        assert_eq!(cancelled.status, GenerationJobStatus::Cancelled);
        assert!(cancelled.cancel_requested_at.is_some());
        assert!(cancelled.finished_at.is_some());
        assert_eq!(
            fixture.generation_status(&queued.generation_id),
            "cancelled"
        );

        let running = fixture.enqueue("request-2", "cancel-running");
        let claimed = fixture.claim().expect("claim running job");
        assert_eq!(claimed.id, running.id);
        let requested = {
            let conn = fixture.database.conn.lock().expect("lock database");
            request_cancel(&conn, &running.id).expect("request running cancellation")
        };
        assert_eq!(requested.status, GenerationJobStatus::Running);
        assert!(requested.cancel_requested_at.is_some());
        assert!(requested.finished_at.is_none());
        assert_eq!(fixture.generation_status(&running.generation_id), "running");
    }

    #[test]
    fn cancel_request_wins_over_non_cancel_terminal_update() {
        let fixture = JobFixture::new();
        let queued = fixture.enqueue("request-1", "cancel-first");
        fixture.claim().expect("claim cancel-first job");
        {
            let conn = fixture.database.conn.lock().expect("lock database");
            request_cancel(&conn, &queued.id).expect("persist cancellation request");
        }

        let conn = fixture.database.conn.lock().expect("lock database");
        let error = finish_job(
            &conn,
            &GenerationJobTerminalUpdate {
                job_id: queued.id.clone(),
                expected_status: GenerationJobStatus::Running,
                status: GenerationJobStatus::Completed,
                finished_at: "2026-07-10T00:00:02Z".to_string(),
                error_code: None,
                error_message: None,
                retryable: false,
            },
        )
        .expect_err("cancel-first must block completion");
        drop(conn);
        assert_eq!(stable_code(&error), "generation_job_invalid_transition");
        let job = fixture.get(&queued.id);
        assert_eq!(job.status, GenerationJobStatus::Running);
        assert!(job.cancel_requested_at.is_some());
        assert_eq!(fixture.generation_status(&queued.generation_id), "running");
    }

    #[test]
    fn running_job_cannot_be_cancelled_without_durable_request() {
        let fixture = JobFixture::new();
        let queued = fixture.enqueue("request-1", "spontaneous-cancel");
        fixture.claim().expect("claim spontaneous cancel job");
        let conn = fixture.database.conn.lock().expect("lock database");
        let error = finish_job(
            &conn,
            &GenerationJobTerminalUpdate {
                job_id: queued.id.clone(),
                expected_status: GenerationJobStatus::Running,
                status: GenerationJobStatus::Cancelled,
                finished_at: "2026-07-10T00:00:02Z".to_string(),
                error_code: Some("cancelled_by_user".to_string()),
                error_message: Some("The operation was cancelled".to_string()),
                retryable: false,
            },
        )
        .expect_err("cancel transition requires a durable request");
        drop(conn);
        assert_eq!(stable_code(&error), "generation_job_invalid_transition");
        assert_eq!(fixture.get(&queued.id).status, GenerationJobStatus::Running);
    }

    #[test]
    fn failed_retry_creates_immutable_child_with_fresh_generation_and_reset_attempts() {
        let fixture = JobFixture::new();
        let failed = fixture.fail_retryable("request-1", "retry-parent");
        let parent_before = serde_json::to_value(&failed).expect("serialize parent");

        let retry_result = {
            let mut conn = fixture.database.conn.lock().expect("lock database");
            create_retry_job(&mut conn, &failed.id, "retry-request-1").expect("create retry")
        };
        let retry = fixture.get(&retry_result.job_id);

        assert_eq!(retry.parent_job_id.as_deref(), Some(failed.id.as_str()));
        assert_ne!(retry.generation_id, failed.generation_id);
        assert_eq!(retry.chain_attempt, failed.chain_attempt + 1);
        assert_eq!(retry.auto_attempt, 0);
        assert_eq!(retry.status, GenerationJobStatus::Queued);
        assert_eq!(retry.request, failed.request);
        assert_eq!(retry.provider_kind, failed.provider_kind);
        assert_eq!(retry.provider_profile_id, failed.provider_profile_id);
        assert_eq!(retry.endpoint_snapshot, failed.endpoint_snapshot);
        assert!(retry.error_code.is_none());
        assert!(retry.error_message.is_none());
        assert!(retry.finished_at.is_none());
        assert_eq!(fixture.generation_status(&retry.generation_id), "queued");
        assert_eq!(
            serde_json::to_value(fixture.get(&failed.id)).expect("serialize unchanged parent"),
            parent_before
        );
    }

    #[test]
    fn retry_client_request_reuse_for_different_parent_is_conflict_without_side_effects() {
        let fixture = JobFixture::new();
        let first = fixture.fail_retryable("request-1", "parent-one");
        let second = fixture.fail_retryable("request-2", "parent-two");
        {
            let mut conn = fixture.database.conn.lock().expect("lock database");
            create_retry_job(&mut conn, &first.id, "retry-request").expect("first retry");
        }
        let jobs_before = fixture.count("generation_jobs");
        let generations_before = fixture.count("generations");

        let error = {
            let mut conn = fixture.database.conn.lock().expect("lock database");
            create_retry_job(&mut conn, &second.id, "retry-request")
                .expect_err("cross-parent idempotency reuse must fail")
        };
        assert_eq!(stable_code(&error), "generation_job_idempotency_conflict");
        assert_eq!(fixture.count("generation_jobs"), jobs_before);
        assert_eq!(fixture.count("generations"), generations_before);
    }

    #[test]
    fn retry_client_request_reuse_for_same_parent_returns_existing_child() {
        let fixture = JobFixture::new();
        let parent = fixture.fail_retryable("request-1", "parent");
        let first = {
            let mut conn = fixture.database.conn.lock().expect("lock database");
            create_retry_job(&mut conn, &parent.id, "retry-request").expect("first retry")
        };
        let counts = (
            fixture.count("conversations"),
            fixture.count("generations"),
            fixture.count("generation_jobs"),
            fixture.count("generation_recoveries"),
        );
        let repeated = {
            let mut conn = fixture.database.conn.lock().expect("lock database");
            create_retry_job(&mut conn, &parent.id, "retry-request")
                .expect("same retry is idempotent")
        };
        assert_eq!(first.job_id, repeated.job_id);
        assert_eq!(first.generation_id, repeated.generation_id);
        assert_eq!(
            counts,
            (
                fixture.count("conversations"),
                fixture.count("generations"),
                fixture.count("generation_jobs"),
                fixture.count("generation_recoveries"),
            )
        );
    }

    #[test]
    fn progressed_retry_child_remains_idempotently_addressable() {
        let fixture = JobFixture::new();
        let parent = fixture.fail_retryable("request-1", "progressed-parent");
        let child = {
            let mut conn = fixture.database.conn.lock().expect("lock database");
            create_retry_job(&mut conn, &parent.id, "retry-request").expect("create retry")
        };
        let claimed = fixture.claim().expect("claim retry child");
        assert_eq!(claimed.id, child.job_id);
        {
            let conn = fixture.database.conn.lock().expect("lock database");
            conn.execute(
                "UPDATE generation_jobs SET auto_attempt = 1 WHERE id = ?1 AND status = 'running'",
                params![child.job_id],
            )
            .expect("persist automatic attempt");
        }

        let repeated = {
            let mut conn = fixture.database.conn.lock().expect("lock database");
            create_retry_job(&mut conn, &parent.id, "retry-request")
                .expect("progress does not change retry identity")
        };
        assert_eq!(repeated.job_id, child.job_id);
        assert_eq!(fixture.get(&child.job_id).auto_attempt, 1);
    }

    #[test]
    fn retry_client_request_reuse_for_different_source_override_is_conflict() {
        let fixture = JobFixture::new();
        let parent = fixture.fail_retryable("request-1", "source-aware-parent");
        {
            let mut conn = fixture.open_connection();
            let tx = conn.transaction().expect("begin first source-aware retry");
            insert_retry_job_in_transaction(
                &tx,
                &parent.id,
                "retry-request",
                Some(&json!({ "id": "override-one" })),
            )
            .expect("insert source-aware retry");
            tx.commit().expect("commit source-aware retry");
        }
        let jobs_before = fixture.count("generation_jobs");

        let mut conn = fixture.open_connection();
        let tx = conn.transaction().expect("begin conflicting retry");
        let error = insert_retry_job_in_transaction(
            &tx,
            &parent.id,
            "retry-request",
            Some(&json!({ "id": "override-two" })),
        )
        .expect_err("different source override is a different logical retry");
        assert_eq!(stable_code(&error), "generation_job_idempotency_conflict");
        tx.rollback().expect("rollback conflicting retry");
        assert_eq!(fixture.count("generation_jobs"), jobs_before);
    }

    #[test]
    fn retry_requires_retryable_failed_or_interrupted_parent() {
        let fixture = JobFixture::new();
        let nonretryable = fixture.enqueue("request-1", "nonretryable-failed");
        fixture.claim().expect("claim nonretryable job");
        {
            let conn = fixture.database.conn.lock().expect("lock database");
            finish_job(
                &conn,
                &GenerationJobTerminalUpdate {
                    job_id: nonretryable.id.clone(),
                    expected_status: GenerationJobStatus::Running,
                    status: GenerationJobStatus::Failed,
                    finished_at: "2026-07-10T00:00:02Z".to_string(),
                    error_code: Some("invalid_request".to_string()),
                    error_message: Some("The request cannot be retried".to_string()),
                    retryable: false,
                },
            )
            .expect("finish nonretryable job");
        }
        let error = {
            let mut conn = fixture.database.conn.lock().expect("lock database");
            create_retry_job(&mut conn, &nonretryable.id, "retry-nonretryable")
                .expect_err("failed nonretryable job must reject retry")
        };
        assert_eq!(stable_code(&error), "generation_job_not_retryable");

        let interrupted = fixture.enqueue("request-2", "retryable-interrupted");
        fixture.claim().expect("claim interrupted job");
        {
            let conn = fixture.database.conn.lock().expect("lock database");
            finish_job(
                &conn,
                &GenerationJobTerminalUpdate {
                    job_id: interrupted.id.clone(),
                    expected_status: GenerationJobStatus::Running,
                    status: GenerationJobStatus::Interrupted,
                    finished_at: "2026-07-10T00:00:03Z".to_string(),
                    error_code: Some("app_interrupted".to_string()),
                    error_message: Some("The operation was interrupted".to_string()),
                    retryable: true,
                },
            )
            .expect("finish interrupted job");
        }
        let retry = {
            let mut conn = fixture.database.conn.lock().expect("lock database");
            create_retry_job(&mut conn, &interrupted.id, "retry-interrupted")
                .expect("retry interrupted job")
        };
        assert_eq!(retry.status, GenerationJobStatus::Queued);
    }

    #[test]
    fn retry_rejects_chain_attempt_overflow_without_panicking_or_writing() {
        let fixture = JobFixture::new();
        let parent = fixture.fail_retryable("request-1", "overflow-parent");
        {
            let conn = fixture.database.conn.lock().expect("lock database");
            conn.execute(
                "UPDATE generation_jobs SET chain_attempt = ?1 WHERE id = ?2",
                params![i32::MAX, parent.id],
            )
            .expect("set maximum chain attempt");
        }
        let count_before = fixture.count("generation_jobs");
        let error = {
            let mut conn = fixture.database.conn.lock().expect("lock database");
            create_retry_job(&mut conn, &parent.id, "retry-overflow")
                .expect_err("overflowed retry chain must fail safely")
        };
        assert_eq!(stable_code(&error), "generation_job_corrupt_persisted_data");
        assert_eq!(fixture.count("generation_jobs"), count_before);
    }

    #[test]
    fn generic_retry_rejects_unsupported_source_kind() {
        let fixture = JobFixture::new();
        let mut request = fixture.prepared("request-1", "canvas-parent");
        request.source_kind = "canvas".to_string();
        let queued = fixture
            .enqueue_prepared(&request)
            .expect("enqueue canvas job");
        fixture.claim().expect("claim canvas job");
        {
            let conn = fixture.database.conn.lock().expect("lock database");
            finish_job(
                &conn,
                &GenerationJobTerminalUpdate {
                    job_id: queued.job_id.clone(),
                    expected_status: GenerationJobStatus::Running,
                    status: GenerationJobStatus::Failed,
                    finished_at: "2026-07-10T00:00:02Z".to_string(),
                    error_code: Some("canvas_error".to_string()),
                    error_message: Some("Canvas generation failed".to_string()),
                    retryable: true,
                },
            )
            .expect("fail canvas job");
        }

        let error = {
            let mut conn = fixture.database.conn.lock().expect("lock database");
            create_retry_job(&mut conn, &queued.job_id, "retry-request")
                .expect_err("generic canvas retry must fail")
        };
        assert_eq!(stable_code(&error), "generation_job_source_unsupported");
        assert_eq!(fixture.count("generation_jobs"), 1);
    }

    #[test]
    fn source_aware_retry_supports_canvas_but_generic_replay_rejects_it() {
        let fixture = JobFixture::new();
        let mut request = fixture.prepared("request-1", "canvas-source-aware");
        request.source_kind = "canvas".to_string();
        let queued = fixture
            .enqueue_prepared(&request)
            .expect("enqueue canvas parent");
        fixture.claim().expect("claim canvas parent");
        {
            let conn = fixture.database.conn.lock().expect("lock database");
            finish_job(
                &conn,
                &GenerationJobTerminalUpdate {
                    job_id: queued.job_id.clone(),
                    expected_status: GenerationJobStatus::Running,
                    status: GenerationJobStatus::Failed,
                    finished_at: "2026-07-10T00:00:02Z".to_string(),
                    error_code: Some("canvas_error".to_string()),
                    error_message: Some("Canvas generation failed".to_string()),
                    retryable: true,
                },
            )
            .expect("fail canvas parent");
        }
        let child = {
            let mut conn = fixture.open_connection();
            let tx = conn.transaction().expect("begin source-aware retry");
            let result = insert_retry_job_in_transaction(
                &tx,
                &queued.job_id,
                "canvas-retry-request",
                Some(&json!({ "id": "canvas-round-2" })),
            )
            .expect("source-aware canvas retry");
            tx.commit().expect("commit source-aware retry");
            result
        };
        assert_eq!(
            fixture.get(&child.job_id).source_ref,
            json!({ "id": "canvas-round-2" })
        );

        let error = {
            let mut conn = fixture.database.conn.lock().expect("lock database");
            create_retry_job(&mut conn, &queued.job_id, "canvas-retry-request")
                .expect_err("generic replay cannot acknowledge canvas retry")
        };
        assert_eq!(stable_code(&error), "generation_job_source_unsupported");
    }

    #[test]
    fn list_cursor_is_stable_filtered_and_bounded() {
        let fixture = JobFixture::new();
        let first = fixture.enqueue("request-1", "first");
        let second = fixture.enqueue("request-2", "second");
        let third = fixture.enqueue("request-3", "third");

        let first_page = fixture.list(&GenerationJobFilter {
            limit: Some(2),
            ..GenerationJobFilter::default()
        });
        assert_eq!(
            first_page
                .items
                .iter()
                .map(|job| job.id.as_str())
                .collect::<Vec<_>>(),
            [first.id.as_str(), second.id.as_str()]
        );
        let cursor = first_page.next_cursor.expect("next cursor");
        let second_page = fixture.list(&GenerationJobFilter {
            limit: Some(2),
            cursor: Some(cursor),
            ..GenerationJobFilter::default()
        });
        assert_eq!(second_page.items.len(), 1);
        assert_eq!(second_page.items[0].id, third.id);
        assert!(second_page.next_cursor.is_none());

        let by_generation = fixture.list(&GenerationJobFilter {
            generation_id: Some(second.generation_id.clone()),
            ..GenerationJobFilter::default()
        });
        assert_eq!(by_generation.items.len(), 1);
        assert_eq!(by_generation.items[0].id, second.id);

        let by_source = fixture.list(&GenerationJobFilter {
            source_kind: Some("generate".to_string()),
            source_ref_id: Some("third".to_string()),
            ..GenerationJobFilter::default()
        });
        assert_eq!(by_source.items.len(), 1);
        assert_eq!(by_source.items[0].id, third.id);

        let zero_limit_is_bounded = fixture.list(&GenerationJobFilter {
            limit: Some(0),
            ..GenerationJobFilter::default()
        });
        assert_eq!(zero_limit_is_bounded.items.len(), 1);
    }

    #[test]
    fn list_huge_limit_is_capped_and_status_filter_is_applied() {
        let fixture = JobFixture::new();
        let mut conn = fixture.open_connection();
        let tx = conn.transaction().expect("begin batch enqueue");
        let mut conversation_id = None;
        for index in 0..=crate::models::MAX_GENERATION_JOB_PAGE_LIMIT {
            let mut request =
                fixture.prepared(&format!("request-{index}"), &format!("bounded-{index}"));
            request.requested_conversation_id = conversation_id.clone();
            let result = insert_job_in_transaction(&tx, &request).expect("insert batch job");
            conversation_id = Some(result.conversation_id);
        }
        tx.commit().expect("commit batch enqueue");

        let page = fixture.list(&GenerationJobFilter {
            limit: Some(i32::MAX),
            statuses: Some(vec![GenerationJobStatus::Queued]),
            ..GenerationJobFilter::default()
        });
        assert_eq!(
            page.items.len(),
            crate::models::MAX_GENERATION_JOB_PAGE_LIMIT as usize
        );
        assert!(page
            .items
            .iter()
            .all(|job| job.status == GenerationJobStatus::Queued));
        assert!(page.next_cursor.is_some());
    }

    #[test]
    fn malformed_or_wrong_version_cursor_is_rejected_with_stable_sanitized_error() {
        let fixture = JobFixture::new();
        fixture.enqueue("request-1", "cursor");
        for cursor in [
            "not-base64",
            "eyJ2Ijo5OSwicXVldWVkX2F0IjoieCIsInJvd2lkIjoxfQ",
        ] {
            let conn = fixture.database.conn.lock().expect("lock database");
            let error = list_jobs(
                &conn,
                &GenerationJobFilter {
                    cursor: Some(cursor.to_string()),
                    ..GenerationJobFilter::default()
                },
            )
            .expect_err("invalid cursor must fail");
            assert_eq!(stable_code(&error), "generation_job_corrupt_cursor");
            assert!(!error.to_string().contains(cursor));
        }
    }

    #[test]
    fn noncanonical_rfc3339_job_timestamps_and_cursors_are_rejected() {
        let fixture = JobFixture::new();
        for (index, timestamp) in ["2026-07-10T08:00:00+08:00", "2026-07-10T00:00:00.000Z"]
            .into_iter()
            .enumerate()
        {
            let request =
                fixture.prepared_at(&format!("request-{index}"), "noncanonical-time", timestamp);
            let error = fixture
                .enqueue_prepared(&request)
                .expect_err("queue timestamps must use canonical UTC seconds");
            assert_eq!(stable_code(&error), "generation_job_invalid_snapshot");
        }
        let cursor = URL_SAFE_NO_PAD.encode(
            serde_json::to_vec(&JobCursor {
                version: 1,
                queued_at: "2026-07-10T08:00:00+08:00".to_string(),
                rowid: 1,
            })
            .expect("serialize noncanonical cursor"),
        );
        let conn = fixture.database.conn.lock().expect("lock database");
        let error = list_jobs(
            &conn,
            &GenerationJobFilter {
                cursor: Some(cursor),
                ..GenerationJobFilter::default()
            },
        )
        .expect_err("cursor timestamps must use canonical UTC seconds");
        assert_eq!(stable_code(&error), "generation_job_corrupt_cursor");
    }

    #[test]
    fn malformed_persisted_status_or_json_returns_classified_error_without_panic() {
        let fixture = JobFixture::new();
        let first = fixture.enqueue("request-1", "corrupt-status");
        {
            let conn = fixture.database.conn.lock().expect("lock database");
            conn.execute(
                "UPDATE generation_jobs SET status = 'not-a-status' WHERE id = ?1",
                params![first.id],
            )
            .expect("corrupt status");
            let error = get_job(&conn, &first.id).expect_err("corrupt status must fail");
            assert_eq!(stable_code(&error), "generation_job_corrupt_persisted_data");
        }

        let second = fixture.enqueue("request-2", "corrupt-json");
        let conn = fixture.database.conn.lock().expect("lock database");
        conn.execute(
            "UPDATE generation_jobs SET request_json = '{broken' WHERE id = ?1",
            params![second.id],
        )
        .expect("corrupt JSON");
        let error = get_job(&conn, &second.id).expect_err("corrupt JSON must fail");
        assert_eq!(stable_code(&error), "generation_job_corrupt_persisted_data");
    }

    #[test]
    fn fabricated_terminal_state_combinations_are_rejected() {
        for (index, mutation) in [
            "status = 'completed', finished_at = '2026-07-10T00:00:02Z'",
            "status = 'failed', finished_at = '2026-07-10T00:00:02Z', \
             error_code = 'provider_unavailable', \
             error_message = 'The provider is temporarily unavailable', retryable = 1",
            "status = 'cancelled', finished_at = '2026-07-10T00:00:02Z', \
             error_code = 'cancelled_by_user', error_message = 'The operation was cancelled'",
        ]
        .into_iter()
        .enumerate()
        {
            let fixture = JobFixture::new();
            let job = fixture.enqueue(&format!("request-{index}"), "fabricated-terminal");
            let conn = fixture.database.conn.lock().expect("lock database");
            conn.execute(
                &format!("UPDATE generation_jobs SET {mutation} WHERE id = ?1"),
                params![job.id],
            )
            .expect("fabricate terminal state");
            let error = get_job(&conn, &job.id).expect_err("fabricated state must fail");
            assert_eq!(stable_code(&error), "generation_job_corrupt_persisted_data");
        }
    }

    #[test]
    fn persisted_private_or_wrong_shape_snapshot_is_rejected_before_projection() {
        let fixture = JobFixture::new();
        let wrong_shape = fixture.enqueue("request-1", "wrong-shape");
        {
            let conn = fixture.database.conn.lock().expect("lock database");
            conn.execute(
                "UPDATE generation_jobs SET request_json = 'null' WHERE id = ?1",
                params![wrong_shape.id],
            )
            .expect("replace request with wrong JSON shape");
            let error = get_job(&conn, &wrong_shape.id).expect_err("wrong shape must fail");
            assert_eq!(stable_code(&error), "generation_job_corrupt_persisted_data");
        }

        let private = fixture.enqueue("request-2", "private-persisted");
        let conn = fixture.database.conn.lock().expect("lock database");
        conn.execute(
            "UPDATE generation_jobs
             SET request_json = '{\"nested\":{\"apiKey\":\"secret-key\"}}',
                 endpoint_snapshot = 'https://example.test/images?key=secret-key'
             WHERE id = ?1",
            params![private.id],
        )
        .expect("inject private persisted snapshot");
        let error = get_job(&conn, &private.id).expect_err("private snapshot must fail");
        assert_eq!(stable_code(&error), "generation_job_corrupt_persisted_data");
        assert!(!error.to_string().contains("secret-key"));
    }

    #[test]
    fn persisted_sql_type_mismatch_is_classified_as_corrupt_data() {
        let fixture = JobFixture::new();
        let job = fixture.enqueue("request-1", "wrong-sql-type");
        {
            let conn = fixture.database.conn.lock().expect("lock database");
            conn.execute(
                "UPDATE generation_jobs SET status = 42 WHERE id = ?1",
                params![job.id],
            )
            .expect("replace status with integer");
            let error = get_job(&conn, &job.id).expect_err("wrong SQL type must fail");
            assert_eq!(stable_code(&error), "generation_job_corrupt_persisted_data");
            assert!(!error.to_string().contains("generation_jobs"));
        }

        let list_job = fixture.enqueue("request-2", "wrong-list-type");
        let conn = fixture.database.conn.lock().expect("lock database");
        conn.execute(
            "UPDATE generation_jobs SET retryable = 'not-an-integer' WHERE id = ?1",
            params![list_job.id],
        )
        .expect("replace retryable with text");
        let get_error = get_job(&conn, &list_job.id).expect_err("wrong row type must fail get");
        assert_eq!(
            stable_code(&get_error),
            "generation_job_corrupt_persisted_data"
        );
        let list_error = list_jobs(
            &conn,
            &GenerationJobFilter {
                generation_id: Some(list_job.generation_id),
                ..GenerationJobFilter::default()
            },
        )
        .expect_err("wrong row type must fail list");
        assert_eq!(
            stable_code(&list_error),
            "generation_job_corrupt_persisted_data"
        );
    }

    #[test]
    fn malformed_linked_generation_conversation_is_classified_as_corrupt_data() {
        let fixture = JobFixture::new();
        fixture.enqueue("request-1", "bad-conversation-link");
        let conn = fixture.database.conn.lock().expect("lock database");
        conn.execute_batch("PRAGMA foreign_keys=OFF;")
            .expect("disable fixture foreign keys");
        conn.execute(
            "UPDATE generations SET conversation_id = x'3432' WHERE id = (
                SELECT generation_id FROM generation_jobs WHERE client_request_id = 'request-1'
             )",
            [],
        )
        .expect("corrupt linked conversation type");
        let error = find_enqueue_result_by_client_request_id(&conn, "request-1")
            .expect_err("malformed linked conversation must fail");
        conn.execute_batch("PRAGMA foreign_keys=ON;")
            .expect("restore fixture foreign keys");
        assert_eq!(stable_code(&error), "generation_job_corrupt_persisted_data");
        assert!(!error.to_string().contains("conversation_id"));
    }

    #[test]
    fn enqueue_ack_rejects_valid_but_mismatched_linked_conversation() {
        let fixture = JobFixture::new();
        fixture.enqueue("request-1", "mismatched-conversation");
        let conn = fixture.database.conn.lock().expect("lock database");
        conn.execute(
            "INSERT INTO conversations (id, project_id, title)
             VALUES ('different-conversation', 'default', 'Different')",
            [],
        )
        .expect("insert different conversation");
        conn.execute(
            "UPDATE generations SET conversation_id = 'different-conversation' WHERE id = (
                SELECT generation_id FROM generation_jobs WHERE client_request_id = 'request-1'
             )",
            [],
        )
        .expect("mismatch linked conversation");
        let error = find_enqueue_result_by_client_request_id(&conn, "request-1")
            .expect_err("acknowledgement identity mismatch must fail");
        assert_eq!(stable_code(&error), "generation_job_corrupt_persisted_data");
    }

    #[test]
    fn terminal_update_derives_fixed_message_without_persisting_provider_text() {
        let fixture = JobFixture::new();
        let queued = fixture.enqueue("request-1", "unsafe-terminal");
        fixture.claim().expect("claim unsafe terminal fixture");
        let conn = fixture.database.conn.lock().expect("lock database");
        let finished = finish_job(
            &conn,
            &GenerationJobTerminalUpdate {
                job_id: queued.id.clone(),
                expected_status: GenerationJobStatus::Running,
                status: GenerationJobStatus::Failed,
                finished_at: "2026-07-10T00:00:02Z".to_string(),
                error_code: Some("provider_error".to_string()),
                error_message: Some("Incorrect API key provided: sk-secret".to_string()),
                retryable: true,
            },
        )
        .expect("terminal message is derived from the stable code");
        drop(conn);
        assert_eq!(finished.status, GenerationJobStatus::Failed);
        assert_eq!(finished.error_code.as_deref(), Some("provider_error"));
        assert_eq!(
            finished.error_message.as_deref(),
            Some("The generation job failed")
        );
        assert!(!serde_json::to_string(&finished)
            .expect("serialize terminal job")
            .contains("sk-secret"));
        let conn = fixture.database.conn.lock().expect("lock database");
        let generation_error: Option<String> = conn
            .query_row(
                "SELECT error_message FROM generations WHERE id = ?1",
                params![queued.generation_id],
                |row| row.get(0),
            )
            .expect("read sanitized generation error");
        assert_eq!(
            generation_error.as_deref(),
            Some("The generation job failed")
        );
    }

    #[test]
    fn terminal_update_rejects_secret_shaped_error_code_without_mutation() {
        let fixture = JobFixture::new();
        let queued = fixture.enqueue("request-1", "secret-code");
        fixture.claim().expect("claim secret-code fixture");
        let conn = fixture.database.conn.lock().expect("lock database");
        let error = finish_job(
            &conn,
            &GenerationJobTerminalUpdate {
                job_id: queued.id.clone(),
                expected_status: GenerationJobStatus::Running,
                status: GenerationJobStatus::Failed,
                finished_at: "2026-07-10T00:00:02Z".to_string(),
                error_code: Some("ghp_secret_token".to_string()),
                error_message: None,
                retryable: false,
            },
        )
        .expect_err("secret-shaped error code must fail");
        drop(conn);
        assert_eq!(stable_code(&error), "generation_job_invalid_snapshot");
        assert_eq!(fixture.get(&queued.id).status, GenerationJobStatus::Running);
    }

    #[test]
    fn missing_job_and_nonretryable_terminal_job_have_stable_codes() {
        let fixture = JobFixture::new();
        {
            let conn = fixture.database.conn.lock().expect("lock database");
            let error = get_job(&conn, "missing").expect_err("missing job must fail");
            assert_eq!(stable_code(&error), "generation_job_not_found");
        }

        let queued = fixture.enqueue("request-1", "completed");
        fixture.claim().expect("claim completed job");
        {
            let conn = fixture.database.conn.lock().expect("lock database");
            finish_job(
                &conn,
                &GenerationJobTerminalUpdate {
                    job_id: queued.id.clone(),
                    expected_status: GenerationJobStatus::Running,
                    status: GenerationJobStatus::Completed,
                    finished_at: "2026-07-10T00:00:02Z".to_string(),
                    error_code: None,
                    error_message: None,
                    retryable: false,
                },
            )
            .expect("complete job");
        }
        let error = {
            let mut conn = fixture.database.conn.lock().expect("lock database");
            create_retry_job(&mut conn, &queued.id, "retry-request")
                .expect_err("completed job cannot retry")
        };
        assert_eq!(stable_code(&error), "generation_job_not_retryable");
    }

    #[test]
    fn preflight_lookup_returns_committed_enqueue_identity() {
        let fixture = JobFixture::new();
        let job = fixture.enqueue("request-1", "lookup");
        let result = fixture.get_result("request-1").expect("existing result");
        assert_eq!(result.job_id, job.id);
        assert_eq!(result.generation_id, job.generation_id);
        assert_eq!(result.status, GenerationJobStatus::Queued);
    }

    #[test]
    fn fixture_uses_real_v16_file_database() {
        let fixture = JobFixture::new();
        assert!(Path::new(&fixture.path).is_file());
        let conn = fixture.database.conn.lock().expect("lock database");
        let version_exists: i64 = conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM schema_migrations WHERE version = 16)",
                [],
                |row| row.get(0),
            )
            .expect("read schema version");
        assert_eq!(version_exists, 1);
    }
}
