//! Database migrations
//!
//! This module provides database schema migration functionality with automatic backup and rollback.

use crate::core::error::{Result, TingError};
use chrono::Local;
use rusqlite::Connection;
use std::fs;
use std::path::{Path, PathBuf};
use tracing::{error, info, warn};

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
    UNIQUE(user_id, book_id, chapter_id)
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

/// Ninth schema migration (version 9)
const MIGRATION_V9: &str = r#"
-- Series table
CREATE TABLE IF NOT EXISTS series (
    id TEXT PRIMARY KEY,
    library_id TEXT NOT NULL,
    title TEXT NOT NULL,
    author TEXT,
    narrator TEXT,
    cover_url TEXT,
    description TEXT,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (library_id) REFERENCES libraries(id) ON DELETE CASCADE
);

-- Series books junction table
CREATE TABLE IF NOT EXISTS series_books (
    series_id TEXT NOT NULL,
    book_id TEXT NOT NULL,
    book_order INTEGER NOT NULL,
    PRIMARY KEY (series_id, book_id),
    FOREIGN KEY (series_id) REFERENCES series(id) ON DELETE CASCADE,
    FOREIGN KEY (book_id) REFERENCES books(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_series_library_id ON series(library_id);
CREATE INDEX IF NOT EXISTS idx_series_books_series_id ON series_books(series_id);
"#;

/// Tenth schema migration (version 10)
const MIGRATION_V10: &str = r#"
-- Add genre to books
ALTER TABLE books ADD COLUMN genre TEXT;
"#;

/// Eleventh schema migration (version 11)
const MIGRATION_V11: &str = r#"
-- Fix progress table to track progress per chapter instead of per book
CREATE TABLE IF NOT EXISTS progress_new (
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
    UNIQUE(user_id, book_id, chapter_id)
);

INSERT INTO progress_new SELECT * FROM progress;
DROP TABLE progress;
ALTER TABLE progress_new RENAME TO progress;

CREATE INDEX IF NOT EXISTS idx_progress_user_id ON progress(user_id);
CREATE INDEX IF NOT EXISTS idx_progress_updated_at ON progress(user_id, updated_at);
"#;

/// Twelfth schema migration (version 12)
const MIGRATION_V12: &str = r#"
-- Add year field to books
ALTER TABLE books ADD COLUMN year INTEGER;
CREATE INDEX IF NOT EXISTS idx_books_year ON books(year);
"#;

/// Thirteenth schema migration (version 13)
const MIGRATION_V13: &str = r#"
-- System settings table for JWT key rotation
CREATE TABLE IF NOT EXISTS system_settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
);
"#;

/// Fourteenth schema migration (version 14)
const MIGRATION_V14: &str = r#"
-- Remove unused plugin persistence tables. Runtime plugins are loaded from metadata files.
DROP TABLE IF EXISTS merge_suggestions;
DROP TABLE IF EXISTS plugin_dependencies;
DROP TABLE IF EXISTS plugin_registry;
"#;

/// Fifteenth schema migration (version 15)
const MIGRATION_V15: &str = r#"
-- User playlists
CREATE TABLE IF NOT EXISTS playlists (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL,
    title TEXT NOT NULL,
    description TEXT,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);

-- Playlist books junction table
CREATE TABLE IF NOT EXISTS playlist_books (
    playlist_id TEXT NOT NULL,
    book_id TEXT NOT NULL,
    book_order INTEGER NOT NULL,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (playlist_id, book_id),
    FOREIGN KEY (playlist_id) REFERENCES playlists(id) ON DELETE CASCADE,
    FOREIGN KEY (book_id) REFERENCES books(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_playlists_user_id ON playlists(user_id);
CREATE INDEX IF NOT EXISTS idx_playlist_books_playlist_id ON playlist_books(playlist_id);
CREATE INDEX IF NOT EXISTS idx_playlist_books_book_id ON playlist_books(book_id);
"#;

/// Sixteenth schema migration (version 16)
const MIGRATION_V16: &str = r#"
-- Durable listening statistics. These records are not deleted when users clear visible history.
CREATE TABLE IF NOT EXISTS listening_events (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL,
    book_id TEXT NOT NULL,
    chapter_id TEXT,
    position REAL DEFAULT 0,
    duration REAL,
    listen_seconds REAL DEFAULT 0,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE,
    FOREIGN KEY (book_id) REFERENCES books(id) ON DELETE CASCADE,
    FOREIGN KEY (chapter_id) REFERENCES chapters(id) ON DELETE SET NULL
);

INSERT OR IGNORE INTO listening_events (
    id,
    user_id,
    book_id,
    chapter_id,
    position,
    duration,
    listen_seconds,
    created_at
)
SELECT
    'legacy-' || id,
    user_id,
    book_id,
    chapter_id,
    position,
    duration,
    CASE WHEN position > 0 THEN position ELSE 0 END,
    updated_at
FROM progress;

CREATE INDEX IF NOT EXISTS idx_listening_events_user_id ON listening_events(user_id);
CREATE INDEX IF NOT EXISTS idx_listening_events_book_id ON listening_events(book_id);
CREATE INDEX IF NOT EXISTS idx_listening_events_created_at ON listening_events(created_at);
CREATE INDEX IF NOT EXISTS idx_listening_events_user_created_at ON listening_events(user_id, created_at);
"#;

/// Seventeenth schema migration (version 17)
const MIGRATION_V17: &str = r#"
-- Admin-configured webhook notification listeners.
CREATE TABLE IF NOT EXISTS notification_webhooks (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    url TEXT NOT NULL,
    enabled INTEGER NOT NULL DEFAULT 1,
    events TEXT NOT NULL DEFAULT '[]',
    secret TEXT,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_notification_webhooks_enabled ON notification_webhooks(enabled);
"#;

/// Eighteenth schema migration (version 18)
const MIGRATION_V18: &str = r#"
-- Playlist items preserve whether a playlist entry is a book or an entire series.
CREATE TABLE IF NOT EXISTS playlist_items (
    playlist_id TEXT NOT NULL,
    item_type TEXT NOT NULL CHECK(item_type IN ('book', 'series')),
    item_id TEXT NOT NULL,
    item_order INTEGER NOT NULL,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (playlist_id, item_type, item_id),
    FOREIGN KEY (playlist_id) REFERENCES playlists(id) ON DELETE CASCADE
);

INSERT OR IGNORE INTO playlist_items (playlist_id, item_type, item_id, item_order, created_at)
SELECT playlist_id, 'book', book_id, book_order, created_at
FROM playlist_books;

CREATE INDEX IF NOT EXISTS idx_playlist_items_playlist_id ON playlist_items(playlist_id);
CREATE INDEX IF NOT EXISTS idx_playlist_items_item ON playlist_items(item_type, item_id);
"#;

/// Twentieth schema migration (version 20)
const MIGRATION_V20: &str = r#"
-- Speed up large chapter list paging and progress joins.
CREATE INDEX IF NOT EXISTS idx_chapters_book_extra_index ON chapters(book_id, is_extra, chapter_index);
CREATE INDEX IF NOT EXISTS idx_progress_user_chapter ON progress(user_id, chapter_id);
"#;

/// Twenty-first schema migration (version 21)
const MIGRATION_V21: &str = r#"
-- Allow each webhook to define its own request headers and body template.
ALTER TABLE notification_webhooks ADD COLUMN headers TEXT NOT NULL DEFAULT '{}';
ALTER TABLE notification_webhooks ADD COLUMN body_template TEXT NOT NULL DEFAULT '{{json:payload}}';
"#;

/// Twenty-second schema migration (version 22)
const MIGRATION_V22: &str = r#"
-- Keep chapter progress durable while allowing users to hide visible history.
ALTER TABLE progress ADD COLUMN history_hidden_at DATETIME;
CREATE INDEX IF NOT EXISTS idx_progress_user_visible_updated
ON progress(user_id, history_hidden_at, updated_at);
"#;

/// Twenty-third schema migration (version 23)
const MIGRATION_V23: &str = r#"
-- Store localizable task progress separately from legacy free-form messages.
ALTER TABLE tasks ADD COLUMN message_key TEXT;
ALTER TABLE tasks ADD COLUMN message_params TEXT;
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
        let has_retries: bool = conn
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('tasks') WHERE name = 'retries'",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0)
            > 0;

        let has_max_retries: bool = conn
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('tasks') WHERE name = 'max_retries'",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0)
            > 0;

        if !has_retries {
            info!("Adding missing column 'retries' to tasks table");
            conn.execute("ALTER TABLE tasks ADD COLUMN retries INTEGER DEFAULT 0", [])
                .map_err(TingError::DatabaseError)?;
        }

        if !has_max_retries {
            info!("Adding missing column 'max_retries' to tasks table");
            conn.execute(
                "ALTER TABLE tasks ADD COLUMN max_retries INTEGER DEFAULT 3",
                [],
            )
            .map_err(TingError::DatabaseError)?;
        }

        // Update version manually since we are not using apply_migration for conditional logic
        // Use INSERT OR IGNORE just in case, though the version check above should prevent duplicates
        conn.execute(
            "INSERT OR IGNORE INTO schema_migrations (version) VALUES (8)",
            [],
        )
        .map_err(TingError::DatabaseError)?;
        info!("Migration v8 applied successfully");
    }

    if current_version < 9 {
        info!("Applying migration v9: Series System");
        apply_migration(conn, 9, MIGRATION_V9)?;
    }

    if current_version < 10 {
        info!("Applying migration v10: Genre field");
        apply_migration(conn, 10, MIGRATION_V10)?;
    }

    if current_version < 11 {
        info!("Applying migration v11: Fix progress constraint");
        apply_migration(conn, 11, MIGRATION_V11)?;
    }

    if current_version < 12 {
        info!("Applying migration v12: Add year field");
        apply_migration(conn, 12, MIGRATION_V12)?;
    }

    if current_version < 13 {
        info!("Applying migration v13: System settings for JWT rotation");
        apply_migration(conn, 13, MIGRATION_V13)?;
    }

    if current_version < 14 {
        info!("Applying migration v14: Remove unused plugin persistence tables");
        apply_migration(conn, 14, MIGRATION_V14)?;
    }

    if current_version < 15 {
        info!("Applying migration v15: User playlists");
        apply_migration(conn, 15, MIGRATION_V15)?;
    }

    if current_version < 16 {
        info!("Applying migration v16: Durable listening statistics");
        apply_migration(conn, 16, MIGRATION_V16)?;
    }

    if current_version < 17 {
        info!("Applying migration v17: Notification webhooks");
        apply_migration(conn, 17, MIGRATION_V17)?;
    }

    if current_version < 18 {
        info!("Applying migration v18: Playlist typed items");
        apply_migration(conn, 18, MIGRATION_V18)?;
    }

    if current_version < 19 {
        info!("Applying migration v19: Remove playlist accent column");
        migrate_playlist_without_accent(conn)?;
    }

    if current_version < 20 {
        info!("Applying migration v20: Large chapter list indexes");
        apply_migration(conn, 20, MIGRATION_V20)?;
    }

    if current_version < 21 {
        info!("Applying migration v21: Configurable webhook requests");
        apply_migration(conn, 21, MIGRATION_V21)?;
    }

    if current_version < 22 {
        info!("Applying migration v22: Separated visible history from progress");
        apply_migration(conn, 22, MIGRATION_V22)?;
    }

    if current_version < 23 {
        info!("Applying migration v23: Task log localization keys");
        apply_migration(conn, 23, MIGRATION_V23)?;
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
    let mut conn = Connection::open(db_path).map_err(|e| TingError::DatabaseError(e))?;

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
    let backup_dir = db_path
        .parent()
        .ok_or_else(|| TingError::ConfigError("Invalid database path".to_string()))?
        .join("backups");

    // Create backup directory if it doesn't exist
    fs::create_dir_all(&backup_dir).map_err(|e| TingError::IoError(e))?;

    let backup_path = backup_dir.join(format!("migration_backup_{}.db", timestamp));

    // Copy database file
    fs::copy(db_path, &backup_path).map_err(|e| TingError::IoError(e))?;

    Ok(backup_path)
}

/// Restore database from backup
fn restore_from_backup(backup_path: &Path, db_path: &Path) -> Result<()> {
    fs::copy(backup_path, db_path).map_err(|e| TingError::IoError(e))?;

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
        return Err(TingError::ConfigError(format!(
            "Backup file not found: {}",
            backup_path.display()
        )));
    }

    // Restore from backup
    restore_from_backup(backup_path, db_path)?;

    info!("Database rolled back to version {}", target_version);
    Ok(())
}

/// Apply a single migration
fn apply_migration(conn: &mut Connection, version: i64, sql: &str) -> Result<()> {
    // Start transaction
    let tx = conn
        .transaction()
        .map_err(|e| TingError::DatabaseError(e))?;

    // Execute migration SQL
    tx.execute_batch(sql).map_err(|e| {
        warn!("Migration v{} failed: {}", version, e);
        TingError::DatabaseError(e)
    })?;

    // Record migration
    tx.execute(
        "INSERT INTO schema_migrations (version) VALUES (?)",
        [version],
    )
    .map_err(|e| TingError::DatabaseError(e))?;

    // Commit transaction
    tx.commit().map_err(|e| TingError::DatabaseError(e))?;

    info!("Migration v{} applied successfully", version);
    Ok(())
}

fn migrate_playlist_without_accent(conn: &mut Connection) -> Result<()> {
    let has_accent: bool = conn
        .query_row(
            "SELECT COUNT(*) FROM pragma_table_info('playlists') WHERE name = 'accent'",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0)
        > 0;

    if !has_accent {
        conn.execute(
            "INSERT OR IGNORE INTO schema_migrations (version) VALUES (19)",
            [],
        )
        .map_err(TingError::DatabaseError)?;
        info!("Migration v19 applied successfully");
        return Ok(());
    }

    conn.execute_batch("PRAGMA foreign_keys = OFF;")
        .map_err(TingError::DatabaseError)?;

    let migration_result = (|| -> Result<()> {
        conn.execute_batch(
            r#"
CREATE TABLE playlists_new (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL,
    title TEXT NOT NULL,
    description TEXT,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);

INSERT INTO playlists_new (id, user_id, title, description, created_at, updated_at)
SELECT id, user_id, title, description, created_at, updated_at
FROM playlists;

DROP TABLE playlists;
ALTER TABLE playlists_new RENAME TO playlists;
CREATE INDEX IF NOT EXISTS idx_playlists_user_id ON playlists(user_id);
"#,
        )
        .map_err(TingError::DatabaseError)?;
        conn.execute("INSERT INTO schema_migrations (version) VALUES (19)", [])
            .map_err(TingError::DatabaseError)?;
        Ok(())
    })();

    conn.execute_batch("PRAGMA foreign_keys = ON;")
        .map_err(TingError::DatabaseError)?;
    migration_result?;

    info!("Migration v19 applied successfully");
    Ok(())
}
