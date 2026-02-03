require('dotenv').config();
const express = require('express');
const path = require('path');
const fs = require('fs');
const cors = require('cors');
const bcrypt = require('bcryptjs');
const jwt = require('jsonwebtoken');
const { v4: uuidv4 } = require('uuid');
const db = require('./db');
const { scanLibrary } = require('./scanner');
const mm = require('music-metadata');
const mime = require('mime-types');
const { getStorageClient } = require('./storage-client');
const { decryptXM } = require('./xm-decryptor');
const { calculateThemeColor } = require('./color-utils');
const { scrapeXimalaya } = require('./scraper');
const cacheManager = require('./cache-manager');

const app = express();
const PORT = process.env.PORT || 3000;
const JWT_SECRET = process.env.JWT_SECRET || 'ting-reader-secret-key';
const STORAGE_ROOT = path.join(__dirname, 'storage');

// Ensure storage root exists
if (!fs.existsSync(STORAGE_ROOT)) {
  fs.mkdirSync(STORAGE_ROOT, { recursive: true });
}

app.use(cors());
app.use(express.json());
app.use(express.static(path.join(__dirname, 'public')));

// Auth Middleware
const authenticate = (req, res, next) => {
  // Allow OPTIONS preflight requests
  if (req.method === 'OPTIONS') {
    return next();
  }

  const authHeader = req.headers.authorization;
  const queryToken = req.query.token;
  
  if (!authHeader && !queryToken) return res.status(401).json({ error: 'No token provided' });

  const token = authHeader ? authHeader.split(' ')[1] : queryToken;
  try {
    const decoded = jwt.verify(token, JWT_SECRET);
    req.userId = decoded.userId;
    
    // Get user info and verify existence
    const user = db.prepare('SELECT role FROM users WHERE id = ?').get(req.userId);
    if (!user) {
      return res.status(401).json({ error: 'User no longer exists' });
    }
    
    req.userRole = user.role;
    next();
  } catch (err) {
    res.status(401).json({ error: 'Invalid token' });
  }
};

const isAdmin = (req, res, next) => {
  if (req.userRole !== 'admin') {
    return res.status(403).json({ error: 'Admin access required' });
  }
  next();
};

// --- Auth Routes ---
app.post('/api/auth/register', async (req, res) => {
  const { username, password } = req.body;
  const count = db.prepare('SELECT count(*) as count FROM users').get().count;
  const role = count === 0 ? 'admin' : 'user';
  
  try {
    const passwordHash = await bcrypt.hash(password, 10);
    const userId = uuidv4();
    db.prepare('INSERT INTO users (id, username, password_hash, role) VALUES (?, ?, ?, ?)').run(userId, username, passwordHash, role);
    res.json({ success: true });
  } catch (err) {
    res.status(400).json({ error: 'Username already exists' });
  }
});

app.post('/api/auth/login', async (req, res) => {
  const { username, password } = req.body;
  const user = db.prepare('SELECT * FROM users WHERE username = ?').get(username);
  
  if (!user || !(await bcrypt.compare(password, user.password_hash))) {
    return res.status(401).json({ error: 'Invalid credentials' });
  }
  
  const token = jwt.sign({ userId: user.id }, JWT_SECRET, { expiresIn: '7d' });
  res.json({ user: { id: user.id, username: user.username, role: user.role }, token });
});

// --- Library Routes ---
app.get('/api/libraries', authenticate, isAdmin, (req, res) => {
  const libraries = db.prepare('SELECT * FROM libraries').all();
  res.json(libraries);
});

app.get('/api/storage/folders', authenticate, isAdmin, async (req, res) => {
  const { subPath = '' } = req.query;
  const targetPath = path.join(STORAGE_ROOT, subPath);
  
  // Security check: ensure targetPath is within STORAGE_ROOT
  if (!targetPath.startsWith(STORAGE_ROOT)) {
    return res.status(403).json({ error: 'Access denied' });
  }

  try {
    const items = await fs.promises.readdir(targetPath, { withFileTypes: true });
    const folders = items
      .filter(item => item.isDirectory())
      .map(item => ({
        name: item.name,
        path: path.join(subPath, item.name).replace(/\\/g, '/')
      }));
    res.json(folders);
  } catch (err) {
    res.status(500).json({ error: err.message });
  }
});

