use crate::core::app::error::{Result, TingError};
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
        start_time: row.get(10).unwrap_or(None),
        end_time: row.get(11).unwrap_or(None),
    })
}

const CHAPTER_COLUMNS: &str = "id, book_id, title, path, duration, chapter_index, is_extra, hash, created_at, manual_corrected, start_time, end_time";

pub(super) fn insert_chapter(conn: &rusqlite::Connection, chapter: &Chapter) -> Result<()> {
    conn.execute(
        "INSERT INTO chapters (id, book_id, title, path, duration, chapter_index, is_extra, hash, manual_corrected, start_time, end_time)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        rusqlite::params![
            chapter.id, chapter.book_id, chapter.title, chapter.path, chapter.duration,
            chapter.chapter_index, chapter.is_extra, chapter.hash, chapter.manual_corrected,
            chapter.start_time, chapter.end_time,
        ],
    )?;
    Ok(())
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
    /// Chapters with no usable duration, excluding URL-reference STRM files.
    pub missing_duration: usize,
    /// Distinct audio files referenced by the book, used to recognise
    /// single-file audiobooks split into embedded chapters.
    pub path_count: usize,
    /// Chapters that carry an explicit start offset inside their file.
    pub segmented: usize,
}

impl ChapterRepository {
    /// Create a new ChapterRepository
    pub fn new(db: Arc<DatabaseManager>) -> Self {
        Self { db }
    }

    /// Find chapters by book ID
    pub async fn find_by_book(&self, book_id: &str) -> Result<Vec<Chapter>> {
        let book_id = book_id.to_string();
        self.db
            .execute(move |conn| {
                let mut stmt = conn
                    .prepare(&format!(
                        "SELECT {CHAPTER_COLUMNS} \
                 FROM chapters WHERE book_id = ? ORDER BY is_extra ASC, chapter_index ASC"
                    ))
                    .map_err(TingError::DatabaseError)?;

                let chapters = stmt
                    .query_map([&book_id], map_chapter_row)
                    .map_err(TingError::DatabaseError)?
                    .collect::<std::result::Result<Vec<_>, _>>()
                    .map_err(TingError::DatabaseError)?;

                Ok(chapters)
            })
            .await
    }

