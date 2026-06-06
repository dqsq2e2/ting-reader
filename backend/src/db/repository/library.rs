use crate::core::error::{Result, TingError};
use crate::db::manager::DatabaseManager;
use crate::db::models::Library;
use rusqlite::OptionalExtension;
use std::sync::Arc;

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
    pub async fn find_all(&self) -> Result<Vec<Library>> {
        self.db.execute(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, name, type, url, username, password, root_path, last_scanned_at, created_at, scraper_config \
                 FROM libraries ORDER BY name"
            ).map_err(TingError::DatabaseError)?;

            let libraries = stmt.query_map([], |row| {
                Ok(Library {
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
    pub async fn find_by_user_access(&self, user_id: &str) -> Result<Vec<Library>> {
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
                Ok(Library {
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
    pub async fn find_by_id(&self, id: &str) -> Result<Option<Library>> {
        let id = id.to_string();
        self.db.execute(move |conn| {
            conn.query_row(
                "SELECT id, name, type, url, username, password, root_path, last_scanned_at, created_at, scraper_config \
                 FROM libraries WHERE id = ?",
                [&id],
                |row| {
                    Ok(Library {
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
    pub async fn create(&self, library: &Library) -> Result<()> {
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
    pub async fn update(&self, library: &Library) -> Result<()> {
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