app.post('/api/libraries', authenticate, isAdmin, (req, res) => {
  const { name, type = 'webdav', url = '', username, password, root_path = '/' } = req.body;
  
  // Security check for local type: ensure url is relative and within STORAGE_ROOT
  if (type === 'local') {
    const fullPath = path.join(STORAGE_ROOT, url);
    if (!fullPath.startsWith(STORAGE_ROOT)) {
      return res.status(400).json({ error: 'Invalid local path' });
    }
  }

  const id = uuidv4();
  db.prepare(`
    INSERT INTO libraries (id, name, type, url, username, password, root_path)
    VALUES (?, ?, ?, ?, ?, ?, ?)
  `).run(id, name, type, url, username, password, root_path);

  // Auto-scan after adding library
  const taskId = uuidv4();
  db.prepare("INSERT INTO tasks (id, type, status, payload) VALUES (?, ?, ?, ?)").run(taskId, 'scan', 'pending', JSON.stringify({ libraryId: id }));
  
  scanLibrary(id, taskId).catch(err => {
    console.error('Auto-scan error:', err);
    db.prepare("UPDATE tasks SET status = ?, error = ? WHERE id = ?").run('failed', err.message, taskId);
  });

  res.json({ id, taskId });
});

app.delete('/api/libraries/:id', authenticate, isAdmin, (req, res) => {
  db.prepare('DELETE FROM libraries WHERE id = ?').run(req.params.id);
  res.json({ success: true });
});

app.patch('/api/chapters/:id', authenticate, (req, res) => {
  const { duration } = req.body;
  if (duration) {
    db.prepare('UPDATE chapters SET duration = ? WHERE id = ?').run(Math.round(duration), req.params.id);
  }
  res.json({ success: true });
});

app.patch('/api/libraries/:id', authenticate, isAdmin, (req, res) => {
  const { name, type, url, username, password, root_path } = req.body;
  db.prepare(`
    UPDATE libraries SET 
      name = COALESCE(?, name),
      type = COALESCE(?, type),
      url = COALESCE(?, url),
      username = COALESCE(?, username),
      password = COALESCE(?, password),
      root_path = COALESCE(?, root_path)
    WHERE id = ?
  `).run(name, type, url, username, password, root_path, req.params.id);
  res.json({ success: true });
});

app.post('/api/libraries/:id/scan', authenticate, isAdmin, async (req, res) => {
  const library = db.prepare('SELECT * FROM libraries WHERE id = ?').get(req.params.id);
  if (!library) return res.status(404).json({ error: 'Library not found' });

  const taskId = uuidv4();
  db.prepare("INSERT INTO tasks (id, type, status, payload) VALUES (?, ?, ?, ?)").run(taskId, 'scan', 'pending', JSON.stringify({ libraryId: library.id }));

  scanLibrary(library.id, taskId).catch(err => {
    console.error('Scan error:', err);
    db.prepare("UPDATE tasks SET status = ?, error = ? WHERE id = ?").run('failed', err.message, taskId);
  });

  res.json({ taskId });
});

// --- Book Routes ---
app.get('/api/books', authenticate, (req, res) => {
  const { search, tag } = req.query;
  let query = 'SELECT * FROM books';
  const params = [];

  const conditions = [];
  if (search) {
    conditions.push('(title LIKE ? OR author LIKE ? OR description LIKE ? OR narrator LIKE ?)');
    const searchParam = `%${search}%`;
    params.push(searchParam, searchParam, searchParam, searchParam);
  }
  
  if (tag) {
    conditions.push('tags LIKE ?');
    params.push(`%${tag}%`);
  }

  if (conditions.length > 0) {
    query += ' WHERE ' + conditions.join(' AND ');
  }

  query += ' ORDER BY created_at DESC';
  
  const books = db.prepare(query).all(...params);
  res.json(books);
});

