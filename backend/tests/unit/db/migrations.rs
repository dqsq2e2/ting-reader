use super::*;

#[test]
fn migration_v31_adds_embedded_chapter_time_ranges() {
    let mut conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(MIGRATION_TABLE).unwrap();
    conn.execute_batch(
        "CREATE TABLE chapters (
            id TEXT PRIMARY KEY,
            book_id TEXT,
            title TEXT,
            path TEXT NOT NULL,
            duration INTEGER,
            chapter_index INTEGER,
            is_extra INTEGER,
            hash TEXT,
            created_at TEXT
        );
        INSERT INTO chapters (id, path) VALUES ('legacy', 'book.m4b');",
    )
    .unwrap();

    apply_migration(&mut conn, 31, MIGRATION_V31).unwrap();

    let (start, end): (Option<i64>, Option<i64>) = conn
        .query_row(
            "SELECT start_time, end_time FROM chapters WHERE id = 'legacy'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!((start, end), (None, None));

    conn.execute(
        "INSERT INTO chapters (id, path, start_time, end_time) VALUES ('c1', 'book.m4b', 0, 38173)",
        [],
    )
    .unwrap();
    let (start, end): (i64, i64) = conn
        .query_row(
            "SELECT start_time, end_time FROM chapters WHERE id = 'c1'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!((start, end), (0, 38_173));
}

#[test]
fn migration_v30_moves_legacy_title_flags_into_priority_without_losing_settings() {
    let mut conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(MIGRATION_TABLE).unwrap();
    conn.execute_batch(
        "CREATE TABLE libraries (id TEXT PRIMARY KEY, type TEXT, scraper_config TEXT);
         CREATE TABLE chapters (id TEXT PRIMARY KEY, book_id TEXT, path TEXT NOT NULL);
         CREATE TABLE tasks (id TEXT PRIMARY KEY);
         INSERT INTO schema_migrations (version) VALUES (29);",
    )
    .unwrap();
    let cases = [
        (
            "default",
            "local",
            r#"{"cloud_mode":true}"#,
            Some(vec![
                "filename",
                "local_metadata",
                "audio_metadata",
                "scraper",
            ]),
        ),
        (
            "enabled",
            "webdav",
            r#"{"use_filename_as_title":true,"metadata_priority":["audio_metadata","local_metadata","scraper"],"cloud_mode":true}"#,
            Some(vec![
                "filename",
                "audio_metadata",
                "local_metadata",
                "scraper",
            ]),
        ),
        (
            "disabled",
            "local",
            r#"{"use_filename_as_title":false,"metadata_priority":["scraper","local_metadata","audio_metadata"],"cloud_mode":true}"#,
            Some(vec![
                "scraper",
                "local_metadata",
                "audio_metadata",
                "filename",
            ]),
        ),
        (
            "empty",
            "webdav",
            r#"{"use_filename_as_title":false,"metadata_priority":[],"cloud_mode":true}"#,
            Some(vec![
                "local_metadata",
                "audio_metadata",
                "scraper",
                "filename",
            ]),
        ),
        (
            "explicit",
            "local",
            r#"{"use_filename_as_title":true,"metadata_priority":["local_metadata","filename","scraper"],"cloud_mode":true}"#,
            Some(vec!["local_metadata", "filename", "scraper"]),
        ),
        ("rss", "rss", r#"{"scheduled_sync_enabled":true}"#, None),
        ("invalid", "local", "invalid JSON", None),
    ];
    for (id, kind, json, _) in &cases {
        conn.execute(
            "INSERT INTO libraries VALUES (?, ?, ?)",
            rusqlite::params![id, kind, json],
        )
        .unwrap();
    }
    for _ in 0..2 {
        run_migrations(&mut conn).unwrap();
        for (id, _, original, priority) in &cases {
            let saved: String = conn
                .query_row(
                    "SELECT scraper_config FROM libraries WHERE id = ?",
                    [id],
                    |row| row.get(0),
                )
                .unwrap();
            if let Some(priority) = priority {
                let value: serde_json::Value = serde_json::from_str(&saved).unwrap();
                assert_eq!(
                    value["metadata_priority"],
                    serde_json::json!(priority),
                    "{id}"
                );
                assert_eq!(value["cloud_mode"], true);
                assert!(value.get("use_filename_as_title").is_none());
            } else {
                assert_eq!(&saved, original);
            }
        }
    }
}

#[test]
fn migration_v26_creates_persistent_scanner_state() {
    let mut conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(MIGRATION_TABLE).unwrap();
    conn.execute_batch("CREATE TABLE libraries (id TEXT PRIMARY KEY);")
        .unwrap();

    apply_migration(&mut conn, 26, MIGRATION_V26).unwrap();

    let table_exists: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'library_scan_state'",
                [],
                |row| row.get(0),
            )
            .unwrap();
    let index_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'index' AND name IN ('idx_library_scan_state_parent', 'idx_library_scan_state_kind')",
                [],
                |row| row.get(0),
            )
            .unwrap();

    assert_eq!(table_exists, 1);
    assert_eq!(index_count, 2);
}

