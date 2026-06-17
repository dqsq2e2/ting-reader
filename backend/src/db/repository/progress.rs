use crate::core::error::{Result, TingError};
use crate::db::{manager::DatabaseManager, models::Progress};
use rusqlite::OptionalExtension;
use std::sync::Arc;

/// Repository for Progress entities
pub struct ProgressRepository {
    db: Arc<DatabaseManager>,
}

impl ProgressRepository {
    /// Create a new ProgressRepository
    pub fn new(db: Arc<DatabaseManager>) -> Self {
        Self { db }
    }

    /// Get recent progress for a user
    pub async fn get_recent(&self, user_id: &str, limit: i32) -> Result<Vec<Progress>> {
        let user_id = user_id.to_string();
        self.db
            .execute(move |conn| {
                let mut stmt = conn
                    .prepare(
                        "SELECT p.id, p.user_id, p.book_id, p.chapter_id, p.position, p.duration, p.updated_at \
                 FROM progress p \
                 WHERE p.user_id = ? \
                 AND NOT EXISTS ( \
                   SELECT 1 FROM progress newer \
                   WHERE newer.user_id = p.user_id \
                   AND newer.book_id = p.book_id \
                   AND (newer.updated_at > p.updated_at OR (newer.updated_at = p.updated_at AND newer.id > p.id)) \
                 ) \
                 ORDER BY p.updated_at DESC LIMIT ?",
                    )
                    .map_err(TingError::DatabaseError)?;

                let progress = stmt
                    .query_map(rusqlite::params![&user_id, limit], |row| {
                        Ok(Progress {
                            id: row.get(0)?,
                            user_id: row.get(1)?,
                            book_id: row.get(2)?,
                            chapter_id: row.get(3)?,
                            position: row.get(4)?,
                            duration: row.get(5)?,
                            updated_at: row.get(6)?,
                        })
                    })
                    .map_err(TingError::DatabaseError)?
                    .collect::<std::result::Result<Vec<_>, _>>()
                    .map_err(TingError::DatabaseError)?;

                Ok(progress)
            })
            .await
    }

    /// Get recent progress enriched with book and chapter details
    pub async fn get_recent_enriched(
        &self,
        user_id: &str,
        limit: i32,
    ) -> Result<
        Vec<(
            Progress,
            Option<String>,
            Option<String>,
            Option<String>,
            Option<String>,
            Option<i32>,
        )>,
    > {
        let user_id = user_id.to_string();
        self.db.execute(move |conn| {
            let mut stmt = conn.prepare(
                "SELECT p.id, p.user_id, p.book_id, p.chapter_id, p.position, p.duration, p.updated_at, \
                 b.title as book_title, b.cover_url, b.library_id, c.title as chapter_title, c.duration as chapter_duration \
                 FROM progress p \
                 JOIN books b ON p.book_id = b.id \
                 LEFT JOIN chapters c ON p.chapter_id = c.id \
                 WHERE p.user_id = ? \
                 AND NOT EXISTS ( \
                   SELECT 1 FROM progress newer \
                   WHERE newer.user_id = p.user_id \
                   AND newer.book_id = p.book_id \
                   AND (newer.updated_at > p.updated_at OR (newer.updated_at = p.updated_at AND newer.id > p.id)) \
                 ) \
                 ORDER BY p.updated_at DESC \
                 LIMIT ?"
            ).map_err(TingError::DatabaseError)?;

            let progress = stmt.query_map(rusqlite::params![&user_id, limit], |row| {
                let progress = Progress {
                    id: row.get(0)?,
                    user_id: row.get(1)?,
                    book_id: row.get(2)?,
                    chapter_id: row.get(3)?,
                    position: row.get(4)?,
                    duration: row.get(5)?,
                    updated_at: row.get(6)?,
                };
                let book_title: Option<String> = row.get(7)?;
                let cover_url: Option<String> = row.get(8)?;
                let library_id: Option<String> = row.get(9)?;
                let chapter_title: Option<String> = row.get(10)?;
                let chapter_duration: Option<i32> = row.get(11)?;

                Ok((progress, book_title, cover_url, library_id, chapter_title, chapter_duration))
            }).map_err(TingError::DatabaseError)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(TingError::DatabaseError)?;

            Ok(progress)
        }).await
    }

    /// Get progress for a specific book
    pub async fn get_by_book(&self, user_id: &str, book_id: &str) -> Result<Option<Progress>> {
        let user_id = user_id.to_string();
        let book_id = book_id.to_string();
        self.db
            .execute(move |conn| {
                conn.query_row(
                    "SELECT id, user_id, book_id, chapter_id, position, duration, updated_at \
                 FROM progress WHERE user_id = ? AND book_id = ? \
                 ORDER BY updated_at DESC LIMIT 1",
                    rusqlite::params![&user_id, &book_id],
                    |row| {
                        Ok(Progress {
                            id: row.get(0)?,
                            user_id: row.get(1)?,
                            book_id: row.get(2)?,
                            chapter_id: row.get(3)?,
                            position: row.get(4)?,
                            duration: row.get(5)?,
                            updated_at: row.get(6)?,
                        })
                    },
                )
                .optional()
                .map_err(TingError::DatabaseError)
            })
            .await
    }

