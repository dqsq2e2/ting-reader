//! Repository pattern implementation for data access layer
//!
//! This module provides the Repository pattern for abstracting database operations.

use crate::core::error::{Result, TingError};
use crate::db::manager::DatabaseManager;
use crate::db::models::{Book, Chapter, PluginRecord, TaskRecord, MergeSuggestion};
use async_trait::async_trait;
use rusqlite::OptionalExtension;
use std::sync::Arc;

/// Generic repository trait for CRUD operations
#[async_trait]
pub trait Repository<T>: Send + Sync {
    /// Find an entity by its ID
    async fn find_by_id(&self, id: &str) -> Result<Option<T>>;
    
    /// Find all entities
    async fn find_all(&self) -> Result<Vec<T>>;
    
    /// Create a new entity
    async fn create(&self, entity: &T) -> Result<()>;
    
    /// Update an existing entity
    async fn update(&self, entity: &T) -> Result<()>;
    
    /// Delete an entity by its ID
    async fn delete(&self, id: &str) -> Result<()>;
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
        self.db.execute(move |conn| {
            let mut stmt = conn.prepare(
                "SELECT id, library_id, title, author, narrator, cover_url, theme_color, \
                 description, skip_intro, skip_outro, path, hash, tags, created_at, \
                 manual_corrected, match_pattern, chapter_regex \
                 FROM books WHERE library_id = ? ORDER BY created_at DESC"
            ).map_err(TingError::DatabaseError)?;
            
            let books = stmt.query_map([&library_id], |row| {
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
                    created_at: row.get(13)?,
                    manual_corrected: row.get(14).unwrap_or(0),
                    match_pattern: row.get(15).unwrap_or(None),
                    chapter_regex: row.get(16).unwrap_or(None),
                })
            }).map_err(TingError::DatabaseError)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(TingError::DatabaseError)?;
            
            Ok(books)
        }).await
    }
    
    /// Find a book by its hash
    pub async fn find_by_hash(&self, hash: &str) -> Result<Option<Book>> {
        let hash = hash.to_string();
        self.db.execute(move |conn| {
            conn.query_row(
                "SELECT id, library_id, title, author, narrator, cover_url, theme_color, \
                 description, skip_intro, skip_outro, path, hash, tags, created_at, \
                 manual_corrected, match_pattern, chapter_regex \
                 FROM books WHERE hash = ?",
                [&hash],
                |row| {
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
                        created_at: row.get(13)?,
                        manual_corrected: row.get(14).unwrap_or(0),
                        match_pattern: row.get(15).unwrap_or(None),
                        chapter_regex: row.get(16).unwrap_or(None),
                    })
                }
            ).optional()
            .map_err(TingError::DatabaseError)
        }).await
    }

    /// Get collection stats
    pub async fn get_stats(&self) -> Result<(usize, usize, i64)> {
        self.db.execute(|conn| {
            let total_books: usize = conn.query_row(
                "SELECT COUNT(*) FROM books",
                [],
                |row| row.get(0),
            ).map_err(TingError::DatabaseError)?;

            let total_chapters: usize = conn.query_row(
                "SELECT COUNT(*) FROM chapters",
                [],
                |row| row.get(0),
            ).map_err(TingError::DatabaseError)?;
            
            let total_duration: Option<i64> = conn.query_row(
                "SELECT SUM(duration) FROM chapters",
                [],
                |row| row.get(0),
            ).map_err(TingError::DatabaseError)?;

            Ok((total_books, total_chapters, total_duration.unwrap_or(0)))
        }).await
    }

    /// Find a book by title and author
    pub async fn find_by_title_and_author(&self, title: &str, author: &str) -> Result<Option<Book>> {
        let title = title.to_string();
        let author = author.to_string();
        self.db.execute(move |conn| {
            conn.query_row(
                "SELECT id, library_id, title, author, narrator, cover_url, theme_color, \
                 description, skip_intro, skip_outro, path, hash, tags, created_at, \
                 manual_corrected, match_pattern, chapter_regex \
                 FROM books WHERE title = ? AND author = ?",
                [&title, &author],
                |row| {
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
                        created_at: row.get(13)?,
                        manual_corrected: row.get(14).unwrap_or(0),
                        match_pattern: row.get(15).unwrap_or(None),
                        chapter_regex: row.get(16).unwrap_or(None),
                    })
                }
            ).optional()
            .map_err(TingError::DatabaseError)
        }).await
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
                             b.description, b.skip_intro, b.skip_outro, b.path, b.hash, b.tags, b.created_at, \
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
            
            let books = stmt.query_map(rusqlite::params_from_iter(params.iter()), |row| {
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
                    created_at: row.get(13)?,
                    manual_corrected: row.get(14).unwrap_or(0),
                    match_pattern: row.get(15).unwrap_or(None),
                    chapter_regex: row.get(16).unwrap_or(None),
                })
            }).map_err(TingError::DatabaseError)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(TingError::DatabaseError)?;
            
            Ok(books)
        }).await
    }
    pub async fn cleanup_orphans(&self) -> Result<()> {
        self.db.execute(move |conn| {
            conn.execute(
                "DELETE FROM books WHERE library_id NOT IN (SELECT id FROM libraries)",
                [],
            ).map_err(TingError::DatabaseError)?;
            Ok(())
        }).await
    }
}


