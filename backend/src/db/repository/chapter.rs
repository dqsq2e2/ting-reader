use crate::core::error::{Result, TingError};
use crate::db::manager::DatabaseManager;
use crate::db::models::Chapter;
use crate::db::repository::base::Repository;
use async_trait::async_trait;
use rusqlite::{OptionalExtension, Row};
use std::sync::Arc;

fn map_chapter_row(row: &Row<'_>) -> rusqlite::Result<Chapter> {
    Ok(Chapter {
        id: row.get(0)?,
        book_id: row.get(1)?,
        title: row.get(2)?,
        path: row.get(3)?,
        duration: row.get(4)?,
        chapter_index: row.get(5)?,
        is_extra: row.get(6)?,
        hash: row.get(7)?,
        created_at: row.get(8)?,
        manual_corrected: row.get(9).unwrap_or(0),
    })
}

/// Repository for Chapter entities
pub struct ChapterRepository {
    db: Arc<DatabaseManager>,
}

impl ChapterRepository {
    /// Create a new ChapterRepository
    pub fn new(db: Arc<DatabaseManager>) -> Self {
        Self { db }
    }

    /// Find chapters by book ID
    pub async fn find_by_book(&self, book_id: &str) -> Result<Vec<Chapter>> {
        let book_id = book_id.to_string();
        self.db.execute(move |conn| {
            let mut stmt = conn.prepare(
                "SELECT id, book_id, title, path, duration, chapter_index, is_extra, hash, created_at, manual_corrected \
                 FROM chapters WHERE book_id = ? ORDER BY is_extra ASC, chapter_index ASC"
            ).map_err(TingError::DatabaseError)?;

            let chapters = stmt.query_map([&book_id], map_chapter_row)
            .map_err(TingError::DatabaseError)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(TingError::DatabaseError)?;

            Ok(chapters)
        }).await
    }

    /// Find a chapter by its hash
    pub async fn find_by_hash(&self, hash: &str) -> Result<Option<Chapter>> {
        let hash = hash.to_string();
        self.db.execute(move |conn| {
            conn.query_row(
                "SELECT id, book_id, title, path, duration, chapter_index, is_extra, hash, created_at, manual_corrected \
                 FROM chapters WHERE hash = ?",
                [&hash],
                map_chapter_row
            ).optional()
            .map_err(TingError::DatabaseError)
        }).await
    }

    /// Delete chapters by book ID
    pub async fn delete_by_book(&self, book_id: &str) -> Result<()> {
        let book_id = book_id.to_string();
        self.db
            .execute(move |conn| {
                conn.execute("DELETE FROM chapters WHERE book_id = ?", [&book_id])
                    .map_err(TingError::DatabaseError)?;
                Ok(())
            })
            .await
    }

    /// Find chapters by book ID with user progress
    pub async fn find_by_book_with_progress(
        &self,
        book_id: &str,
        user_id: &str,
    ) -> Result<Vec<(Chapter, Option<f64>, Option<String>)>> {
        let book_id = book_id.to_string();
        let user_id = user_id.to_string();

        self.db.execute(move |conn| {
            let mut stmt = conn.prepare(
                "SELECT c.id, c.book_id, c.title, c.path, c.duration, c.chapter_index, c.is_extra, c.hash, c.created_at, \
                 p.position, p.updated_at, c.manual_corrected \
                 FROM chapters c \
                 LEFT JOIN progress p ON c.id = p.chapter_id AND p.user_id = ? \
                 WHERE c.book_id = ? \
                 ORDER BY c.chapter_index ASC"
            ).map_err(TingError::DatabaseError)?;

            let chapters = stmt.query_map(rusqlite::params![&user_id, &book_id], |row| {
                let chapter = Chapter {
                    id: row.get(0)?,
                    book_id: row.get(1)?,
                    title: row.get(2)?,
                    path: row.get(3)?,
                    duration: row.get(4)?,
                    chapter_index: row.get(5)?,
                    is_extra: row.get(6)?,
                    hash: row.get(7)?,
                    created_at: row.get(8)?,
                    manual_corrected: row.get(11).unwrap_or(0),
                };
                let progress_position: Option<f64> = row.get(9)?;
                let progress_updated_at: Option<String> = row.get(10)?;

                Ok((chapter, progress_position, progress_updated_at))
            }).map_err(TingError::DatabaseError)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(TingError::DatabaseError)?;

            Ok(chapters)
        }).await
    }
}

#[async_trait]
impl Repository<Chapter> for ChapterRepository {
    async fn find_by_id(&self, id: &str) -> Result<Option<Chapter>> {
        let id = id.to_string();
        self.db.execute(move |conn| {
            conn.query_row(
                "SELECT id, book_id, title, path, duration, chapter_index, is_extra, hash, created_at, manual_corrected \
                 FROM chapters WHERE id = ?",
                [&id],
                map_chapter_row
            ).optional()
            .map_err(TingError::DatabaseError)
        }).await
    }

    async fn find_all(&self) -> Result<Vec<Chapter>> {
        self.db.execute(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, book_id, title, path, duration, chapter_index, is_extra, hash, created_at, manual_corrected \
                 FROM chapters ORDER BY book_id, chapter_index ASC"
            ).map_err(TingError::DatabaseError)?;

            let chapters = stmt.query_map([], map_chapter_row)
            .map_err(TingError::DatabaseError)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(TingError::DatabaseError)?;

            Ok(chapters)
        }).await
    }

    async fn create(&self, chapter: &Chapter) -> Result<()> {
        let chapter = chapter.clone();
        self.db.execute(move |conn| {
            conn.execute(
                "INSERT INTO chapters (id, book_id, title, path, duration, chapter_index, is_extra, hash, manual_corrected) \
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
                rusqlite::params![
                    &chapter.id,
                    &chapter.book_id,
                    &chapter.title,
                    &chapter.path,
                    chapter.duration,
                    chapter.chapter_index,
                    chapter.is_extra,
                    &chapter.hash,
                    chapter.manual_corrected,
                ],
            ).map_err(TingError::DatabaseError)?;
            Ok(())
        }).await
    }

    async fn update(&self, chapter: &Chapter) -> Result<()> {
        let chapter = chapter.clone();
        self.db
            .execute(move |conn| {
                conn.execute(
                    "UPDATE chapters SET book_id = ?, title = ?, path = ?, duration = ?, \
                 chapter_index = ?, is_extra = ?, hash = ?, manual_corrected = ? WHERE id = ?",
                    rusqlite::params![
                        &chapter.book_id,
                        &chapter.title,
                        &chapter.path,
                        chapter.duration,
                        chapter.chapter_index,
                        chapter.is_extra,
                        &chapter.hash,
                        chapter.manual_corrected,
                        &chapter.id,
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
                conn.execute("DELETE FROM chapters WHERE id = ?", [&id])
                    .map_err(TingError::DatabaseError)?;
                Ok(())
            })
            .await
    }
}