    pub async fn exists_for_chapter(
        &self,
        user_id: &str,
        book_id: &str,
        chapter_id: Option<&str>,
    ) -> Result<bool> {
        let user_id = user_id.to_string();
        let book_id = book_id.to_string();
        let chapter_id = chapter_id.map(str::to_string);
        self.db
            .execute(move |conn| {
                let count: i64 = if let Some(chapter_id) = chapter_id {
                    conn.query_row(
                        "SELECT COUNT(*) FROM progress WHERE user_id = ? AND book_id = ? AND chapter_id = ?",
                        rusqlite::params![&user_id, &book_id, &chapter_id],
                        |row| row.get(0),
                    )
                } else {
                    conn.query_row(
                        "SELECT COUNT(*) FROM progress WHERE user_id = ? AND book_id = ? AND chapter_id IS NULL",
                        rusqlite::params![&user_id, &book_id],
                        |row| row.get(0),
                    )
                }
                .map_err(TingError::DatabaseError)?;

                Ok(count > 0)
            })
            .await
    }

    /// Clear all playback progress for a user
    pub async fn clear_by_user(&self, user_id: &str) -> Result<usize> {
        let user_id = user_id.to_string();
        self.db
            .execute(move |conn| {
                conn.execute("DELETE FROM progress WHERE user_id = ?", [&user_id])
                    .map_err(TingError::DatabaseError)
            })
            .await
    }

    /// Upsert progress (insert or update)
    pub async fn upsert(&self, progress: &Progress) -> Result<()> {
        let progress = progress.clone();
        self.db.execute(move |conn| {
            let progress_position: Option<f64> = if progress.chapter_id.is_none() {
                conn.query_row(
                    "SELECT position FROM progress WHERE user_id = ? AND book_id = ? AND chapter_id IS NULL",
                    rusqlite::params![&progress.user_id, &progress.book_id],
                    |row| row.get(0),
                ).optional().map_err(TingError::DatabaseError)?
            } else {
                conn.query_row(
                    "SELECT position FROM progress WHERE user_id = ? AND book_id = ? AND chapter_id = ?",
                    rusqlite::params![&progress.user_id, &progress.book_id, &progress.chapter_id],
                    |row| row.get(0),
                ).optional().map_err(TingError::DatabaseError)?
            };
            let previous_position: Option<f64> = if progress_position.is_some() {
                progress_position
            } else if progress.chapter_id.is_none() {
                conn.query_row(
                    "SELECT position FROM listening_events WHERE user_id = ? AND book_id = ? AND chapter_id IS NULL ORDER BY created_at DESC LIMIT 1",
                    rusqlite::params![&progress.user_id, &progress.book_id],
                    |row| row.get(0),
                ).optional().map_err(TingError::DatabaseError)?
            } else {
                conn.query_row(
                    "SELECT position FROM listening_events WHERE user_id = ? AND book_id = ? AND chapter_id = ? ORDER BY created_at DESC LIMIT 1",
                    rusqlite::params![&progress.user_id, &progress.book_id, &progress.chapter_id],
                    |row| row.get(0),
                ).optional().map_err(TingError::DatabaseError)?
            };

            let listen_seconds = if !progress.position.is_finite() || progress.position <= 0.0 {
                0.0
            } else if let Some(previous) = previous_position {
                (progress.position - previous).max(0.0)
            } else {
                progress.position
            };

            conn.execute(
                "INSERT INTO listening_events (id, user_id, book_id, chapter_id, position, duration, listen_seconds, created_at) \
                 VALUES (?, ?, ?, ?, ?, ?, ?, STRFTIME('%Y-%m-%dT%H:%M:%fZ', 'now'))",
                rusqlite::params![
                    &progress.id,
                    &progress.user_id,
                    &progress.book_id,
                    &progress.chapter_id,
                    progress.position,
                    progress.duration,
                    listen_seconds,
                ],
            )
            .map_err(TingError::DatabaseError)?;

            // Handle NULL chapter_id carefully since SQLite UNIQUE constraint treats NULLs as distinct
            if progress.chapter_id.is_none() {
                // If chapter_id is NULL, we just insert or update based on user_id and book_id
                // This is a fallback case, normally chapter_id should be provided
                let existing_id: Option<String> = conn.query_row(
                    "SELECT id FROM progress WHERE user_id = ? AND book_id = ? AND chapter_id IS NULL",
                    rusqlite::params![&progress.user_id, &progress.book_id],
                    |row| row.get(0)
                ).optional().unwrap_or(None);

                if let Some(id) = existing_id {
                    conn.execute(
                        "UPDATE progress SET position = ?, duration = ?, updated_at = STRFTIME('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ?",
                        rusqlite::params![progress.position, progress.duration, id],
                    ).map_err(TingError::DatabaseError)?;
                } else {
                    conn.execute(
                        "INSERT INTO progress (id, user_id, book_id, chapter_id, position, duration, updated_at) \
                         VALUES (?, ?, ?, ?, ?, ?, STRFTIME('%Y-%m-%dT%H:%M:%fZ', 'now'))",
                        rusqlite::params![
                            &progress.id,
                            &progress.user_id,
                            &progress.book_id,
                            &progress.chapter_id,
                            progress.position,
                            progress.duration,
                        ],
                    ).map_err(TingError::DatabaseError)?;
                }
            } else {
                conn.execute(
                    "INSERT INTO progress (id, user_id, book_id, chapter_id, position, duration, updated_at) \
                     VALUES (?, ?, ?, ?, ?, ?, STRFTIME('%Y-%m-%dT%H:%M:%fZ', 'now')) \
                     ON CONFLICT(user_id, book_id, chapter_id) DO UPDATE SET \
                     position = excluded.position, \
                     duration = excluded.duration, \
                     updated_at = STRFTIME('%Y-%m-%dT%H:%M:%fZ', 'now')",
                    rusqlite::params![
                        &progress.id,
                        &progress.user_id,
                        &progress.book_id,
                        &progress.chapter_id,
                        progress.position,
                        progress.duration,
                    ],
                ).map_err(TingError::DatabaseError)?;
            }
            Ok(())
        }).await
    }
}