app.get('/api/tags', authenticate, (req, res) => {
  try {
    const books = db.prepare("SELECT tags FROM books WHERE tags IS NOT NULL AND tags != ''").all();
    const tagSet = new Set();
    books.forEach(book => {
      if (typeof book.tags === 'string') {
        book.tags.split(/[,，]/).forEach(t => {
          const trimmed = t.trim();
          if (trimmed) tagSet.add(trimmed);
        });
      }
    });
    res.json(Array.from(tagSet).sort());
  } catch (err) {
    console.error('Failed to get tags:', err);
    res.status(500).json({ error: 'Failed to get tags', details: err.message });
  }
});

app.get('/api/books/:id', authenticate, (req, res) => {
  const book = db.prepare(`
    SELECT b.*, l.type as library_type, (f.user_id IS NOT NULL) as is_favorite
    FROM books b
    JOIN libraries l ON b.library_id = l.id
    LEFT JOIN favorites f ON b.id = f.book_id AND f.user_id = ?
    WHERE b.id = ?
  `).get(req.userId, req.params.id);
  
  if (!book) return res.status(404).json({ error: 'Book not found' });
  
  res.json(book);
});

app.delete('/api/books/:id', authenticate, isAdmin, async (req, res) => {
  const { deleteFiles } = req.query;
  const bookId = req.params.id;
  
  const book = db.prepare('SELECT * FROM books WHERE id = ?').get(bookId);
  if (!book) return res.status(404).json({ error: 'Book not found' });
  
  const library = db.prepare('SELECT * FROM libraries WHERE id = ?').get(book.library_id);
  
  try {
    // 1. Clear cache files regardless of source deletion
    await cacheManager.clearCacheForBook(bookId, db);
    
    // 2. Delete source files if requested and library is local
    if (deleteFiles === 'true' && library.type === 'local') {
      const fullPath = path.join(STORAGE_ROOT, book.path);
      if (fs.existsSync(fullPath) && fullPath.startsWith(STORAGE_ROOT)) {
        console.log(`Deleting local source files: ${fullPath}`);
        // Be careful: use fs.rmSync or similar for recursive deletion
        fs.rmSync(fullPath, { recursive: true, force: true });
      }
    }
    
    // 3. Delete from database (FK will handle chapters and progress)
    db.prepare('DELETE FROM books WHERE id = ?').run(bookId);
    
    res.json({ success: true });
  } catch (err) {
    console.error('Delete book error:', err);
    res.status(500).json({ error: 'Failed to delete book', details: err.message });
  }
});

app.get('/api/books/:id/chapters', authenticate, (req, res) => {
  const chapters = db.prepare(`
    SELECT c.*, p.position as progress_position, p.updated_at as progress_updated_at
    FROM chapters c
    LEFT JOIN progress p ON c.id = p.chapter_id AND p.user_id = ?
    WHERE c.book_id = ? 
    ORDER BY c.is_extra ASC, c.chapter_index ASC
  `).all(req.userId, req.params.id);
  res.json(chapters);
});

// --- Favorite Routes ---
app.get('/api/favorites', authenticate, (req, res) => {
  const books = db.prepare(`
    SELECT b.*, 1 as is_favorite
    FROM books b
    JOIN favorites f ON b.id = f.book_id
    WHERE f.user_id = ?
    ORDER BY f.created_at DESC
  `).all(req.userId);
  res.json(books);
});

app.post('/api/favorites/:bookId', authenticate, (req, res) => {
  try {
    db.prepare('INSERT INTO favorites (user_id, book_id) VALUES (?, ?)').run(req.userId, req.params.bookId);
    res.json({ success: true });
  } catch (err) {
    res.json({ success: true }); // Already favorited
  }
});

// --- User Management Routes ---
app.get('/api/users', authenticate, isAdmin, (req, res) => {
  const users = db.prepare('SELECT id, username, role, created_at FROM users').all();
  res.json(users);
});

app.post('/api/users', authenticate, isAdmin, async (req, res) => {
  const { username, password, role } = req.body;
  
  if (!username || !password) {
    return res.status(400).json({ error: '用户名和密码是必填项' });
  }

  try {
    const passwordHash = await bcrypt.hash(password, 10);
    const userId = uuidv4();
    db.prepare('INSERT INTO users (id, username, password_hash, role) VALUES (?, ?, ?, ?)').run(userId, username, passwordHash, role || 'user');
    res.json({ success: true, id: userId });
  } catch (err) {
    res.status(400).json({ error: '用户名已存在' });
  }
});

