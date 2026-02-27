//! Database manager implementation
//!
//! This module provides database connection management with:
//! - SQLite connection pool using r2d2
//! - Async wrapper for database operations
//! - Transaction support
//! - Database backup functionality
//! - Error handling integration with TingError

use crate::core::error::{Result, TingError};
use r2d2::{Pool, PooledConnection};
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::Connection;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::task;

/// Database manager with connection pool
pub struct DatabaseManager {
    pool: Pool<SqliteConnectionManager>,
    db_path: PathBuf,
}

impl DatabaseManager {
    /// Create a new DatabaseManager with the specified database path and pool size
    pub fn new(db_path: &Path, pool_size: u32, busy_timeout: Duration) -> Result<Self> {
        // Ensure parent directory exists
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent).map_err(|_e| {
                TingError::DatabaseError(rusqlite::Error::InvalidPath(parent.to_path_buf()))
            })?;
        }

        // Create connection manager
        let manager = SqliteConnectionManager::file(db_path)
            .with_init(move |conn| {
                // Enable foreign keys
                conn.execute_batch("PRAGMA foreign_keys = ON;")?;
                // Set busy timeout
                conn.busy_timeout(busy_timeout)?;
                // Enable WAL mode for better concurrency
                conn.execute_batch("PRAGMA journal_mode = WAL;")?;
                Ok(())
            });

        // Build connection pool
        let pool = Pool::builder()
            .max_size(pool_size)
            .connection_timeout(Duration::from_secs(30))
            .build(manager)
            .map_err(|_e| TingError::DatabaseError(rusqlite::Error::InvalidQuery))?;

        let manager = Self {
            pool,
            db_path: db_path.to_path_buf(),
        };

        // Run migrations on initialization
        manager.migrate()?;

        Ok(manager)
    }

    /// Create a new DatabaseManager with an in-memory database for testing
    pub fn new_in_memory() -> Result<Self> {
        // Create connection manager for in-memory database
        let manager = SqliteConnectionManager::memory()
            .with_init(|conn| {
                // Enable foreign keys
                conn.execute_batch("PRAGMA foreign_keys = ON;")?;
                Ok(())
            });

        // Build connection pool
        let pool = Pool::builder()
            .max_size(1) // In-memory databases should use a single connection
            .connection_timeout(Duration::from_secs(30))
            .build(manager)
            .map_err(|_e| TingError::DatabaseError(rusqlite::Error::InvalidQuery))?;

        let manager = Self {
            pool,
            db_path: PathBuf::from(":memory:"),
        };

        // Run migrations on initialization
        manager.migrate()?;

        Ok(manager)
    }

    /// Get a connection from the pool
    pub fn get_connection(&self) -> Result<PooledConnection<SqliteConnectionManager>> {
        self.pool.get().map_err(|_e| {
            TingError::DatabaseError(rusqlite::Error::InvalidQuery)
        })
    }

    /// Execute a database operation asynchronously
    ///
    /// This wraps synchronous database operations in tokio::task::spawn_blocking
    /// to avoid blocking the async runtime.
    pub async fn execute<F, T>(&self, f: F) -> Result<T>
    where
        F: FnOnce(&Connection) -> Result<T> + Send + 'static,
        T: Send + 'static,
    {
        let pool = self.pool.clone();
        
        task::spawn_blocking(move || {
            let conn = pool.get().map_err(|_e| {
                TingError::DatabaseError(rusqlite::Error::InvalidQuery)
            })?;
            f(&conn)
        })
        .await
        .map_err(|e| TingError::TaskError(format!("Database task panicked: {}", e)))?
    }

    /// Execute a database operation within a transaction
    ///
    /// The transaction is automatically committed if the closure returns Ok,
    /// or rolled back if it returns Err.
    pub async fn transaction<F, T>(&self, f: F) -> Result<T>
    where
        F: FnOnce(&rusqlite::Transaction) -> Result<T> + Send + 'static,
        T: Send + 'static,
    {
        let pool = self.pool.clone();
        
        task::spawn_blocking(move || {
            let mut conn = pool.get().map_err(|_e| {
                TingError::DatabaseError(rusqlite::Error::InvalidQuery)
            })?;
            
            let tx = conn.transaction().map_err(TingError::DatabaseError)?;
            let result = f(&tx)?;
            tx.commit().map_err(TingError::DatabaseError)?;
            
            Ok(result)
        })
        .await
        .map_err(|e| TingError::TaskError(format!("Transaction task panicked: {}", e)))?
    }

    /// Execute database migrations
    pub fn migrate(&self) -> Result<()> {
        let mut conn = self.get_connection()?;
        crate::db::migrations::run_migrations(&mut conn)
    }
    
    /// Execute database migrations with automatic backup
    ///
    /// This creates a backup before applying migrations.
    /// If any migration fails, it automatically restores from the backup.
    pub fn migrate_with_backup(&self) -> Result<()> {
        crate::db::migrations::run_migrations_with_backup(&self.db_path)
    }

    /// Backup the database to the specified path
    ///
    /// This creates a consistent backup of the database using SQLite's backup API.
    pub fn backup(&self, backup_path: &Path) -> Result<()> {
        // Ensure parent directory exists
        if let Some(parent) = backup_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                TingError::IoError(e)
            })?;
        }

        let src_conn = self.get_connection()?;
        
        // Open destination database
        let mut dst_conn = Connection::open(backup_path)
            .map_err(TingError::DatabaseError)?;

        // Perform backup
        let backup = rusqlite::backup::Backup::new(&src_conn, &mut dst_conn)
            .map_err(TingError::DatabaseError)?;

        backup
            .run_to_completion(5, Duration::from_millis(250), None)
            .map_err(TingError::DatabaseError)?;

        Ok(())
    }

    /// Backup the database asynchronously
    pub async fn backup_async(&self, backup_path: PathBuf) -> Result<()> {
        let pool = self.pool.clone();
        
        task::spawn_blocking(move || {
            // Ensure parent directory exists
            if let Some(parent) = backup_path.parent() {
                std::fs::create_dir_all(parent).map_err(|e| {
                    TingError::IoError(e)
                })?;
            }

            let src_conn = pool.get().map_err(|_e| {
                TingError::DatabaseError(rusqlite::Error::InvalidQuery)
            })?;
            
            // Open destination database
            let mut dst_conn = Connection::open(&backup_path)
                .map_err(TingError::DatabaseError)?;

            // Perform backup
            let backup = rusqlite::backup::Backup::new(&src_conn, &mut dst_conn)
                .map_err(TingError::DatabaseError)?;

            backup
                .run_to_completion(5, Duration::from_millis(250), None)
                .map_err(TingError::DatabaseError)?;

            Ok(())
        })
        .await
        .map_err(|e| TingError::TaskError(format!("Backup task panicked: {}", e)))?
    }

    /// Get the database file path
    pub fn db_path(&self) -> &Path {
        &self.db_path
    }

    /// Get the current pool size
    pub fn pool_size(&self) -> u32 {
        self.pool.max_size()
    }

    /// Get the number of idle connections in the pool
    pub fn idle_connections(&self) -> u32 {
        self.pool.state().idle_connections
    }

    /// Get the number of active connections in the pool
    pub fn active_connections(&self) -> u32 {
        self.pool.state().connections - self.pool.state().idle_connections
    }
}

