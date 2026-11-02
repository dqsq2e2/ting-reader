use crate::core::app::error::{Result, TingError};
use crate::db::manager::DatabaseManager;
use crate::db::models::Book;
use crate::db::repository::base::Repository;
use async_trait::async_trait;
use rusqlite::{OptionalExtension, Row};
use std::sync::Arc;

pub(crate) fn map_book_row(row: &Row<'_>) -> rusqlite::Result<Book> {
    Ok(Book {
        id: row.get(0)?,
        library_id: row.get(1)?,
        title: row.get(2)?,
        author: row.get(3)?,
        narrator: row.get(4)?,
        cover_url: row.get(5)?,
        theme_color: row.get(6)?,
        description: row.get(7)?,
        skip_intro: row.get(8)?,
        skip_outro: row.get(9)?,
        path: row.get(10)?,
        hash: row.get(11)?,
        tags: row.get(12)?,
        genre: row.get(13)?,
        year: row.get(14)?,
        created_at: row.get(15)?,
        manual_corrected: row.get(16).unwrap_or(0),
        match_pattern: row.get(17).unwrap_or(None),
        chapter_regex: row.get(18).unwrap_or(None),
    })
}

fn insert_book(conn: &rusqlite::Connection, book: &Book) -> Result<()> {
    conn.execute(
        "INSERT INTO books (id, library_id, title, author, narrator, cover_url,
         theme_color, description, skip_intro, skip_outro, path, hash, tags, genre, year, manual_corrected, match_pattern, chapter_regex)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        rusqlite::params![
            book.id, book.library_id, book.title, book.author, book.narrator, book.cover_url,
            book.theme_color, book.description, book.skip_intro, book.skip_outro, book.path,
            book.hash, book.tags, book.genre, book.year, book.manual_corrected, book.match_pattern, book.chapter_regex,
        ],
    )?;
    Ok(())
}

/// Repository for Book entities
pub struct BookRepository {
    db: Arc<DatabaseManager>,
}

impl BookRepository {
    /// Refresh metadata and the currently returned chapters for a source book.
    ///
    /// Source chapter IDs are stable, so upserting keeps existing playback
    /// progress while allowing titles and durations to change upstream.
    pub async fn update_source_book(
        &self,
        book_id: &str,
        detail: ting_plugin_contract::scraper::SourceBookDetail,
        theme_color: Option<String>,
    ) -> Result<()> {
        let book_id = book_id.to_owned();
        let metadata = detail.metadata;
        let chapters = detail.chapters;
        self.db
            .transaction(move |tx| {
                tx.execute(
                    "UPDATE books
                     SET title = ?, author = ?, narrator = ?, cover_url = ?,
                         theme_color = COALESCE(?, theme_color), description = ?,
                         tags = ?, genre = ?, year = ?
                     WHERE id = ?",
                    rusqlite::params![
                        metadata.title,
                        metadata.author,
                        metadata.narrator,
                        metadata.cover_url,
                        theme_color,
                        metadata.intro,
                        metadata.tags.join(","),
                        metadata.genre,
                        metadata.published_year.map(i32::from),
                        &book_id,
                    ],
                )?;

                for chapter in chapters {
                    let id = crate::core::books::sources::source_record_id(&[
                        "chapter",
                        &book_id,
                        &chapter.id,
                    ]);
                    tx.execute(
                        "INSERT INTO chapters
                         (id, book_id, title, path, duration, chapter_index, is_extra, hash,
                          manual_corrected, start_time, end_time)
                         VALUES (?, ?, ?, ?, ?, ?, 0, NULL, 0, NULL, NULL)
                         ON CONFLICT(id) DO UPDATE SET
                           title = excluded.title,
                           path = excluded.path,
                           duration = excluded.duration,
                           chapter_index = excluded.chapter_index",
                        rusqlite::params![
                            id,
                            &book_id,
                            chapter.title,
                            chapter.id,
                            chapter.duration.map(|duration| duration as i32),
                            chapter.index as i32,
                        ],
                    )?;
                }
                Ok(())
            })
            .await
    }

    pub async fn source_chapter_count(&self, book_id: &str) -> Result<usize> {
        let book_id = book_id.to_owned();
        self.db
            .execute(move |conn| {
                let count: i64 = conn.query_row(
                    "SELECT COUNT(*) FROM chapters WHERE book_id = ?",
                    [&book_id],
                    |row| row.get(0),
                )?;
                Ok(count.max(0) as usize)
            })
            .await
    }

