use crate::core::error::{Result, TingError};
use crate::db::manager::DatabaseManager;
use crate::db::models::TaskRecord;
use crate::db::repository::base::Repository;
use async_trait::async_trait;
use rusqlite::OptionalExtension;
use std::sync::Arc;

/// Repository for TaskRecord entities
#[derive(Clone)]
pub struct TaskRepository {
    db: Arc<DatabaseManager>,
}

impl TaskRepository {
    /// Create a new TaskRepository
    pub fn new(db: Arc<DatabaseManager>) -> Self {
        Self { db }
    }

    fn normalize_date(s: String) -> String {
        let mut s = s;

        // 1. Replace space with T (SQL format)
        // Check if index 10 is space (YYYY-MM-DD HH:MM:SS)
        if s.len() > 10 && s.as_bytes()[10] == b' ' {
            s.replace_range(10..11, "T");
        }

        // 2. Handle Nanoseconds: truncate to 3 digits
        // Find dot position
        if let Some(dot_pos) = s.find('.') {
            // Find where numbers end or timezone starts
            // We scan from after the dot until we find a non-digit
            let end_pos = s[dot_pos + 1..]
                .find(|c: char| !c.is_numeric())
                .map(|p| p + dot_pos + 1)
                .unwrap_or(s.len());

            // If we have more than 3 digits of fractional seconds
            if end_pos - dot_pos > 4 {
                // .123 is 4 chars. if > 4, we have more than 3 digits
                // Keep only first 3 digits after dot (.123)
                let tail = s[end_pos..].to_string(); // Save timezone part like +00:00 or Z
                let head = &s[..dot_pos + 4];
                s = format!("{}{}", head, tail);
            }
        }

        // 3. Add Z if missing timezone
        // We check if it ends with Z or has +HH:MM or -HH:MM
        // Simple heuristic: if it doesn't end with Z and doesn't contain +, and length is short enough or looks like no timezone
        if !s.ends_with('Z') && !s.contains('+') {
            // Check if we have -HH:MM (e.g. -05:00)
            // Date parts use - (YYYY-MM-DD), so count dashes.
            // Standard date has 2 dashes. If more, might be timezone.
            // But simpler: if length matches standard without timezone (19 chars YYYY-MM-DDTHH:MM:SS)
            // or 23 chars (YYYY-MM-DDTHH:MM:SS.mmm)
            if s.len() == 19 || (s.len() == 23 && s.contains('.')) {
                s.push('Z');
            }
        }

        s
    }

    /// Find tasks by status
    pub async fn find_by_status(&self, status: &str) -> Result<Vec<TaskRecord>> {
        let status = status.to_string();
        self.db.execute(move |conn| {
            let mut stmt = conn.prepare(
                "SELECT id, type, status, payload, message, error, retries, max_retries, created_at, updated_at \
                 FROM tasks WHERE status = ? ORDER BY created_at DESC"
            ).map_err(TingError::DatabaseError)?;

            let tasks = stmt.query_map([&status], |row| {
                Ok(TaskRecord {
                    id: row.get(0)?,
                    task_type: row.get(1)?,
                    status: row.get(2)?,
                    payload: row.get(3)?,
                    message: row.get(4)?,
                    error: row.get(5)?,
                    retries: row.get(6)?,
                    max_retries: row.get(7)?,
                    created_at: Self::normalize_date(row.get(8)?),
                    updated_at: Self::normalize_date(row.get(9)?),
                })
            }).map_err(TingError::DatabaseError)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(TingError::DatabaseError)?;

            Ok(tasks)
        }).await
    }

    /// Find tasks by type
    pub async fn find_by_type(&self, task_type: &str) -> Result<Vec<TaskRecord>> {
        let task_type = task_type.to_string();
        self.db.execute(move |conn| {
            let mut stmt = conn.prepare(
                "SELECT id, type, status, payload, message, error, retries, max_retries, created_at, updated_at \
                 FROM tasks WHERE type = ? ORDER BY created_at DESC"
            ).map_err(TingError::DatabaseError)?;

            let tasks = stmt.query_map([&task_type], |row| {
                Ok(TaskRecord {
                    id: row.get(0)?,
                    task_type: row.get(1)?,
                    status: row.get(2)?,
                    payload: row.get(3)?,
                    message: row.get(4)?,
                    error: row.get(5)?,
                    retries: row.get(6)?,
                    max_retries: row.get(7)?,
                    created_at: Self::normalize_date(row.get(8)?),
                    updated_at: Self::normalize_date(row.get(9)?),
                })
            }).map_err(TingError::DatabaseError)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(TingError::DatabaseError)?;

            Ok(tasks)
        }).await
    }

