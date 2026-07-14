use serde::{Deserialize, Serialize};

#[derive(Debug, thiserror::Error, Serialize, Deserialize)]
#[serde(tag = "kind", content = "detail")]
pub enum AppError {
    #[error("API key not configured for model '{model}'")]
    ApiKeyNotSet { model: String },

    #[error("Active provider profile not found for model '{model}'")]
    ProviderProfileNotFound { model: String },

    #[error("API returned {status}: {body_preview}")]
    Api {
        status: u16,
        endpoint: String,
        body_preview: String,
    },

    #[error("Request to {endpoint} failed: {reason}")]
    Network { endpoint: String, reason: String },

    #[error("Database error: {message}")]
    Database { message: String },

    #[error("File system error: {message}")]
    FileSystem { message: String },

    #[error("{message}")]
    Validation { message: String },

    #[error("Generation job was not found")]
    GenerationJobNotFound,

    #[error("Generation job state changed before the operation completed")]
    GenerationJobInvalidTransition,

    #[error("Generation job cannot be retried")]
    GenerationJobNotRetryable,

    #[error("Stored generation job data is invalid")]
    GenerationJobCorruptPersistedData,

    #[error("Client request ID is already associated with a different operation")]
    GenerationJobIdempotencyConflict,

    #[error("This generation source requires a source-aware retry")]
    GenerationJobUnsupportedSource,

    #[error("Generation job cursor is invalid or expired")]
    GenerationJobCorruptCursor,

    #[error("Generation job snapshot contains invalid or private data")]
    GenerationJobInvalidSnapshot,
}

impl AppError {
    pub fn stable_code(&self) -> &'static str {
        match self {
            AppError::ApiKeyNotSet { .. } => "api_key_not_set",
            AppError::ProviderProfileNotFound { .. } => "provider_profile_not_found",
            AppError::Api { .. } => "api_error",
            AppError::Network { .. } => "network_error",
            AppError::Database { .. } => "database_error",
            AppError::FileSystem { .. } => "file_system_error",
            AppError::Validation { .. } => "validation_error",
            AppError::GenerationJobNotFound => "generation_job_not_found",
            AppError::GenerationJobInvalidTransition => "generation_job_invalid_transition",
            AppError::GenerationJobNotRetryable => "generation_job_not_retryable",
            AppError::GenerationJobCorruptPersistedData => "generation_job_corrupt_persisted_data",
            AppError::GenerationJobIdempotencyConflict => "generation_job_idempotency_conflict",
            AppError::GenerationJobUnsupportedSource => "generation_job_source_unsupported",
            AppError::GenerationJobCorruptCursor => "generation_job_corrupt_cursor",
            AppError::GenerationJobInvalidSnapshot => "generation_job_invalid_snapshot",
        }
    }
}

impl From<rusqlite::Error> for AppError {
    fn from(e: rusqlite::Error) -> Self {
        AppError::Database {
            message: e.to_string(),
        }
    }
}

impl From<std::io::Error> for AppError {
    fn from(e: std::io::Error) -> Self {
        AppError::FileSystem {
            message: e.to_string(),
        }
    }
}

impl From<serde_json::Error> for AppError {
    fn from(e: serde_json::Error) -> Self {
        AppError::Database {
            message: format!("JSON serialization error: {}", e),
        }
    }
}

impl From<reqwest::Error> for AppError {
    fn from(e: reqwest::Error) -> Self {
        AppError::Network {
            endpoint: String::new(),
            reason: e.to_string(),
        }
    }
}

impl From<String> for AppError {
    fn from(s: String) -> Self {
        AppError::Validation { message: s }
    }
}