app.delete('/api/users/:id', authenticate, isAdmin, (req, res) => {
  if (req.params.id === req.userId) {
    return res.status(400).json({ error: 'Cannot delete your own account' });
  }
  db.prepare('DELETE FROM users WHERE id = ?').run(req.params.id);
  res.json({ success: true });
});

app.patch('/api/users/:id', authenticate, isAdmin, async (req, res) => {
  const { username, password, role } = req.body;
  const updates = [];
  const params = [];

  if (username) {
    updates.push('username = ?');
    params.push(username);
  }
  if (password) {
    const passwordHash = await bcrypt.hash(password, 10);
    updates.push('password_hash = ?');
    params.push(passwordHash);
  }
  if (role) {
    updates.push('role = ?');
    params.push(role);
  }

  if (updates.length === 0) {
    return res.status(400).json({ error: '没有提供更新内容' });
  }

  params.push(req.params.id);
  try {
    db.prepare(`UPDATE users SET ${updates.join(', ')} WHERE id = ?`).run(...params);
    res.json({ success: true });
  } catch (err) {
    console.error('Admin update user error:', err);
    if (err.code === 'SQLITE_CONSTRAINT') {
      return res.status(400).json({ error: '用户名已存在' });
    }
    res.status(500).json({ error: '服务器内部错误' });
  }
});

app.patch('/api/me', authenticate, async (req, res) => {
  const { username, password } = req.body;
  const updates = [];
  const params = [];

  if (username) {
    updates.push('username = ?');
    params.push(username);
  }
  if (password) {
    const passwordHash = await bcrypt.hash(password, 10);
    updates.push('password_hash = ?');
    params.push(passwordHash);
  }

  if (updates.length === 0) {
    return res.status(400).json({ error: '没有提供更新内容' });
  }

  params.push(req.userId);
  try {
    db.prepare(`UPDATE users SET ${updates.join(', ')} WHERE id = ?`).run(...params);
    res.json({ success: true });
  } catch (err) {
    console.error('Update self error:', err);
    if (err.code === 'SQLITE_CONSTRAINT') {
      return res.status(400).json({ error: '用户名已存在' });
    }
    res.status(500).json({ error: '服务器内部错误' });
  }
});

app.delete('/api/favorites/:bookId', authenticate, (req, res) => {
  db.prepare('DELETE FROM favorites WHERE user_id = ? AND book_id = ?').run(req.userId, req.params.bookId);
  res.json({ success: true });
});

app.patch('/api/books/:id', authenticate, isAdmin, async (req, res) => {
  const { title, author, narrator, description, cover_url, skip_intro, skip_outro, tags } = req.body;
  
  // If cover_url changed, recalculate theme color
  let theme_color = undefined;
  if (cover_url !== undefined) {
    const book = db.prepare('SELECT library_id FROM books WHERE id = ?').get(req.params.id);
    const library = db.prepare('SELECT * FROM libraries WHERE id = ?').get(book.library_id);
    const client = getStorageClient(library);
    theme_color = await calculateThemeColor(cover_url, client);
  }

  db.prepare(`
    UPDATE books SET 
      title = COALESCE(?, title),
      author = COALESCE(?, author),
      narrator = COALESCE(?, narrator),
      description = COALESCE(?, description),
      cover_url = COALESCE(?, cover_url),
      theme_color = COALESCE(?, theme_color),
      skip_intro = COALESCE(?, skip_intro),
      skip_outro = COALESCE(?, skip_outro),
      tags = COALESCE(?, tags)
    WHERE id = ?
  `).run(title, author, narrator, description, cover_url, theme_color, skip_intro, skip_outro, tags, req.params.id);
  res.json({ success: true });
});

// --- Task Routes ---
app.get('/api/tasks', authenticate, isAdmin, (req, res) => {
  const tasks = db.prepare('SELECT * FROM tasks ORDER BY created_at DESC LIMIT 50').all();
  res.json(tasks);
});