    /// Persist one lazily fetched page for a source-backed book.
    /// Chapter IDs are derived from the stable source chapter reference so
    /// retries and concurrent detail requests remain idempotent.
    pub async fn append_source_chapters(
        &self,
        book_id: &str,
        chapters: &[ting_plugin_contract::scraper::ChapterItem],
    ) -> Result<()> {
        let book_id = book_id.to_owned();
        let chapters = chapters.to_owned();
        self.db
            .transaction(move |tx| {
                for chapter in chapters {
                    let id = crate::core::books::sources::source_record_id(&[
                        "chapter",
                        &book_id,
                        &chapter.id,
                    ]);
                    tx.execute(
                        "INSERT OR IGNORE INTO chapters
                         (id, book_id, title, path, duration, chapter_index, is_extra, hash,
                          manual_corrected, start_time, end_time)
                         VALUES (?, ?, ?, ?, ?, ?, 0, NULL, 0, NULL, NULL)",
                        rusqlite::params![
                            id,
                            book_id,
                            chapter.title,
                            chapter.id,
                            chapter.duration.map(|duration| duration as i32),
                            chapter.index as i32,
                        ],
                    )?;
                }
                Ok(())
            })
            .await
    }

    /// All source records and the caller's access grant commit together.
    /// Deterministic IDs plus SQLite's write transaction make concurrent adds
    /// idempotent, including adds by different users and plugin upgrades.
    pub async fn import_source_book(
        &self,
        plugin_id: &str,
        source_name: &str,
        user_id: &str,
        theme_color: Option<String>,
        detail: ting_plugin_contract::scraper::SourceBookDetail,
    ) -> Result<crate::core::books::sources::AddedSourceBook> {
        use crate::core::app::error::InputErrorKind;
        use crate::core::books::sources::{AddedSourceBook, BookSourceBinding, source_record_id};
        detail.validate().map_err(|message| {
            TingError::validation(
                InputErrorKind::InvalidValue,
                "book_source.import",
                message.into(),
            )
        })?;
        let library_id = source_record_id(&["library", plugin_id]);
        let book_id = source_record_id(&[
            "book",
            plugin_id,
            detail.metadata.id.as_deref().unwrap_or_default(),
        ]);
        let binding = serde_json::json!(BookSourceBinding {
            plugin_id: plugin_id.into()
        })
        .to_string();
        let source_name = source_name.to_owned();
        let user_id = user_id.to_owned();
        self.db
            .transaction(move |tx| {
                tx.execute(
                "INSERT OR IGNORE INTO libraries (id, name, type, url, root_path, scraper_config)
                 VALUES (?, ?, 'source', '', '', ?)",
                rusqlite::params![library_id, source_name, binding],
            )?;
                let existing: bool = tx.query_row(
                    "SELECT EXISTS(SELECT 1 FROM books WHERE id = ?)",
                    [&book_id],
                    |row| row.get(0),
                )?;
                if !existing {
                    let metadata = detail.metadata;
                    insert_book(
                        tx,
                        &Book {
                            id: book_id.clone(),
                            library_id: library_id.clone(),
                            title: Some(metadata.title),
                            author: metadata.author,
                            narrator: metadata.narrator,
                            cover_url: metadata.cover_url,
                            theme_color: theme_color.clone(),
                            description: metadata.intro,
                            path: metadata.id.unwrap_or_default(),
                            hash: book_id.clone(),
                            tags: Some(metadata.tags.join(",")),
                            genre: metadata.genre,
                            year: metadata.published_year.map(i32::from),
                            ..Default::default()
                        },
                    )?;
                    for chapter in detail.chapters {
                        let id = source_record_id(&["chapter", &book_id, &chapter.id]);
                        super::chapter::insert_chapter(
                            tx,
                            &crate::db::models::Chapter {
                                id,
                                book_id: book_id.clone(),
                                title: Some(chapter.title),
                                path: chapter.id,
                                duration: chapter.duration.map(|duration| duration as i32),
                                chapter_index: Some(chapter.index as i32),
                                is_extra: 0,
                                hash: None,
                                manual_corrected: 0,
                                start_time: None,
                                end_time: None,
                                created_at: String::new(),
                            },
                        )?;
                    }
                } else if let Some(theme_color) = theme_color.as_deref() {
                    tx.execute(
                        "UPDATE books SET theme_color = ?
                         WHERE id = ? AND (theme_color IS NULL OR theme_color = '')",
                        rusqlite::params![theme_color, book_id],
                    )?;
                }
                tx.execute(
                    "INSERT OR IGNORE INTO user_book_access (user_id, book_id) VALUES (?, ?)",
                    rusqlite::params![user_id, book_id],
                )?;
                Ok(AddedSourceBook {
                    book_id,
                    already_added: existing,
                })
            })
            .await
    }