#[async_trait]
impl Repository<Book> for BookRepository {
    async fn find_by_id(&self, id: &str) -> Result<Option<Book>> {
        let id = id.to_string();
        self.db.execute(move |conn| {
            conn.query_row(
                "SELECT id, library_id, title, author, narrator, cover_url, theme_color, \
                 description, skip_intro, skip_outro, path, hash, tags, created_at, \
                 manual_corrected, match_pattern, chapter_regex \
                 FROM books WHERE id = ?",
                [&id],
                |row| {
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
                        created_at: row.get(13)?,
                        manual_corrected: row.get(14).unwrap_or(0),
                        match_pattern: row.get(15).unwrap_or(None),
                        chapter_regex: row.get(16).unwrap_or(None),
                    })
                }
            ).optional()
            .map_err(TingError::DatabaseError)
        }).await
    }
    
    async fn find_all(&self) -> Result<Vec<Book>> {
        self.db.execute(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, library_id, title, author, narrator, cover_url, theme_color, \
                 description, skip_intro, skip_outro, path, hash, tags, created_at, \
                 manual_corrected, match_pattern, chapter_regex \
                 FROM books ORDER BY created_at DESC"
            ).map_err(TingError::DatabaseError)?;
            
            let books = stmt.query_map([], |row| {
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
                    created_at: row.get(13)?,
                    manual_corrected: row.get(14).unwrap_or(0),
                    match_pattern: row.get(15).unwrap_or(None),
                    chapter_regex: row.get(16).unwrap_or(None),
                })
            }).map_err(TingError::DatabaseError)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(TingError::DatabaseError)?;
            
            Ok(books)
        }).await
    }
    
    async fn create(&self, book: &Book) -> Result<()> {
        let book = book.clone();
        self.db.execute(move |conn| {
            conn.execute(
                "INSERT INTO books (id, library_id, title, author, narrator, cover_url, \
                 theme_color, description, skip_intro, skip_outro, path, hash, tags, manual_corrected, match_pattern, chapter_regex) \
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
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
                 skip_outro = ?, path = ?, hash = ?, tags = ?, manual_corrected = ?, match_pattern = ?, chapter_regex = ? WHERE id = ?",
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
        self.db.execute(move |conn| {
            conn.execute("DELETE FROM books WHERE id = ?", [&id])
                .map_err(TingError::DatabaseError)?;
            Ok(())
        }).await
    }
}

/// Repository for MergeSuggestion entities
pub struct MergeSuggestionRepository {
    db: Arc<DatabaseManager>,
}

impl MergeSuggestionRepository {
    /// Create a new MergeSuggestionRepository
    pub fn new(db: Arc<DatabaseManager>) -> Self {
        Self { db }
    }
    
    /// Find suggestions by status
    pub async fn find_by_status(&self, status: &str) -> Result<Vec<MergeSuggestion>> {
        let status = status.to_string();
        self.db.execute(move |conn| {
            let mut stmt = conn.prepare(
                "SELECT id, book_a_id, book_b_id, score, reason, status, created_at \
                 FROM merge_suggestions WHERE status = ? ORDER BY score DESC"
            ).map_err(TingError::DatabaseError)?;
            
            let suggestions = stmt.query_map([&status], |row| {
                Ok(MergeSuggestion {
                    id: row.get(0)?,
                    book_a_id: row.get(1)?,
                    book_b_id: row.get(2)?,
                    score: row.get(3)?,
                    reason: row.get(4)?,
                    status: row.get(5)?,
                    created_at: row.get(6)?,
                })
            }).map_err(TingError::DatabaseError)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(TingError::DatabaseError)?;
            
            Ok(suggestions)
        }).await
    }
    
    /// Check if suggestion exists
    pub async fn exists(&self, book_a_id: &str, book_b_id: &str) -> Result<bool> {
        let book_a_id = book_a_id.to_string();
        let book_b_id = book_b_id.to_string();
        self.db.execute(move |conn| {
            // Check both directions (a,b) or (b,a)
            let count: i64 = conn.query_row(
                "SELECT COUNT(*) FROM merge_suggestions WHERE \
                 (book_a_id = ? AND book_b_id = ?) OR (book_a_id = ? AND book_b_id = ?)",
                rusqlite::params![&book_a_id, &book_b_id, &book_b_id, &book_a_id],
                |row| row.get(0)
            ).map_err(TingError::DatabaseError)?;
            Ok(count > 0)
        }).await
    }
}

#[async_trait]
impl Repository<MergeSuggestion> for MergeSuggestionRepository {
    async fn find_by_id(&self, id: &str) -> Result<Option<MergeSuggestion>> {
        let id = id.to_string();
        self.db.execute(move |conn| {
            conn.query_row(
                "SELECT id, book_a_id, book_b_id, score, reason, status, created_at \
                 FROM merge_suggestions WHERE id = ?",
                [&id],
                |row| {
                    Ok(MergeSuggestion {
                        id: row.get(0)?,
                        book_a_id: row.get(1)?,
                        book_b_id: row.get(2)?,
                        score: row.get(3)?,
                        reason: row.get(4)?,
                        status: row.get(5)?,
                        created_at: row.get(6)?,
                    })
                }
            ).optional()
            .map_err(TingError::DatabaseError)
        }).await
    }
    
    async fn find_all(&self) -> Result<Vec<MergeSuggestion>> {
        self.db.execute(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, book_a_id, book_b_id, score, reason, status, created_at \
                 FROM merge_suggestions ORDER BY created_at DESC"
            ).map_err(TingError::DatabaseError)?;
            
            let suggestions = stmt.query_map([], |row| {
                Ok(MergeSuggestion {
                    id: row.get(0)?,
                    book_a_id: row.get(1)?,
                    book_b_id: row.get(2)?,
                    score: row.get(3)?,
                    reason: row.get(4)?,
                    status: row.get(5)?,
                    created_at: row.get(6)?,
                })
            }).map_err(TingError::DatabaseError)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(TingError::DatabaseError)?;
            
            Ok(suggestions)
        }).await
    }
    
    async fn create(&self, suggestion: &MergeSuggestion) -> Result<()> {
        let suggestion = suggestion.clone();
        self.db.execute(move |conn| {
            conn.execute(
                "INSERT INTO merge_suggestions (id, book_a_id, book_b_id, score, reason, status, created_at) \
                 VALUES (?, ?, ?, ?, ?, ?, ?)",
                rusqlite::params![
                    &suggestion.id,
                    &suggestion.book_a_id,
                    &suggestion.book_b_id,
                    suggestion.score,
                    &suggestion.reason,
                    &suggestion.status,
                    &suggestion.created_at,
                ],
            ).map_err(TingError::DatabaseError)?;
            Ok(())
        }).await
    }
    
    async fn update(&self, suggestion: &MergeSuggestion) -> Result<()> {
        let suggestion = suggestion.clone();
        self.db.execute(move |conn| {
            conn.execute(
                "UPDATE merge_suggestions SET book_a_id = ?, book_b_id = ?, score = ?, \
                 reason = ?, status = ? WHERE id = ?",
                rusqlite::params![
                    &suggestion.book_a_id,
                    &suggestion.book_b_id,
                    suggestion.score,
                    &suggestion.reason,
                    &suggestion.status,
                    &suggestion.id,
                ],
            ).map_err(TingError::DatabaseError)?;
            Ok(())
        }).await
    }
    
    async fn delete(&self, id: &str) -> Result<()> {
        let id = id.to_string();
        self.db.execute(move |conn| {
            conn.execute("DELETE FROM merge_suggestions WHERE id = ?", [&id])
                .map_err(TingError::DatabaseError)?;
            Ok(())
        }).await
    }
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
                 FROM chapters WHERE book_id = ? ORDER BY chapter_index ASC"
            ).map_err(TingError::DatabaseError)?;
            
            let chapters = stmt.query_map([&book_id], |row| {
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
            }).map_err(TingError::DatabaseError)?
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
                |row| {
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
            ).optional()
            .map_err(TingError::DatabaseError)
        }).await
    }

    /// Find chapters by book ID with user progress
    pub async fn find_by_book_with_progress(
        &self, 
        book_id: &str, 
        user_id: &str
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
                |row| {
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
            
            let chapters = stmt.query_map([], |row| {
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
            }).map_err(TingError::DatabaseError)?
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
        self.db.execute(move |conn| {
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
            ).map_err(TingError::DatabaseError)?;
            Ok(())
        }).await
    }

    
    async fn delete(&self, id: &str) -> Result<()> {
        let id = id.to_string();
        self.db.execute(move |conn| {
            conn.execute("DELETE FROM chapters WHERE id = ?", [&id])
                .map_err(TingError::DatabaseError)?;
            Ok(())
        }).await
    }
}