// --- Stats Routes ---
app.get('/api/stats', authenticate, (req, res) => {
  const stats = {
    total_books: db.prepare('SELECT count(*) as count FROM books').get().count,
    total_chapters: db.prepare('SELECT count(*) as count FROM chapters').get().count,
    total_duration: db.prepare('SELECT sum(duration) as sum FROM chapters').get().sum || 0,
    last_scan_time: db.prepare('SELECT max(last_scanned_at) as max FROM libraries').get().max
  };
  res.json(stats);
});

// --- User Settings Routes ---
app.get('/api/settings', authenticate, (req, res) => {
  const settings = db.prepare('SELECT * FROM user_settings WHERE user_id = ?').get(req.userId);
  res.json(settings || { playback_speed: 1.0, sleep_timer_default: 0, auto_preload: 0, theme: 'system' });
});

app.post('/api/settings', authenticate, (req, res) => {
  const { playback_speed, sleep_timer_default, auto_preload, theme } = req.body;
  const autoPreloadInt = auto_preload ? 1 : 0;
  db.prepare(`
    INSERT INTO user_settings (user_id, playback_speed, sleep_timer_default, auto_preload, theme)
    VALUES (?, ?, ?, ?, ?)
    ON CONFLICT(user_id) DO UPDATE SET
      playback_speed = COALESCE(excluded.playback_speed, playback_speed),
      sleep_timer_default = COALESCE(excluded.sleep_timer_default, sleep_timer_default),
      auto_preload = COALESCE(excluded.auto_preload, auto_preload),
      theme = COALESCE(excluded.theme, theme)
  `).run(req.userId, playback_speed, sleep_timer_default, autoPreloadInt, theme);
  res.json({ success: true });
});

app.get('/api/scrape/ximalaya', authenticate, async (req, res) => {
  const { keyword } = req.query;
  if (!keyword) return res.status(400).json({ error: 'Keyword is required' });
  
  const result = await scrapeXimalaya(keyword);
  if (!result) return res.status(404).json({ error: 'No metadata found' });
  
  res.json(result);
});

// --- Progress Routes ---
app.get('/api/progress/recent', authenticate, (req, res) => {
  const recent = db.prepare(`
    SELECT p.*, b.title as book_title, b.cover_url, b.library_id, c.title as chapter_title, c.duration as chapter_duration
    FROM progress p
    JOIN books b ON p.book_id = b.id
    JOIN chapters c ON p.chapter_id = c.id
    WHERE p.id IN (
      SELECT id FROM progress 
      WHERE user_id = ? 
      GROUP BY book_id 
      HAVING MAX(updated_at)
    )
    ORDER BY p.updated_at DESC
    LIMIT 4
  `).all(req.userId);
  res.json(recent);
});

app.get('/api/progress/:bookId', authenticate, (req, res) => {
  const progress = db.prepare(`
    SELECT * FROM progress 
    WHERE user_id = ? AND book_id = ? 
    ORDER BY updated_at DESC 
    LIMIT 1
  `).get(req.userId, req.params.bookId);
  res.json(progress || { position: 0 });
});

app.post('/api/progress', authenticate, (req, res) => {
  const { bookId, chapterId, position } = req.body;
  try {
    // Verify book exists first to give a better error if needed, 
    // or just let the FK constraint catch it and handle the error.
    db.prepare(`
      INSERT INTO progress (id, user_id, book_id, chapter_id, position, updated_at)
      VALUES (?, ?, ?, ?, ?, CURRENT_TIMESTAMP)
      ON CONFLICT(user_id, chapter_id) DO UPDATE SET
        position = excluded.position,
        updated_at = CURRENT_TIMESTAMP
    `).run(uuidv4(), req.userId, bookId, chapterId, position);
    res.json({ success: true });
  } catch (err) {
    if (err.code === 'SQLITE_CONSTRAINT_FOREIGNKEY') {
      console.warn(`Progress update failed: Book ${bookId} or User ${req.userId} not found`);
      return res.status(400).json({ error: 'Book or User not found' });
    }
    console.error('Progress update error:', err);
    res.status(500).json({ error: 'Failed to update progress' });
  }
});

