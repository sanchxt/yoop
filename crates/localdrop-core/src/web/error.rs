//! HTTP error handling for the web API.
//!
//! This module provides conversion from core library errors to appropriate
//! HTTP responses with JSON error bodies.

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;

/// API error response body.
#[derive(Debug, Clone, Serialize)]
pub struct ApiError {
    /// Error code (e.g., "E003" for code not found)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    /// Human-readable error message
    pub message: String,
    /// Additional details about the error
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<String>,
}

impl ApiError {
    /// Create a new API error with a message.
    #[must_use]
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            code: None,
            message: message.into(),
            details: None,
        }
    }

    /// Create a new API error with code and message.
    #[must_use]
    pub fn with_code(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: Some(code.into()),
            message: message.into(),
            details: None,
        }
    }

    /// Add details to the error.
    #[must_use]
    pub fn with_details(mut self, details: impl Into<String>) -> Self {
        self.details = Some(details.into());
        self
    }

    /// Get the HTTP status code for this error.
    #[must_use]
    pub fn status_code(&self) -> StatusCode {
        match self.code.as_deref() {
            Some("E001" | "E002") => StatusCode::SERVICE_UNAVAILABLE,
            Some("E003") => StatusCode::NOT_FOUND,
            Some("E004") => StatusCode::GONE,
            Some("E005") => StatusCode::BAD_GATEWAY,
            Some("E006") => StatusCode::UNPROCESSABLE_ENTITY,
            Some("E007" | "E010") => StatusCode::FORBIDDEN,
            Some("E008") => StatusCode::INSUFFICIENT_STORAGE,
            Some("E009") => StatusCode::TOO_MANY_REQUESTS,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    /// Create a bad request error.
    #[must_use]
    pub fn bad_request(message: impl Into<String>) -> Self {
        Self {
            code: None,
            message: message.into(),
            details: None,
        }
    }

    /// Create a not found error.
    #[must_use]
    pub fn not_found(message: impl Into<String>) -> Self {
        Self {
            code: Some("E003".into()),
            message: message.into(),
            details: None,
        }
    }

    /// Create a conflict error (operation already in progress).
    #[must_use]
    pub fn conflict(message: impl Into<String>) -> Self {
        Self {
            code: None,
            message: message.into(),
            details: None,
        }
    }

    /// Create a service unavailable error.
    #[must_use]
    pub fn service_unavailable(message: impl Into<String>) -> Self {
        Self {
            code: None,
            message: message.into(),
            details: None,
        }
    }

    /// Create an internal server error.
    #[must_use]
    pub fn internal(message: impl Into<String>) -> Self {
        Self {
            code: None,
            message: message.into(),
            details: None,
        }
    }

    /// Create a gone error (resource expired).
    #[must_use]
    pub fn gone(message: impl Into<String>) -> Self {
        Self {
            code: Some("E004".into()),
            message: message.into(),
            details: None,
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status = self.status_code();

        let status = match self.code.as_deref() {
            None if self.message.contains("already") => StatusCode::CONFLICT,
            None if self.message.contains("not found") => StatusCode::NOT_FOUND,
            None if self.message.contains("invalid") => StatusCode::BAD_REQUEST,
            _ => status,
        };

        (status, Json(self)).into_response()
    }
}

impl From<crate::error::Error> for ApiError {
    fn from(err: crate::error::Error) -> Self {
        Self {
            code: err.code().map(String::from),
            message: err.to_string(),
            details: None,
        }
    }
}

impl From<std::io::Error> for ApiError {
    fn from(err: std::io::Error) -> Self {
        Self {
            code: None,
            message: format!("I/O error: {err}"),
            details: None,
        }
    }
}

/// Result type for web handlers.
pub type ApiResult<T> = Result<T, ApiError>;

/// Extension trait for converting Results to ApiResults.
pub trait IntoApiResult<T> {
    /// Convert to an API result.
    #[allow(clippy::missing_errors_doc)]
    fn into_api_result(self) -> ApiResult<T>;
}

impl<T> IntoApiResult<T> for crate::error::Result<T> {
    fn into_api_result(self) -> ApiResult<T> {
        self.map_err(ApiError::from)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_api_error_creation() {
        let err = ApiError::new("Something went wrong");
        assert!(err.code.is_none());
        assert_eq!(err.message, "Something went wrong");
        assert!(err.details.is_none());
    }

    #[test]
    fn test_api_error_with_code() {
        let err = ApiError::with_code("E003", "Code not found");
        assert_eq!(err.code, Some("E003".into()));
        assert_eq!(err.message, "Code not found");
    }

    #[test]
    fn test_api_error_with_details() {
        let err = ApiError::new("Error").with_details("More info");
        assert_eq!(err.details, Some("More info".into()));
    }

    #[test]
    fn test_status_code_mapping() {
        assert_eq!(
            ApiError::with_code("E003", "").status_code(),
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            ApiError::with_code("E004", "").status_code(),
            StatusCode::GONE
        );
        assert_eq!(
            ApiError::with_code("E007", "").status_code(),
            StatusCode::FORBIDDEN
        );
        assert_eq!(
            ApiError::with_code("E008", "").status_code(),
            StatusCode::INSUFFICIENT_STORAGE
        );
        assert_eq!(
            ApiError::with_code("E009", "").status_code(),
            StatusCode::TOO_MANY_REQUESTS
        );
    }

    #[test]
    fn test_from_core_error() {
        let core_err = crate::error::Error::CodeNotFound("ABCD".into());
        let api_err: ApiError = core_err.into();
        assert_eq!(api_err.code, Some("E003".into()));
        assert!(api_err.message.contains("ABCD"));
    }

    #[test]
    fn test_serialization() {
        let err = ApiError::with_code("E003", "Code not found");
        let json = serde_json::to_string(&err).unwrap();
        assert!(json.contains("\"code\":\"E003\""));
        assert!(json.contains("\"message\":\"Code not found\""));
        assert!(!json.contains("details"));
    }

    #[test]
    fn test_serialization_with_details() {
        let err = ApiError::new("Error").with_details("Extra info");
        let json = serde_json::to_string(&err).unwrap();
        assert!(json.contains("\"details\":\"Extra info\""));
    }
}
