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

#[derive(Debug, Clone, Copy)]
pub struct ChapterCounts {
    pub total: usize,
    pub main: usize,
    pub extra: usize,
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
                 ORDER BY c.is_extra ASC, COALESCE(c.chapter_index, 0) ASC, c.created_at ASC, c.id ASC"
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

    /// Count chapters by book ID, split by main/extra.
    pub async fn count_by_book(&self, book_id: &str) -> Result<ChapterCounts> {
        let book_id = book_id.to_string();
        self.db
            .execute(move |conn| {
                let (total, main, extra): (i64, i64, i64) = conn
                    .query_row(
                        "SELECT \
                         COUNT(*) AS total, \
                         COALESCE(SUM(CASE WHEN is_extra = 0 THEN 1 ELSE 0 END), 0) AS main, \
                         COALESCE(SUM(CASE WHEN is_extra != 0 THEN 1 ELSE 0 END), 0) AS extra \
                         FROM chapters WHERE book_id = ?",
                        [&book_id],
                        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                    )
                    .map_err(TingError::DatabaseError)?;

                Ok(ChapterCounts {
                    total: total.max(0) as usize,
                    main: main.max(0) as usize,
                    extra: extra.max(0) as usize,
                })
            })
            .await
    }

    /// Resolve the page offset that contains a target chapter in the selected chapter type.
    pub async fn page_offset_for_chapter(
        &self,
        book_id: &str,
        chapter_id: &str,
        requested_is_extra: Option<i32>,
        limit: usize,
        descending: bool,
    ) -> Result<Option<(usize, i32)>> {
        if limit == 0 {
            return Ok(None);
        }

        let book_id = book_id.to_string();
        let chapter_id = chapter_id.to_string();
        self.db
            .execute(move |conn| {
                let target = conn
                    .query_row(
                        "SELECT is_extra, COALESCE(chapter_index, 0) \
                         FROM chapters WHERE book_id = ? AND id = ?",
                        rusqlite::params![&book_id, &chapter_id],
                        |row| Ok((row.get::<_, i32>(0)?, row.get::<_, i32>(1)?)),
                    )
                    .optional()
                    .map_err(TingError::DatabaseError)?;

                let Some((chapter_is_extra, chapter_index)) = target else {
                    return Ok(None);
                };

                let resolved_is_extra = requested_is_extra.unwrap_or(chapter_is_extra);
                let comparison = if descending { ">" } else { "<" };
                let sql = format!(
                    "SELECT COUNT(*) FROM chapters \
                     WHERE book_id = ?1 AND is_extra = ?2 \
                     AND COALESCE(chapter_index, 0) {} ?3",
                    comparison
                );

                let before_count: i64 = conn
                    .query_row(
                        &sql,
                        rusqlite::params![&book_id, resolved_is_extra, chapter_index],
                        |row| row.get(0),
                    )
                    .map_err(TingError::DatabaseError)?;

                let index = before_count.max(0) as usize;
                Ok(Some(((index / limit) * limit, resolved_is_extra)))
            })
            .await
    }

    /// Find a page of chapters by book ID with user progress.
    pub async fn find_by_book_with_progress_page(
        &self,
        book_id: &str,
        user_id: &str,
        is_extra: Option<i32>,
        offset: usize,
        limit: usize,
        descending: bool,
    ) -> Result<Vec<(Chapter, Option<f64>, Option<String>)>> {
        let book_id = book_id.to_string();
        let user_id = user_id.to_string();
        let direction = if descending { "DESC" } else { "ASC" };

        self.db.execute(move |conn| {
            let sql = format!(
                "SELECT c.id, c.book_id, c.title, c.path, c.duration, c.chapter_index, c.is_extra, c.hash, c.created_at, \
                 p.position, p.updated_at, c.manual_corrected \
                 FROM chapters c \
                 LEFT JOIN progress p ON c.id = p.chapter_id AND p.user_id = ?1 \
                 WHERE c.book_id = ?2 AND (?3 IS NULL OR c.is_extra = ?3) \
                 ORDER BY c.is_extra ASC, COALESCE(c.chapter_index, 0) {direction}, c.created_at {direction}, c.id {direction} \
                 LIMIT ?4 OFFSET ?5"
            );

            let mut stmt = conn.prepare(&sql).map_err(TingError::DatabaseError)?;
            let chapters = stmt
                .query_map(
                    rusqlite::params![
                        &user_id,
                        &book_id,
                        is_extra,
                        limit as i64,
                        offset as i64,
                    ],
                    |row| {
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
                    },
                )
                .map_err(TingError::DatabaseError)?
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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn find_by_book_with_progress_groups_main_chapters_before_extras() {
        let db = Arc::new(DatabaseManager::new_in_memory().unwrap());
        db.execute(|conn| {
            conn.execute_batch("PRAGMA foreign_keys = OFF;")
                .map_err(TingError::DatabaseError)?;

            for (id, chapter_index, is_extra) in [
                ("main-1", 1, 0),
                ("extra-1", 1, 1),
                ("main-2", 2, 0),
                ("extra-2", 2, 1),
            ] {
                conn.execute(
                    "INSERT INTO chapters (id, book_id, title, path, chapter_index, is_extra) \
                     VALUES (?1, 'book-1', ?1, ?1, ?2, ?3)",
                    rusqlite::params![id, chapter_index, is_extra],
                )
                .map_err(TingError::DatabaseError)?;
            }

            Ok(())
        })
        .await
        .unwrap();

        let repository = ChapterRepository::new(db);
        let chapters = repository
            .find_by_book_with_progress("book-1", "user-1")
            .await
            .unwrap();
        let chapter_ids: Vec<_> = chapters
            .into_iter()
            .map(|(chapter, _, _)| chapter.id)
            .collect();

        assert_eq!(chapter_ids, vec!["main-1", "main-2", "extra-1", "extra-2"]);

        let paged_chapters = repository
            .find_by_book_with_progress_page("book-1", "user-1", None, 0, 10, false)
            .await
            .unwrap();
        let paged_chapter_ids: Vec<_> = paged_chapters
            .into_iter()
            .map(|(chapter, _, _)| chapter.id)
            .collect();

        assert_eq!(
            paged_chapter_ids,
            vec!["main-1", "main-2", "extra-1", "extra-2"]
        );
    }
}
