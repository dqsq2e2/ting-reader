use crate::core::error::{Result, TingError};
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

/// Repository for Book entities
pub struct BookRepository {
    db: Arc<DatabaseManager>,
}

impl BookRepository {
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
        self.db.execute(move |conn| {
            conn.execute(
                "INSERT INTO books (id, library_id, title, author, narrator, cover_url, \
                 theme_color, description, skip_intro, skip_outro, path, hash, tags, genre, year, manual_corrected, match_pattern, chapter_regex) \
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                rusqlite::params![
                    &book.id,
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
                ],
            ).map_err(TingError::DatabaseError)?;
            Ok(())
        }).await
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
