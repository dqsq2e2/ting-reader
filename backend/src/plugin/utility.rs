//! Utility plugin interface
//!
//! This module defines the interface for utility plugins that provide auxiliary
//! functionality and enhancements to the system.
//!
//! Utility plugins must implement the `UtilityPlugin` trait in addition to the base
//! `Plugin` trait. They provide functionality for:
//! - Declaring custom capabilities
//! - JSON-based invocation interface
//! - Registering custom API endpoints
//! - Subscribing to system events
//! - Supporting user configuration

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use crate::core::error::Result;
use super::Plugin;

/// Utility plugin trait
///
/// All utility plugins must implement this trait to provide auxiliary
/// functionality and system enhancements.
#[async_trait]
pub trait UtilityPlugin: Plugin {
    /// Get the list of capabilities provided by this plugin
    ///
    /// Capabilities describe the functions that this plugin provides,
    /// including their names, descriptions, and input/output schemas.
    ///
    /// # Returns
    /// A vector of capability definitions
    fn capabilities(&self) -> Vec<Capability>;

    /// Invoke a plugin capability with JSON parameters
    ///
    /// # Arguments
    /// * `method` - Name of the capability/method to invoke
    /// * `params` - JSON parameters for the method
    ///
    /// # Returns
    /// JSON result from the method execution
    ///
    /// # Errors
    /// Returns an error if:
    /// - The method name is not recognized
    /// - The parameters don't match the expected schema
    /// - The method execution fails
    async fn invoke(&self, method: &str, params: Value) -> Result<Value>;

    /// Register custom API endpoints (optional)
    ///
    /// Plugins can register custom HTTP endpoints that will be mounted
    /// under `/api/v1/plugins/{plugin_name}/`.
    ///
    /// # Returns
    /// A vector of endpoint definitions, or an empty vector if no endpoints
    fn register_endpoints(&self) -> Vec<Endpoint> {
        Vec::new()
    }

    /// Subscribe to system events (optional)
    ///
    /// Plugins can subscribe to system events to react to changes in the system.
    ///
    /// # Returns
    /// A vector of event types to subscribe to, or an empty vector if no subscriptions
    fn subscribe_events(&self) -> Vec<EventType> {
        Vec::new()
    }

    /// Handle a system event
    ///
    /// This method is called when an event that the plugin subscribed to is published.
    ///
    /// # Arguments
    /// * `event` - The event that was published
    ///
    /// # Returns
    /// `Ok(())` if the event was handled successfully
    ///
    /// # Errors
    /// Returns an error if event handling fails. Errors are logged but don't
    /// prevent other event handlers from running.
    async fn handle_event(&self, event: &Event) -> Result<()> {
        // Default implementation does nothing
        let _ = event;
        Ok(())
    }
}

/// Capability definition
///
/// Describes a function or feature provided by a utility plugin.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Capability {
    /// Capability name (used as the method name in invoke)
    pub name: String,

    /// Human-readable description of what this capability does
    pub description: String,

    /// JSON Schema for the input parameters
    ///
    /// This should be a valid JSON Schema object describing the expected
    /// structure and types of the input parameters.
    pub input_schema: Value,

    /// JSON Schema for the output result
    ///
    /// This should be a valid JSON Schema object describing the structure
    /// and types of the output result.
    pub output_schema: Value,
}

impl Capability {
    /// Create a new capability definition
    pub fn new(
        name: String,
        description: String,
        input_schema: Value,
        output_schema: Value,
    ) -> Self {
        Self {
            name,
            description,
            input_schema,
            output_schema,
        }
    }
}

/// Custom API endpoint definition
///
/// Allows plugins to register custom HTTP endpoints.
#[derive(Clone)]
pub struct Endpoint {
    /// HTTP method (GET, POST, PUT, DELETE)
    pub method: HttpMethod,

    /// Endpoint path (relative to `/api/v1/plugins/{plugin_name}/`)
    ///
    /// For example, if path is "status", the full path will be
    /// `/api/v1/plugins/my-plugin/status`
    pub path: String,

    /// Handler function for this endpoint
    pub handler: EndpointHandler,
}

impl Endpoint {
    /// Create a new endpoint definition
    pub fn new(method: HttpMethod, path: String, handler: EndpointHandler) -> Self {
        Self {
            method,
            path,
            handler,
        }
    }
}

