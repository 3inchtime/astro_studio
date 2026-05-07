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
    Network {
        endpoint: String,
        reason: String,
    },

    #[error("Database error: {message}")]
    Database { message: String },

    #[error("File system error: {message}")]
    FileSystem { message: String },

    #[error("{message}")]
    Validation { message: String },
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