// --- Playback Routes ---
app.get('/api/stream/:chapterId', authenticate, async (req, res) => {
  try {
    const chapterId = req.params.chapterId;
    
    // Comprehensive CORS and Security Headers for Media
    res.setHeader('Access-Control-Allow-Origin', '*');
    res.setHeader('Access-Control-Allow-Methods', 'GET, OPTIONS');
    res.setHeader('Access-Control-Allow-Headers', 'Range, Authorization, Content-Type');
    res.setHeader('Access-Control-Expose-Headers', 'Content-Range, Content-Length, Accept-Ranges');
    res.setHeader('Cross-Origin-Resource-Policy', 'cross-origin');
    res.setHeader('Accept-Ranges', 'bytes');
    
    if (req.method === 'OPTIONS') {
      return res.sendStatus(200);
    }

    // 1. Check cache first (for decrypted XM or preloaded files)
    if (cacheManager.isCached(chapterId)) {
      const cachePath = cacheManager.getCachePath(chapterId);
      const contentType = await cacheManager.getMimeType(chapterId);
      
      console.log(`Serving cached file for ${chapterId}, type: ${contentType}`);
      
      res.type(contentType);
      return res.sendFile(cachePath, {
        acceptRanges: true,
        lastModified: true,
        headers: {
          'Content-Type': contentType,
          'Cross-Origin-Resource-Policy': 'cross-origin',
          'Access-Control-Allow-Origin': '*',
          'X-Content-Type-Options': 'nosniff'
        }
      });
    }

    const chapter = db.prepare('SELECT * FROM chapters WHERE id = ?').get(chapterId);
    if (!chapter) return res.status(404).json({ error: 'Chapter not found' });

    const book = db.prepare('SELECT * FROM books WHERE id = ?').get(chapter.book_id);
    const library = db.prepare('SELECT * FROM libraries WHERE id = ?').get(book.library_id);
    
    const client = getStorageClient(library);
    const isXM = chapter.path.toLowerCase().endsWith('.xm');
    const isLocal = library.type === 'local';

    // Optimization: Local sources (non-XM) don't need caching and can be served directly
    if (isLocal && !isXM) {
      const fullPath = path.join(STORAGE_ROOT, chapter.path);
      if (fs.existsSync(fullPath)) {
        console.log(`Serving local file directly: ${fullPath}`);
        return res.sendFile(fullPath, {
          acceptRanges: true,
          headers: {
            'Cross-Origin-Resource-Policy': 'cross-origin',
            'Access-Control-Allow-Origin': '*',
            'X-Content-Type-Options': 'nosniff'
          }
        });
      }
    }

    if (isXM) {
      // XM Decryption Logic
      try {
        console.log(`Decrypting XM: ${chapter.path}`);
        
        // Use an AbortController to allow canceling the download if the client disconnects
        const controller = new AbortController();
        res.on('close', () => controller.abort());

        const content = await client.getFileContents(chapter.path, { 
          format: 'binary',
          signal: controller.signal 
        });
        const decrypted = await decryptXM(content);
        
        if (!decrypted || decrypted.length === 0) {
          throw new Error('Decryption resulted in empty data');
        }
        
        const cachePath = cacheManager.getCachePath(chapterId);
        await cacheManager.saveToCache(chapterId, decrypted);
        
        const contentType = await cacheManager.getMimeType(chapterId);
        
        res.type(contentType);
        return res.sendFile(cachePath, {
          acceptRanges: true,
          headers: {
            'Content-Type': contentType,
            'Cross-Origin-Resource-Policy': 'cross-origin',
            'Access-Control-Allow-Origin': '*',
            'X-Content-Type-Options': 'nosniff'
          }
        });
      } catch (decryptErr) {
        console.error('Decryption failed:', decryptErr);
        return res.status(500).json({ error: 'Decryption failed', details: decryptErr.message });
      }
    } else {
      // WebDAV Regular File Streaming
      try {
        const stat = await client.stat(chapter.path);
        const fileSize = stat.size;
        const range = req.headers.range;
        
        let contentType = mime.lookup(chapter.path) || 'audio/mpeg';
        if (chapter.path.toLowerCase().endsWith('.m4a')) {
          contentType = 'audio/mp4'; 
        }

        if (range) {
          const parts = range.replace(/bytes=/, "").split("-");
          const start = parseInt(parts[0], 10);
          const end = parts[1] ? parseInt(parts[1], 10) : fileSize - 1;
          
          if (start >= fileSize) {
            res.setHeader('Content-Range', `bytes */${fileSize}`);
            return res.status(416).send('Requested range not satisfiable');
          }

          const chunksize = (end - start) + 1;
          const stream = client.createReadStream(chapter.path, { range: { start, end } });
          
          console.log(`Streaming range ${start}-${end}/${fileSize} for ${chapter.path}`);
          
          res.writeHead(206, {
            'Content-Range': `bytes ${start}-${end}/${fileSize}`,
            'Accept-Ranges': 'bytes',
            'Content-Length': chunksize,
            'Content-Type': contentType,
            'Access-Control-Allow-Origin': '*',
            'Cross-Origin-Resource-Policy': 'cross-origin',
            'X-Content-Type-Options': 'nosniff'
          });
          
          stream.pipe(res);

          // Handle client disconnect
          res.on('close', () => {
            if (stream && stream.destroy) {
              stream.destroy();
            }
          });
          
          stream.on('error', (err) => {
            console.error(`Stream error for ${chapter.path}:`, err.message);
            if (!res.headersSent) {
              res.status(500).send('Streaming error');
            }
          });
        } else {
          console.log(`Streaming full file: ${chapter.path}`);
          res.writeHead(200, {
            'Content-Length': fileSize,
            'Content-Type': contentType,
            'Accept-Ranges': 'bytes',
            'Access-Control-Allow-Origin': '*',
            'Cross-Origin-Resource-Policy': 'cross-origin',
            'X-Content-Type-Options': 'nosniff'
          });
          const stream = client.createReadStream(chapter.path);
          stream.pipe(res);

          // Handle client disconnect
          res.on('close', () => {
            if (stream && stream.destroy) {
              stream.destroy();
            }
          });

          stream.on('error', (err) => {
            console.error(`Full stream error for ${chapter.path}:`, err.message);
            if (!res.headersSent) {
              res.status(500).send('Streaming error');
            }
          });
        }
      } catch (err) {
        console.error('WebDAV Stream error:', err);
        if (!res.headersSent) {
          res.status(500).send('Streaming error');
        }
      }
    }

    // Background: check if we should preload the next chapter
    const settings = db.prepare('SELECT auto_preload FROM user_settings WHERE user_id = ?').get(req.userId);
    if (settings && settings.auto_preload) {
      const nextChapter = db.prepare('SELECT id FROM chapters WHERE book_id = ? AND chapter_index = ?').get(chapter.book_id, chapter.chapter_index + 1);
      if (nextChapter && !cacheManager.isCached(nextChapter.id)) {
        preloadChapter(nextChapter.id, library);
      }
    }

  } catch (err) {
    console.error('Stream error:', err);
    if (!res.headersSent) {
      res.status(500).json({ error: 'Failed to stream audio' });
    }
  }
});

