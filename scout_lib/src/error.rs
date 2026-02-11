//! Error types for ScoutAPM API client.

use thiserror::Error;

/// Base error type for ScoutAPM operations.
#[derive(Error, Debug)]
pub enum Error {
    #[error("Authentication failed: {0}")]
    Auth(#[from] AuthError),

    #[error("API error: {0}")]
    Api(#[from] ApiError),

    #[error("{0}")]
    Other(String),
}

/// Raised when authentication fails (e.g. invalid or missing API key).
#[derive(Error, Debug)]
#[error("{message}")]
pub struct AuthError {
    pub message: String,
}

/// Raised when the API returns an error response.
#[derive(Error, Debug)]
#[error("{message}")]
pub struct ApiError {
    pub message: String,
    pub status_code: Option<u16>,
    pub response_data: Option<serde_json::Value>,
}

impl ApiError {
    pub fn new(
        message: impl Into<String>,
        status_code: Option<u16>,
        response_data: Option<serde_json::Value>,
    ) -> Self {
        Self {
            message: message.into(),
            status_code,
            response_data,
        }
    }
}
