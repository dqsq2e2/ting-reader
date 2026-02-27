use serde::{Deserialize, Serialize};

// Metrics API models

/// Response for metrics endpoint
#[derive(Debug, Serialize)]
pub struct MetricsResponse {
    /// System-level metrics
    pub system: SystemMetrics,
    /// Plugin-level metrics
    pub plugins: Vec<PluginMetrics>,
    /// Task queue metrics
    pub task_queue: TaskQueueMetrics,
    /// Database metrics
    pub database: DatabaseMetrics,
    /// Timestamp of metrics collection
    pub timestamp: String,
}

/// System-level metrics
#[derive(Debug, Serialize)]
pub struct SystemMetrics {
    /// Total number of HTTP requests
    pub total_requests: u64,
    /// Average response time in milliseconds
    pub avg_response_time_ms: f64,
    /// Total number of errors
    pub total_errors: u64,
    /// Error rate (errors / total requests)
    pub error_rate: f64,
    /// System uptime in seconds
    pub uptime_seconds: u64,
}

/// Plugin-level metrics
#[derive(Debug, Serialize)]
pub struct PluginMetrics {
    /// Plugin ID
    pub plugin_id: String,
    /// Plugin name
    pub plugin_name: String,
    /// Total number of calls
    pub total_calls: u64,
    /// Number of successful calls
    pub successful_calls: u64,
    /// Number of failed calls
    pub failed_calls: u64,
    /// Success rate (successful / total)
    pub success_rate: f64,
    /// Minimum execution time in milliseconds
    pub min_execution_time_ms: Option<u64>,
    /// Maximum execution time in milliseconds
    pub max_execution_time_ms: Option<u64>,
    /// Average execution time in milliseconds
    pub avg_execution_time_ms: Option<f64>,
    /// P95 execution time in milliseconds
    pub p95_execution_time_ms: Option<u64>,
    /// Current memory usage in bytes
    pub memory_usage_bytes: Option<u64>,
    /// Peak memory usage in bytes
    pub peak_memory_bytes: Option<u64>,
    /// Error distribution (error type -> count)
    pub error_distribution: std::collections::HashMap<String, u64>,
}

/// Task queue metrics
#[derive(Debug, Serialize)]
pub struct TaskQueueMetrics {
    /// Number of queued tasks
    pub queued_tasks: usize,
    /// Number of running tasks
    pub running_tasks: usize,
    /// Number of completed tasks
    pub completed_tasks: usize,
    /// Number of failed tasks
    pub failed_tasks: usize,
    /// Number of cancelled tasks
    pub cancelled_tasks: usize,
    /// Total number of tasks
    pub total_tasks: usize,
    /// Average task processing time in milliseconds
    pub avg_processing_time_ms: f64,
    /// Task failure rate (failed / total)
    pub failure_rate: f64,
}

/// Database metrics
#[derive(Debug, Serialize)]
pub struct DatabaseMetrics {
    /// Number of active connections
    pub active_connections: u32,
    /// Number of idle connections
    pub idle_connections: u32,
    /// Total number of queries executed
    pub total_queries: u64,
    /// Average query execution time in milliseconds
    pub avg_query_time_ms: f64,
}

// Configuration Management API models

/// Response for GET /api/v1/config - Get system configuration
#[derive(Debug, Serialize)]
pub struct ConfigResponse {
    /// Server configuration
    pub server: ServerConfigResponse,
    /// Database configuration
    pub database: DatabaseConfigResponse,
    /// Plugin configuration
    pub plugins: PluginSystemConfigResponse,
    /// Task queue configuration
    pub task_queue: TaskQueueConfigResponse,
    /// Logging configuration
    pub logging: LoggingConfigResponse,
    /// Security configuration (sensitive fields masked)
    pub security: SecurityConfigResponse,
    /// Storage configuration
    pub storage: StorageConfigResponse,
}

/// Server configuration response
#[derive(Debug, Serialize)]
pub struct ServerConfigResponse {
    pub host: String,
    pub port: u16,
    pub max_connections: usize,
    pub request_timeout: u64,
}

/// Database configuration response
#[derive(Debug, Serialize)]
pub struct DatabaseConfigResponse {
    pub path: String,
    pub connection_pool_size: usize,
    pub busy_timeout: u64,
}

/// Plugin configuration response
#[derive(Debug, Serialize)]
pub struct PluginSystemConfigResponse {
    pub plugin_dir: String,
    pub enable_hot_reload: bool,
    pub max_memory_per_plugin: usize,
    pub max_execution_time: u64,
}

/// Task queue configuration response
#[derive(Debug, Serialize)]
pub struct TaskQueueConfigResponse {
    pub max_concurrent_tasks: usize,
    pub default_retry_count: u32,
    pub task_timeout: u64,
}

/// Logging configuration response
#[derive(Debug, Serialize)]
pub struct LoggingConfigResponse {
    pub level: String,
    pub format: String,
    pub output: String,
    pub log_file: Option<String>,
    pub max_file_size: usize,
    pub max_backups: usize,
}

