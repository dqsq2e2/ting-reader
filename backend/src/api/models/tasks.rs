use serde::{Deserialize, Serialize};

// Task Management API models

/// Query parameters for task list
#[derive(Debug, Deserialize)]
pub struct TasksQuery {
    /// Filter by status (queued, running, completed, failed, cancelled)
    pub status: Option<String>,
    /// Page number (1-indexed, default: 1)
    #[serde(default = "default_page")]
    pub page: u32,
    /// Page size (default: 20)
    #[serde(default = "default_page_size")]
    pub page_size: u32,
    /// Sort by field (created_at, status, task_type)
    #[serde(default = "default_sort_by")]
    pub sort_by: String,
    /// Sort order (asc, desc)
    #[serde(default = "default_sort_order")]
    pub sort_order: String,
}

fn default_page() -> u32 {
    1
}

fn default_page_size() -> u32 {
    20
}

fn default_sort_by() -> String {
    "created_at".to_string()
}

fn default_sort_order() -> String {
    "desc".to_string()
}

/// Response for task list
#[derive(Debug, Serialize)]
pub struct TasksListResponse {
    /// List of tasks
    pub tasks: Vec<TaskInfoResponse>,
    /// Total number of tasks (before pagination)
    pub total: usize,
    /// Current page number
    pub page: u32,
    /// Number of items per page
    pub page_size: u32,
}

/// Task information response
#[derive(Debug, Serialize)]
pub struct TaskInfoResponse {
    /// Task ID
    pub id: String,
    /// Task type
    pub task_type: String,
    /// Task status (queued, running, completed, failed, cancelled)
    pub status: String,
    /// Task payload (JSON string)
    pub payload: Option<String>,
    /// Task progress message
    pub message: Option<String>,
    /// Task error message (if failed)
    pub error: Option<String>,
    /// Number of retries attempted
    pub retries: i32,
    /// Maximum number of retries allowed
    pub max_retries: i32,
    /// Task creation timestamp
    pub created_at: String,
    /// Task start timestamp (if started)
    pub started_at: Option<String>,
    /// Task completion timestamp (if finished)
    pub finished_at: Option<String>,
}

/// Response for task detail
#[derive(Debug, Serialize)]
pub struct TaskDetailResponse {
    /// Task ID
    pub id: String,
    /// Task type
    pub task_type: String,
    /// Task status
    pub status: String,
    /// Task payload (JSON value)
    pub payload: Option<serde_json::Value>,
    /// Task progress message
    pub message: Option<String>,
    /// Task result (JSON value, if completed)
    pub result: Option<serde_json::Value>,
    /// Task error message (if failed)
    pub error: Option<String>,
    /// Number of retries attempted
    pub retries: i32,
    /// Maximum number of retries allowed
    pub max_retries: i32,
    /// Task creation timestamp
    pub created_at: String,
    /// Task start timestamp (if started)
    pub started_at: Option<String>,
    /// Task completion timestamp (if finished)
    pub finished_at: Option<String>,
}

/// Response for task cancellation
#[derive(Debug, Serialize)]
pub struct CancelTaskResponse {
    /// Success message
    pub message: String,
}