    /// Find tasks with filtering, sorting, and pagination
    pub async fn find_with_filters(
        &self,
        status: Option<String>,
        page: u32,
        page_size: u32,
        sort_by: String,
        sort_order: String,
    ) -> Result<(Vec<TaskRecord>, usize)> {
        self.db.execute(move |conn| {
            // Build the WHERE clause
            let where_clause = if status.is_some() {
                "WHERE status = ?"
            } else {
                ""
            };

            // Validate sort_by field
            let sort_field = match sort_by.as_str() {
                "created_at" => "created_at",
                "status" => "status",
                "task_type" | "type" => "type",
                _ => "created_at", // default
            };

            // Validate sort_order
            let order = match sort_order.as_str() {
                "asc" => "ASC",
                "desc" => "DESC",
                _ => "DESC", // default
            };

            // First, get the total count
            let count_query = format!("SELECT COUNT(*) FROM tasks {}", where_clause);
            let total: usize = if let Some(ref status_val) = status {
                conn.query_row(&count_query, [status_val], |row| row.get(0))
                    .map_err(TingError::DatabaseError)?
            } else {
                conn.query_row(&count_query, [], |row| row.get(0))
                    .map_err(TingError::DatabaseError)?
            };

            // Calculate offset
            let offset = (page.saturating_sub(1)) * page_size;

            // Build the main query with pagination
            let query = format!(
                "SELECT id, type, status, payload, message, error, retries, max_retries, created_at, updated_at \
                 FROM tasks {} ORDER BY {} {} LIMIT ? OFFSET ?",
                where_clause, sort_field, order
            );

            let mut stmt = conn.prepare(&query).map_err(TingError::DatabaseError)?;

            let tasks = if let Some(ref status_val) = status {
                stmt.query_map(
                    rusqlite::params![status_val, page_size, offset],
                    |row| {
                        Ok(TaskRecord {
                            id: row.get(0)?,
                            task_type: row.get(1)?,
                            status: row.get(2)?,
                            payload: row.get(3)?,
                            message: row.get(4)?,
                            error: row.get(5)?,
                            retries: row.get(6)?,
                            max_retries: row.get(7)?,
                            created_at: Self::normalize_date(row.get(8)?),
                            updated_at: Self::normalize_date(row.get(9)?),
                        })
                    }
                ).map_err(TingError::DatabaseError)?
                .collect::<std::result::Result<Vec<_>, _>>()
                .map_err(TingError::DatabaseError)?
            } else {
                stmt.query_map(
                    rusqlite::params![page_size, offset],
                    |row| {
                        Ok(TaskRecord {
                            id: row.get(0)?,
                            task_type: row.get(1)?,
                            status: row.get(2)?,
                            payload: row.get(3)?,
                            message: row.get(4)?,
                            error: row.get(5)?,
                            retries: row.get(6)?,
                            max_retries: row.get(7)?,
                            created_at: Self::normalize_date(row.get(8)?),
                            updated_at: Self::normalize_date(row.get(9)?),
                        })
                    }
                ).map_err(TingError::DatabaseError)?
                .collect::<std::result::Result<Vec<_>, _>>()
                .map_err(TingError::DatabaseError)?
            };

            Ok((tasks, total))
        }).await
    }

    /// Update task progress message
    pub async fn update_progress(&self, id: &str, message: &str) -> Result<()> {
        let id = id.to_string();
        let message = message.to_string();
        self.db.execute(move |conn| {
            conn.execute(
                "UPDATE tasks SET message = ?, updated_at = STRFTIME('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ?",
                rusqlite::params![&message, &id],
            ).map_err(TingError::DatabaseError)?;
            Ok(())
        }).await
    }

    /// Update task status
    pub async fn update_status(
        &self,
        id: &str,
        status: &str,
        error: Option<&str>,
        retries: i32,
    ) -> Result<()> {
        let id = id.to_string();
        let status = status.to_string();
        let error = error.map(|s| s.to_string());
        self.db.execute(move |conn| {
            conn.execute(
                "UPDATE tasks SET status = ?, error = ?, retries = ?, updated_at = STRFTIME('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ?",
                rusqlite::params![&status, &error, retries, &id],
            ).map_err(TingError::DatabaseError)?;
            Ok(())
        }).await
    }