/// Repository for PluginRecord entities
pub struct PluginRepository {
    db: Arc<DatabaseManager>,
}

impl PluginRepository {
    /// Create a new PluginRepository
    pub fn new(db: Arc<DatabaseManager>) -> Self {
        Self { db }
    }
    
    /// Find plugins by type
    pub async fn find_by_type(&self, plugin_type: &str) -> Result<Vec<PluginRecord>> {
        let plugin_type = plugin_type.to_string();
        self.db.execute(move |conn| {
            let mut stmt = conn.prepare(
                "SELECT id, name, version, plugin_type, description, author, enabled, config, \
                 created_at, updated_at FROM plugin_registry WHERE plugin_type = ? ORDER BY name"
            ).map_err(TingError::DatabaseError)?;
            
            let plugins = stmt.query_map([&plugin_type], |row| {
                Ok(PluginRecord {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    version: row.get(2)?,
                    plugin_type: row.get(3)?,
                    description: row.get(4)?,
                    author: row.get(5)?,
                    enabled: row.get(6)?,
                    config: row.get(7)?,
                    created_at: row.get(8)?,
                    updated_at: row.get(9)?,
                })
            }).map_err(TingError::DatabaseError)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(TingError::DatabaseError)?;
            
            Ok(plugins)
        }).await
    }
    
    /// Find enabled plugins
    pub async fn find_enabled(&self) -> Result<Vec<PluginRecord>> {
        self.db.execute(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, name, version, plugin_type, description, author, enabled, config, \
                 created_at, updated_at FROM plugin_registry WHERE enabled = 1 ORDER BY name"
            ).map_err(TingError::DatabaseError)?;
            
            let plugins = stmt.query_map([], |row| {
                Ok(PluginRecord {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    version: row.get(2)?,
                    plugin_type: row.get(3)?,
                    description: row.get(4)?,
                    author: row.get(5)?,
                    enabled: row.get(6)?,
                    config: row.get(7)?,
                    created_at: row.get(8)?,
                    updated_at: row.get(9)?,
                })
            }).map_err(TingError::DatabaseError)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(TingError::DatabaseError)?;
            
            Ok(plugins)
        }).await
    }
}

