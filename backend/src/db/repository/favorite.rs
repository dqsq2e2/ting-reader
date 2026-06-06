use crate::core::error::{Result, TingError};
use crate::db::manager::DatabaseManager;
use crate::db::models::Favorite;
use std::sync::Arc;

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
    pub async fn get_by_user(&self, user_id: &str) -> Result<Vec<Favorite>> {
        let user_id = user_id.to_string();
        self.db
            .execute(move |conn| {
                let mut stmt = conn
                    .prepare(
                        "SELECT id, user_id, book_id, created_at \
                 FROM favorites WHERE user_id = ? ORDER BY created_at DESC",
                    )
                    .map_err(TingError::DatabaseError)?;

                let favorites = stmt
                    .query_map([&user_id], |row| {
                        Ok(Favorite {
                            id: row.get(0)?,
                            user_id: row.get(1)?,
                            book_id: row.get(2)?,
                            created_at: row.get(3)?,
                        })
                    })
                    .map_err(TingError::DatabaseError)?
                    .collect::<std::result::Result<Vec<_>, _>>()
                    .map_err(TingError::DatabaseError)?;

                Ok(favorites)
            })
            .await
    }

    /// Check if a book is favorited
    pub async fn is_favorited(&self, user_id: &str, book_id: &str) -> Result<bool> {
        let user_id = user_id.to_string();
        let book_id = book_id.to_string();
        self.db
            .execute(move |conn| {
                let count: i64 = conn
                    .query_row(
                        "SELECT COUNT(*) FROM favorites WHERE user_id = ? AND book_id = ?",
                        rusqlite::params![&user_id, &book_id],
                        |row| row.get(0),
                    )
                    .map_err(TingError::DatabaseError)?;
                Ok(count > 0)
            })
            .await
    }

    /// Add a favorite
    pub async fn add(&self, favorite: &Favorite) -> Result<()> {
        let favorite = favorite.clone();
        self.db
            .execute(move |conn| {
                conn.execute(
                    "INSERT OR IGNORE INTO favorites (id, user_id, book_id) VALUES (?, ?, ?)",
                    rusqlite::params![&favorite.id, &favorite.user_id, &favorite.book_id,],
                )
                .map_err(TingError::DatabaseError)?;
                Ok(())
            })
            .await
    }

    /// Remove a favorite
    pub async fn remove(&self, user_id: &str, book_id: &str) -> Result<()> {
        let user_id = user_id.to_string();
        let book_id = book_id.to_string();
        self.db
            .execute(move |conn| {
                conn.execute(
                    "DELETE FROM favorites WHERE user_id = ? AND book_id = ?",
                    rusqlite::params![&user_id, &book_id],
                )
                .map_err(TingError::DatabaseError)?;
                Ok(())
            })
            .await
    }
}