/// HTTP method enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum HttpMethod {
    /// HTTP GET method
    Get,
    /// HTTP POST method
    Post,
    /// HTTP PUT method
    Put,
    /// HTTP DELETE method
    Delete,
}

impl std::fmt::Display for HttpMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HttpMethod::Get => write!(f, "GET"),
            HttpMethod::Post => write!(f, "POST"),
            HttpMethod::Put => write!(f, "PUT"),
            HttpMethod::Delete => write!(f, "DELETE"),
        }
    }
}

/// HTTP request representation for plugin endpoints
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Request {
    /// Request method
    pub method: HttpMethod,

    /// Request path
    pub path: String,

    /// Query parameters
    #[serde(default)]
    pub query: std::collections::HashMap<String, String>,

    /// Request headers
    #[serde(default)]
    pub headers: std::collections::HashMap<String, String>,

    /// Request body (JSON)
    #[serde(default)]
    pub body: Option<Value>,
}

/// HTTP response representation for plugin endpoints
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Response {
    /// HTTP status code
    pub status: u16,

    /// Response headers
    #[serde(default)]
    pub headers: std::collections::HashMap<String, String>,

    /// Response body (JSON)
    #[serde(default)]
    pub body: Option<Value>,
}

impl Response {
    /// Create a successful response with JSON body
    pub fn ok(body: Value) -> Self {
        Self {
            status: 200,
            headers: std::collections::HashMap::new(),
            body: Some(body),
        }
    }

    /// Create a response with a specific status code
    pub fn with_status(status: u16, body: Option<Value>) -> Self {
        Self {
            status,
            headers: std::collections::HashMap::new(),
            body,
        }
    }

    /// Create an error response
    pub fn error(status: u16, message: &str) -> Self {
        Self {
            status,
            headers: std::collections::HashMap::new(),
            body: Some(serde_json::json!({
                "error": message
            })),
        }
    }
}

/// Endpoint handler function type
///
/// Handlers receive a Request and return a Future that resolves to a Response.
pub type EndpointHandler = Arc<
    dyn Fn(Request) -> Pin<Box<dyn Future<Output = Result<Response>> + Send>> + Send + Sync
>;

/// System event type enumeration
///
/// Defines the types of events that plugins can subscribe to.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventType {
    /// System started event
    SystemStarted,

    /// System shutdown event
    SystemShutdown,

    /// Plugin loaded event
    PluginLoaded,

    /// Plugin unloaded event
    PluginUnloaded,

    /// Plugin error event
    PluginError,

    /// Task submitted event
    TaskSubmitted,

    /// Task completed event
    TaskCompleted,

    /// Task failed event
    TaskFailed,

    /// Book added event
    BookAdded,

    /// Book updated event
    BookUpdated,

    /// Book deleted event
    BookDeleted,

    /// File added event
    FileAdded,

    /// File deleted event
    FileDeleted,

    /// Custom event (plugin-defined)
    Custom(String),
}

impl std::fmt::Display for EventType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EventType::SystemStarted => write!(f, "system_started"),
            EventType::SystemShutdown => write!(f, "system_shutdown"),
            EventType::PluginLoaded => write!(f, "plugin_loaded"),
            EventType::PluginUnloaded => write!(f, "plugin_unloaded"),
            EventType::PluginError => write!(f, "plugin_error"),
            EventType::TaskSubmitted => write!(f, "task_submitted"),
            EventType::TaskCompleted => write!(f, "task_completed"),
            EventType::TaskFailed => write!(f, "task_failed"),
            EventType::BookAdded => write!(f, "book_added"),
            EventType::BookUpdated => write!(f, "book_updated"),
            EventType::BookDeleted => write!(f, "book_deleted"),
            EventType::FileAdded => write!(f, "file_added"),
            EventType::FileDeleted => write!(f, "file_deleted"),
            EventType::Custom(name) => write!(f, "custom:{}", name),
        }
    }
}

/// System event
///
/// Represents an event that occurred in the system.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    /// Unique event identifier
    pub id: String,

    /// Event type
    pub event_type: EventType,

    /// Timestamp when the event occurred (Unix timestamp)
    pub timestamp: i64,

    /// Source of the event
    pub source: EventSource,

    /// Event data (JSON)
    pub data: Value,
}