#[async_trait]
impl Repository<PluginRecord> for PluginRepository {
    async fn find_by_id(&self, id: &str) -> Result<Option<PluginRecord>> {
        let id = id.to_string();
        self.db.execute(move |conn| {
            conn.query_row(
                "SELECT id, name, version, plugin_type, description, author, enabled, config, \
                 created_at, updated_at FROM plugin_registry WHERE id = ?",
                [&id],
                |row| {
                    Ok(PluginRecord {
                        id: row.get(0)?,
                        name: row.get(1)?,
                        version: row.get(2)?,
                        plugin_type: row.get(3)?,
                        description: row.get(4)?,
                        author: row.get(5)?,
                        enabled: row.get(6)?,
                        config: row.get(7)?,
                        created_at: row.get(8)?,
                        updated_at: row.get(9)?,
                    })
                }
            ).optional()
            .map_err(TingError::DatabaseError)
        }).await
    }
    
    async fn find_all(&self) -> Result<Vec<PluginRecord>> {
        self.db.execute(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, name, version, plugin_type, description, author, enabled, config, \
                 created_at, updated_at FROM plugin_registry ORDER BY name"
            ).map_err(TingError::DatabaseError)?;
            
            let plugins = stmt.query_map([], |row| {
                Ok(PluginRecord {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    version: row.get(2)?,
                    plugin_type: row.get(3)?,
                    description: row.get(4)?,
                    author: row.get(5)?,
                    enabled: row.get(6)?,
                    config: row.get(7)?,
                    created_at: row.get(8)?,
                    updated_at: row.get(9)?,
                })
            }).map_err(TingError::DatabaseError)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(TingError::DatabaseError)?;
            
            Ok(plugins)
        }).await
    }
    
    async fn create(&self, plugin: &PluginRecord) -> Result<()> {
        let plugin = plugin.clone();
        self.db.execute(move |conn| {
            conn.execute(
                "INSERT INTO plugin_registry (id, name, version, plugin_type, description, \
                 author, enabled, config) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
                rusqlite::params![
                    &plugin.id,
                    &plugin.name,
                    &plugin.version,
                    &plugin.plugin_type,
                    &plugin.description,
                    &plugin.author,
                    plugin.enabled,
                    &plugin.config,
                ],
            ).map_err(TingError::DatabaseError)?;
            Ok(())
        }).await
    }
    
    async fn update(&self, plugin: &PluginRecord) -> Result<()> {
        let plugin = plugin.clone();
        self.db.execute(move |conn| {
            conn.execute(
                "UPDATE plugin_registry SET name = ?, version = ?, plugin_type = ?, \
                 description = ?, author = ?, enabled = ?, config = ?, updated_at = STRFTIME('%Y-%m-%dT%H:%M:%fZ', 'now') \
                 WHERE id = ?",
                rusqlite::params![
                    &plugin.name,
                    &plugin.version,
                    &plugin.plugin_type,
                    &plugin.description,
                    &plugin.author,
                    plugin.enabled,
                    &plugin.config,
                    &plugin.id,
                ],
            ).map_err(TingError::DatabaseError)?;
            Ok(())
        }).await
    }
    
    async fn delete(&self, id: &str) -> Result<()> {
        let id = id.to_string();
        self.db.execute(move |conn| {
            conn.execute("DELETE FROM plugin_registry WHERE id = ?", [&id])
                .map_err(TingError::DatabaseError)?;
            Ok(())
        }).await
    }
}

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
            let end_pos = s[dot_pos+1..].find(|c: char| !c.is_numeric()).map(|p| p + dot_pos + 1).unwrap_or(s.len());
            
            // If we have more than 3 digits of fractional seconds
            if end_pos - dot_pos > 4 { // .123 is 4 chars. if > 4, we have more than 3 digits
                // Keep only first 3 digits after dot (.123)
                 let tail = s[end_pos..].to_string(); // Save timezone part like +00:00 or Z
                 let head = &s[..dot_pos+4];
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
    pub async fn update_status(&self, id: &str, status: &str, error: Option<&str>, retries: i32) -> Result<()> {
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
        self.db.execute(move |conn| {
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
            ).map_err(TingError::DatabaseError)?;
            Ok(())
        }).await
    }

    async fn delete(&self, id: &str) -> Result<()> {
        let id = id.to_string();
        self.db.execute(move |conn| {
            conn.execute("DELETE FROM tasks WHERE id = ?", [&id])
                .map_err(TingError::DatabaseError)?;
            Ok(())
        }).await
    }
}

/// Repository for User entities
pub struct UserRepository {
    db: Arc<DatabaseManager>,
}

impl UserRepository {
    /// Create a new UserRepository
    pub fn new(db: Arc<DatabaseManager>) -> Self {
        Self { db }
    }
    
    /// Find a user by username
    pub async fn find_by_username(&self, username: &str) -> Result<Option<crate::db::models::User>> {
        let username = username.to_string();
        self.db.execute(move |conn| {
            conn.query_row(
                "SELECT id, username, password_hash, role, created_at FROM users WHERE username = ?",
                [&username],
                |row| {
                    Ok(crate::db::models::User {
                        id: row.get(0)?,
                        username: row.get(1)?,
                        password_hash: row.get(2)?,
                        role: row.get(3)?,
                        created_at: row.get(4)?,
                    })
                }
            ).optional()
            .map_err(TingError::DatabaseError)
        }).await
    }
    