    pub async fn source_books_for_user(
        &self,
        plugin_id: &str,
        user_id: &str,
        is_admin: bool,
    ) -> Result<std::collections::HashMap<String, String>> {
        let library_id = crate::core::books::sources::source_record_id(&["library", plugin_id]);
        let user_id = user_id.to_owned();
        self.db.execute(move |conn| {
            let mut statement = conn.prepare(
                "SELECT b.path, b.id FROM books b WHERE b.library_id = ? AND
                 (? OR EXISTS (SELECT 1 FROM user_book_access a WHERE a.book_id = b.id AND a.user_id = ?)
                    OR EXISTS (SELECT 1 FROM user_library_access a WHERE a.library_id = b.library_id AND a.user_id = ?))"
            )?;
            let books = statement.query_map(rusqlite::params![library_id, is_admin, user_id, user_id], |row| {
                Ok((row.get(0)?, row.get(1)?))
            })?.collect::<rusqlite::Result<_>>()?;
            Ok(books)
        }).await
    }

    /// Create a new BookRepository
    pub fn new(db: Arc<DatabaseManager>) -> Self {
        Self { db }
    }

    /// Get a reference to the database manager
    pub fn db(&self) -> &Arc<DatabaseManager> {
        &self.db
    }

    /// Find books by library ID
    pub async fn find_by_library(&self, library_id: &str) -> Result<Vec<Book>> {
        let library_id = library_id.to_string();
        self.db
            .execute(move |conn| {
                let mut stmt = conn
                    .prepare(
                        "SELECT id, library_id, title, author, narrator, cover_url, theme_color, \
                 description, skip_intro, skip_outro, path, hash, tags, genre, year, created_at, \
                 manual_corrected, match_pattern, chapter_regex \
                 FROM books WHERE library_id = ? ORDER BY created_at DESC",
                    )
                    .map_err(TingError::DatabaseError)?;

                let books = stmt
                    .query_map([&library_id], map_book_row)
                    .map_err(TingError::DatabaseError)?
                    .collect::<std::result::Result<Vec<_>, _>>()
                    .map_err(TingError::DatabaseError)?;

                Ok(books)
            })
            .await
    }

    /// Load only the title groups that can be affected by the supplied books.
    /// This keeps post-scan auto-merge work proportional to the changed title
    /// neighbourhood instead of regrouping every book in the library.
    pub async fn find_merge_candidates_for_books(
        &self,
        library_id: &str,
        book_ids: &[String],
    ) -> Result<Vec<Book>> {
        if book_ids.is_empty() {
            return Ok(Vec::new());
        }

        let library_id = library_id.to_string();
        let book_ids = book_ids.to_vec();
        self.db
            .execute(move |conn| {
                let placeholders = std::iter::repeat_n("?", book_ids.len())
                    .collect::<Vec<_>>()
                    .join(",");
                let query = format!(
                    "SELECT id, library_id, title, author, narrator, cover_url, theme_color,
                            description, skip_intro, skip_outro, path, hash, tags, genre, year, created_at,
                            manual_corrected, match_pattern, chapter_regex
                       FROM books
                      WHERE library_id = ?
                        AND title IN (
                            SELECT DISTINCT title
                              FROM books
                             WHERE library_id = ? AND id IN ({})
                        )
                      ORDER BY created_at DESC",
                    placeholders
                );
                let params = std::iter::once(library_id.as_str())
                    .chain(std::iter::once(library_id.as_str()))
                    .chain(book_ids.iter().map(String::as_str));
                let mut stmt = conn.prepare(&query).map_err(TingError::DatabaseError)?;
                let books = stmt
                    .query_map(rusqlite::params_from_iter(params), map_book_row)
                    .map_err(TingError::DatabaseError)?
                    .collect::<std::result::Result<Vec<_>, _>>()
                    .map_err(TingError::DatabaseError)?;
                Ok(books)
            })
            .await
    }

