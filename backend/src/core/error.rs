//! Error type system for Ting Reader
//!
//! This module provides a comprehensive error type system with:
//! - Hierarchical error classification
//! - Error context and chaining support
//! - HTTP status code mapping
//! - Detailed error messages with trace IDs

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;

/// Main error type for the Ting Reader system
#[derive(Debug, thiserror::Error)]
pub enum TingError {
    // System-level errors
    #[error("System initialization failed: {0}")]
    InitializationError(String),

    #[error("Configuration error: {0}")]
    ConfigError(String),

    #[error("Database error: {0}")]
    DatabaseError(#[from] rusqlite::Error),

    // Plugin-related errors
    #[error("Plugin not found: {0}")]
    PluginNotFound(String),

    #[error("Plugin load failed: {0}")]
    PluginLoadError(String),

    #[error("Plugin execution error: {0}")]
    PluginExecutionError(String),

    #[error("Plugin dependency error: {0}")]
    DependencyError(String),

    // API-related errors
    #[error("Invalid request: {0}")]
    InvalidRequest(String),

    #[error("Authentication failed: {0}")]
    AuthenticationError(String),

    #[error("Resource not found: {0}")]
    NotFound(String),

    // Security-related errors
    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    #[error("Security violation: {0}")]
    SecurityViolation(String),

    // Resource-related errors
    #[error("Resource limit exceeded: {0}")]
    ResourceLimitExceeded(String),

    #[error("Timeout: {0}")]
    Timeout(String),

    // I/O errors
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Network error: {0}")]
    NetworkError(String),

    // Serialization errors
    #[error("Serialization error: {0}")]
    SerializationError(String),

    #[error("Deserialization error: {0}")]
    DeserializationError(String),

    #[error("Validation error: {0}")]
    ValidationError(String),

    // Task queue errors
    #[error("Task error: {0}")]
    TaskError(String),

    // Event bus errors
    #[error("Event error: {0}")]
    EventError(String),
}

impl TingError {
    /// Get the HTTP status code for this error
    pub fn status_code(&self) -> StatusCode {
        match self {
            // 400 Bad Request
            TingError::InvalidRequest(_) 
            | TingError::SerializationError(_) 
            | TingError::DeserializationError(_)
            | TingError::ValidationError(_) => {
                StatusCode::BAD_REQUEST
            }

            // 401 Unauthorized
            TingError::AuthenticationError(_) => StatusCode::UNAUTHORIZED,

            // 403 Forbidden
            TingError::PermissionDenied(_) | TingError::SecurityViolation(_) => {
                StatusCode::FORBIDDEN
            }

            // 404 Not Found
            TingError::NotFound(_) | TingError::PluginNotFound(_) => StatusCode::NOT_FOUND,

            // 408 Request Timeout
            TingError::Timeout(_) => StatusCode::REQUEST_TIMEOUT,

            // 429 Too Many Requests
            TingError::ResourceLimitExceeded(_) => StatusCode::TOO_MANY_REQUESTS,

            // 500 Internal Server Error
            TingError::InitializationError(_)
            | TingError::ConfigError(_)
            | TingError::DatabaseError(_)
            | TingError::PluginLoadError(_)
            | TingError::PluginExecutionError(_)
            | TingError::DependencyError(_)
            | TingError::IoError(_)
            | TingError::NetworkError(_)
            | TingError::TaskError(_)
            | TingError::EventError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    /// Get the error type name for API responses
    pub fn error_type(&self) -> &'static str {
        match self {
            TingError::InitializationError(_) => "InitializationError",
            TingError::ConfigError(_) => "ConfigError",
            TingError::DatabaseError(_) => "DatabaseError",
            TingError::PluginNotFound(_) => "PluginNotFound",
            TingError::PluginLoadError(_) => "PluginLoadError",
            TingError::PluginExecutionError(_) => "PluginExecutionError",
            TingError::DependencyError(_) => "DependencyError",
            TingError::InvalidRequest(_) => "InvalidRequest",
            TingError::AuthenticationError(_) => "AuthenticationError",
            TingError::NotFound(_) => "NotFound",
            TingError::PermissionDenied(_) => "PermissionDenied",
            TingError::SecurityViolation(_) => "SecurityViolation",
            TingError::ResourceLimitExceeded(_) => "ResourceLimitExceeded",
            TingError::Timeout(_) => "Timeout",
            TingError::IoError(_) => "IoError",
            TingError::NetworkError(_) => "NetworkError",
            TingError::SerializationError(_) => "SerializationError",
            TingError::DeserializationError(_) => "DeserializationError",
            TingError::ValidationError(_) => "ValidationError",
            TingError::TaskError(_) => "TaskError",
            TingError::EventError(_) => "EventError",
        }
    }

    /// Check if this error is retryable
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            TingError::DatabaseError(_)
                | TingError::NetworkError(_)
                | TingError::Timeout(_)
                | TingError::ResourceLimitExceeded(_)
        )
    }
}