impl Event {
    /// Create a new event
    pub fn new(event_type: EventType, source: EventSource, data: Value) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            event_type,
            timestamp: chrono::Utc::now().timestamp(),
            source,
            data,
        }
    }
}

/// Event source enumeration
///
/// Identifies where an event originated from.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventSource {
    /// Event from the core system
    System,

    /// Event from a plugin
    Plugin(String),

    /// Event from a user action
    User(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_capability_serialization() {
        let capability = Capability {
            name: "test_method".to_string(),
            description: "A test method".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "param1": {"type": "string"}
                }
            }),
            output_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "result": {"type": "string"}
                }
            }),
        };

        let json = serde_json::to_string(&capability).unwrap();
        let deserialized: Capability = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.name, "test_method");
        assert_eq!(deserialized.description, "A test method");
    }

    #[test]
    fn test_http_method_display() {
        assert_eq!(HttpMethod::Get.to_string(), "GET");
        assert_eq!(HttpMethod::Post.to_string(), "POST");
        assert_eq!(HttpMethod::Put.to_string(), "PUT");
        assert_eq!(HttpMethod::Delete.to_string(), "DELETE");
    }

    #[test]
    fn test_request_serialization() {
        let mut query = std::collections::HashMap::new();
        query.insert("key".to_string(), "value".to_string());

        let request = Request {
            method: HttpMethod::Get,
            path: "/test".to_string(),
            query,
            headers: std::collections::HashMap::new(),
            body: None,
        };

        let json = serde_json::to_string(&request).unwrap();
        let deserialized: Request = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.method, HttpMethod::Get);
        assert_eq!(deserialized.path, "/test");
        assert_eq!(deserialized.query.get("key"), Some(&"value".to_string()));
    }

    #[test]
    fn test_response_ok() {
        let response = Response::ok(serde_json::json!({"status": "success"}));

        assert_eq!(response.status, 200);
        assert!(response.body.is_some());
    }

    #[test]
    fn test_response_error() {
        let response = Response::error(404, "Not found");

        assert_eq!(response.status, 404);
        assert!(response.body.is_some());
    }

    #[test]
    fn test_event_type_display() {
        assert_eq!(EventType::SystemStarted.to_string(), "system_started");
        assert_eq!(EventType::PluginLoaded.to_string(), "plugin_loaded");
        assert_eq!(EventType::BookAdded.to_string(), "book_added");
        assert_eq!(
            EventType::Custom("my_event".to_string()).to_string(),
            "custom:my_event"
        );
    }

    #[test]
    fn test_event_creation() {
        let event = Event::new(
            EventType::BookAdded,
            EventSource::System,
            serde_json::json!({"book_id": 123}),
        );

        assert_eq!(event.event_type, EventType::BookAdded);
        assert!(!event.id.is_empty());
        assert!(event.timestamp > 0);
    }

    #[test]
    fn test_event_serialization() {
        let event = Event {
            id: "test-id".to_string(),
            event_type: EventType::TaskCompleted,
            timestamp: 1234567890,
            source: EventSource::Plugin("test-plugin".to_string()),
            data: serde_json::json!({"task_id": "task-123"}),
        };

        let json = serde_json::to_string(&event).unwrap();
        let deserialized: Event = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.id, "test-id");
        assert_eq!(deserialized.event_type, EventType::TaskCompleted);
        assert_eq!(deserialized.timestamp, 1234567890);
    }

    #[test]
    fn test_event_source_serialization() {
        let sources = vec![
            EventSource::System,
            EventSource::Plugin("my-plugin".to_string()),
            EventSource::User("user123".to_string()),
        ];

        for source in sources {
            let json = serde_json::to_string(&source).unwrap();
            let deserialized: EventSource = serde_json::from_str(&json).unwrap();

            match (&source, &deserialized) {
                (EventSource::System, EventSource::System) => (),
                (EventSource::Plugin(a), EventSource::Plugin(b)) => assert_eq!(a, b),
                (EventSource::User(a), EventSource::User(b)) => assert_eq!(a, b),
                _ => panic!("Deserialization mismatch"),
            }
        }
    }
}