    /// Find books by library ID with minimal fields (id, path, hash, manual_corrected, match_pattern)
    pub async fn find_all_minimal_by_library(
        &self,
        library_id: &str,
    ) -> Result<Vec<(String, String, String, i32, Option<String>)>> {
        let library_id = library_id.to_string();
        self.db
            .execute(move |conn| {
                let mut stmt = conn
                    .prepare(
                        "SELECT id, path, hash, manual_corrected, match_pattern \
                 FROM books WHERE library_id = ?",
                    )
                    .map_err(TingError::DatabaseError)?;

                let books = stmt
                    .query_map([&library_id], |row| {
                        Ok((
                            row.get(0)?,
                            row.get(1)?,
                            row.get(2)?,
                            row.get(3).unwrap_or(0),
                            row.get(4).unwrap_or(None),
                        ))
                    })
                    .map_err(TingError::DatabaseError)?
                    .collect::<std::result::Result<Vec<_>, _>>()
                    .map_err(TingError::DatabaseError)?;

                Ok(books)
            })
            .await
    }

    /// Find a book by its hash
    pub async fn find_by_hash(&self, hash: &str) -> Result<Option<Book>> {
        let hash = hash.to_string();
        self.db
            .execute(move |conn| {
                conn.query_row(
                    "SELECT id, library_id, title, author, narrator, cover_url, theme_color, \
                 description, skip_intro, skip_outro, path, hash, tags, genre, year, created_at, \
                 manual_corrected, match_pattern, chapter_regex \
                 FROM books WHERE hash = ?",
                    [&hash],
                    map_book_row,
                )
                .optional()
                .map_err(TingError::DatabaseError)
            })
            .await
    }

    /// Get collection stats
    pub async fn get_stats(&self) -> Result<(usize, usize, i64)> {
        self.db
            .execute(|conn| {
                let total_books: usize = conn
                    .query_row("SELECT COUNT(*) FROM books", [], |row| row.get(0))
                    .map_err(TingError::DatabaseError)?;

                let total_chapters: usize = conn
                    .query_row("SELECT COUNT(*) FROM chapters", [], |row| row.get(0))
                    .map_err(TingError::DatabaseError)?;

                let total_duration: Option<i64> = conn
                    .query_row("SELECT SUM(duration) FROM chapters", [], |row| row.get(0))
                    .map_err(TingError::DatabaseError)?;

                Ok((total_books, total_chapters, total_duration.unwrap_or(0)))
            })
            .await
    }

    /// Find a book by title and author
    pub async fn find_by_title_and_author(
        &self,
        title: &str,
        author: &str,
    ) -> Result<Option<Book>> {
        let title = title.to_string();
        let author = author.to_string();
        self.db
            .execute(move |conn| {
                conn.query_row(
                    "SELECT id, library_id, title, author, narrator, cover_url, theme_color, \
                 description, skip_intro, skip_outro, path, hash, tags, genre, year, created_at, \
                 manual_corrected, match_pattern, chapter_regex \
                 FROM books WHERE title = ? AND author = ?",
                    [&title, &author],
                    map_book_row,
                )
                .optional()
                .map_err(TingError::DatabaseError)
            })
            .await
    }

    /// Find books with filters and access control
    pub async fn find_with_filters(
        &self,
        user_id: &str,
        is_admin: bool,
        search: Option<String>,
        tag: Option<String>,
        library_id: Option<String>,
    ) -> Result<Vec<Book>> {
        let user_id = user_id.to_string();
        self.db.execute(move |conn| {
            let mut query = "SELECT b.id, b.library_id, b.title, b.author, b.narrator, b.cover_url, b.theme_color, \
                             b.description, b.skip_intro, b.skip_outro, b.path, b.hash, b.tags, b.genre, b.year, b.created_at, \
                             b.manual_corrected, b.match_pattern, b.chapter_regex \
                             FROM books b".to_string();
            // We store params as String to make them easy to handle and Send
            let mut params: Vec<String> = Vec::new();
            let mut conditions: Vec<String> = Vec::new();

            // Filters
            if let Some(s) = search {
                conditions.push("(b.title LIKE ? OR b.author LIKE ? OR b.description LIKE ? OR b.narrator LIKE ?)".to_string());
                let pattern = format!("%{}%", s);
                params.push(pattern.clone());
                params.push(pattern.clone());
                params.push(pattern.clone());
                params.push(pattern.clone());
            }

            if let Some(t) = tag {
                conditions.push("b.tags LIKE ?".to_string());
                params.push(format!("%{}%", t));
            }

            if let Some(lid) = library_id {
                conditions.push("b.library_id = ?".to_string());
                params.push(lid);
            }

            // Access Control
            if !is_admin {
                conditions.push("(
                    b.library_id IN (SELECT library_id FROM user_library_access WHERE user_id = ?)
                    OR
                    b.id IN (SELECT book_id FROM user_book_access WHERE user_id = ?)
                )".to_string());
                params.push(user_id.clone());
                params.push(user_id.clone());
            }

            if !conditions.is_empty() {
                query += " WHERE ";
                query += &conditions.join(" AND ");
            }

            query += " ORDER BY b.created_at DESC";

            let mut stmt = conn.prepare(&query).map_err(TingError::DatabaseError)?;

            let books = stmt.query_map(rusqlite::params_from_iter(params.iter()), map_book_row)
            .map_err(TingError::DatabaseError)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(TingError::DatabaseError)?;

            Ok(books)
        }).await
    }

