use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::time::Duration;
use uuid::Uuid;

/// Task priority levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Priority {
    Low = 1,
    Normal = 2,
    High = 3,
}

/// Task status
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskStatus {
    Queued,
    Running,
    Completed,
    Failed,
    Cancelled,
}

impl TaskStatus {
    pub fn as_str(&self) -> &str {
        match self {
            TaskStatus::Queued => "queued",
            TaskStatus::Running => "running",
            TaskStatus::Completed => "completed",
            TaskStatus::Failed => "failed",
            TaskStatus::Cancelled => "cancelled",
        }
    }
}

/// Retry policy for tasks
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryPolicy {
    pub max_retries: u32,
    pub backoff: BackoffStrategy,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_retries: 3,
            backoff: BackoffStrategy::Exponential {
                base: Duration::from_secs(1),
                max: Duration::from_secs(60),
            },
        }
    }
}

/// Backoff strategy for retries
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BackoffStrategy {
    Fixed(Duration),
    Exponential { base: Duration, max: Duration },
}

impl BackoffStrategy {
    pub fn calculate_delay(&self, attempt: u32) -> Duration {
        match self {
            BackoffStrategy::Fixed(duration) => *duration,
            BackoffStrategy::Exponential { base, max } => {
                // For exponential backoff, first retry (attempt=1) should be base * 2^0 = base
                // Second retry (attempt=2) should be base * 2^1 = base * 2
                // Third retry (attempt=3) should be base * 2^2 = base * 4
                let exponent = if attempt > 0 { attempt - 1 } else { 0 };

                // Work in milliseconds to avoid losing precision for sub-second durations
                let base_ms = base.as_millis() as u64;
                let max_ms = max.as_millis() as u64;
                let delay_ms = base_ms.saturating_mul(2u64.pow(exponent));

                Duration::from_millis(delay_ms.min(max_ms))
            }
        }
    }
}

/// Task payload types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TaskPayload {
    ScraperSearch {
        plugin_id: String,
        query: String,
    },
    FormatConvert {
        plugin_id: String,
        input: String,
        output: String,
    },
    PluginInvoke {
        plugin_id: String,
        method: String,
        params: serde_json::Value,
    },
    Custom {
        task_type: String,
        data: serde_json::Value,
    },
}

/// A task to be executed
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: String,
    pub name: String,
    pub priority: Priority,
    pub payload: TaskPayload,
    pub retry_policy: RetryPolicy,
    pub timeout: Duration,
    pub status: TaskStatus,
    pub retries: u32,
    pub error: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

impl Task {
    pub fn new(name: String, priority: Priority, payload: TaskPayload) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            name,
            priority,
            payload,
            retry_policy: RetryPolicy::default(),
            timeout: Duration::from_secs(600),
            status: TaskStatus::Queued,
            retries: 0,
            error: None,
            created_at: chrono::Utc::now(),
        }
    }

    pub fn with_retry_policy(mut self, policy: RetryPolicy) -> Self {
        self.retry_policy = policy;
        self
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }
}

/// Wrapper for priority queue ordering
#[derive(Debug, Clone)]
pub(crate) struct PriorityTask {
    pub(crate) task: Task,
}

impl PartialEq for PriorityTask {
    fn eq(&self, other: &Self) -> bool {
        self.task.priority == other.task.priority
    }
}

impl Eq for PriorityTask {}

impl PartialOrd for PriorityTask {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for PriorityTask {
    fn cmp(&self, other: &Self) -> Ordering {
        // Higher priority first, then earlier creation time
        self.task
            .priority
            .cmp(&other.task.priority)
            .then_with(|| other.task.created_at.cmp(&self.task.created_at))
    }
}