/// Security configuration response
#[derive(Debug, Serialize)]
pub struct SecurityConfigResponse {
    pub enable_auth: bool,
    pub api_key: Option<String>,
    pub allowed_origins: Vec<String>,
    pub rate_limit_requests: usize,
    pub rate_limit_window: u64,
    pub enable_hsts: bool,
    pub hsts_max_age: u64,
}

/// Storage configuration response
#[derive(Debug, Serialize)]
pub struct StorageConfigResponse {
    pub data_dir: String,
    pub temp_dir: String,
    pub local_storage_root: String,
    pub max_disk_usage: u64,
}

/// Request for PUT /api/v1/config - Update system configuration
#[derive(Debug, Deserialize)]
pub struct UpdateConfigRequest {
    /// Server configuration (optional)
    pub server: Option<UpdateServerConfigRequest>,
    /// Database configuration (optional)
    pub database: Option<UpdateDatabaseConfigRequest>,
    /// Plugin configuration (optional)
    pub plugins: Option<UpdatePluginSystemConfigRequest>,
    /// Task queue configuration (optional)
    pub task_queue: Option<UpdateTaskQueueConfigRequest>,
    /// Logging configuration (optional)
    pub logging: Option<UpdateLoggingConfigRequest>,
    /// Security configuration (optional)
    pub security: Option<UpdateSecurityConfigRequest>,
    /// Storage configuration (optional)
    pub storage: Option<UpdateStorageConfigRequest>,
}

/// Server configuration update request
#[derive(Debug, Deserialize)]
pub struct UpdateServerConfigRequest {
    pub host: Option<String>,
    pub port: Option<u16>,
    pub max_connections: Option<usize>,
    pub request_timeout: Option<u64>,
}

/// Database configuration update request
#[derive(Debug, Deserialize)]
pub struct UpdateDatabaseConfigRequest {
    pub path: Option<String>,
    pub connection_pool_size: Option<usize>,
    pub busy_timeout: Option<u64>,
}

/// Plugin configuration update request
#[derive(Debug, Deserialize)]
pub struct UpdatePluginSystemConfigRequest {
    pub plugin_dir: Option<String>,
    pub enable_hot_reload: Option<bool>,
    pub max_memory_per_plugin: Option<usize>,
    pub max_execution_time: Option<u64>,
}

/// Task queue configuration update request
#[derive(Debug, Deserialize)]
pub struct UpdateTaskQueueConfigRequest {
    pub max_concurrent_tasks: Option<usize>,
    pub default_retry_count: Option<u32>,
    pub task_timeout: Option<u64>,
}

/// Logging configuration update request
#[derive(Debug, Deserialize)]
pub struct UpdateLoggingConfigRequest {
    pub level: Option<String>,
    pub format: Option<String>,
    pub output: Option<String>,
    pub log_file: Option<String>,
    pub max_file_size: Option<usize>,
    pub max_backups: Option<usize>,
}

/// Security configuration update request
#[derive(Debug, Deserialize)]
pub struct UpdateSecurityConfigRequest {
    pub enable_auth: Option<bool>,
    pub api_key: Option<String>,
    pub allowed_origins: Option<Vec<String>>,
    pub rate_limit_requests: Option<usize>,
    pub rate_limit_window: Option<u64>,
    pub enable_hsts: Option<bool>,
    pub hsts_max_age: Option<u64>,
}

/// Storage configuration update request
#[derive(Debug, Deserialize)]
pub struct UpdateStorageConfigRequest {
    pub data_dir: Option<String>,
    pub temp_dir: Option<String>,
    pub max_disk_usage: Option<u64>,
}

/// Response for PUT /api/v1/config - Configuration update result
#[derive(Debug, Serialize)]
pub struct UpdateConfigResponse {
    /// Success message
    pub message: String,
    /// List of configuration parameters that were updated
    pub updated_fields: Vec<String>,
    /// List of configuration parameters that require restart
    pub requires_restart: Vec<String>,
}

/// Health check response
#[derive(Debug, Serialize)]
pub struct HealthResponse {
    /// Overall system status
    pub status: HealthStatus,
    /// Individual component health statuses
    pub components: ComponentsHealth,
    /// Timestamp of the health check
    pub timestamp: String,
}

/// Overall health status
#[derive(Debug, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum HealthStatus {
    /// All components are healthy
    Healthy,
    /// One or more components are unhealthy
    Unhealthy,
}

/// Health status of individual components
#[derive(Debug, Serialize)]
pub struct ComponentsHealth {
    /// Database health status
    pub database: ComponentHealth,
    /// Plugin system health status
    pub plugin_system: ComponentHealth,
}

/// Health status of a single component
#[derive(Debug, Serialize)]
pub struct ComponentHealth {
    /// Component status
    pub status: ComponentStatus,
    /// Optional message with details
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    /// Optional additional details
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
}

/// Component status
#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ComponentStatus {
    /// Component is healthy
    Healthy,
    /// Component is unhealthy
    Unhealthy,
}