async function preloadChapter(chapterId, library) {
  try {
    const chapter = db.prepare('SELECT * FROM chapters WHERE id = ?').get(chapterId);
    if (!chapter) return;
    
    // Optimization: Local sources (non-XM) don't need preloading/caching
    const isXM = chapter.path.toLowerCase().endsWith('.xm');
    if (library.type === 'local' && !isXM) {
      console.log(`Skipping preload for local file: ${chapterId}`);
      return;
    }
    
    const client = getStorageClient(library);
    
    let data;
    if (isXM) {
      const content = await client.getFileContents(chapter.path, { format: 'binary' });
      data = await decryptXM(content);
    } else {
      data = await client.getFileContents(chapter.path, { format: 'binary' });
    }
    
    if (data) {
      await cacheManager.saveToCache(chapterId, data);
      console.log(`Preloaded chapter: ${chapterId}`);
    }
  } catch (err) {
    console.error('Preload failed:', err.message);
  }
}

app.post('/api/cache/:chapterId', authenticate, async (req, res) => {
  const chapterId = req.params.chapterId;
  if (cacheManager.isCached(chapterId)) return res.json({ success: true, message: 'Already cached' });

  const chapter = db.prepare('SELECT * FROM chapters WHERE id = ?').get(chapterId);
  if (!chapter) return res.status(404).json({ error: 'Chapter not found' });

  const book = db.prepare('SELECT * FROM books WHERE id = ?').get(chapter.book_id);
  const library = db.prepare('SELECT * FROM libraries WHERE id = ?').get(book.library_id);

  preloadChapter(chapterId, library);
  res.json({ success: true, message: 'Caching started' });
});

