const fs = require('fs');
const fsPromises = require('fs').promises;
const path = require('path');

const CACHE_DIR = path.join(__dirname, 'cache');
const MAX_FILES = 50;
const MAX_SIZE_BYTES = 2 * 1024 * 1024 * 1024; // 2GB

if (!fs.existsSync(CACHE_DIR)) {
  fs.mkdirSync(CACHE_DIR);
}

function getCachePath(chapterId) {
  return path.join(CACHE_DIR, `${chapterId}.bin`);
}

function isCached(chapterId) {
  return fs.existsSync(getCachePath(chapterId));
}

async function getMimeType(chapterId) {
  const cachePath = getCachePath(chapterId);
  try {
    const handle = await fsPromises.open(cachePath, 'r');
    const { buffer } = await handle.read(Buffer.alloc(32), 0, 32, 0);
    await handle.close();

    const header = buffer.toString('binary');
    if (header.includes('ftyp') || header.includes('m4a') || (buffer[4] === 0x66 && buffer[5] === 0x74 && buffer[6] === 0x79 && buffer[7] === 0x70)) {
      return 'audio/mp4';
    } else if (header.includes('fLaC')) {
      return 'audio/flac';
    } else if (header.includes('WAVE')) {
      return 'audio/wav';
    }
    return 'audio/mpeg';
  } catch (err) {
    return 'audio/mpeg';
  }
}

async function saveToCache(chapterId, data) {
  const cachePath = getCachePath(chapterId);
  await fsPromises.writeFile(cachePath, data);
  scheduleCleanup();
}

let cleanupTimeout = null;
function scheduleCleanup() {
  if (cleanupTimeout) return;
  cleanupTimeout = setTimeout(async () => {
    try {
      await clearOldCache();
    } catch (err) {
      console.error('Cache cleanup failed:', err.message);
    } finally {
      cleanupTimeout = null;
    }
  }, 10000); // Cleanup at most once every 10 seconds
}

async function clearOldCache() {
  const files = await fsPromises.readdir(CACHE_DIR);
  const fileDetails = await Promise.all(
    files.map(async (name) => {
      const filePath = path.join(CACHE_DIR, name);
      const stats = await fsPromises.stat(filePath);
      return { name, path: filePath, mtime: stats.mtime, size: stats.size };
    })
  );

  // Sort by modification time (oldest first)
  fileDetails.sort((a, b) => a.mtime - b.mtime);

  let currentCount = fileDetails.length;
  let currentSize = fileDetails.reduce((sum, f) => sum + f.size, 0);

  const toDelete = [];

  // Check file count limit
  if (currentCount > MAX_FILES) {
    const countToDelete = currentCount - MAX_FILES;
    const deletedByCount = fileDetails.splice(0, countToDelete);
    toDelete.push(...deletedByCount);
    currentSize = fileDetails.reduce((sum, f) => sum + f.size, 0);
  }

  // Check total size limit
  if (currentSize > MAX_SIZE_BYTES) {
    for (const file of fileDetails) {
      if (currentSize <= MAX_SIZE_BYTES) break;
      toDelete.push(file);
      currentSize -= file.size;
    }
  }

  for (const file of toDelete) {
    try {
      await fsPromises.unlink(file.path);
    } catch (e) {
      // Ignore errors during deletion
    }
  }

  if (toDelete.length > 0) {
    console.log(`Cache cleanup: deleted ${toDelete.length} files. Remaining size: ${(currentSize / 1024 / 1024).toFixed(2)} MB`);
  }
}

async function deleteChapterCache(chapterId) {
  try {
    const cachePath = getCachePath(chapterId);
    if (fs.existsSync(cachePath)) {
      await fsPromises.unlink(cachePath);
    }
  } catch (err) {
    // Ignore errors
  }
}

async function clearCacheForBook(bookId, db) {
  try {
    const chapters = db.prepare('SELECT id FROM chapters WHERE book_id = ?').all(bookId);
    for (const chapter of chapters) {
      await deleteChapterCache(chapter.id);
    }
    console.log(`Cleared cache for book ${bookId}`);
  } catch (err) {
    console.error(`Failed to clear cache for book ${bookId}:`, err.message);
  }
}

module.exports = {
  isCached,
  saveToCache,
  getCachePath,
  clearOldCache,
  getMimeType,
  clearCacheForBook,
  deleteChapterCache
};
