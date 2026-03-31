//! Task queue implementation
//!
//! This module provides an asynchronous task queue system with priority scheduling,
//! task persistence, and automatic retry mechanisms.

use crate::core::config::TaskQueueConfig;
use crate::core::error::{Result, TingError};
use crate::db::manager::DatabaseManager;
use crate::db::models::TaskRecord;
use crate::db::repository::{Repository, TaskRepository, LibraryRepository};
use crate::core::services::ScraperService;
use crate::core::merge_service::MergeService;
use crate::core::text_cleaner::TextCleaner;
use crate::core::nfo_manager::NfoManager;
use crate::core::audio_streamer::AudioStreamer;
use crate::core::StorageService;
use crate::plugin::manager::PluginManager;

use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::BinaryHeap;
use std::sync::Arc;
use std::time::Duration;
use std::path::PathBuf;
use tokio::sync::{mpsc, RwLock, Semaphore};
use tokio::time::timeout;
use tracing::{debug, error, info, warn};
use chrono::SecondsFormat;
use uuid::Uuid;

// Native ID3 support
use id3::{Tag, TagLike, Version};
use id3::frame::{Picture, PictureType as Id3PictureType};

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
struct PriorityTask {
    task: Task,
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

/// Task queue for managing asynchronous tasks
pub struct TaskQueue {
    config: TaskQueueConfig,
    queue: Arc<RwLock<BinaryHeap<PriorityTask>>>,
    task_repo: TaskRepository,
    semaphore: Arc<Semaphore>,
    shutdown_tx: mpsc::Sender<()>,
    shutdown_rx: Arc<RwLock<mpsc::Receiver<()>>>,
    book_repo: Option<Arc<crate::db::repository::BookRepository>>,
    chapter_repo: Option<Arc<crate::db::repository::ChapterRepository>>,
    series_repo: Option<Arc<crate::db::repository::SeriesRepository>>,
    library_repo: Option<Arc<LibraryRepository>>,
    scraper_service: Option<Arc<ScraperService>>,
    text_cleaner: Option<Arc<TextCleaner>>,
    nfo_manager: Option<Arc<NfoManager>>,
    audio_streamer: Option<Arc<AudioStreamer>>,
    plugin_manager: Option<Arc<PluginManager>>,
    storage_service: Option<Arc<StorageService>>,
    merge_service: Option<Arc<MergeService>>,
    encryption_key: Option<Arc<[u8; 32]>>,
    temp_dir: std::path::PathBuf,
}

impl TaskQueue {
    /// Create a new task queue
    pub fn new(config: TaskQueueConfig, db: Arc<DatabaseManager>, temp_dir: PathBuf) -> Self {
        let (shutdown_tx, shutdown_rx) = mpsc::channel(1);
        
        Self {
            semaphore: Arc::new(Semaphore::new(config.max_concurrent_tasks)),
            config,
            queue: Arc::new(RwLock::new(BinaryHeap::new())),
            task_repo: TaskRepository::new(db),
            shutdown_tx,
            shutdown_rx: Arc::new(RwLock::new(shutdown_rx)),
            book_repo: None,
            chapter_repo: None,
            series_repo: None,
            library_repo: None,
            scraper_service: None,
            text_cleaner: None,
            nfo_manager: None,
            audio_streamer: None,
            plugin_manager: None,
            storage_service: None,
            merge_service: None,
            encryption_key: None,
            temp_dir,
        }
    }

    /// Set repositories for task execution
    pub fn with_repositories(
        mut self,
        book_repo: Arc<crate::db::repository::BookRepository>,
        chapter_repo: Arc<crate::db::repository::ChapterRepository>,
        series_repo: Arc<crate::db::repository::SeriesRepository>,
    ) -> Self {
        self.book_repo = Some(book_repo);
        self.chapter_repo = Some(chapter_repo);
        self.series_repo = Some(series_repo);
        self
    }

    /// Set library repository for task execution
    pub fn with_library_repo(mut self, library_repo: Arc<LibraryRepository>) -> Self {
        self.library_repo = Some(library_repo);
        self
    }

    /// Set scraper service for task execution
    pub fn with_scraper_service(mut self, scraper_service: Arc<ScraperService>) -> Self {
        self.scraper_service = Some(scraper_service);
        self
    }

    /// Set text cleaner for task execution
    pub fn with_text_cleaner(mut self, text_cleaner: Arc<TextCleaner>) -> Self {
        self.text_cleaner = Some(text_cleaner);
        self
    }

    /// Set NFO manager for task execution
    pub fn with_nfo_manager(mut self, nfo_manager: Arc<NfoManager>) -> Self {
        self.nfo_manager = Some(nfo_manager);
        self
    }

    /// Set audio streamer for task execution
    pub fn with_audio_streamer(mut self, audio_streamer: Arc<AudioStreamer>) -> Self {
        self.audio_streamer = Some(audio_streamer);
        self
    }

    /// Set plugin manager for task execution
    pub fn with_plugin_manager(mut self, plugin_manager: Arc<PluginManager>) -> Self {
        self.plugin_manager = Some(plugin_manager);
        self
    }

    /// Set storage service for task execution
    pub fn with_storage_service(mut self, storage_service: Arc<StorageService>) -> Self {
        self.storage_service = Some(storage_service);
        self
    }

    /// Set merge service for task execution
    pub fn with_merge_service(mut self, merge_service: Arc<MergeService>) -> Self {
        self.merge_service = Some(merge_service);
        self
    }