#[test]
fn migration_v27_indexes_library_title_neighbourhoods() {
    let mut conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(MIGRATION_TABLE).unwrap();
    conn.execute_batch(
        "CREATE TABLE books (id TEXT PRIMARY KEY, library_id TEXT NOT NULL, title TEXT);",
    )
    .unwrap();

    apply_migration(&mut conn, 27, MIGRATION_V27).unwrap();

    let index_exists: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'index' AND name = 'idx_books_library_title'",
                [],
                |row| row.get(0),
            )
            .unwrap();
    assert_eq!(index_exists, 1);
}

#[test]
fn migration_v28_normalizes_heartbeat_counters() {
    let mut conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(MIGRATION_TABLE).unwrap();
    conn.execute_batch(
        r#"
CREATE TABLE listening_events (progress_updates INTEGER NOT NULL DEFAULT 0);
CREATE TABLE listening_totals (progress_updates INTEGER NOT NULL DEFAULT 0);
INSERT INTO listening_events (progress_updates) VALUES (110000), (7), (1);
INSERT INTO listening_totals (progress_updates) VALUES (110000), (0);
"#,
    )
    .unwrap();

    apply_migration(&mut conn, 28, MIGRATION_V28).unwrap();

    let event_updates: i64 = conn
        .query_row(
            "SELECT SUM(progress_updates) FROM listening_events",
            [],
            |row| row.get(0),
        )
        .unwrap();
    let total_updates: i64 = conn
        .query_row(
            "SELECT SUM(progress_updates) FROM listening_totals",
            [],
            |row| row.get(0),
        )
        .unwrap();

    assert_eq!(event_updates, 3);
    assert_eq!(total_updates, 0);
}

#[test]
fn migration_v25_compacts_raw_events_without_losing_totals() {
    let mut conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(MIGRATION_TABLE).unwrap();
    conn.execute_batch(
        r#"
CREATE TABLE users (id TEXT PRIMARY KEY);
CREATE TABLE books (id TEXT PRIMARY KEY);
CREATE TABLE chapters (id TEXT PRIMARY KEY);
INSERT INTO users (id) VALUES ('user-1');
INSERT INTO books (id) VALUES ('book-1');
INSERT INTO chapters (id) VALUES ('chapter-1');
CREATE TABLE listening_events (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL,
    book_id TEXT NOT NULL,
    chapter_id TEXT,
    position REAL DEFAULT 0,
    duration REAL,
    listen_seconds REAL DEFAULT 0,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP
);
"#,
    )
    .unwrap();

    let tx = conn.transaction().unwrap();
    {
        for index in 0..10_000 {
            let day = if index < 5_000 {
                "STRFTIME('%Y-%m-%dT10:00:00.000Z', 'now', '-1 day')"
            } else {
                "STRFTIME('%Y-%m-%dT10:00:00.000Z', 'now')"
            };
            tx.execute(
                    &format!(
                        "INSERT INTO listening_events (id, user_id, book_id, chapter_id, position, listen_seconds, created_at) \
                         VALUES (?, 'user-1', 'book-1', 'chapter-1', ?, 2, {day})"
                    ),
                    rusqlite::params![format!("event-{index}"), index],
                )
                .unwrap();
        }
    }
    tx.commit().unwrap();

    apply_migration(&mut conn, 25, MIGRATION_V25).unwrap();

    let daily_rows: i64 = conn
        .query_row("SELECT COUNT(*) FROM listening_events", [], |row| {
            row.get(0)
        })
        .unwrap();
    let (total_rows, updates, seconds): (i64, i64, f64) = conn
        .query_row(
            "SELECT COUNT(*), SUM(progress_updates), SUM(listen_seconds) FROM listening_totals",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap();

    assert_eq!(daily_rows, 2);
    assert_eq!(total_rows, 1);
    assert_eq!(updates, 10_000);
    assert_eq!(seconds, 20_000.0);
}
