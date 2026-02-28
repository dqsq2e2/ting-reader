//! Database migrations
//!
//! This module provides database schema migration functionality with automatic backup and rollback.

use crate::core::error::{Result, TingError};
use rusqlite::Connection;
use tracing::{info, warn, error};
use std::path::{Path, PathBuf};
use std::fs;
use chrono::Local;

/// Migration version tracking table
const MIGRATION_TABLE: &str = r#"
CREATE TABLE IF NOT EXISTS schema_migrations (
    version INTEGER PRIMARY KEY,
    applied_at DATETIME DEFAULT CURRENT_TIMESTAMP
)
"#;

/// Initial schema migration (version 1)
const MIGRATION_V1: &str = r#"
-- Users table (authentication)
CREATE TABLE IF NOT EXISTS users (
    id TEXT PRIMARY KEY,
    username TEXT UNIQUE NOT NULL,
    password_hash TEXT NOT NULL,
    role TEXT NOT NULL DEFAULT 'user',
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- Books table (compatible with Node.js version)
CREATE TABLE IF NOT EXISTS books (
    id TEXT PRIMARY KEY,
    library_id TEXT NOT NULL,
    title TEXT,
    author TEXT,
    narrator TEXT,
    cover_url TEXT,
    theme_color TEXT,
    description TEXT,
    skip_intro INTEGER DEFAULT 0,
    skip_outro INTEGER DEFAULT 0,
    path TEXT NOT NULL,
    hash TEXT UNIQUE NOT NULL,
    tags TEXT,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- Chapters table (compatible with Node.js version)
CREATE TABLE IF NOT EXISTS chapters (
    id TEXT PRIMARY KEY,
    book_id TEXT NOT NULL,
    title TEXT,
    path TEXT NOT NULL,
    duration INTEGER,
    chapter_index INTEGER,
    is_extra INTEGER DEFAULT 0,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (book_id) REFERENCES books(id) ON DELETE CASCADE
);

-- Plugin registry table
CREATE TABLE IF NOT EXISTS plugin_registry (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    version TEXT NOT NULL,
    plugin_type TEXT NOT NULL,
    description TEXT,
    author TEXT,
    enabled INTEGER DEFAULT 1,
    config TEXT,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- Plugin dependencies table
CREATE TABLE IF NOT EXISTS plugin_dependencies (
    plugin_id TEXT NOT NULL,
    dependency_id TEXT NOT NULL,
    version_requirement TEXT NOT NULL,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (plugin_id, dependency_id),
    FOREIGN KEY (plugin_id) REFERENCES plugin_registry(id) ON DELETE CASCADE,
    FOREIGN KEY (dependency_id) REFERENCES plugin_registry(id) ON DELETE CASCADE
);

-- Tasks table (compatible with Node.js version)
CREATE TABLE IF NOT EXISTS tasks (
    id TEXT PRIMARY KEY,
    type TEXT NOT NULL,
    status TEXT DEFAULT 'pending',
    payload TEXT,
    message TEXT,
    error TEXT,
    retries INTEGER DEFAULT 0,
    max_retries INTEGER DEFAULT 3,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- Progress table (playback progress tracking)
CREATE TABLE IF NOT EXISTS progress (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL,
    book_id TEXT NOT NULL,
    chapter_id TEXT,
    position REAL DEFAULT 0,
    duration REAL,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE,
    FOREIGN KEY (book_id) REFERENCES books(id) ON DELETE CASCADE,
    FOREIGN KEY (chapter_id) REFERENCES chapters(id) ON DELETE SET NULL,
    UNIQUE(user_id, book_id)
);

-- Favorites table
CREATE TABLE IF NOT EXISTS favorites (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL,
    book_id TEXT NOT NULL,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE,
    FOREIGN KEY (book_id) REFERENCES books(id) ON DELETE CASCADE,
    UNIQUE(user_id, book_id)
);

-- User settings table
CREATE TABLE IF NOT EXISTS user_settings (
    user_id TEXT PRIMARY KEY,
    playback_speed REAL DEFAULT 1.0,
    theme TEXT DEFAULT 'auto',
    auto_play INTEGER DEFAULT 1,
    skip_intro INTEGER DEFAULT 0,
    skip_outro INTEGER DEFAULT 0,
    settings_json TEXT,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);

-- Libraries table (compatible with Node.js version)
CREATE TABLE IF NOT EXISTS libraries (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    type TEXT DEFAULT 'webdav',
    url TEXT NOT NULL,
    username TEXT,
    password TEXT,
    root_path TEXT DEFAULT '/',
    last_scanned_at DATETIME,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- Indexes for performance
CREATE INDEX IF NOT EXISTS idx_books_library_id ON books(library_id);
CREATE INDEX IF NOT EXISTS idx_books_hash ON books(hash);
CREATE INDEX IF NOT EXISTS idx_chapters_book_id ON chapters(book_id);
CREATE INDEX IF NOT EXISTS idx_chapters_index ON chapters(book_id, chapter_index);
CREATE INDEX IF NOT EXISTS idx_plugin_registry_type ON plugin_registry(plugin_type);
CREATE INDEX IF NOT EXISTS idx_plugin_registry_enabled ON plugin_registry(enabled);
CREATE INDEX IF NOT EXISTS idx_tasks_status ON tasks(status);
CREATE INDEX IF NOT EXISTS idx_tasks_type ON tasks(type);
CREATE INDEX IF NOT EXISTS idx_tasks_created_at ON tasks(created_at);
CREATE INDEX IF NOT EXISTS idx_progress_user_id ON progress(user_id);
CREATE INDEX IF NOT EXISTS idx_progress_updated_at ON progress(user_id, updated_at);
CREATE INDEX IF NOT EXISTS idx_favorites_user_id ON favorites(user_id);
"#;

/// Second schema migration (version 2)
const MIGRATION_V2: &str = r#"
-- User library access
CREATE TABLE IF NOT EXISTS user_library_access (
    user_id TEXT NOT NULL,
    library_id TEXT NOT NULL,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (user_id, library_id),
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE,
    FOREIGN KEY (library_id) REFERENCES libraries(id) ON DELETE CASCADE
);

-- User book access
CREATE TABLE IF NOT EXISTS user_book_access (
    user_id TEXT NOT NULL,
    book_id TEXT NOT NULL,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (user_id, book_id),
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE,
    FOREIGN KEY (book_id) REFERENCES books(id) ON DELETE CASCADE
);
"#;

/// Third schema migration (version 3)
const MIGRATION_V3: &str = r#"
-- Add hash column to chapters
ALTER TABLE chapters ADD COLUMN hash TEXT;
CREATE INDEX IF NOT EXISTS idx_chapters_hash ON chapters(hash);
"#;

/// Fourth schema migration (version 4)
const MIGRATION_V4: &str = r#"
--// Add scraper_config column to libraries
ALTER TABLE libraries ADD COLUMN scraper_config TEXT;
"#;

/// Fifth schema migration (version 5)
const MIGRATION_V5: &str = r#"
-- Add manual_corrected and match_pattern to books
ALTER TABLE books ADD COLUMN manual_corrected INTEGER DEFAULT 0;
ALTER TABLE books ADD COLUMN match_pattern TEXT;

-- Merge suggestions table
CREATE TABLE IF NOT EXISTS merge_suggestions (
    id TEXT PRIMARY KEY,
    book_a_id TEXT NOT NULL,
    book_b_id TEXT NOT NULL,
    score REAL,
    reason TEXT,
    status TEXT DEFAULT 'pending',
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (book_a_id) REFERENCES books(id) ON DELETE CASCADE,
    FOREIGN KEY (book_b_id) REFERENCES books(id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_merge_suggestions_status ON merge_suggestions(status);
"#;

/// Sixth schema migration (version 6)
const MIGRATION_V6: &str = r#"
-- Add chapter_regex to books
ALTER TABLE books ADD COLUMN chapter_regex TEXT;
"#;

/// Seventh schema migration (version 7)
const MIGRATION_V7: &str = r#"
--// Add manual_corrected to chapters
ALTER TABLE chapters ADD COLUMN manual_corrected INTEGER DEFAULT 0;
"#;

/// Run all pending database migrations
///
/// This function applies database schema migrations in order.
/// It tracks which migrations have been applied using the schema_migrations table.
/// Before applying migrations, it creates a backup of the database.
/// If a migration fails, it automatically rolls back to the backup.
pub fn run_migrations(conn: &mut Connection) -> Result<()> {
    info!("Running database migrations");
    
    // Create migration tracking table
    conn.execute_batch(MIGRATION_TABLE)
        .map_err(|e| TingError::DatabaseError(e))?;
    
    // Check current version
    let current_version: i64 = conn
        .query_row(
            "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
            [],
            |row| row.get(0),
        )
        .map_err(|e| TingError::DatabaseError(e))?;
    
    info!("Current database schema version: {}", current_version);
    
    // Apply migrations
    if current_version < 1 {
        info!("Applying migration v1: Initial schema");
        apply_migration(conn, 1, MIGRATION_V1)?;
    }

    if current_version < 2 {
        info!("Applying migration v2: User access control");
        apply_migration(conn, 2, MIGRATION_V2)?;
    }

    if current_version < 3 {
        info!("Applying migration v3: Chapter hash column");
        apply_migration(conn, 3, MIGRATION_V3)?;
    }

    if current_version < 4 {
        info!("Applying migration v4: Library scraper config");
        apply_migration(conn, 4, MIGRATION_V4)?;
    }

    if current_version < 5 {
        info!("Applying migration v5: Chapter Management System");
        apply_migration(conn, 5, MIGRATION_V5)?;
    }

    if current_version < 6 {
        info!("Applying migration v6: Regex Chapter Cleaning");
        apply_migration(conn, 6, MIGRATION_V6)?;
    }

    if current_version < 7 {
        info!("Applying migration v7: Chapter Lock");
        apply_migration(conn, 7, MIGRATION_V7)?;
    }
    
    if current_version < 8 {
        info!("Applying migration v8: Fix Tasks Table Schema");
        // Check if columns exist before applying
        let has_retries: bool = conn.query_row(
            "SELECT COUNT(*) FROM pragma_table_info('tasks') WHERE name = 'retries'",
            [],
            |row| row.get(0),
        ).unwrap_or(0) > 0;

        let has_max_retries: bool = conn.query_row(
            "SELECT COUNT(*) FROM pragma_table_info('tasks') WHERE name = 'max_retries'",
            [],
            |row| row.get(0),
        ).unwrap_or(0) > 0;

        if !has_retries {
            info!("Adding missing column 'retries' to tasks table");
            conn.execute("ALTER TABLE tasks ADD COLUMN retries INTEGER DEFAULT 0", []).map_err(TingError::DatabaseError)?;
        }

        if !has_max_retries {
             info!("Adding missing column 'max_retries' to tasks table");
             conn.execute("ALTER TABLE tasks ADD COLUMN max_retries INTEGER DEFAULT 3", []).map_err(TingError::DatabaseError)?;
        }

        // Update version manually since we are not using apply_migration for conditional logic
        // Use INSERT OR IGNORE just in case, though the version check above should prevent duplicates
        conn.execute("INSERT OR IGNORE INTO schema_migrations (version) VALUES (8)", []).map_err(TingError::DatabaseError)?;
        info!("Migration v8 applied successfully");
    }

    info!("Database migrations completed successfully");
    Ok(())
}

/// Run migrations with automatic backup
///
/// This function creates a backup before applying migrations.
/// If any migration fails, it restores from the backup.
pub fn run_migrations_with_backup(db_path: &Path) -> Result<()> {
    info!("Running database migrations with automatic backup");
    
    // Create backup before migration
    let backup_path = create_migration_backup(db_path)?;
    info!("Created migration backup at: {}", backup_path.display());
    
    // Open connection and run migrations
    let mut conn = Connection::open(db_path)
        .map_err(|e| TingError::DatabaseError(e))?;
    
    match run_migrations(&mut conn) {
        Ok(_) => {
            info!("Migrations completed successfully, keeping backup");
            Ok(())
        }
        Err(e) => {
            error!("Migration failed: {}, restoring from backup", e);
            drop(conn); // Close connection before restoring
            
            // Restore from backup
            restore_from_backup(&backup_path, db_path)?;
            info!("Database restored from backup");
            
            Err(e)
        }
    }
}

/// Create a backup of the database before migration
fn create_migration_backup(db_path: &Path) -> Result<PathBuf> {
    let timestamp = Local::now().format("%Y%m%d_%H%M%S");
    let backup_dir = db_path.parent()
        .ok_or_else(|| TingError::ConfigError("Invalid database path".to_string()))?
        .join("backups");
    
    // Create backup directory if it doesn't exist
    fs::create_dir_all(&backup_dir)
        .map_err(|e| TingError::IoError(e))?;
    
    let backup_path = backup_dir.join(format!("migration_backup_{}.db", timestamp));
    
    // Copy database file
    fs::copy(db_path, &backup_path)
        .map_err(|e| TingError::IoError(e))?;
    
    Ok(backup_path)
}

/// Restore database from backup
fn restore_from_backup(backup_path: &Path, db_path: &Path) -> Result<()> {
    fs::copy(backup_path, db_path)
        .map_err(|e| TingError::IoError(e))?;
    
    Ok(())
}

/// Rollback to a specific version
///
/// This function rolls back the database to a specific version by restoring from a backup.
/// Note: This requires a backup file to exist for the target version.
pub fn rollback_to_version(db_path: &Path, target_version: i64, backup_path: &Path) -> Result<()> {
    info!("Rolling back database to version {}", target_version);
    
    // Verify backup exists
    if !backup_path.exists() {
        return Err(TingError::ConfigError(
            format!("Backup file not found: {}", backup_path.display())
        ));
    }
    
    // Restore from backup
    restore_from_backup(backup_path, db_path)?;
    
    info!("Database rolled back to version {}", target_version);
    Ok(())
}

/// Apply a single migration
fn apply_migration(conn: &mut Connection, version: i64, sql: &str) -> Result<()> {
    // Start transaction
    let tx = conn.transaction()
        .map_err(|e| TingError::DatabaseError(e))?;
    
    // Execute migration SQL
    tx.execute_batch(sql)
        .map_err(|e| {
            warn!("Migration v{} failed: {}", version, e);
            TingError::DatabaseError(e)
        })?;
    
    // Record migration
    tx.execute(
        "INSERT INTO schema_migrations (version) VALUES (?)",
        [version],
    ).map_err(|e| TingError::DatabaseError(e))?;
    
    // Commit transaction
    tx.commit()
        .map_err(|e| TingError::DatabaseError(e))?;
    
    info!("Migration v{} applied successfully", version);
    Ok(())
}