impl Clone for DatabaseManager {
    fn clone(&self) -> Self {
        Self {
            pool: self.pool.clone(),
            db_path: self.db_path.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_db() -> (DatabaseManager, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let manager = DatabaseManager::new(&db_path, 5, Duration::from_secs(5)).unwrap();
        (manager, temp_dir)
    }

    #[test]
    fn test_database_manager_creation() {
        let (manager, _temp_dir) = create_test_db();
        assert_eq!(manager.pool_size(), 5);
    }

    #[test]
    fn test_get_connection() {
        let (manager, _temp_dir) = create_test_db();
        let conn = manager.get_connection();
        assert!(conn.is_ok());
    }

    #[tokio::test]
    async fn test_execute_async() {
        let (manager, _temp_dir) = create_test_db();
        
        let result = manager.execute(|conn| {
            conn.execute(
                "CREATE TABLE test (id INTEGER PRIMARY KEY, name TEXT)",
                [],
            ).map_err(TingError::DatabaseError)?;
            Ok(())
        }).await;
        
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_transaction_commit() {
        let (manager, _temp_dir) = create_test_db();
        
        // Create table
        manager.execute(|conn| {
            conn.execute(
                "CREATE TABLE test (id INTEGER PRIMARY KEY, value INTEGER)",
                [],
            ).map_err(TingError::DatabaseError)?;
            Ok(())
        }).await.unwrap();
        
        // Insert in transaction
        let result = manager.transaction(|tx| {
            tx.execute("INSERT INTO test (value) VALUES (?)", [42])
                .map_err(TingError::DatabaseError)?;
            Ok(())
        }).await;
        
        assert!(result.is_ok());
        
        // Verify data was committed
        let count: i64 = manager.execute(|conn| {
            conn.query_row("SELECT COUNT(*) FROM test", [], |row| row.get(0))
                .map_err(TingError::DatabaseError)
        }).await.unwrap();
        
        assert_eq!(count, 1);
    }

    #[tokio::test]
    async fn test_transaction_rollback() {
        let (manager, _temp_dir) = create_test_db();
        
        // Create table
        manager.execute(|conn| {
            conn.execute(
                "CREATE TABLE test (id INTEGER PRIMARY KEY, value INTEGER)",
                [],
            ).map_err(TingError::DatabaseError)?;
            Ok(())
        }).await.unwrap();
        
        // Insert in transaction that fails
        let result: Result<()> = manager.transaction(|tx| {
            tx.execute("INSERT INTO test (value) VALUES (?)", [42])
                .map_err(TingError::DatabaseError)?;
            // Simulate error
            Err(TingError::InvalidRequest("test error".into()))
        }).await;
        
        assert!(result.is_err());
        
        // Verify data was rolled back
        let count: i64 = manager.execute(|conn| {
            conn.query_row("SELECT COUNT(*) FROM test", [], |row| row.get(0))
                .map_err(TingError::DatabaseError)
        }).await.unwrap();
        
        assert_eq!(count, 0);
    }

    #[test]
    fn test_backup() {
        let (manager, temp_dir) = create_test_db();
        
        // Create and populate table
        let conn = manager.get_connection().unwrap();
        conn.execute(
            "CREATE TABLE test (id INTEGER PRIMARY KEY, name TEXT)",
            [],
        ).unwrap();
        conn.execute("INSERT INTO test (name) VALUES ('test')", []).unwrap();
        drop(conn);
        
        // Backup database
        let backup_path = temp_dir.path().join("backup.db");
        let result = manager.backup(&backup_path);
        assert!(result.is_ok());
        
        // Verify backup exists and contains data
        let backup_conn = Connection::open(&backup_path).unwrap();
        let count: i64 = backup_conn
            .query_row("SELECT COUNT(*) FROM test", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 1);
    }

    #[tokio::test]
    async fn test_backup_async() {
        let (manager, temp_dir) = create_test_db();
        
        // Create and populate table
        manager.execute(|conn| {
            conn.execute(
                "CREATE TABLE test (id INTEGER PRIMARY KEY, name TEXT)",
                [],
            ).map_err(TingError::DatabaseError)?;
            conn.execute("INSERT INTO test (name) VALUES ('test')", [])
                .map_err(TingError::DatabaseError)?;
            Ok(())
        }).await.unwrap();
        
        // Backup database asynchronously
        let backup_path = temp_dir.path().join("backup_async.db");
        let result = manager.backup_async(backup_path.clone()).await;
        assert!(result.is_ok());
        
        // Verify backup exists and contains data
        let backup_conn = Connection::open(&backup_path).unwrap();
        let count: i64 = backup_conn
            .query_row("SELECT COUNT(*) FROM test", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_connection_pool_stats() {
        let (manager, _temp_dir) = create_test_db();
        
        assert_eq!(manager.pool_size(), 5);
        assert!(manager.idle_connections() > 0);
        
        // Get a connection
        let _conn = manager.get_connection().unwrap();
        
        // Active connections should increase
        assert!(manager.active_connections() > 0);
    }
}