    pub async fn check_access(&self, book_id: &str, user_id: &str, is_admin: bool) -> Result<bool> {
        if is_admin {
            return Ok(true);
        }

        let book_id = book_id.to_string();
        let user_id = user_id.to_string();
        self.db
            .execute(move |conn| {
                conn.query_row(
                    "SELECT EXISTS(
                        SELECT 1 FROM books b
                        WHERE b.id = ? AND (
                            b.library_id IN (SELECT library_id FROM user_library_access WHERE user_id = ?)
                            OR
                            b.id IN (SELECT book_id FROM user_book_access WHERE user_id = ?)
                        )
                    )",
                    rusqlite::params![&book_id, &user_id, &user_id],
                    |row| row.get(0),
                )
                .map_err(TingError::DatabaseError)
            })
            .await
    }

    pub async fn cleanup_orphans(&self) -> Result<()> {
        self.db
            .execute(move |conn| {
                conn.execute(
                    "DELETE FROM books WHERE library_id NOT IN (SELECT id FROM libraries)",
                    [],
                )
                .map_err(TingError::DatabaseError)?;
                Ok(())
            })
            .await
    }
}

#[async_trait]
impl Repository<Book> for BookRepository {
    async fn find_by_id(&self, id: &str) -> Result<Option<Book>> {
        let id = id.to_string();
        self.db
            .execute(move |conn| {
                conn.query_row(
                    "SELECT id, library_id, title, author, narrator, cover_url, theme_color, \
                 description, skip_intro, skip_outro, path, hash, tags, genre, year, created_at, \
                 manual_corrected, match_pattern, chapter_regex \
                 FROM books WHERE id = ?",
                    [&id],
                    map_book_row,
                )
                .optional()
                .map_err(TingError::DatabaseError)
            })
            .await
    }

    async fn find_all(&self) -> Result<Vec<Book>> {
        self.db
            .execute(|conn| {
                let mut stmt = conn
                    .prepare(
                        "SELECT id, library_id, title, author, narrator, cover_url, theme_color, \
                 description, skip_intro, skip_outro, path, hash, tags, genre, year, created_at, \
                 manual_corrected, match_pattern, chapter_regex \
                 FROM books ORDER BY created_at DESC",
                    )
                    .map_err(TingError::DatabaseError)?;

                let books = stmt
                    .query_map([], map_book_row)
                    .map_err(TingError::DatabaseError)?
                    .collect::<std::result::Result<Vec<_>, _>>()
                    .map_err(TingError::DatabaseError)?;

                Ok(books)
            })
            .await
    }

    async fn create(&self, book: &Book) -> Result<()> {
        let book = book.clone();
        self.db.execute(move |conn| insert_book(conn, &book)).await
    }

    async fn update(&self, book: &Book) -> Result<()> {
        let book = book.clone();
        self.db.execute(move |conn| {
            conn.execute(
                "UPDATE books SET library_id = ?, title = ?, author = ?, narrator = ?, \
                 cover_url = ?, theme_color = ?, description = ?, skip_intro = ?, \
                 skip_outro = ?, path = ?, hash = ?, tags = ?, genre = ?, year = ?, manual_corrected = ?, match_pattern = ?, chapter_regex = ? WHERE id = ?",
                rusqlite::params![
                    &book.library_id,
                    &book.title,
                    &book.author,
                    &book.narrator,
                    &book.cover_url,
                    &book.theme_color,
                    &book.description,
                    book.skip_intro,
                    book.skip_outro,
                    &book.path,
                    &book.hash,
                    &book.tags,
                    &book.genre,
                    &book.year,
                    book.manual_corrected,
                    &book.match_pattern,
                    &book.chapter_regex,
                    &book.id,
                ],
            ).map_err(TingError::DatabaseError)?;
            Ok(())
        }).await
    }

    async fn delete(&self, id: &str) -> Result<()> {
        let id = id.to_string();
        self.db
            .execute(move |conn| {
                conn.execute("DELETE FROM books WHERE id = ?", [&id])
                    .map_err(TingError::DatabaseError)?;
                Ok(())
            })
            .await
    }
}