    /// Count total users
    pub async fn count(&self) -> Result<i64> {
        self.db.execute(|conn| {
            conn.query_row("SELECT COUNT(*) FROM users", [], |row| row.get(0))
                .map_err(TingError::DatabaseError)
        }).await
    }
    
    /// Update user password
    pub async fn update_password(&self, user_id: &str, password_hash: &str) -> Result<()> {
        let user_id = user_id.to_string();
        let password_hash = password_hash.to_string();
        self.db.execute(move |conn| {
            conn.execute(
                "UPDATE users SET password_hash = ? WHERE id = ?",
                rusqlite::params![&password_hash, &user_id],
            ).map_err(TingError::DatabaseError)?;
            Ok(())
        }).await
    }

    /// Update user permissions (accessible libraries and books)
    pub async fn update_permissions(
        &self, 
        user_id: &str, 
        library_ids: Option<Vec<String>>, 
        book_ids: Option<Vec<String>>
    ) -> Result<()> {
        let user_id = user_id.to_string();
        let library_ids = library_ids.unwrap_or_default();
        let book_ids = book_ids.unwrap_or_default();
        
        self.db.transaction(move |tx| {
            // Update library access
            tx.execute(
                "DELETE FROM user_library_access WHERE user_id = ?",
                [&user_id],
            ).map_err(TingError::DatabaseError)?;
            
            for lib_id in library_ids {
                tx.execute(
                    "INSERT INTO user_library_access (user_id, library_id) VALUES (?, ?)",
                    [&user_id, &lib_id],
                ).map_err(TingError::DatabaseError)?;
            }
            
            // Update book access
            tx.execute(
                "DELETE FROM user_book_access WHERE user_id = ?",
                [&user_id],
            ).map_err(TingError::DatabaseError)?;
            
            for book_id in book_ids {
                tx.execute(
                    "INSERT INTO user_book_access (user_id, book_id) VALUES (?, ?)",
                    [&user_id, &book_id],
                ).map_err(TingError::DatabaseError)?;
            }
            
            Ok(())
        }).await
    }

    /// Get accessible library IDs for a user
    pub async fn get_accessible_libraries(&self, user_id: &str) -> Result<Vec<String>> {
        let user_id = user_id.to_string();
        self.db.execute(move |conn| {
            let mut stmt = conn.prepare(
                "SELECT library_id FROM user_library_access WHERE user_id = ?"
            ).map_err(TingError::DatabaseError)?;
            
            let ids = stmt.query_map([&user_id], |row| row.get(0))
                .map_err(TingError::DatabaseError)?
                .collect::<std::result::Result<Vec<String>, _>>()
                .map_err(TingError::DatabaseError)?;
                
            Ok(ids)
        }).await
    }

    /// Get accessible book IDs for a user
    pub async fn get_accessible_books(&self, user_id: &str) -> Result<Vec<String>> {
        let user_id = user_id.to_string();
        self.db.execute(move |conn| {
            let mut stmt = conn.prepare(
                "SELECT book_id FROM user_book_access WHERE user_id = ?"
            ).map_err(TingError::DatabaseError)?;
            
            let ids = stmt.query_map([&user_id], |row| row.get(0))
                .map_err(TingError::DatabaseError)?
                .collect::<std::result::Result<Vec<String>, _>>()
                .map_err(TingError::DatabaseError)?;
                
            Ok(ids)
        }).await
    }
}

#[async_trait]
impl Repository<crate::db::models::User> for UserRepository {
    async fn find_by_id(&self, id: &str) -> Result<Option<crate::db::models::User>> {
        let id = id.to_string();
        self.db.execute(move |conn| {
            conn.query_row(
                "SELECT id, username, password_hash, role, created_at FROM users WHERE id = ?",
                [&id],
                |row| {
                    Ok(crate::db::models::User {
                        id: row.get(0)?,
                        username: row.get(1)?,
                        password_hash: row.get(2)?,
                        role: row.get(3)?,
                        created_at: row.get(4)?,
                    })
                }
            ).optional()
            .map_err(TingError::DatabaseError)
        }).await
    }
    
    async fn find_all(&self) -> Result<Vec<crate::db::models::User>> {
        self.db.execute(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, username, password_hash, role, created_at FROM users ORDER BY created_at DESC"
            ).map_err(TingError::DatabaseError)?;
            
            let users = stmt.query_map([], |row| {
                Ok(crate::db::models::User {
                    id: row.get(0)?,
                    username: row.get(1)?,
                    password_hash: row.get(2)?,
                    role: row.get(3)?,
                    created_at: row.get(4)?,
                })
            }).map_err(TingError::DatabaseError)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(TingError::DatabaseError)?;
            
            Ok(users)
        }).await
    }
    
    async fn create(&self, user: &crate::db::models::User) -> Result<()> {
        let user = user.clone();
        self.db.execute(move |conn| {
            conn.execute(
                "INSERT INTO users (id, username, password_hash, role) VALUES (?, ?, ?, ?)",
                rusqlite::params![
                    &user.id,
                    &user.username,
                    &user.password_hash,
                    &user.role,
                ],
            ).map_err(TingError::DatabaseError)?;
            Ok(())
        }).await
    }
    
    async fn update(&self, user: &crate::db::models::User) -> Result<()> {
        let user = user.clone();
        self.db.execute(move |conn| {
            conn.execute(
                "UPDATE users SET username = ?, password_hash = ?, role = ? WHERE id = ?",
                rusqlite::params![
                    &user.username,
                    &user.password_hash,
                    &user.role,
                    &user.id,
                ],
            ).map_err(TingError::DatabaseError)?;
            Ok(())
        }).await
    }
    
    async fn delete(&self, id: &str) -> Result<()> {
        let id = id.to_string();
        self.db.execute(move |conn| {
            conn.execute("DELETE FROM users WHERE id = ?", [&id])
                .map_err(TingError::DatabaseError)?;
            Ok(())
        }).await
    }
}