    /// Find a chapter by its hash
    pub async fn find_by_hash(&self, hash: &str) -> Result<Option<Chapter>> {
        let hash = hash.to_string();
        self.db
            .execute(move |conn| {
                conn.query_row(
                    &format!("SELECT {CHAPTER_COLUMNS} FROM chapters WHERE hash = ?"),
                    [&hash],
                    map_chapter_row,
                )
                .optional()
                .map_err(TingError::DatabaseError)
            })
            .await
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
                 p.position, p.updated_at, c.manual_corrected, c.start_time, c.end_time \
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
                    start_time: row.get(12).unwrap_or(None),
                    end_time: row.get(13).unwrap_or(None),
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
                let (total, main, extra, missing_duration, path_count, segmented): (
                    i64,
                    i64,
                    i64,
                    i64,
                    i64,
                    i64,
                ) = conn
                    .query_row(
                        "SELECT \
                         COUNT(*) AS total, \
                         COALESCE(SUM(CASE WHEN is_extra = 0 THEN 1 ELSE 0 END), 0) AS main, \
                         COALESCE(SUM(CASE WHEN is_extra != 0 THEN 1 ELSE 0 END), 0) AS extra, \
                         COALESCE(SUM(CASE WHEN COALESCE(duration, 0) <= 0 \
                             AND LOWER(path) NOT LIKE '%.strm' THEN 1 ELSE 0 END), 0) AS missing_duration, \
                         COUNT(DISTINCT path) AS path_count, \
                         COALESCE(SUM(CASE WHEN start_time IS NOT NULL THEN 1 ELSE 0 END), 0) AS segmented \
                         FROM chapters WHERE book_id = ?",
                        [&book_id],
                        |row| {
                            Ok((
                                row.get(0)?,
                                row.get(1)?,
                                row.get(2)?,
                                row.get(3)?,
                                row.get(4)?,
                                row.get(5)?,
                            ))
                        },
                    )
                    .map_err(TingError::DatabaseError)?;

                Ok(ChapterCounts {
                    total: total.max(0) as usize,
                    main: main.max(0) as usize,
                    extra: extra.max(0) as usize,
                    missing_duration: missing_duration.max(0) as usize,
                    path_count: path_count.max(0) as usize,
                    segmented: segmented.max(0) as usize,
                })
            })
            .await
    }

    /// Count chapters for every book in a library in one query. Scanners use
    /// this to avoid issuing one COUNT query per directory during incremental
    /// scans.
    pub async fn count_by_library(
        &self,
        library_id: &str,
    ) -> Result<std::collections::HashMap<String, ChapterCounts>> {
        let library_id = library_id.to_string();
        self.db
            .execute(move |conn| {
                let mut stmt = conn
                    .prepare(
                        "SELECT c.book_id,
                                COUNT(*) AS total,
                                COALESCE(SUM(CASE WHEN c.is_extra = 0 THEN 1 ELSE 0 END), 0) AS main,
                                COALESCE(SUM(CASE WHEN c.is_extra != 0 THEN 1 ELSE 0 END), 0) AS extra,
                                COALESCE(SUM(CASE WHEN COALESCE(c.duration, 0) <= 0
                                    AND LOWER(c.path) NOT LIKE '%.strm' THEN 1 ELSE 0 END), 0) AS missing_duration,
                                COUNT(DISTINCT c.path) AS path_count,
                                COALESCE(SUM(CASE WHEN c.start_time IS NOT NULL THEN 1 ELSE 0 END), 0) AS segmented
                           FROM chapters c
                           JOIN books b ON b.id = c.book_id
                          WHERE b.library_id = ?
                          GROUP BY c.book_id",
                    )
                    .map_err(TingError::DatabaseError)?;

                let rows = stmt
                    .query_map([&library_id], |row| {
                        let total: i64 = row.get(1)?;
                        let main: i64 = row.get(2)?;
                        let extra: i64 = row.get(3)?;
                        let missing_duration: i64 = row.get(4)?;
                        let path_count: i64 = row.get(5)?;
                        let segmented: i64 = row.get(6)?;
                        Ok((
                            row.get::<_, String>(0)?,
                            ChapterCounts {
                                total: total.max(0) as usize,
                                main: main.max(0) as usize,
                                extra: extra.max(0) as usize,
                                missing_duration: missing_duration.max(0) as usize,
                                path_count: path_count.max(0) as usize,
                                segmented: segmented.max(0) as usize,
                            },
                        ))
                    })
                    .map_err(TingError::DatabaseError)?;

                rows.collect::<std::result::Result<std::collections::HashMap<_, _>, _>>()
                    .map_err(TingError::DatabaseError)
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
                 p.position, p.updated_at, c.manual_corrected, c.start_time, c.end_time \
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
                            start_time: row.get(12).unwrap_or(None),
                            end_time: row.get(13).unwrap_or(None),
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
        self.db
            .execute(move |conn| {
                conn.query_row(
                    &format!("SELECT {CHAPTER_COLUMNS} FROM chapters WHERE id = ?"),
                    [&id],
                    map_chapter_row,
                )
                .optional()
                .map_err(TingError::DatabaseError)
            })
            .await
    }

    async fn find_all(&self) -> Result<Vec<Chapter>> {
        self.db
            .execute(|conn| {
                let mut stmt = conn
                    .prepare(&format!(
                        "SELECT {CHAPTER_COLUMNS} FROM chapters ORDER BY book_id, chapter_index ASC"
                    ))
                    .map_err(TingError::DatabaseError)?;

                let chapters = stmt
                    .query_map([], map_chapter_row)
                    .map_err(TingError::DatabaseError)?
                    .collect::<std::result::Result<Vec<_>, _>>()
                    .map_err(TingError::DatabaseError)?;

                Ok(chapters)
            })
            .await
    }

    async fn create(&self, chapter: &Chapter) -> Result<()> {
        let chapter = chapter.clone();
        self.db
            .execute(move |conn| insert_chapter(conn, &chapter))
            .await
    }

    async fn update(&self, chapter: &Chapter) -> Result<()> {
        let chapter = chapter.clone();
        self.db
            .execute(move |conn| {
                conn.execute(
                    "UPDATE chapters SET book_id = ?, title = ?, path = ?, duration = ?, \
                 chapter_index = ?, is_extra = ?, hash = ?, manual_corrected = ?, start_time = ?, end_time = ? WHERE id = ?",
                    rusqlite::params![
                        &chapter.book_id,
                        &chapter.title,
                        &chapter.path,
                        chapter.duration,
                        chapter.chapter_index,
                        chapter.is_extra,
                        &chapter.hash,
                        chapter.manual_corrected,
                        chapter.start_time,
                        chapter.end_time,
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
#[path = "../../../tests/unit/db/repository/chapter.rs"]
mod tests;