/// Error response structure for API endpoints
#[derive(Debug, Serialize, Deserialize)]
pub struct ErrorResponse {
    /// Error type identifier
    pub error: String,
    /// Human-readable error message
    pub message: String,
    /// Optional additional details
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
    /// Unique trace ID for this error
    pub trace_id: String,
}

impl ErrorResponse {
    /// Create a new error response with a generated trace ID
    pub fn new(error: String, message: String) -> Self {
        Self {
            error,
            message,
            details: None,
            trace_id: Uuid::new_v4().to_string(),
        }
    }
    
    /// Create a new error response with a specific trace ID
    pub fn with_trace_id(error: String, message: String, trace_id: String) -> Self {
        Self {
            error,
            message,
            details: None,
            trace_id,
        }
    }

    /// Create an error response with additional details
    pub fn with_details(
        error: String,
        message: String,
        details: serde_json::Value,
    ) -> Self {
        Self {
            error,
            message,
            details: Some(details),
            trace_id: Uuid::new_v4().to_string(),
        }
    }
    
    /// Create an error response with additional details and a specific trace ID
    pub fn with_details_and_trace_id(
        error: String,
        message: String,
        details: serde_json::Value,
        trace_id: String,
    ) -> Self {
        Self {
            error,
            message,
            details: Some(details),
            trace_id,
        }
    }

    /// Create an error response from a TingError
    pub fn from_error(error: &TingError) -> Self {
        Self::new(error.error_type().to_string(), error.to_string())
    }
    
    /// Create an error response from a TingError with a specific trace ID
    pub fn from_error_with_trace_id(error: &TingError, trace_id: String) -> Self {
        Self::with_trace_id(error.error_type().to_string(), error.to_string(), trace_id)
    }

    /// Create an error response from a TingError with details
    pub fn from_error_with_details(error: &TingError, details: serde_json::Value) -> Self {
        Self::with_details(error.error_type().to_string(), error.to_string(), details)
    }
    
    /// Create an error response from a TingError with details and a specific trace ID
    pub fn from_error_with_details_and_trace_id(
        error: &TingError,
        details: serde_json::Value,
        trace_id: String,
    ) -> Self {
        Self::with_details_and_trace_id(
            error.error_type().to_string(),
            error.to_string(),
            details,
            trace_id,
        )
    }
}

impl fmt::Display for ErrorResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "[{}] {}: {} (trace_id: {})",
            self.error, self.message, self.details.as_ref().map_or("", |_| "with details"), self.trace_id
        )
    }
}

/// Implement IntoResponse for TingError to enable automatic error handling in Axum
impl IntoResponse for TingError {
    fn into_response(self) -> Response {
        let status_code = self.status_code();
        let error_response = ErrorResponse::from_error(&self);

        // Log the error with trace ID
        tracing::error!(
            error_type = self.error_type(),
            trace_id = %error_response.trace_id,
            status_code = %status_code,
            "Request failed: {}",
            self
        );

        (status_code, Json(error_response)).into_response()
    }
}

/// Result type alias for operations that can fail with TingError
pub type Result<T> = std::result::Result<T, TingError>;

/// Context extension trait for adding context to errors
pub trait ErrorContext<T> {
    /// Add context to an error
    fn context(self, context: impl Into<String>) -> Result<T>;

    /// Add context to an error using a closure
    fn with_context<F>(self, f: F) -> Result<T>
    where
        F: FnOnce() -> String;
}