// --- Proxy Routes ---
app.get('/api/proxy/cover', authenticate, async (req, res) => {
  const { path, libraryId } = req.query;
  if (!path || !libraryId) return res.status(400).json({ error: 'Path and libraryId are required' });

  try {
    const library = db.prepare('SELECT * FROM libraries WHERE id = ?').get(libraryId);
    if (!library) return res.status(404).json({ error: 'Library not found' });

    const client = getStorageClient(library);

    // Handle embedded covers
    if (path === 'embedded://first-chapter') {
      // Find the first chapter of the book to extract cover from
      // Note: We need the book ID to find its chapters. 
      // The scanner uses this special path, so let's check if we can get book context.
      // Better approach: Since we don't have bookId here easily, let's allow passing bookId
      const { bookId } = req.query;
      if (!bookId) return res.status(400).json({ error: 'bookId required for embedded covers' });
      
      const firstChapter = db.prepare('SELECT path FROM chapters WHERE book_id = ? ORDER BY chapter_index ASC LIMIT 1').get(bookId);
      if (!firstChapter) return res.status(404).json({ error: 'No chapters found for book' });
      
      console.log(`Extracting embedded cover from: ${firstChapter.path}`);
      const stream = client.createReadStream(firstChapter.path, { range: { start: 0, end: 4194304 } });
      const metadata = await mm.parseStream(stream, { mimeType: 'audio/mpeg', size: 4194304 }, { skipPostProcess: true });
      
      if (metadata && metadata.common && metadata.common.picture && metadata.common.picture.length > 0) {
        const pic = metadata.common.picture[0];
        res.setHeader('Content-Type', pic.format || 'image/jpeg');
        res.setHeader('Cross-Origin-Resource-Policy', 'cross-origin');
        res.setHeader('Access-Control-Allow-Origin', '*');
        res.setHeader('Cache-Control', 'public, max-age=31536000');
        return res.send(pic.data);
      }
      return res.status(404).send('No embedded cover found');
    }

    const stream = client.createReadStream(path);
    
    res.setHeader('Content-Type', mime.lookup(path) || 'image/jpeg');
    res.setHeader('Cross-Origin-Resource-Policy', 'cross-origin');
    res.setHeader('Access-Control-Allow-Origin', '*');
    
    stream.pipe(res);
  } catch (err) {
    console.error('Proxy cover error:', err);
    res.status(500).send('Error loading cover');
  }
});

// SPA fallback
app.get('*path', (req, res) => {
  if (req.path.startsWith('/api/')) return res.status(404).json({ error: 'API route not found' });
  res.sendFile(path.join(__dirname, 'public', 'index.html'));
});

app.listen(PORT, async () => {
  console.log(`Ting Reader Backend running on port ${PORT}`);
  
  // Bootstrap: Create default admin user if none exists
  try {
    const userCount = db.prepare('SELECT count(*) as count FROM users').get().count;
    if (userCount === 0) {
      const defaultAdmin = 'admin';
      const defaultPass = 'admin123';
      const passwordHash = await bcrypt.hash(defaultPass, 10);
      const userId = uuidv4();
      db.prepare('INSERT INTO users (id, username, password_hash, role) VALUES (?, ?, ?, ?)').run(userId, defaultAdmin, passwordHash, 'admin');
      console.log('-------------------------------------------');
      console.log('  INITIAL ADMIN CREATED');
      console.log(`  Username: ${defaultAdmin}`);
      console.log(`  Password: ${defaultPass}`);
      console.log('  Please change your password after login!');
      console.log('-------------------------------------------');
    }
  } catch (err) {
    console.error('Failed to bootstrap admin user:', err);
  }
});
