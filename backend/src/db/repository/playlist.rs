use crate::core::error::{Result, TingError};
use crate::db::manager::DatabaseManager;
use crate::db::models::{Book, Playlist, PlaylistBook, PlaylistItem};
use crate::db::repository::base::Repository;
use crate::db::repository::book::map_book_row;
use async_trait::async_trait;
use rusqlite::OptionalExtension;
use std::sync::Arc;

/// Repository for user playlists
pub struct PlaylistRepository {
    db: Arc<DatabaseManager>,
}

impl PlaylistRepository {
    pub fn new(db: Arc<DatabaseManager>) -> Self {
        Self { db }
    }

    pub async fn find_by_user(&self, user_id: &str) -> Result<Vec<Playlist>> {
        let user_id = user_id.to_string();
        self.db
            .execute(move |conn| {
                let mut stmt = conn
                    .prepare(
                        "SELECT id, user_id, title, description, created_at, updated_at \
                         FROM playlists WHERE user_id = ? ORDER BY updated_at DESC, created_at DESC",
                    )
                    .map_err(TingError::DatabaseError)?;

                let playlists = stmt
                    .query_map([&user_id], map_playlist_row)
                    .map_err(TingError::DatabaseError)?
                    .collect::<std::result::Result<Vec<_>, _>>()
                    .map_err(TingError::DatabaseError)?;

                Ok(playlists)
            })
            .await
    }

    pub async fn find_by_user_and_id(
        &self,
        playlist_id: &str,
        user_id: &str,
    ) -> Result<Option<Playlist>> {
        let playlist_id = playlist_id.to_string();
        let user_id = user_id.to_string();
        self.db
            .execute(move |conn| {
                conn.query_row(
                    "SELECT id, user_id, title, description, created_at, updated_at \
                     FROM playlists WHERE id = ? AND user_id = ?",
                    [&playlist_id, &user_id],
                    map_playlist_row,
                )
                .optional()
                .map_err(TingError::DatabaseError)
            })
            .await
    }