/// Repository for Progress entities
pub struct ProgressRepository {
    db: Arc<DatabaseManager>,
}

impl ProgressRepository {
    /// Create a new ProgressRepository
    pub fn new(db: Arc<DatabaseManager>) -> Self {
        Self { db }
    }
    
    /// Get recent progress for a user (last 4 books)
    pub async fn get_recent(&self, user_id: &str, limit: i32) -> Result<Vec<crate::db::models::Progress>> {
        let user_id = user_id.to_string();
        self.db.execute(move |conn| {
            let mut stmt = conn.prepare(
                "SELECT id, user_id, book_id, chapter_id, position, duration, updated_at \
                 FROM progress WHERE user_id = ? ORDER BY updated_at DESC LIMIT ?"
            ).map_err(TingError::DatabaseError)?;
            
            let progress = stmt.query_map(rusqlite::params![&user_id, limit], |row| {
                Ok(crate::db::models::Progress {
                    id: row.get(0)?,
                    user_id: row.get(1)?,
                    book_id: row.get(2)?,
                    chapter_id: row.get(3)?,
                    position: row.get(4)?,
                    duration: row.get(5)?,
                    updated_at: row.get(6)?,
                })
            }).map_err(TingError::DatabaseError)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(TingError::DatabaseError)?;
            
            Ok(progress)
        }).await
    }

