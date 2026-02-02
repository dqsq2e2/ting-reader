const Database = require('better-sqlite3');
const path = require('path');

const dbPath = process.env.DB_PATH || path.join(__dirname, 'ting-reader.db');
const db = new Database(dbPath);

// Enable foreign keys for cascade deletes
db.pragma('foreign_keys = ON');

// Initialize tables
db.exec(`
  CREATE TABLE IF NOT EXISTS users (
    id TEXT PRIMARY KEY,
    username TEXT UNIQUE NOT NULL,
    password_hash TEXT NOT NULL,
    role TEXT DEFAULT 'user',
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP
  );

  CREATE TABLE IF NOT EXISTS libraries (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    type TEXT DEFAULT 'webdav', -- 'webdav' or 'local'
    url TEXT NOT NULL,
    username TEXT,
    password TEXT,
    root_path TEXT DEFAULT '/',
    last_scanned_at DATETIME,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP
  );

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
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (library_id) REFERENCES libraries(id) ON DELETE CASCADE
  );

  CREATE TABLE IF NOT EXISTS user_settings (
    user_id TEXT PRIMARY KEY,
    playback_speed REAL DEFAULT 1.0,
    sleep_timer_default INTEGER DEFAULT 0,
    auto_preload INTEGER DEFAULT 0, -- 0: false, 1: true
    theme TEXT DEFAULT 'system', -- 'light', 'dark', 'system'
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
  );

  CREATE TABLE IF NOT EXISTS chapters (
    id TEXT PRIMARY KEY,
    book_id TEXT NOT NULL,
    title TEXT,
    path TEXT NOT NULL,
    duration INTEGER,
    chapter_index INTEGER,
    is_extra INTEGER DEFAULT 0, -- 0: regular, 1: extra (番外)
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (book_id) REFERENCES books(id) ON DELETE CASCADE
  );

  CREATE TABLE IF NOT EXISTS tasks (
    id TEXT PRIMARY KEY,
    type TEXT NOT NULL,
    status TEXT DEFAULT 'pending', -- pending, processing, completed, failed
    payload TEXT,
    message TEXT,
    error TEXT,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
  );

  CREATE TABLE IF NOT EXISTS progress (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL,
    book_id TEXT NOT NULL,
    chapter_id TEXT NOT NULL,
    position INTEGER DEFAULT 0,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE,
    FOREIGN KEY (book_id) REFERENCES books(id) ON DELETE CASCADE,
    UNIQUE(user_id, chapter_id)
  );

  CREATE TABLE IF NOT EXISTS favorites (
    user_id TEXT NOT NULL,
    book_id TEXT NOT NULL,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (user_id, book_id),
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE,
    FOREIGN KEY (book_id) REFERENCES books(id) ON DELETE CASCADE
  );
`);

// Migrations
try {
  db.prepare('ALTER TABLE books ADD COLUMN narrator TEXT').run();
} catch (e) {}

try {
  db.prepare('ALTER TABLE books ADD COLUMN theme_color TEXT').run();
} catch (e) {}

try {
  db.prepare("ALTER TABLE libraries ADD COLUMN type TEXT DEFAULT 'webdav'").run();
} catch (e) {}

try {
  db.prepare("ALTER TABLE chapters ADD COLUMN is_extra INTEGER DEFAULT 0").run();
} catch (e) {}

try {
  db.prepare("ALTER TABLE books ADD COLUMN tags TEXT").run();
} catch (e) {}

// Migration for per-chapter progress
const tableInfo = db.prepare("PRAGMA table_info(progress)").all();
const hasChapterId = tableInfo.some(col => col.name === 'chapter_id');
const indexInfo = db.prepare("PRAGMA index_list(progress)").all();
const hasOldUnique = indexInfo.some(idx => idx.unique === 1 && idx.name.includes('progress') && !idx.name.includes('chapter_id'));

if (hasOldUnique || !hasChapterId) {
  console.log('Migrating progress table for per-chapter tracking...');
  db.transaction(() => {
    // 1. Create temporary table
    db.exec(`
      CREATE TABLE progress_new (
        id TEXT PRIMARY KEY,
        user_id TEXT NOT NULL,
        book_id TEXT NOT NULL,
        chapter_id TEXT NOT NULL,
        position INTEGER DEFAULT 0,
        updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
        FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE,
        FOREIGN KEY (book_id) REFERENCES books(id) ON DELETE CASCADE,
        UNIQUE(user_id, chapter_id)
      )
    `);
    
    // 2. Copy data (if chapter_id was already there)
    try {
      db.exec(`INSERT INTO progress_new (id, user_id, book_id, chapter_id, position, updated_at) 
               SELECT id, user_id, book_id, chapter_id, position, updated_at FROM progress WHERE chapter_id IS NOT NULL`);
    } catch (e) {
      console.log('No valid data to migrate from progress table');
    }
    
    // 3. Swap tables
    db.exec("DROP TABLE progress");
    db.exec("ALTER TABLE progress_new RENAME TO progress");
  })();
}

module.exports = db;