impl<T, E> ErrorContext<T> for std::result::Result<T, E>
where
    E: std::fmt::Display,
{
    fn context(self, context: impl Into<String>) -> Result<T> {
        self.map_err(|e| {
            let context_str = context.into();
            TingError::InitializationError(format!("{}: {}", context_str, e))
        })
    }

    fn with_context<F>(self, f: F) -> Result<T>
    where
        F: FnOnce() -> String,
    {
        self.map_err(|e| {
            let context_str = f();
            TingError::InitializationError(format!("{}: {}", context_str, e))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_status_codes() {
        assert_eq!(
            TingError::InvalidRequest("test".into()).status_code(),
            StatusCode::BAD_REQUEST
        );
        assert_eq!(
            TingError::AuthenticationError("test".into()).status_code(),
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            TingError::PermissionDenied("test".into()).status_code(),
            StatusCode::FORBIDDEN
        );
        assert_eq!(
            TingError::NotFound("test".into()).status_code(),
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            TingError::Timeout("test".into()).status_code(),
            StatusCode::REQUEST_TIMEOUT
        );
        assert_eq!(
            TingError::DatabaseError(rusqlite::Error::InvalidQuery).status_code(),
            StatusCode::INTERNAL_SERVER_ERROR
        );
    }

    #[test]
    fn test_error_types() {
        assert_eq!(
            TingError::PluginNotFound("test".into()).error_type(),
            "PluginNotFound"
        );
        assert_eq!(
            TingError::InvalidRequest("test".into()).error_type(),
            "InvalidRequest"
        );
        assert_eq!(
            TingError::SecurityViolation("test".into()).error_type(),
            "SecurityViolation"
        );
    }

    #[test]
    fn test_error_retryable() {
        assert!(TingError::NetworkError("test".into()).is_retryable());
        assert!(TingError::Timeout("test".into()).is_retryable());
        assert!(!TingError::InvalidRequest("test".into()).is_retryable());
        assert!(!TingError::PermissionDenied("test".into()).is_retryable());
    }

    #[test]
    fn test_error_response_creation() {
        let error = TingError::PluginNotFound("test-plugin".into());
        let response = ErrorResponse::from_error(&error);

        assert_eq!(response.error, "PluginNotFound");
        assert!(response.message.contains("test-plugin"));
        assert!(!response.trace_id.is_empty());
        assert!(response.details.is_none());
    }
    
    #[test]
    fn test_error_response_with_trace_id() {
        let error = TingError::PluginNotFound("test-plugin".into());
        let trace_id = "test-trace-id-123".to_string();
        let response = ErrorResponse::from_error_with_trace_id(&error, trace_id.clone());

        assert_eq!(response.error, "PluginNotFound");
        assert!(response.message.contains("test-plugin"));
        assert_eq!(response.trace_id, trace_id);
        assert!(response.details.is_none());
    }

    #[test]
    fn test_error_response_with_details() {
        let details = serde_json::json!({
            "plugin_id": "test-plugin",
            "available_plugins": ["plugin1", "plugin2"]
        });

        let response = ErrorResponse::with_details(
            "PluginNotFound".into(),
            "Plugin not found".into(),
            details.clone(),
        );

        assert_eq!(response.error, "PluginNotFound");
        assert_eq!(response.message, "Plugin not found");
        assert_eq!(response.details, Some(details));
    }
    
    #[test]
    fn test_error_response_with_details_and_trace_id() {
        let details = serde_json::json!({
            "plugin_id": "test-plugin",
            "available_plugins": ["plugin1", "plugin2"]
        });
        let trace_id = "test-trace-id-456".to_string();

        let response = ErrorResponse::from_error_with_details_and_trace_id(
            &TingError::PluginNotFound("test-plugin".into()),
            details.clone(),
            trace_id.clone(),
        );

        assert_eq!(response.error, "PluginNotFound");
        assert!(response.message.contains("test-plugin"));
        assert_eq!(response.details, Some(details));
        assert_eq!(response.trace_id, trace_id);
    }

    #[test]
    fn test_error_context() {
        let result: std::result::Result<(), std::io::Error> =
            Err(std::io::Error::new(std::io::ErrorKind::NotFound, "file not found"));

        let contexted = result.context("Failed to read plugin metadata");

        assert!(contexted.is_err());
        let err = contexted.unwrap_err();
        assert!(err.to_string().contains("Failed to read plugin metadata"));
        assert!(err.to_string().contains("file not found"));
    }
}