    /// Get recent progress enriched with book and chapter details
    pub async fn get_recent_enriched(
        &self, 
        user_id: &str, 
        limit: i32
    ) -> Result<Vec<(crate::db::models::Progress, Option<String>, Option<String>, Option<String>, Option<String>, Option<i32>)>> {
        let user_id = user_id.to_string();
        self.db.execute(move |conn| {
            let mut stmt = conn.prepare(
                "SELECT p.id, p.user_id, p.book_id, p.chapter_id, p.position, p.duration, p.updated_at, \
                 b.title as book_title, b.cover_url, b.library_id, c.title as chapter_title, c.duration as chapter_duration \
                 FROM progress p \
                 JOIN books b ON p.book_id = b.id \
                 LEFT JOIN chapters c ON p.chapter_id = c.id \
                 WHERE p.id IN ( \
                   SELECT id FROM progress \
                   WHERE user_id = ? \
                   GROUP BY book_id \
                   HAVING MAX(updated_at) \
                 ) \
                 ORDER BY p.updated_at DESC \
                 LIMIT ?"
            ).map_err(TingError::DatabaseError)?;
            
            let progress = stmt.query_map(rusqlite::params![&user_id, limit], |row| {
                let progress = crate::db::models::Progress {
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
    pub async fn get_by_book(&self, user_id: &str, book_id: &str) -> Result<Option<crate::db::models::Progress>> {
        let user_id = user_id.to_string();
        let book_id = book_id.to_string();
        self.db.execute(move |conn| {
            conn.query_row(
                "SELECT id, user_id, book_id, chapter_id, position, duration, updated_at \
                 FROM progress WHERE user_id = ? AND book_id = ?",
                rusqlite::params![&user_id, &book_id],
                |row| {
                    Ok(crate::db::models::Progress {
                        id: row.get(0)?,
                        user_id: row.get(1)?,
                        book_id: row.get(2)?,
                        chapter_id: row.get(3)?,
                        position: row.get(4)?,
                        duration: row.get(5)?,
                        updated_at: row.get(6)?,
                    })
                }
            ).optional()
            .map_err(TingError::DatabaseError)
        }).await
    }
    
    /// Upsert progress (insert or update)
    pub async fn upsert(&self, progress: &crate::db::models::Progress) -> Result<()> {
        let progress = progress.clone();
        self.db.execute(move |conn| {
            conn.execute(
                "INSERT INTO progress (id, user_id, book_id, chapter_id, position, duration, updated_at) \
                 VALUES (?, ?, ?, ?, ?, ?, STRFTIME('%Y-%m-%dT%H:%M:%fZ', 'now')) \
                 ON CONFLICT(user_id, book_id) DO UPDATE SET \
                 chapter_id = excluded.chapter_id, \
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
            Ok(())
        }).await
    }
}

/// Repository for Favorite entities
pub struct FavoriteRepository {
    db: Arc<DatabaseManager>,
}

impl FavoriteRepository {
    /// Create a new FavoriteRepository
    pub fn new(db: Arc<DatabaseManager>) -> Self {
        Self { db }
    }
    
    /// Get all favorites for a user
    pub async fn get_by_user(&self, user_id: &str) -> Result<Vec<crate::db::models::Favorite>> {
        let user_id = user_id.to_string();
        self.db.execute(move |conn| {
            let mut stmt = conn.prepare(
                "SELECT id, user_id, book_id, created_at \
                 FROM favorites WHERE user_id = ? ORDER BY created_at DESC"
            ).map_err(TingError::DatabaseError)?;
            
            let favorites = stmt.query_map([&user_id], |row| {
                Ok(crate::db::models::Favorite {
                    id: row.get(0)?,
                    user_id: row.get(1)?,
                    book_id: row.get(2)?,
                    created_at: row.get(3)?,
                })
            }).map_err(TingError::DatabaseError)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(TingError::DatabaseError)?;
            
            Ok(favorites)
        }).await
    }
    
    /// Check if a book is favorited
    pub async fn is_favorited(&self, user_id: &str, book_id: &str) -> Result<bool> {
        let user_id = user_id.to_string();
        let book_id = book_id.to_string();
        self.db.execute(move |conn| {
            let count: i64 = conn.query_row(
                "SELECT COUNT(*) FROM favorites WHERE user_id = ? AND book_id = ?",
                rusqlite::params![&user_id, &book_id],
                |row| row.get(0)
            ).map_err(TingError::DatabaseError)?;
            Ok(count > 0)
        }).await
    }
    
    /// Add a favorite
    pub async fn add(&self, favorite: &crate::db::models::Favorite) -> Result<()> {
        let favorite = favorite.clone();
        self.db.execute(move |conn| {
            conn.execute(
                "INSERT OR IGNORE INTO favorites (id, user_id, book_id) VALUES (?, ?, ?)",
                rusqlite::params![
                    &favorite.id,
                    &favorite.user_id,
                    &favorite.book_id,
                ],
            ).map_err(TingError::DatabaseError)?;
            Ok(())
        }).await
    }
    
    /// Remove a favorite
    pub async fn remove(&self, user_id: &str, book_id: &str) -> Result<()> {
        let user_id = user_id.to_string();
        let book_id = book_id.to_string();
        self.db.execute(move |conn| {
            conn.execute(
                "DELETE FROM favorites WHERE user_id = ? AND book_id = ?",
                rusqlite::params![&user_id, &book_id],
            ).map_err(TingError::DatabaseError)?;
            Ok(())
        }).await
    }
}

/// Repository for UserSettings entities
pub struct UserSettingsRepository {
    db: Arc<DatabaseManager>,
}

impl UserSettingsRepository {
    /// Create a new UserSettingsRepository
    pub fn new(db: Arc<DatabaseManager>) -> Self {
        Self { db }
    }
    
    /// Get settings for a user
    pub async fn get_by_user(&self, user_id: &str) -> Result<Option<crate::db::models::UserSettings>> {
        let user_id = user_id.to_string();
        self.db.execute(move |conn| {
            conn.query_row(
                "SELECT user_id, playback_speed, theme, auto_play, skip_intro, skip_outro, settings_json, updated_at \
                 FROM user_settings WHERE user_id = ?",
                [&user_id],
                |row| {
                    Ok(crate::db::models::UserSettings {
                        user_id: row.get(0)?,
                        playback_speed: row.get(1)?,
                        theme: row.get(2)?,
                        auto_play: row.get(3)?,
                        skip_intro: row.get(4)?,
                        skip_outro: row.get(5)?,
                        settings_json: row.get(6)?,
                        updated_at: row.get(7)?,
                    })
                }
            ).optional()
            .map_err(TingError::DatabaseError)
        }).await
    }
    
    /// Upsert user settings
    pub async fn upsert(&self, settings: &crate::db::models::UserSettings) -> Result<()> {
        let settings = settings.clone();
        self.db.execute(move |conn| {
            conn.execute(
                "INSERT INTO user_settings (user_id, playback_speed, theme, auto_play, skip_intro, skip_outro, settings_json, updated_at) \
                 VALUES (?, ?, ?, ?, ?, ?, ?, STRFTIME('%Y-%m-%dT%H:%M:%fZ', 'now')) \
                 ON CONFLICT(user_id) DO UPDATE SET \
                 playback_speed = excluded.playback_speed, \
                 theme = excluded.theme, \
                 auto_play = excluded.auto_play, \
                 skip_intro = excluded.skip_intro, \
                 skip_outro = excluded.skip_outro, \
                 settings_json = excluded.settings_json, \
                 updated_at = STRFTIME('%Y-%m-%dT%H:%M:%fZ', 'now')",
                rusqlite::params![
                    &settings.user_id,
                    settings.playback_speed,
                    &settings.theme,
                    settings.auto_play,
                    settings.skip_intro,
                    settings.skip_outro,
                    &settings.settings_json,
                ],
            ).map_err(TingError::DatabaseError)?;
            Ok(())
        }).await
    }
}

/// Repository for Library entities
pub struct LibraryRepository {
    db: Arc<DatabaseManager>,
}

impl LibraryRepository {
    /// Create a new LibraryRepository
    pub fn new(db: Arc<DatabaseManager>) -> Self {
        Self { db }
    }
    
    /// Get all libraries
    pub async fn find_all(&self) -> Result<Vec<crate::db::models::Library>> {
        self.db.execute(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, name, type, url, username, password, root_path, last_scanned_at, created_at, scraper_config \
                 FROM libraries ORDER BY name"
            ).map_err(TingError::DatabaseError)?;
            
            let libraries = stmt.query_map([], |row| {
                Ok(crate::db::models::Library {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    library_type: row.get(2)?,
                    url: row.get(3)?,
                    username: row.get(4)?,
                    password: row.get(5)?,
                    root_path: row.get(6)?,
                    last_scanned_at: row.get(7)?,
                    created_at: row.get(8)?,
                    scraper_config: row.get(9)?,
                })
            }).map_err(TingError::DatabaseError)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(TingError::DatabaseError)?;
            
            Ok(libraries)
        }).await
    }

    /// Get libraries accessible by user
    pub async fn find_by_user_access(&self, user_id: &str) -> Result<Vec<crate::db::models::Library>> {
        let user_id = user_id.to_string();
        self.db.execute(move |conn| {
            let mut stmt = conn.prepare(
                "SELECT l.id, l.name, l.type, l.url, l.username, l.password, l.root_path, l.last_scanned_at, l.created_at, l.scraper_config \
                 FROM libraries l \
                 JOIN user_library_access ula ON l.id = ula.library_id \
                 WHERE ula.user_id = ? \
                 ORDER BY l.name"
            ).map_err(TingError::DatabaseError)?;
            
            let libraries = stmt.query_map([&user_id], |row| {
                Ok(crate::db::models::Library {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    library_type: row.get(2)?,
                    url: row.get(3)?,
                    username: row.get(4)?,
                    password: row.get(5)?,
                    root_path: row.get(6)?,
                    last_scanned_at: row.get(7)?,
                    created_at: row.get(8)?,
                    scraper_config: row.get(9)?,
                })
            }).map_err(TingError::DatabaseError)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(TingError::DatabaseError)?;
            
            Ok(libraries)
        }).await
    }
    
    /// Find library by ID
    pub async fn find_by_id(&self, id: &str) -> Result<Option<crate::db::models::Library>> {
        let id = id.to_string();
        self.db.execute(move |conn| {
            conn.query_row(
                "SELECT id, name, type, url, username, password, root_path, last_scanned_at, created_at, scraper_config \
                 FROM libraries WHERE id = ?",
                [&id],
                |row| {
                    Ok(crate::db::models::Library {
                        id: row.get(0)?,
                        name: row.get(1)?,
                        library_type: row.get(2)?,
                        url: row.get(3)?,
                        username: row.get(4)?,
                        password: row.get(5)?,
                        root_path: row.get(6)?,
                        last_scanned_at: row.get(7)?,
                        created_at: row.get(8)?,
                        scraper_config: row.get(9)?,
                    })
                }
            ).optional()
            .map_err(TingError::DatabaseError)
        }).await
    }
    
    /// Create a new library
    pub async fn create(&self, library: &crate::db::models::Library) -> Result<()> {
        let library = library.clone();
        self.db.execute(move |conn| {
            conn.execute(
                "INSERT INTO libraries (id, name, type, url, username, password, root_path, scraper_config, created_at) \
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP)",
                rusqlite::params![
                    &library.id,
                    &library.name,
                    &library.library_type,
                    &library.url,
                    &library.username,
                    &library.password,
                    &library.root_path,
                    &library.scraper_config,
                ],
            ).map_err(TingError::DatabaseError)?;
            Ok(())
        }).await
    }
    
    /// Update a library
    pub async fn update(&self, library: &crate::db::models::Library) -> Result<()> {
        let library = library.clone();
        self.db.execute(move |conn| {
            conn.execute(
                "UPDATE libraries SET name = ?, type = ?, url = ?, username = ?, password = ?, root_path = ?, scraper_config = ? \
                 WHERE id = ?",
                rusqlite::params![
                    &library.name,
                    &library.library_type,
                    &library.url,
                    &library.username,
                    &library.password,
                    &library.root_path,
                    &library.scraper_config,
                    &library.id,
                ],
            ).map_err(TingError::DatabaseError)?;
            Ok(())
        }).await
    }
    
    /// Update library's last scanned time
    pub async fn update_last_scanned(&self, id: &str) -> Result<()> {
        let id = id.to_string();
        self.db.execute(move |conn| {
            conn.execute(
                "UPDATE libraries SET last_scanned_at = STRFTIME('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ?",
                [&id],
            ).map_err(TingError::DatabaseError)?;
            Ok(())
        }).await
    }
    
    /// Delete a library
    pub async fn delete(&self, id: &str) -> Result<()> {
        let id = id.to_string();
        self.db.execute(move |conn| {
            // First delete chapters associated with books in this library
            conn.execute(
                "DELETE FROM chapters WHERE book_id IN (SELECT id FROM books WHERE library_id = ?)",
                [&id],
            ).map_err(TingError::DatabaseError)?;

            // Delete user book access
            conn.execute(
                "DELETE FROM user_book_access WHERE book_id IN (SELECT id FROM books WHERE library_id = ?)",
                [&id],
            ).map_err(TingError::DatabaseError)?;

            // Delete books
            conn.execute(
                "DELETE FROM books WHERE library_id = ?",
                [&id],
            ).map_err(TingError::DatabaseError)?;

            // Delete user library access
            conn.execute(
                "DELETE FROM user_library_access WHERE library_id = ?",
                [&id],
            ).map_err(TingError::DatabaseError)?;

            // Finally delete the library
            conn.execute("DELETE FROM libraries WHERE id = ?", [&id])
                .map_err(TingError::DatabaseError)?;
            Ok(())
        }).await
    }
}