    pub async fn find_books_by_playlist(
        &self,
        playlist_id: &str,
        user_id: &str,
        is_admin: bool,
    ) -> Result<Vec<(Book, i32)>> {
        let playlist_id = playlist_id.to_string();
        let user_id = user_id.to_string();
        self.db
            .execute(move |conn| {
                let mut query = "SELECT b.id, b.library_id, b.title, b.author, b.narrator, b.cover_url, b.theme_color, \
                             b.description, b.skip_intro, b.skip_outro, b.path, b.hash, b.tags, b.genre, b.year, b.created_at, \
                             b.manual_corrected, b.match_pattern, b.chapter_regex, pb.book_order \
                             FROM books b \
                             JOIN playlist_books pb ON b.id = pb.book_id \
                             WHERE pb.playlist_id = ?"
                    .to_string();
                let mut params = vec![playlist_id];

                if !is_admin {
                    query += " AND (
                        b.library_id IN (SELECT library_id FROM user_library_access WHERE user_id = ?)
                        OR
                        b.id IN (SELECT book_id FROM user_book_access WHERE user_id = ?)
                    )";
                    params.push(user_id.clone());
                    params.push(user_id);
                }

                query += " ORDER BY pb.book_order ASC";

                let mut stmt = conn.prepare(&query).map_err(TingError::DatabaseError)?;
                let books = stmt
                    .query_map(rusqlite::params_from_iter(params.iter()), |row| {
                        let book = map_book_row(row)?;
                        let order: i32 = row.get(19)?;
                        Ok((book, order))
                    })
                    .map_err(TingError::DatabaseError)?
                    .collect::<std::result::Result<Vec<_>, _>>()
                    .map_err(TingError::DatabaseError)?;

                Ok(books)
            })
            .await
    }

    pub async fn find_items_by_playlist(&self, playlist_id: &str) -> Result<Vec<PlaylistItem>> {
        let playlist_id = playlist_id.to_string();
        self.db
            .execute(move |conn| {
                let has_items_table: bool = conn
                    .query_row(
                        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'playlist_items'",
                        [],
                        |row| row.get(0),
                    )
                    .unwrap_or(0)
                    > 0;

                if has_items_table {
                    let mut stmt = conn
                        .prepare(
                            "SELECT playlist_id, item_type, item_id, item_order \
                             FROM playlist_items WHERE playlist_id = ? ORDER BY item_order ASC",
                        )
                        .map_err(TingError::DatabaseError)?;

                    let items = stmt
                        .query_map([&playlist_id], |row| {
                            Ok(PlaylistItem {
                                playlist_id: row.get(0)?,
                                item_type: row.get(1)?,
                                item_id: row.get(2)?,
                                item_order: row.get(3)?,
                            })
                        })
                        .map_err(TingError::DatabaseError)?
                        .collect::<std::result::Result<Vec<_>, _>>()
                        .map_err(TingError::DatabaseError)?;

                    return Ok(items);
                }

                let mut stmt = conn
                    .prepare(
                        "SELECT playlist_id, book_id, book_order \
                         FROM playlist_books WHERE playlist_id = ? ORDER BY book_order ASC",
                    )
                    .map_err(TingError::DatabaseError)?;

                let items = stmt
                    .query_map([&playlist_id], |row| {
                        Ok(PlaylistItem {
                            playlist_id: row.get(0)?,
                            item_type: "book".to_string(),
                            item_id: row.get(1)?,
                            item_order: row.get(2)?,
                        })
                    })
                    .map_err(TingError::DatabaseError)?
                    .collect::<std::result::Result<Vec<_>, _>>()
                    .map_err(TingError::DatabaseError)?;

                Ok(items)
            })
            .await
    }

    pub async fn replace_books(
        &self,
        playlist_id: &str,
        user_id: &str,
        is_admin: bool,
        book_ids: Vec<String>,
    ) -> Result<()> {
        let playlist_id = playlist_id.to_string();
        let user_id = user_id.to_string();
        self.db
            .transaction(move |tx| {
                if !is_admin {
                    for book_id in &book_ids {
                        let has_access: bool = tx
                            .query_row(
                                "SELECT EXISTS(
                                    SELECT 1 FROM books b
                                    WHERE b.id = ? AND (
                                        b.library_id IN (SELECT library_id FROM user_library_access WHERE user_id = ?)
                                        OR
                                        b.id IN (SELECT book_id FROM user_book_access WHERE user_id = ?)
                                    )
                                )",
                                rusqlite::params![book_id, &user_id, &user_id],
                                |row| row.get(0),
                            )
                            .unwrap_or(false);

                        if !has_access {
                            return Err(TingError::PermissionDenied(
                                "No access to one or more books".to_string(),
                            ));
                        }
                    }
                }

                tx.execute(
                    "DELETE FROM playlist_books WHERE playlist_id = ?",
                    [&playlist_id],
                )
                .map_err(TingError::DatabaseError)?;

                for (idx, book_id) in book_ids.iter().enumerate() {
                    let playlist_book = PlaylistBook {
                        playlist_id: playlist_id.clone(),
                        book_id: book_id.clone(),
                        book_order: (idx + 1) as i32,
                    };
                    tx.execute(
                        "INSERT INTO playlist_books (playlist_id, book_id, book_order) VALUES (?, ?, ?)",
                        rusqlite::params![
                            &playlist_book.playlist_id,
                            &playlist_book.book_id,
                            playlist_book.book_order,
                        ],
                    )
                    .map_err(TingError::DatabaseError)?;
                }

                tx.execute(
                    "UPDATE playlists SET updated_at = STRFTIME('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ?",
                    [&playlist_id],
                )
                .map_err(TingError::DatabaseError)?;

                Ok(())
            })
            .await
    }

    pub async fn add_item(
        &self,
        playlist_id: &str,
        user_id: &str,
        is_admin: bool,
        item: PlaylistItem,
    ) -> Result<()> {
        let playlist_id = playlist_id.to_string();
        let user_id = user_id.to_string();
        self.db
            .transaction(move |tx| {
                if !is_admin {
                    let has_access: bool = match item.item_type.as_str() {
                        "book" => tx
                            .query_row(
                                "SELECT EXISTS(
                                    SELECT 1 FROM books b
                                    WHERE b.id = ? AND (
                                        b.library_id IN (SELECT library_id FROM user_library_access WHERE user_id = ?)
                                        OR
                                        b.id IN (SELECT book_id FROM user_book_access WHERE user_id = ?)
                                    )
                                )",
                                rusqlite::params![&item.item_id, &user_id, &user_id],
                                |row| row.get(0),
                            )
                            .unwrap_or(false),
                        "series" => tx
                            .query_row(
                                "SELECT EXISTS(
                                    SELECT 1 FROM series s
                                    WHERE s.id = ? AND (
                                        s.library_id IN (SELECT library_id FROM user_library_access WHERE user_id = ?)
                                        OR
                                        s.id IN (
                                            SELECT series_id FROM series_books
                                            WHERE book_id IN (SELECT book_id FROM user_book_access WHERE user_id = ?)
                                        )
                                    )
                                )",
                                rusqlite::params![&item.item_id, &user_id, &user_id],
                                |row| row.get(0),
                            )
                            .unwrap_or(false),
                        _ => false,
                    };

                    if !has_access {
                        return Err(TingError::PermissionDenied(
                            "No access to playlist item".to_string(),
                        ));
                    }
                }

                let next_order: i32 = tx
                    .query_row(
                        "SELECT COALESCE(MAX(item_order), 0) + 1 FROM playlist_items WHERE playlist_id = ?",
                        [&playlist_id],
                        |row| row.get(0),
                    )
                    .unwrap_or(1);

                tx.execute(
                    "INSERT INTO playlist_items (playlist_id, item_type, item_id, item_order) VALUES (?, ?, ?, ?)",
                    rusqlite::params![
                        &playlist_id,
                        &item.item_type,
                        &item.item_id,
                        next_order,
                    ],
                )
                .map_err(TingError::DatabaseError)?;

                if item.item_type == "book" {
                    tx.execute(
                        "INSERT INTO playlist_books (playlist_id, book_id, book_order) VALUES (?, ?, ?)",
                        rusqlite::params![
                            &playlist_id,
                            &item.item_id,
                            next_order,
                        ],
                    )
                    .map_err(TingError::DatabaseError)?;
                }

                tx.execute(
                    "UPDATE playlists SET updated_at = STRFTIME('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ?",
                    [&playlist_id],
                )
                .map_err(TingError::DatabaseError)?;

                Ok(())
            })
            .await
    }

    pub async fn remove_item(
        &self,
        playlist_id: &str,
        item_type: &str,
        item_id: &str,
    ) -> Result<()> {
        let playlist_id = playlist_id.to_string();
        let item_type = item_type.to_string();
        let item_id = item_id.to_string();
        self.db
            .transaction(move |tx| {
                tx.execute(
                    "DELETE FROM playlist_items WHERE playlist_id = ? AND item_type = ? AND item_id = ?",
                    rusqlite::params![&playlist_id, &item_type, &item_id],
                )
                .map_err(TingError::DatabaseError)?;

                if item_type == "book" {
                    tx.execute(
                        "DELETE FROM playlist_books WHERE playlist_id = ? AND book_id = ?",
                        rusqlite::params![&playlist_id, &item_id],
                    )
                    .map_err(TingError::DatabaseError)?;
                }

                tx.execute(
                    "UPDATE playlists SET updated_at = STRFTIME('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ?",
                    [&playlist_id],
                )
                .map_err(TingError::DatabaseError)?;

                Ok(())
            })
            .await
    }

    pub async fn replace_items(
        &self,
        playlist_id: &str,
        user_id: &str,
        is_admin: bool,
        items: Vec<PlaylistItem>,
    ) -> Result<()> {
        let playlist_id = playlist_id.to_string();
        let user_id = user_id.to_string();
        self.db
            .transaction(move |tx| {
                if !is_admin {
                    for item in &items {
                        let has_access: bool = match item.item_type.as_str() {
                            "book" => tx
                                .query_row(
                                    "SELECT EXISTS(
                                        SELECT 1 FROM books b
                                        WHERE b.id = ? AND (
                                            b.library_id IN (SELECT library_id FROM user_library_access WHERE user_id = ?)
                                            OR
                                            b.id IN (SELECT book_id FROM user_book_access WHERE user_id = ?)
                                        )
                                    )",
                                    rusqlite::params![&item.item_id, &user_id, &user_id],
                                    |row| row.get(0),
                                )
                                .unwrap_or(false),
                            "series" => tx
                                .query_row(
                                    "SELECT EXISTS(
                                        SELECT 1 FROM series s
                                        WHERE s.id = ? AND (
                                            s.library_id IN (SELECT library_id FROM user_library_access WHERE user_id = ?)
                                            OR
                                            s.id IN (
                                                SELECT series_id FROM series_books
                                                WHERE book_id IN (SELECT book_id FROM user_book_access WHERE user_id = ?)
                                            )
                                        )
                                    )",
                                    rusqlite::params![&item.item_id, &user_id, &user_id],
                                    |row| row.get(0),
                                )
                                .unwrap_or(false),
                            _ => false,
                        };

                        if !has_access {
                            return Err(TingError::PermissionDenied(
                                "No access to one or more playlist items".to_string(),
                            ));
                        }
                    }
                }

                tx.execute(
                    "DELETE FROM playlist_items WHERE playlist_id = ?",
                    [&playlist_id],
                )
                .map_err(TingError::DatabaseError)?;

                tx.execute(
                    "DELETE FROM playlist_books WHERE playlist_id = ?",
                    [&playlist_id],
                )
                .map_err(TingError::DatabaseError)?;

                for (idx, item) in items.iter().enumerate() {
                    let order = (idx + 1) as i32;
                    tx.execute(
                        "INSERT INTO playlist_items (playlist_id, item_type, item_id, item_order) VALUES (?, ?, ?, ?)",
                        rusqlite::params![
                            &playlist_id,
                            &item.item_type,
                            &item.item_id,
                            order,
                        ],
                    )
                    .map_err(TingError::DatabaseError)?;

                    if item.item_type == "book" {
                        let playlist_book = PlaylistBook {
                            playlist_id: playlist_id.clone(),
                            book_id: item.item_id.clone(),
                            book_order: order,
                        };
                        tx.execute(
                            "INSERT INTO playlist_books (playlist_id, book_id, book_order) VALUES (?, ?, ?)",
                            rusqlite::params![
                                &playlist_book.playlist_id,
                                &playlist_book.book_id,
                                playlist_book.book_order,
                            ],
                        )
                        .map_err(TingError::DatabaseError)?;
                    }
                }

                tx.execute(
                    "UPDATE playlists SET updated_at = STRFTIME('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ?",
                    [&playlist_id],
                )
                .map_err(TingError::DatabaseError)?;

                Ok(())
            })
            .await
    }
}

