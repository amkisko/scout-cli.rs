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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn api_error_new() {
        let e = ApiError::new("bad request", Some(400), None);
        assert_eq!(e.message, "bad request");
        assert_eq!(e.status_code, Some(400));
        assert!(e.response_data.is_none());
    }

    #[test]
    fn api_error_display() {
        let e = ApiError::new("not found", Some(404), None);
        assert_eq!(e.to_string(), "not found");
    }

    #[test]
    fn auth_error_display() {
        let e = AuthError {
            message: "invalid key".to_string(),
        };
        assert_eq!(e.to_string(), "invalid key");
    }

    #[test]
    fn error_from_auth() {
        let auth = AuthError {
            message: "unauthorized".to_string(),
        };
        let e: Error = auth.into();
        assert!(matches!(e, Error::Auth(_)));
        assert!(e.to_string().contains("Authentication failed"));
    }

    #[test]
    fn error_from_api() {
        let api = ApiError::new("server error", Some(500), None);
        let e: Error = api.into();
        assert!(matches!(e, Error::Api(_)));
        assert!(e.to_string().contains("API error"));
    }
}