    /// Set encryption key for task execution
    pub fn with_encryption_key(mut self, encryption_key: Arc<[u8; 32]>) -> Self {
        self.encryption_key = Some(encryption_key);
        self
    }

    /// Recover incomplete tasks from database after system restart
    pub async fn recover_tasks(&self) -> Result<usize> {
        info!("正在从数据库恢复未完成的任务");

        // Find all tasks that were queued or running when system shut down
        let mut recovered_count = 0;

        // Recover queued tasks
        let queued_tasks = self.task_repo.find_by_status("queued").await?;
        debug!("Found {} queued tasks to recover", queued_tasks.len());
        
        for task_record in queued_tasks {
            match self.record_to_task(&task_record) {
                Ok(task) => {
                    let mut queue = self.queue.write().await;
                    queue.push(PriorityTask { task });
                    recovered_count += 1;
                }
                Err(e) => {
                    warn!(
                        task_id = %task_record.id,
                        error = %e,
                        "Failed to deserialize queued task, skipping"
                    );
                }
            }
        }

        // Recover running tasks (mark them as queued to retry)
        let running_tasks = self.task_repo.find_by_status("running").await?;
        debug!("Found {} running tasks to recover", running_tasks.len());
        
        for mut task_record in running_tasks {
            // Mark as queued so they will be retried
            task_record.status = "queued".to_string();
            task_record.updated_at = chrono::Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true);
            
            if let Err(e) = self.task_repo.update(&task_record).await {
                error!(
                    task_id = %task_record.id,
                    error = %e,
                    "Failed to update running task status"
                );
                continue;
            }

            match self.record_to_task(&task_record) {
                Ok(task) => {
                    let mut queue = self.queue.write().await;
                    queue.push(PriorityTask { task });
                    recovered_count += 1;
                }
                Err(e) => {
                    warn!(
                        task_id = %task_record.id,
                        error = %e,
                        "Failed to deserialize running task, skipping"
                    );
                }
            }
        }

        info!(
            recovered_count = recovered_count,
            "任务恢复完成"
        );

        Ok(recovered_count)
    }