fn map_playlist_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Playlist> {
    Ok(Playlist {
        id: row.get(0)?,
        user_id: row.get(1)?,
        title: row.get(2)?,
        description: row.get(3)?,
        created_at: row.get(4)?,
        updated_at: row.get(5)?,
    })
}

#[async_trait]
impl Repository<Playlist> for PlaylistRepository {
    async fn find_by_id(&self, id: &str) -> Result<Option<Playlist>> {
        let id = id.to_string();
        self.db
            .execute(move |conn| {
                conn.query_row(
                    "SELECT id, user_id, title, description, created_at, updated_at \
                     FROM playlists WHERE id = ?",
                    [&id],
                    map_playlist_row,
                )
                .optional()
                .map_err(TingError::DatabaseError)
            })
            .await
    }

    async fn find_all(&self) -> Result<Vec<Playlist>> {
        self.db
            .execute(|conn| {
                let mut stmt = conn
                    .prepare(
                        "SELECT id, user_id, title, description, created_at, updated_at \
                         FROM playlists ORDER BY updated_at DESC, created_at DESC",
                    )
                    .map_err(TingError::DatabaseError)?;

                let playlists = stmt
                    .query_map([], map_playlist_row)
                    .map_err(TingError::DatabaseError)?
                    .collect::<std::result::Result<Vec<_>, _>>()
                    .map_err(TingError::DatabaseError)?;

                Ok(playlists)
            })
            .await
    }

    async fn create(&self, playlist: &Playlist) -> Result<()> {
        let playlist = playlist.clone();
        self.db
            .execute(move |conn| {
                conn.execute(
                    "INSERT INTO playlists (id, user_id, title, description, created_at, updated_at) \
                     VALUES (?, ?, ?, ?, ?, ?)",
                    rusqlite::params![
                        &playlist.id,
                        &playlist.user_id,
                        &playlist.title,
                        &playlist.description,
                        &playlist.created_at,
                        &playlist.updated_at,
                    ],
                )
                .map_err(TingError::DatabaseError)?;
                Ok(())
            })
            .await
    }

    async fn update(&self, playlist: &Playlist) -> Result<()> {
        let playlist = playlist.clone();
        self.db
            .execute(move |conn| {
                conn.execute(
                    "UPDATE playlists SET title = ?, description = ?, \
                     updated_at = STRFTIME('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ? AND user_id = ?",
                    rusqlite::params![
                        &playlist.title,
                        &playlist.description,
                        &playlist.id,
                        &playlist.user_id,
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
                conn.execute("DELETE FROM playlists WHERE id = ?", [&id])
                    .map_err(TingError::DatabaseError)?;
                Ok(())
            })
            .await
    }
}