    pub async fn delete_all(&self) -> Result<()> {
        self.db
            .execute(move |conn| {
                conn.execute("DELETE FROM tasks", [])
                    .map_err(TingError::DatabaseError)?;
                Ok(())
            })
            .await
    }

    pub async fn delete_by_status(&self, status: &str) -> Result<()> {
        let status = status.to_string();
        self.db
            .execute(move |conn| {
                conn.execute("DELETE FROM tasks WHERE status = ?", [&status])
                    .map_err(TingError::DatabaseError)?;
                Ok(())
            })
            .await
    }

    pub async fn delete_batch(&self, ids: Vec<String>) -> Result<usize> {
        self.db
            .transaction(move |tx| {
                let mut count = 0;
                {
                    // Only delete non-running tasks for safety
                    let mut stmt = tx
                        .prepare("DELETE FROM tasks WHERE id = ? AND status != 'running'")
                        .map_err(TingError::DatabaseError)?;
                    for id in &ids {
                        count += stmt.execute([id]).map_err(TingError::DatabaseError)?;
                    }
                }
                Ok(count)
            })
            .await
    }
}

#[async_trait]
impl Repository<TaskRecord> for TaskRepository {
    async fn find_by_id(&self, id: &str) -> Result<Option<TaskRecord>> {
        let id = id.to_string();
        self.db.execute(move |conn| {
            conn.query_row(
                "SELECT id, type, status, payload, message, error, retries, max_retries, created_at, updated_at \
                 FROM tasks WHERE id = ?",
                [&id],
                |row| {
                    Ok(TaskRecord {
                        id: row.get(0)?,
                        task_type: row.get(1)?,
                        status: row.get(2)?,
                        payload: row.get(3)?,
                        message: row.get(4)?,
                        error: row.get(5)?,
                        retries: row.get(6)?,
                        max_retries: row.get(7)?,
                        created_at: Self::normalize_date(row.get(8)?),
                        updated_at: Self::normalize_date(row.get(9)?),
                    })
                }
            ).optional()
            .map_err(TingError::DatabaseError)
        }).await
    }

    async fn find_all(&self) -> Result<Vec<TaskRecord>> {
        self.db.execute(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, type, status, payload, message, error, retries, max_retries, created_at, updated_at \
                 FROM tasks ORDER BY created_at DESC"
            ).map_err(TingError::DatabaseError)?;

            let tasks = stmt.query_map([], |row| {
                Ok(TaskRecord {
                    id: row.get(0)?,
                    task_type: row.get(1)?,
                    status: row.get(2)?,
                    payload: row.get(3)?,
                    message: row.get(4)?,
                    error: row.get(5)?,
                    retries: row.get(6)?,
                    max_retries: row.get(7)?,
                    created_at: Self::normalize_date(row.get(8)?),
                    updated_at: Self::normalize_date(row.get(9)?),
                })
            }).map_err(TingError::DatabaseError)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(TingError::DatabaseError)?;

            Ok(tasks)
        }).await
    }

    async fn create(&self, task: &TaskRecord) -> Result<()> {
        let task = task.clone();
        self.db.execute(move |conn| {
            conn.execute(
                "INSERT INTO tasks (id, type, status, payload, message, error, retries, max_retries, created_at, updated_at) \
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                rusqlite::params![
                    &task.id,
                    &task.task_type,
                    &task.status,
                    &task.payload,
                    &task.message,
                    &task.error,
                    &task.retries,
                    &task.max_retries,
                    &task.created_at,
                    &task.updated_at,
                ],
            ).map_err(TingError::DatabaseError)?;
            Ok(())
        }).await
    }

    async fn update(&self, task: &TaskRecord) -> Result<()> {
        let task = task.clone();
        self.db
            .execute(move |conn| {
                conn.execute(
                    "UPDATE tasks SET type = ?, status = ?, payload = ?, message = ?, error = ?, \
                 retries = ?, max_retries = ?, updated_at = ? WHERE id = ?",
                    rusqlite::params![
                        &task.task_type,
                        &task.status,
                        &task.payload,
                        &task.message,
                        &task.error,
                        &task.retries,
                        &task.max_retries,
                        &task.updated_at,
                        &task.id,
                    ],
                )
                .map_err(TingError::DatabaseError)?;
                Ok(())
            })
            .await
    }

    async fn delete(&self, id: &str) -> Result<()> {
        let id = id.to_string();
        self.db
            .execute(move |conn| {
                conn.execute("DELETE FROM tasks WHERE id = ?", [&id])
                    .map_err(TingError::DatabaseError)?;
                Ok(())
            })
            .await
    }
}