    /// Get task history (completed and failed tasks)
    pub async fn get_task_history(&self, limit: Option<usize>) -> Result<Vec<TaskRecord>> {
        let all_tasks = self.task_repo.find_all().await?;
        
        // Filter to only completed and failed tasks
        let mut history: Vec<TaskRecord> = all_tasks
            .into_iter()
            .filter(|t| t.status == "completed" || t.status == "failed")
            .collect();

        // Sort by updated_at descending (most recent first)
        history.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));

        // Apply limit if specified
        if let Some(limit) = limit {
            history.truncate(limit);
        }

        Ok(history)
    }

    /// Clean up old completed tasks from database
    pub async fn cleanup_old_tasks(&self, older_than_days: u32) -> Result<usize> {
        let cutoff_date = chrono::Utc::now() - chrono::Duration::days(older_than_days as i64);
        let cutoff_str = cutoff_date.to_rfc3339();

        debug!(
            cutoff_date = %cutoff_str,
            older_than_days = older_than_days,
            "Starting cleanup of old completed tasks"
        );

        let all_tasks = self.task_repo.find_all().await?;
        let mut deleted_count = 0;

        for task in all_tasks {
            debug!(
                task_id = %task.id,
                task_status = %task.status,
                task_updated_at = %task.updated_at,
                cutoff = %cutoff_str,
                is_completed = %(task.status == "completed"),
                is_older = %(task.updated_at < cutoff_str),
                "Evaluating task for cleanup"
            );

            // Only delete completed tasks older than cutoff
            if task.status == "completed" && task.updated_at < cutoff_str {
                debug!(
                    task_id = %task.id,
                    "Deleting old completed task"
                );
                
                if let Err(e) = self.task_repo.delete(&task.id).await {
                    error!(
                        task_id = %task.id,
                        error = %e,
                        "Failed to delete old task"
                    );
                } else {
                    deleted_count += 1;
                }
            }
        }

        info!(
            deleted_count = deleted_count,
            older_than_days = older_than_days,
            "Cleaned up old completed tasks"
        );

        Ok(deleted_count)
    }

    /// Cancel all tasks associated with a library
    pub async fn cancel_library_tasks(&self, library_id: &str) -> Result<()> {
        info!(library_id = %library_id, "Cancelling all tasks for library");

        // 1. Cancel queued tasks
        if let Ok(queued_tasks) = self.task_repo.find_by_status("queued").await {
            for t in queued_tasks {
                if let Some(payload_str) = &t.payload {
                    if let Ok(TaskPayload::Custom { data, .. }) = serde_json::from_str::<TaskPayload>(payload_str) {
                        if let Some(lid) = data.get("library_id").and_then(|v| v.as_str()) {
                            if lid == library_id {
                                info!(task_id = %t.id, library_id = %library_id, "Cancelling queued library task");
                                let _ = self.cancel(&t.id).await;
                            }
                        }
                    }
                }
            }
        }

        // 2. Mark running tasks as cancelled
        if let Ok(running_tasks) = self.task_repo.find_by_status("running").await {
            for t in running_tasks {
                if let Some(payload_str) = &t.payload {
                    if let Ok(TaskPayload::Custom { data, .. }) = serde_json::from_str::<TaskPayload>(payload_str) {
                        if let Some(lid) = data.get("library_id").and_then(|v| v.as_str()) {
                            if lid == library_id {
                                info!(task_id = %t.id, library_id = %library_id, "Marking running library task as cancelled");
                                
                                // Manually update status
                                if let Err(e) = self.task_repo.update_status(
                                    &t.id, 
                                    TaskStatus::Cancelled.as_str(), 
                                    Some("Cancelled due to library deletion"), 
                                    t.retries
                                ).await {
                                    error!(task_id = %t.id, error = %e, "Failed to mark running task as cancelled");
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Submit a task to the queue
    pub async fn submit(&self, mut task: Task) -> Result<String> {
        let task_id = task.id.clone();
        
        // Handle library_scan specific logic (single instance enforcement and timeout)
        if let TaskPayload::Custom { ref task_type, ref data } = task.payload {
            if task_type == "library_scan" {
                // Set a very long timeout for library scans (24 hours) to avoid timeouts on large libraries
                task.timeout = Duration::from_secs(86400);

                if let Some(library_id) = data.get("library_id").and_then(|v| v.as_str()) {
                    let library_id = library_id.to_string();
                    info!(library_id = %library_id, "Checking for existing library scan tasks");

                    // 1. Cancel queued tasks for this library
                    if let Ok(queued_tasks) = self.task_repo.find_by_status("queued").await {
                        for t in queued_tasks {
                            if t.task_type == "library_scan" {
                                if let Some(payload_str) = &t.payload {
                                    if let Ok(TaskPayload::Custom { data: t_data, .. }) = serde_json::from_str::<TaskPayload>(payload_str) {
                                        if let Some(lid) = t_data.get("library_id").and_then(|v| v.as_str()) {
                                            if lid == library_id {
                                                info!(task_id = %t.id, library_id = %library_id, "Cancelling duplicate queued library scan");
                                                let _ = self.cancel(&t.id).await;
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }

                    // 2. Mark running tasks as cancelled (best effort since we can't kill the thread easily)
                    if let Ok(running_tasks) = self.task_repo.find_by_status("running").await {
                        for t in running_tasks {
                            if t.task_type == "library_scan" {
                                if let Some(payload_str) = &t.payload {
                                    if let Ok(TaskPayload::Custom { data: t_data, .. }) = serde_json::from_str::<TaskPayload>(payload_str) {
                                        if let Some(lid) = t_data.get("library_id").and_then(|v| v.as_str()) {
                                            if lid == library_id {
                                                info!(task_id = %t.id, library_id = %library_id, "Marking running library scan as cancelled");
                                                
                                                // Manually update status since cancel() forbids running tasks
                                                if let Err(e) = self.task_repo.update_status(
                                                    &t.id, 
                                                    TaskStatus::Cancelled.as_str(), 
                                                    Some("Cancelled by new task"), 
                                                    t.retries
                                                ).await {
                                                    error!(task_id = %t.id, error = %e, "Failed to mark running task as cancelled");
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // Set default retry policy if not set
        if task.retry_policy.max_retries == 0 {
            task.retry_policy.max_retries = self.config.default_retry_count;
        }

        // Set default timeout if not set
        if task.timeout.as_secs() == 0 {
            task.timeout = Duration::from_secs(self.config.task_timeout);
        }

        // Persist task to database
        let task_record = self.task_to_record(&task)?;
        self.task_repo.create(&task_record).await?;

        // Add to priority queue
        {
            let mut queue = self.queue.write().await;
            queue.push(PriorityTask { task: task.clone() });
        }

        info!(
            task_id = %task_id,
            task_name = %task.name,
            priority = ?task.priority,
            "Task submitted to queue"
        );

        Ok(task_id)
    }

    /// Get task status
    pub async fn get_status(&self, task_id: &str) -> Result<TaskStatus> {
        let task_record = self
            .task_repo
            .find_by_id(task_id)
            .await?
            .ok_or_else(|| TingError::NotFound(format!("Task not found: {}", task_id)))?;

        Ok(self.status_from_str(&task_record.status))
    }

    /// Cancel a task
    pub async fn cancel(&self, task_id: &str) -> Result<()> {
        // Update task status in database
        let mut task_record = self
            .task_repo
            .find_by_id(task_id)
            .await?
            .ok_or_else(|| TingError::NotFound(format!("Task not found: {}", task_id)))?;

        if task_record.status == TaskStatus::Running.as_str() {
            // Allow cancelling running tasks - the executor will pick this up
            info!(task_id = %task_id, "Marking running task as cancelled");
        }

        task_record.status = TaskStatus::Cancelled.as_str().to_string();
        task_record.updated_at = chrono::Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true);
        self.task_repo.update(&task_record).await?;

        // Remove from queue if present
        {
            let mut queue = self.queue.write().await;
            let tasks: Vec<_> = queue.drain().collect();
            *queue = tasks
                .into_iter()
                .filter(|pt| pt.task.id != task_id)
                .collect();
        }

        info!(task_id = %task_id, "Task cancelled");
        Ok(())
    }

    /// List all tasks
    pub async fn list_tasks(&self) -> Result<Vec<TaskRecord>> {
        self.task_repo.find_all().await
    }

    /// List tasks with filtering, sorting, and pagination
    pub async fn list_tasks_with_filters(
        &self,
        status: Option<String>,
        page: u32,
        page_size: u32,
        sort_by: String,
        sort_order: String,
    ) -> Result<(Vec<TaskRecord>, usize)> {
        self.task_repo
            .find_with_filters(status, page, page_size, sort_by, sort_order)
            .await
    }

    /// List tasks by status
    pub async fn list_tasks_by_status(&self, status: TaskStatus) -> Result<Vec<TaskRecord>> {
        self.task_repo.find_by_status(status.as_str()).await
    }

    /// List tasks by type
    pub async fn list_tasks_by_type(&self, task_type: &str) -> Result<Vec<TaskRecord>> {
        self.task_repo.find_by_type(task_type).await
    }

    /// Get task details
    pub async fn get_task(&self, task_id: &str) -> Result<TaskRecord> {
        self.task_repo
            .find_by_id(task_id)
            .await?
            .ok_or_else(|| TingError::NotFound(format!("Task not found: {}", task_id)))
    }

    /// Delete a task
    pub async fn delete_task(&self, task_id: &str) -> Result<()> {
        let task_record = self
            .task_repo
            .find_by_id(task_id)
            .await?
            .ok_or_else(|| TingError::NotFound(format!("Task not found: {}", task_id)))?;

        if task_record.status == TaskStatus::Running.as_str() {
            return Err(TingError::TaskError(
                "Cannot delete running task. Cancel it first.".to_string(),
            ));
        }

        self.task_repo.delete(task_id).await?;
        
        info!(task_id = %task_id, "Task deleted");
        Ok(())
    }

    /// Batch delete tasks
    pub async fn delete_tasks(&self, ids: Vec<String>) -> Result<usize> {
        let count = self.task_repo.delete_batch(ids).await?;
        info!(count = count, "Batch deleted tasks");
        Ok(count)
    }

    /// Clear tasks
    pub async fn clear_tasks(&self, status: Option<String>) -> Result<usize> {
        if let Some(s) = status {
            if s == "running" {
                return Err(TingError::TaskError(
                    "Cannot clear running tasks. Cancel them first.".to_string(),
                ));
            }
            // Count before deleting
            let tasks = self.task_repo.find_by_status(&s).await?;
            let count = tasks.len();
            self.task_repo.delete_by_status(&s).await?;
            info!(status = %s, count = count, "Cleared tasks by status");
            Ok(count)
        } else {
            // Delete all non-running tasks
            // We can't use delete_all because it would delete running tasks too
            // So we delete by status for each non-running status
            let mut total = 0;
            for s in ["queued", "completed", "failed", "cancelled"] {
                let tasks = self.task_repo.find_by_status(s).await?;
                let count = tasks.len();
                self.task_repo.delete_by_status(s).await?;
                total += count;
            }
            
            // Clear queued tasks from memory queue as well
            let mut queue = self.queue.write().await;
            queue.clear();
            
            info!(count = total, "Cleared all non-running tasks");
            Ok(total)
        }
    }

    /// Start the task executor
    pub async fn start(self: Arc<Self>) {
        info!("任务队列执行器已启动");

        let mut shutdown_rx = {
            let mut guard = self.shutdown_rx.write().await;
            // Take ownership of the receiver
            std::mem::replace(&mut *guard, mpsc::channel(1).1)
        };

        loop {
            tokio::select! {
                _ = shutdown_rx.recv() => {
                    info!("Task queue executor shutting down");
                    break;
                }
                _ = self.process_next_task() => {}
            }
        }
    }

    /// Shutdown the task queue
    pub async fn shutdown(&self) {
        let _ = self.shutdown_tx.send(()).await;
    }

    /// Process the next task in the queue
    async fn process_next_task(self: &Arc<Self>) {
        // Get next task from queue
        let task = {
            let mut queue = self.queue.write().await;
            queue.pop().map(|pt| pt.task)
        };

        if let Some(task) = task {
            // Acquire semaphore permit
            let permit = self.semaphore.clone().acquire_owned().await;
            
            match permit {
                Ok(permit) => {
                    let self_clone = Arc::clone(self);
                    
                    // Spawn task execution
                    tokio::spawn(async move {
                        self_clone.execute_task(task).await;
                        drop(permit);
                    });
                }
                Err(e) => {
                    error!(error = %e, "Failed to acquire semaphore permit");
                }
            }
        } else {
            // No tasks available, wait a bit
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }

    pub async fn enqueue_scan_library(&self, library_id: &str, library_path: &str) -> Result<String> {
        let task_payload = TaskPayload::Custom {
            task_type: "library_scan".to_string(),
            data: serde_json::json!({
                "library_id": library_id,
                "library_path": library_path,
            }),
        };
        
        let task = Task::new(
            format!("library_scan_{}", library_id),
            Priority::Normal,
            task_payload,
        );

        self.submit(task).await
    }

    /// Execute a task
    async fn execute_task(&self, mut task: Task) {
        let task_id = task.id.clone();
        
        info!(
            task_id = %task_id,
            task_name = %task.name,
            "Executing task"
        );

        // Update status to running
        task.status = TaskStatus::Running;
        if let Err(e) = self.update_task_status(&task).await {
            error!(task_id = %task_id, error = %e, "Failed to update task status");
        }

        // Execute with timeout
        let result = timeout(task.timeout, self.run_task(&task)).await;

        match result {
            Ok(Ok(())) => {
                // Task completed successfully
                task.status = TaskStatus::Completed;
                task.error = None;
                
                if let Err(e) = self.update_task_status(&task).await {
                    error!(task_id = %task_id, error = %e, "Failed to update task status");
                }
                
                info!(task_id = %task_id, "Task completed successfully");
            }
            Ok(Err(e)) => {
                // Task failed
                error!(task_id = %task_id, error = %e, "Task execution failed");
                
                task.retries += 1;
                task.error = Some(e.to_string());

                // Check if we should retry
                if task.retries < task.retry_policy.max_retries {
                    let delay = task.retry_policy.backoff.calculate_delay(task.retries);
                    
                    warn!(
                        task_id = %task_id,
                        retry = task.retries,
                        max_retries = task.retry_policy.max_retries,
                        delay_secs = delay.as_secs(),
                        "Retrying task"
                    );

                    // Wait before retry
                    tokio::time::sleep(delay).await;

                    // Re-submit task
                    task.status = TaskStatus::Queued;
                    if let Err(e) = self.update_task_status(&task).await {
                        error!(task_id = %task_id, error = %e, "Failed to update task status");
                    }

                    let mut queue = self.queue.write().await;
                    queue.push(PriorityTask { task });
                } else {
                    // Max retries exceeded
                    task.status = TaskStatus::Failed;
                    if let Err(e) = self.update_task_status(&task).await {
                        error!(task_id = %task_id, error = %e, "Failed to update task status");
                    }
                    
                    error!(
                        task_id = %task_id,
                        retries = task.retries,
                        "Task failed after max retries"
                    );
                }
            }
            Err(_) => {
                // Timeout
                error!(
                    task_id = %task_id,
                    timeout_secs = task.timeout.as_secs(),
                    "Task execution timed out"
                );
                
                task.status = TaskStatus::Failed;
                task.error = Some(format!("Task timed out after {} seconds", task.timeout.as_secs()));
                
                if let Err(e) = self.update_task_status(&task).await {
                    error!(task_id = %task_id, error = %e, "Failed to update task status");
                }
            }
        }
    }

    /// Run the actual task logic
    async fn run_task(&self, task: &Task) -> Result<()> {
        debug!(
            task_id = %task.id,
            payload = ?task.payload,
            "Running task"
        );

        match &task.payload {
            TaskPayload::ScraperSearch { plugin_id, query } => {
                let scraper_service = self.scraper_service.as_ref()
                    .ok_or_else(|| crate::core::error::TingError::TaskError("Scraper service not configured".to_string()))?;
                
                info!(plugin_id = %plugin_id, query = %query, "Executing scraper search task");
                let result = scraper_service.search(query, None, None, Some(plugin_id), 1, 20).await?;
                info!(items = result.items.len(), "Scraper search completed");
            }
            TaskPayload::Custom { task_type, data } => {
                match task_type.as_str() {
                    "library_scan" => {
                        self.handle_library_scan(data, &task.id).await?;
                    }
                    "write_metadata" => {
                        self.handle_write_metadata(data, &task.id).await?;
                    }
                    _ => {
                        warn!(task_type = %task_type, "Unknown task type");
                        return Err(crate::core::error::TingError::TaskError(
                            format!("Unknown task type: {}", task_type)
                        ));
                    }
                }
            }
            _ => {
                // Other task types can be handled here
                debug!("Task payload type not yet implemented");
            }
        }

        Ok(())
    }

    /// Handle library scan task
    async fn handle_library_scan(&self, data: &serde_json::Value, task_id: &str) -> Result<()> {
        let library_id = data["library_id"]
            .as_str()
            .ok_or_else(|| crate::core::error::TingError::TaskError("Missing library_id".to_string()))?;
        let library_path = data["library_path"]
            .as_str()
            .ok_or_else(|| crate::core::error::TingError::TaskError("Missing library_path".to_string()))?;

        info!(library_id = %library_id, path = %library_path, "Handling library scan task");

        // Get repositories
        let book_repo = self.book_repo.as_ref()
            .ok_or_else(|| crate::core::error::TingError::TaskError("Book repository not configured".to_string()))?;
        let chapter_repo = self.chapter_repo.as_ref()
            .ok_or_else(|| crate::core::error::TingError::TaskError("Chapter repository not configured".to_string()))?;
        let series_repo = self.series_repo.as_ref()
            .ok_or_else(|| crate::core::error::TingError::TaskError("Series repository not configured".to_string()))?;
        let library_repo = self.library_repo.as_ref()
            .ok_or_else(|| crate::core::error::TingError::TaskError("Library repository not configured".to_string()))?;

        // Get services
        let text_cleaner = self.text_cleaner.as_ref()
            .ok_or_else(|| crate::core::error::TingError::TaskError("Text cleaner not configured".to_string()))?;
        let nfo_manager = self.nfo_manager.as_ref()
            .ok_or_else(|| crate::core::error::TingError::TaskError("NFO manager not configured".to_string()))?;
        let audio_streamer = self.audio_streamer.as_ref()
            .ok_or_else(|| crate::core::error::TingError::TaskError("Audio streamer not configured".to_string()))?;
        let plugin_manager = self.plugin_manager.as_ref()
            .ok_or_else(|| crate::core::error::TingError::TaskError("Plugin manager not configured".to_string()))?;

        // Create library scanner
        let mut scanner = crate::core::library_scanner::LibraryScanner::new(
            book_repo.clone(),
            chapter_repo.clone(),
            library_repo.clone(),
            series_repo.clone(),
            text_cleaner.clone(),
            nfo_manager.clone(),
            audio_streamer.clone(),
            plugin_manager.clone(),
        )
        .with_task_repo(Arc::new(self.task_repo.clone()))
        .with_scraper_service(self.scraper_service.as_ref().unwrap().clone());

        if let Some(storage) = &self.storage_service {
            scanner = scanner.with_storage_service(storage.clone());
        }
        if let Some(merge_service) = &self.merge_service {
            scanner = scanner.with_merge_service(merge_service.clone());
        }
        if let Some(key) = &self.encryption_key {
            scanner = scanner.with_encryption_key(key.clone());
        }

        // Scan the library
        let result = scanner.scan_library(library_id, library_path, Some(task_id)).await?;

        info!(
            library_id = %library_id,
            books_created = result.books_created,
            books_deleted = result.books_deleted,
            errors = result.errors.len(),
            "Library scan completed"
        );

        // Update task message with result
        let message = format!("图书馆扫描完成，新增 {} 本，更新 {} 本，删除 {} 本", result.books_created, result.books_updated, result.books_deleted);
        if let Err(e) = self.task_repo.update_progress(task_id, &message).await {
            warn!(task_id = %task_id, error = %e, "Failed to update task progress message");
        }

        if !result.errors.is_empty() {
            warn!(errors = ?result.errors, "Library scan completed with errors");
        }

        Ok(())
    }

    /// Handle write metadata task
    async fn handle_write_metadata(&self, data: &serde_json::Value, task_id: &str) -> Result<()> {
        let book_id = data["book_id"]
            .as_str()
            .ok_or_else(|| crate::core::error::TingError::TaskError("Missing book_id".to_string()))?;

        info!(book_id = %book_id, "Handling write metadata task");

        // Get repositories
        let book_repo = self.book_repo.as_ref()
            .ok_or_else(|| crate::core::error::TingError::TaskError("Book repository not configured".to_string()))?;
        let library_repo = self.library_repo.as_ref()
            .ok_or_else(|| crate::core::error::TingError::TaskError("Library repository not configured".to_string()))?;
        let chapter_repo = self.chapter_repo.as_ref()
            .ok_or_else(|| crate::core::error::TingError::TaskError("Chapter repository not configured".to_string()))?;
        let plugin_manager = self.plugin_manager.as_ref()
            .ok_or_else(|| crate::core::error::TingError::TaskError("Plugin manager not configured".to_string()))?;

        let book = book_repo.find_by_id(book_id).await?
            .ok_or_else(|| crate::core::error::TingError::NotFound(format!("Book with id {} not found", book_id)))?;

        // Check if library is local
        let library = library_repo.find_by_id(&book.library_id).await?
            .ok_or_else(|| crate::core::error::TingError::NotFound(format!("Library with id {} not found", book.library_id)))?;

        if library.library_type != "local" {
            return Err(crate::core::error::TingError::InvalidRequest("Only local libraries are supported for metadata writing".to_string()));
        }

        // Resolve cover path
        let mut cover_path_str = None;
        let mut temp_cover_path = None;

        if let Some(ref url) = book.cover_url {
            if url.starts_with("http://") || url.starts_with("https://") {
                // Download to temp
                let temp_dir = self.temp_dir.join("ting-reader-covers");
                if !temp_dir.exists() { tokio::fs::create_dir_all(&temp_dir).await.map_err(crate::core::error::TingError::IoError)?; }
                
                let mut fetch_url = url.clone();
                let mut referer = "".to_string();
                if let Some(idx) = fetch_url.find("#referer=") {
                    referer = fetch_url[idx + 9..].to_string();
                    fetch_url = fetch_url[..idx].to_string();
                }

                let ext = std::path::Path::new(&fetch_url).extension().and_then(|e| e.to_str()).unwrap_or("jpg");
                let file_name = format!("{}.{}", Uuid::new_v4(), ext);
                let path = temp_dir.join(file_name);
                
                // Download
                let client = reqwest::Client::new();
                let mut req = client.get(&fetch_url)
                    .header(reqwest::header::USER_AGENT, "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36");
                if !referer.is_empty() {
                    req = req.header(reqwest::header::REFERER, referer);
                }

                match req.send().await {
                    Ok(resp) => {
                        if let Ok(bytes) = resp.bytes().await {
                            if tokio::fs::write(&path, bytes).await.is_ok() {
                                temp_cover_path = Some(path.clone());
                                cover_path_str = Some(path.to_string_lossy().to_string());
                            }
                        }
                    },
                    Err(e) => warn!("Failed to download cover for metadata writing: {}", e),
                }
            } else {
                // Local path
                let path = std::path::Path::new(url);
                if path.is_absolute() || path.exists() {
                    cover_path_str = Some(url.clone());
                } else {
                     let book_path = std::path::Path::new(&book.path);
                     let joined = book_path.join(url);
                     // If joined path exists, use it. Otherwise fallback to original URL
                     // to avoid double-pathing (e.g. ./storage/./storage/...)
                     if joined.exists() {
                         cover_path_str = Some(joined.to_string_lossy().to_string());
                     } else {
                         cover_path_str = Some(url.clone());
                     }
                }
            }
        }

        // Get chapters
        let chapters = chapter_repo.find_by_book(book_id).await?;

        let mut success_count = 0;
        let mut error_count = 0;
        let total_chapters = chapters.len();
        
        for (index, chapter) in chapters.iter().enumerate() {
            // Update progress
            let progress_msg = format!("正在写入第 {}/{} 章: {}", index + 1, total_chapters, chapter.title.as_deref().unwrap_or(""));
            let _ = self.task_repo.update_progress(task_id, &progress_msg).await;

            let path = std::path::Path::new(&chapter.path);
            if !path.exists() { 
                error_count += 1;
                continue; 
            }
            
            // Find plugin that supports this format
            let ext = path.extension().unwrap_or_default().to_string_lossy().to_lowercase();
            let plugins = plugin_manager.find_plugins_by_type(crate::plugin::types::PluginType::Format).await;
            
            // Prioritize native-audio-support if available for this extension
            let plugin_info = plugins.into_iter()
                .find(|p| p.supported_extensions.as_ref().map(|e| e.contains(&ext)).unwrap_or(false));
                
            if let Some(plugin) = plugin_info {
                let artist = if let Some(narrator) = &book.narrator {
                    if !narrator.trim().is_empty() {
                        narrator.as_str()
                    } else {
                        book.author.as_deref().unwrap_or("")
                    }
                } else {
                    book.author.as_deref().unwrap_or("")
                };

                let metadata = serde_json::json!({
                    "file_path": chapter.path,
                    "title": chapter.title.as_deref().unwrap_or(""),
                    "artist": artist,
                    "album": book.title.as_deref().unwrap_or(""),
                    "genre": book.genre.as_deref().unwrap_or(""),
                    "description": book.description.as_deref().unwrap_or(""),
                    "cover_path": cover_path_str,
                });

                match plugin_manager.call_format(&plugin.id, crate::plugin::manager::FormatMethod::WriteMetadata, metadata).await {
                    Ok(_) => success_count += 1,
                    Err(e) => {
                        warn!("Failed to write metadata for {}: {}", chapter.path, e);
                        error_count += 1;
                    }
                }
            } else {
                // No plugin found, try native/builtin support
                if ext == "mp3" {
                     let path_clone = path.to_path_buf();
                     let title_clone = chapter.title.clone().unwrap_or_default();
                     let artist_clone = if let Some(narrator) = &book.narrator {
                         if !narrator.trim().is_empty() {
                             narrator.clone()
                         } else {
                             book.author.clone().unwrap_or_default()
                         }
                     } else {
                         book.author.clone().unwrap_or_default()
                     };
                     let album_clone = book.title.clone().unwrap_or_default();
                     let genre_clone = book.genre.clone().unwrap_or_default();
                     let desc_clone = book.description.clone().unwrap_or_default();
                     let cover_path_str_clone = cover_path_str.clone();

                     // Spawn blocking task for native ID3 write
                     let native_write_result = tokio::task::spawn_blocking(move || -> Result<()> {
                         let mut tag = match Tag::read_from_path(&path_clone) {
                             Ok(t) => t,
                             Err(_) => Tag::new(),
                         };

                         tag.set_title(&title_clone);
                         tag.set_artist(&artist_clone);
                         tag.set_album(&album_clone);
                         tag.set_genre(&genre_clone);
                         
                         tag.remove_comment(Some("eng"), None);
                         tag.add_frame(id3::frame::Comment {
                             lang: "eng".to_string(),
                             description: "".to_string(),
                             text: desc_clone,
                         });

                         if let Some(cp) = cover_path_str_clone {
                             if let Ok(data) = std::fs::read(&cp) {
                                  let mime_type = if cp.to_lowercase().ends_with("png") {
                                      "image/png".to_string()
                                  } else {
                                      "image/jpeg".to_string()
                                  };
                                  
                                  tag.remove_all_pictures();
                                  tag.add_frame(Picture {
                                      mime_type,
                                      picture_type: Id3PictureType::CoverFront,
                                      description: "Cover".to_string(),
                                      data,
                                  });
                             }
                         }

                         tag.write_to_path(&path_clone, Version::Id3v23)
                             .map_err(|e| crate::core::error::TingError::IoError(std::io::Error::new(std::io::ErrorKind::Other, e.to_string())))?;
                         
                         Ok(())
                     }).await;

                     match native_write_result {
                         Ok(Ok(_)) => {
                             info!("Successfully wrote metadata natively for MP3 (fallback): {:?}", path);
                             success_count += 1;
                         },
                         Ok(Err(e)) => {
                             warn!("Native ID3 write failed for {:?}: {}", path, e);
                             error_count += 1;
                         },
                         Err(e) => {
                             warn!("Native ID3 task panic for {:?}: {}", path, e);
                             error_count += 1;
                         }
                     }
                } else {
                    error_count += 1;
                }
            }
        }

        // Cleanup temp cover
        if let Some(path) = temp_cover_path {
            let _ = tokio::fs::remove_file(path).await;
        }

        let final_msg = format!("元数据写入完成，成功 {} 章，失败 {} 章", success_count, error_count);
        let _ = self.task_repo.update_progress(task_id, &final_msg).await;

        tracing::info!(
            target: "audit::metadata",
            "书籍 '{}' (ID: {}) 音频文件元数据写入完成：成功 {} 章，失败 {} 章",
            book.title.as_deref().unwrap_or("未知"),
            book.id,
            success_count,
            error_count
        );

        Ok(())
    }

    /// Update task status in database
    async fn update_task_status(&self, task: &Task) -> Result<()> {
        self.task_repo.update_status(
            &task.id,
            task.status.as_str(),
            task.error.as_deref(),
            task.retries as i32,
        ).await
    }

    /// Convert Task to TaskRecord
    fn task_to_record(&self, task: &Task) -> Result<TaskRecord> {
        let payload_json = serde_json::to_string(&task.payload)
            .map_err(|e| TingError::SerializationError(e.to_string()))?;

        Ok(TaskRecord {
            id: task.id.clone(),
            task_type: match &task.payload {
                TaskPayload::ScraperSearch { .. } => "scraper_search".to_string(),
                TaskPayload::FormatConvert { .. } => "format_convert".to_string(),
                TaskPayload::PluginInvoke { .. } => "plugin_invoke".to_string(),
                TaskPayload::Custom { task_type, .. } => task_type.clone(),
            },
            status: task.status.as_str().to_string(),
            payload: Some(payload_json),
            message: None,
            error: task.error.clone(),
            retries: task.retries as i32,
            max_retries: task.retry_policy.max_retries as i32,
            // Format time with 3 decimal places for milliseconds (SQL friendly)
            created_at: task.created_at.to_rfc3339_opts(SecondsFormat::Millis, true),
            updated_at: chrono::Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true),
        })
    }

    /// Convert status string to TaskStatus
    fn status_from_str(&self, status: &str) -> TaskStatus {
        match status {
            "queued" => TaskStatus::Queued,
            "running" => TaskStatus::Running,
            "completed" => TaskStatus::Completed,
            "failed" => TaskStatus::Failed,
            "cancelled" => TaskStatus::Cancelled,
            _ => TaskStatus::Queued,
        }
    }

    /// Convert TaskRecord to Task
    fn record_to_task(&self, record: &TaskRecord) -> Result<Task> {
        let payload: TaskPayload = if let Some(ref payload_str) = record.payload {
            serde_json::from_str(payload_str)
                .map_err(|e| {
                    error!(
                        task_id = %record.id,
                        payload = %payload_str,
                        error = %e,
                        "Failed to deserialize task payload"
                    );
                    TingError::SerializationError(format!("Failed to deserialize payload: {}", e))
                })?
        } else {
            return Err(TingError::TaskError("Task payload is missing".to_string()));
        };

        // Try to parse timestamp - handle both RFC3339 and SQLite CURRENT_TIMESTAMP format
        let created_at = if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(&record.created_at) {
            dt.with_timezone(&chrono::Utc)
        } else {
            // Try parsing as naive datetime (SQLite CURRENT_TIMESTAMP format)
            chrono::NaiveDateTime::parse_from_str(&record.created_at, "%Y-%m-%d %H:%M:%S")
                .map_err(|e| {
                    error!(
                        task_id = %record.id,
                        created_at = %record.created_at,
                        error = %e,
                        "Failed to parse created_at timestamp"
                    );
                    TingError::SerializationError(format!("Failed to parse timestamp: {}", e))
                })?
                .and_utc()
        };

        debug!(
            task_id = %record.id,
            task_type = %record.task_type,
            status = %record.status,
            "Successfully deserialized task from database"
        );

        Ok(Task {
            id: record.id.clone(),
            name: format!("{} task", record.task_type),
            priority: Priority::Normal, // Default priority for recovered tasks
            payload,
            retry_policy: RetryPolicy {
                max_retries: record.max_retries as u32,
                backoff: BackoffStrategy::Exponential {
                    base: Duration::from_secs(1),
                    max: Duration::from_secs(60),
                },
            },
            timeout: Duration::from_secs(600), // Default timeout
            status: self.status_from_str(&record.status),
            retries: record.retries as u32,
            error: record.error.clone(),
            created_at,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_priority_ordering() {
        let high = Task::new(
            "high".to_string(),
            Priority::High,
            TaskPayload::Custom {
                task_type: "test".to_string(),
                data: serde_json::json!({}),
            },
        );

        let low = Task::new(
            "low".to_string(),
            Priority::Low,
            TaskPayload::Custom {
                task_type: "test".to_string(),
                data: serde_json::json!({}),
            },
        );

        let pt_high = PriorityTask { task: high };
        let pt_low = PriorityTask { task: low };

        assert!(pt_high > pt_low);
    }

    #[test]
    fn test_backoff_strategy() {
        let fixed = BackoffStrategy::Fixed(Duration::from_secs(5));
        assert_eq!(fixed.calculate_delay(0), Duration::from_secs(5));
        assert_eq!(fixed.calculate_delay(10), Duration::from_secs(5));

        let exponential = BackoffStrategy::Exponential {
            base: Duration::from_secs(1),
            max: Duration::from_secs(60),
        };
        assert_eq!(exponential.calculate_delay(1), Duration::from_secs(1)); // 1 * 2^0
        assert_eq!(exponential.calculate_delay(2), Duration::from_secs(2)); // 1 * 2^1
        assert_eq!(exponential.calculate_delay(3), Duration::from_secs(4)); // 1 * 2^2
        assert_eq!(exponential.calculate_delay(10), Duration::from_secs(60)); // capped at max
    }

    #[test]
    fn test_task_status_as_str() {
        assert_eq!(TaskStatus::Queued.as_str(), "queued");
        assert_eq!(TaskStatus::Running.as_str(), "running");
        assert_eq!(TaskStatus::Completed.as_str(), "completed");
        assert_eq!(TaskStatus::Failed.as_str(), "failed");
        assert_eq!(TaskStatus::Cancelled.as_str(), "cancelled");
    }

    #[test]
    fn test_task_creation() {
        let task = Task::new(
            "test_task".to_string(),
            Priority::Normal,
            TaskPayload::Custom {
                task_type: "test".to_string(),
                data: serde_json::json!({"key": "value"}),
            },
        );

        assert_eq!(task.name, "test_task");
        assert_eq!(task.priority, Priority::Normal);
        assert_eq!(task.status, TaskStatus::Queued);
        assert_eq!(task.retries, 0);
        assert!(task.error.is_none());
        assert_eq!(task.retry_policy.max_retries, 3);
    }

    #[test]
    fn test_task_with_custom_retry_policy() {
        let task = Task::new(
            "test_task".to_string(),
            Priority::High,
            TaskPayload::Custom {
                task_type: "test".to_string(),
                data: serde_json::json!({}),
            },
        )
        .with_retry_policy(RetryPolicy {
            max_retries: 5,
            backoff: BackoffStrategy::Fixed(Duration::from_secs(10)),
        })
        .with_timeout(Duration::from_secs(300));

        assert_eq!(task.retry_policy.max_retries, 5);
        assert_eq!(task.timeout, Duration::from_secs(300));
    }
}
