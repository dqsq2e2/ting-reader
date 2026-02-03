const crypto = require('crypto');
const path = require('path');
const { getStorageClient } = require('./storage-client');
const db = require('./db');
const { v4: uuidv4 } = require('uuid');
const mm = require('music-metadata');
const { scrapeXimalaya } = require('./scraper');
const { decryptXM } = require('./xm-decryptor');
const { calculateThemeColor } = require('./color-utils');
const cacheManager = require('./cache-manager');

// Custom Tokenizer for WebDAV to allow music-metadata to perform Range requests
async function getAccurateMetadata(client, filePath, virtualSize, realSize, isXM = false) {
  const timeoutPromise = new Promise((_, reject) => 
    setTimeout(() => reject(new Error('Metadata extraction timed out')), 15000)
  );

  try {
    const ext = filePath.toLowerCase().split('.').pop();
    let mimeType = 'audio/mpeg';
    if (ext === 'm4a') mimeType = 'audio/mp4';
    else if (ext === 'wav') mimeType = 'audio/wav';
    else if (ext === 'flac') mimeType = 'audio/x-flac';

    console.log(`Getting metadata for ${path.basename(filePath)} (Size: ${virtualSize})`);

    // Helper to fetch range from WebDAV
    const fetchRange = async (offset, length) => {
      if (length <= 0) return Buffer.alloc(0);
      const end = Math.min(offset + length - 1, virtualSize - 1);
      if (offset > end) return Buffer.alloc(0);
      
      return await client.getFileContents(filePath, { 
        range: { start: offset, end },
        format: 'binary'
      });
    };

    const tokenizer = {
      fileInfo: { size: virtualSize, mimeType },
      position: 0,
      supportsRandomAccess: () => true,
      
      readBuffer: async (buffer, options) => {
        const offset = (options && options.position !== undefined) ? options.position : tokenizer.position;
        const length = (options && options.length !== undefined) ? options.length : buffer.length;
        
        if (offset >= virtualSize) return 0;
        const actualLength = Math.min(length, virtualSize - offset);
        if (actualLength <= 0) return 0;

        try {
          const data = await fetchRange(offset, actualLength);
          if (!data || data.length === 0) return 0;
          
          const bytesToCopy = Math.min(data.length, buffer.length - ((options && options.offset) || 0));
          data.copy(buffer, (options && options.offset) || 0, 0, bytesToCopy);
          tokenizer.position = offset + bytesToCopy;
          return bytesToCopy;
        } catch (e) {
          return 0;
        }
      },

      peekBuffer: async (buffer, options) => {
        const offset = (options && options.position !== undefined) ? options.position : tokenizer.position;
        const length = (options && options.length !== undefined) ? options.length : buffer.length;
        
        if (offset >= virtualSize) return 0;
        const actualLength = Math.min(length, virtualSize - offset);
        if (actualLength <= 0) return 0;

        try {
          const data = await fetchRange(offset, actualLength);
          if (!data || data.length === 0) return 0;
          
          const bytesToCopy = Math.min(data.length, buffer.length - ((options && options.offset) || 0));
          data.copy(buffer, (options && options.offset) || 0, 0, bytesToCopy);
          return bytesToCopy;
        } catch (e) {
          return 0;
        }
      },

      readToken: async (token, offset) => {
        const buffer = Buffer.alloc(token.len);
        const bytesRead = await tokenizer.readBuffer(buffer, { position: offset });
        return token.get(buffer, 0);
      },

      peekToken: async (token, offset) => {
        const buffer = Buffer.alloc(token.len);
        const bytesRead = await tokenizer.peekBuffer(buffer, { position: offset });
        return token.get(buffer, 0);
      },

      readNumber: async (token) => tokenizer.readToken(token),
      peekNumber: async (token) => tokenizer.peekToken(token),
      ignore: async (length) => {
        const bytesToIgnore = Math.min(length, virtualSize - tokenizer.position);
        tokenizer.position += bytesToIgnore;
        return bytesToIgnore;
      },
      setPosition: async (pos) => { tokenizer.position = pos; },
      close: async () => {}
    };

    const metadata = await Promise.race([
      mm.parseFromTokenizer(tokenizer),
      timeoutPromise
    ]);
    
    let duration = metadata.format.duration;
    let title = metadata.common.title;
    
    if (!duration && metadata.format.bitrate) {
      const audioDataSize = isXM ? (virtualSize - 128) : realSize;
      duration = (audioDataSize * 8) / metadata.format.bitrate;
    }

    return { duration, title };
  } catch (err) {
    console.warn(`Metadata extraction failed for ${path.basename(filePath)}: ${err.message}`);
    return null;
  }
}

const SUPPORTED_EXTENSIONS = ['.mp3', '.m4a', '.wav', '.flac', '.xm'];
const EXCLUDED_EXTENSIONS = ['.m4b'];
const IMAGE_EXTENSIONS = ['.jpg', '.jpeg', '.png', '.webp'];

function generateBookHash(libraryId, dirPath) {
  return crypto.createHash('md5').update(`${libraryId}|${dirPath}`).digest('hex');
}

function decodeXmlyGibberish(str) {
  if (!str) return '';
  // If it contains the specific marker of Ximalaya's mis-encoded URLs
  if (str.includes('椀洀愀最攀瘀')) {
    try {
      // Convert to buffer as if it was read correctly
      const buf = Buffer.from(str, 'utf16le');
      // The first few bytes might be garbage (BOM or language markers)
      // We look for 'http' or 'image'
      const decoded = buf.toString('utf8').replace(/[^\x20-\x7E]/g, '');
      const match = decoded.match(/(https?:\/\/[^\s]+|imagev2\.xmcdn\.com[^\s]+)/);
      if (match) {
        let url = match[0];
        if (!url.startsWith('http')) url = 'https://' + url;
        return url;
      }
    } catch (e) {
      return '';
    }
  }
  return str;
}

function cleanChapterTitle(filename, bookTitle = '') {
  let title = filename.replace(/\.[^/.]+$/, ""); // Remove extension
  
  // 1. Detect and remove "Extra" markers (番外, etc.)
  const extraPatterns = [/番外[：:\-\s]*/i, /花絮[：:\-\s]*/i, /特典[：:\-\s]*/i, /SP[：:\-\s]*/i, /Extra[：:\-\s]*/i];
  let isExtra = false;
  for (const pattern of extraPatterns) {
    if (pattern.test(title)) {
      isExtra = true;
      title = title.replace(pattern, ''); // Remove the marker from title
    }
  }
  // Also catch mid-title extra markers
  if (!isExtra && /番外|花絮|特典|SP|Extra/i.test(title)) {
    isExtra = true;
  }

  // 2. Remove common promotional suffixes and advertisements
  // Includes patterns like: （请订阅...）, [更多...], （搜新书《...》）, 【新书推荐...】
  const promoKeywords = [
    '请?订阅', '转发', '五星', '好评', '关注', '微信', '群', '更多', 
    '加我', '联系', '点击', '搜新书', '新书', '推荐', '上架', '完本'
  ];
  const promoRegex = new RegExp(`[（\\(\\[\\{【](?:${promoKeywords.join('|')}).*?[）\\)\\]\\}】]`, 'g');
  title = title.replace(promoRegex, '');
  
  // 3. Remove book title if it's present
  if (bookTitle) {
    const cleanBookTitle = bookTitle.split(/[丨|｜\-]/)[0].trim();
    if (cleanBookTitle.length > 1) {
      const escapedTitle = cleanBookTitle.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
      const regex = new RegExp(escapedTitle, 'gi');
      title = title.replace(regex, '');
    }
  }
  
  // 4. Remove "第xxx集/章/回"
  title = title.replace(/第\s*\d+\s*[集回章话]\s*/g, '');
  
  // 5. Remove leading/trailing numbers and separators
  title = title.replace(/^\d+[\s.\-_]+/, '').replace(/[\s.\-_]+\d+$/, '');
  
  // 6. Remove common suffixes: "-ZmAudio"
  title = title.replace(/[-_]ZmAudio$/i, '');
  
  // 7. Final cleanup of any remaining weird characters at start/end
  // Now includes colons and other separators that might be left after removing markers
  title = title.replace(/^[：:\s\-_.]+/, '').replace(/[：:\s\-_.]+$/, '');
  
  return { 
    title: title.trim() || filename.replace(/\.[^/.]+$/, "").replace(/^\d+[\s.\-_]+/, '').trim(), 
    isExtra 
  };
}

function extractChapterIndex(filename, loopIndex) {
  // Try to find numbers like "第95集", "095.", "95_", "Chapter 95"
  const patterns = [
    /第\s*(\d+)\s*[集回章话]/,
    /(\d+)[\s.\-_]+/,
    /[集回章话]\s*(\d+)/,
    /(\d+)$/ // Number at the very end
  ];

  for (const pattern of patterns) {
    const match = filename.match(pattern);
    if (match && match[1]) {
      const num = parseInt(match[1]);
      if (!isNaN(num)) return num;
    }
  }

  return loopIndex;
}

async function scanLibrary(libraryId, taskId) {
  const library = db.prepare('SELECT * FROM libraries WHERE id = ?').get(libraryId);
  if (!library) throw new Error('Library not found');

  const client = getStorageClient(library);
  let rootPath = library.root_path || '/';
  try {
    // Ensure the root path is decoded to avoid double-encoding issues
    rootPath = decodeURIComponent(rootPath);
  } catch (e) {
    // Ignore if not decodable
  }

  console.log(`Scanning library ${library.name} at ${rootPath}...`);
  if (taskId) {
    db.prepare("UPDATE tasks SET message = ? WHERE id = ?").run(`正在准备扫描: ${library.name}`, taskId);
  }

  // Track found items to cleanup missing ones
  const foundBooks = new Set();
  const foundChapters = new Set();

  await recursiveScan(client, libraryId, rootPath, taskId, foundBooks, foundChapters);
  
  // Cleanup logic: Remove books and chapters that are no longer in the storage
  if (taskId) {
    db.prepare("UPDATE tasks SET message = ? WHERE id = ?").run(`正在清理不存在的记录...`, taskId);
  }

  // 1. Find all books in this library that were not found during scan
  const currentBooks = db.prepare('SELECT id FROM books WHERE library_id = ?').all(libraryId);
  for (const book of currentBooks) {
    if (!foundBooks.has(book.id)) {
      console.log(`Book ${book.id} no longer exists, deleting...`);
      // Clear cache for book before deleting
      await cacheManager.clearCacheForBook(book.id, db);
      // Delete chapters first (due to FK)
      db.prepare('DELETE FROM chapters WHERE book_id = ?').run(book.id);
      db.prepare('DELETE FROM books WHERE id = ?').run(book.id);
    } else {
      // 2. For books that still exist, check if any chapters were removed
      const currentChapters = db.prepare('SELECT id FROM chapters WHERE book_id = ?').all(book.id);
      for (const chapter of currentChapters) {
        if (!foundChapters.has(chapter.id)) {
          console.log(`Chapter ${chapter.id} no longer exists, deleting...`);
          // Clear cache for chapter
          await cacheManager.deleteChapterCache(chapter.id);
          db.prepare('DELETE FROM chapters WHERE id = ?').run(chapter.id);
        }
      }
    }
  }

  // Update last scanned time
  db.prepare('UPDATE libraries SET last_scanned_at = CURRENT_TIMESTAMP WHERE id = ?').run(libraryId);
  
  // Mark task as completed
  if (taskId) {
    db.prepare("UPDATE tasks SET status = ?, message = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?").run('completed', '扫描库成功完成', taskId);
  }
}

async function recursiveScan(client, libraryId, currentPath, taskId, foundBooks, foundChapters) {
  let directoryItems;
  try {
    if (taskId) {
      db.prepare("UPDATE tasks SET message = ? WHERE id = ?").run(`正在扫描目录: ${currentPath}`, taskId);
    }
    directoryItems = await client.getDirectoryContents(currentPath);
  } catch (error) {
    console.error(`Error scanning path ${currentPath}:`, error.message);
    // If root fails, we throw to notify the user. If subdirs fail, we just log and continue.
    if (currentPath === '/' || currentPath === '') {
      throw new Error(`Failed to read root directory: ${error.message}`);
    }
    return;
  }
  
  const audioFiles = [];
  const imageFiles = [];
  const subDirs = new Set(); // Use Set for deduplication

  for (const item of directoryItems) {
    const filename = item.filename;
    const ext = '.' + item.basename.split('.').pop().toLowerCase();
    
    if (item.type === 'directory') {
      subDirs.add(filename);
    } else if (item.type === 'file') {
      if (SUPPORTED_EXTENSIONS.includes(ext) && !EXCLUDED_EXTENSIONS.includes(ext)) {
        // Deduplicate audio files by path
        if (!audioFiles.find(f => f.filename === filename)) {
          audioFiles.push(item);
        }
      } else if (IMAGE_EXTENSIONS.includes(ext)) {
        if (!imageFiles.find(f => f.filename === filename)) {
          imageFiles.push(item);
        }
      }
    }
  }

  // If this directory contains audio files, it might be one or more books
  if (audioFiles.length > 0) {
    if (taskId) {
      const bookName = currentPath.split('/').pop() || '未知书籍';
      db.prepare("UPDATE tasks SET message = ? WHERE id = ?").run(`分析目录内容: ${bookName}`, taskId);
    }
    
    // Group files by their album metadata to handle mixed folders
    const groups = await groupFilesByAlbum(client, audioFiles);
    
    for (const [groupKey, albumInfo] of Object.entries(groups)) {
      if (taskId) {
        db.prepare("UPDATE tasks SET message = ? WHERE id = ?").run(`处理书籍: ${albumInfo.name}`, taskId);
      }
      await processBookFiles(libraryId, currentPath, albumInfo, albumInfo.files, imageFiles, taskId, foundBooks, foundChapters);
    }
  }

  // Continue scanning subdirectories
  for (const dir of subDirs) {
    try {
      await recursiveScan(client, libraryId, dir, taskId, foundBooks, foundChapters);
    } catch (error) {
      console.error(`Error scanning subdirectory ${dir}:`, error.message);
      // We don't throw here to allow other subdirectories to be scanned
    }
  }
}

async function groupFilesByAlbum(client, audioFiles) {
  const groups = {};
  const dirName = audioFiles[0].filename.split('/').slice(-2, -1)[0] || 'Unknown Book';
  
  // To avoid excessive metadata reading, we only peek at files if there are more than 1
  // or if the directory name looks generic
  const isGenericDir = dirName.includes('喜马拉雅') || dirName.match(/^\d+$/) || dirName.length < 2;

  for (const file of audioFiles) {
    let album = null;
    
    // Only read metadata for grouping if it's a generic directory or we suspect multiple albums
    try {
      const stream = client.createReadStream(file.filename, { 
        range: { start: 0, end: 1048576 }, // 1MB is usually enough for ID3 tags
        format: 'binary' 
      });
      const metadata = await mm.parseStream(stream, { mimeType: 'audio/mpeg', size: 1048576 }, { skipPostProcess: true });
      if (metadata && metadata.common && metadata.common.album) {
        album = String(metadata.common.album).trim();
      }
    } catch (err) {
      // Ignore errors
    }
    
    // Identity object: tells us if this album name came from metadata or folder
    const groupKey = album || dirName;
    if (!groups[groupKey]) {
      groups[groupKey] = {
        name: groupKey,
        isFromMetadata: !!album,
        files: []
      };
    }
    groups[groupKey].files.push(file);
  }
  
  return groups;
}

async function processBookFiles(libraryId, dirPath, albumInfo, audioFiles, imageFiles, taskId, foundBooks, foundChapters) {
  const { name: albumName, isFromMetadata } = albumInfo;
  
  // If identity is from metadata, we use a global hash (Library + Album) to allow merging across folders.
  // If it's from a folder name, we include the path to keep it unique to this folder.
  const hashInput = isFromMetadata 
    ? `${libraryId}:album:${albumName}` 
    : `${libraryId}:path:${dirPath}:${albumName}`;
    
  const bookHash = crypto.createHash('md5').update(hashInput).digest('hex');
  
  const library = db.prepare('SELECT * FROM libraries WHERE id = ?').get(libraryId);
  const client = getStorageClient(library);

  // Use albumName as initial title
  let title = albumName;
  let author = 'Unknown Author';
  let narrator = 'Unknown Narrator';
  let description = '';
  let localCover = '';
  let tagAlbumTitle = '';

  // Look for cover image in the directory
  const coverFile = imageFiles.find(f => 
    ['cover', 'folder', 'poster', 'album', 'front'].includes(f.basename.split('.')[0].toLowerCase())
  ) || imageFiles[0];

  if (coverFile) {
    localCover = coverFile.filename;
  }

  // Try to extract metadata from the first audio file
  if (audioFiles.length > 0) {
    try {
      // Use a stream-based parser for better metadata support
      // We read up to 4MB to catch metadata that might be further in the file
      try {
        const stream = client.createReadStream(audioFiles[0].filename, { 
          range: { start: 0, end: 4194304 },
          format: 'binary' 
        });
        const metadata = await mm.parseStream(stream, { mimeType: 'audio/mpeg', size: 4194304 }, { skipPostProcess: true });
        
          if (metadata && metadata.common) {
            if (metadata.common.album) tagAlbumTitle = String(metadata.common.album).trim();
            if (metadata.common.title) title = String(metadata.common.title).trim();
            if (metadata.common.artist) author = String(metadata.common.artist).trim();
            if (metadata.common.composer) {
              const comp = Array.isArray(metadata.common.composer) ? metadata.common.composer[0] : metadata.common.composer;
              narrator = String(comp).trim();
            }
            
            // Try to get description from multiple possible fields
            // Handle comments correctly as they might be objects
            let comments = [];
            if (metadata.common.comment) {
              comments = Array.isArray(metadata.common.comment) ? metadata.common.comment : [metadata.common.comment];
            }
            
            description = comments.map(c => {
              if (typeof c === 'string') return c;
              if (typeof c === 'object' && c !== null) {
                // Check if text is gibberish or empty
                const text = c.text || '';
                // If it's the specific gibberish from Ximalaya tags (which contains image URLs)
                // We try to detect it. Often it's UTF-16LE data being misread.
                if (c.descriptor && c.descriptor.includes('xmcdn.com')) {
                   // This is actually an image URL! Let's save it.
                   if (!localCover) localCover = c.descriptor;
                   return '';
                }
                // If text looks like a JSON string or contains mostly non-printable chars, skip it
                if (text.startsWith('{') && text.endsWith('}')) return '';
                return text;
              }
              return String(c);
            }).filter(t => t.length > 5).join('\n') || 
                          metadata.common.description || 
                          metadata.common.longDescription || '';
            
            // Clean up description if it's still gibberish
            if (description.length > 0) {
              // Simple heuristic: if > 50% of characters are outside common ranges, it's likely gibberish
              let nonStandard = 0;
              for (let i = 0; i < description.length; i++) {
                const code = description.charCodeAt(i);
                if (code > 0x9FFF && code < 0xF000) nonStandard++; // Random CJK/Special ranges
              }
              if (nonStandard > description.length * 0.3) {
                console.log('Detected gibberish description, clearing it.');
                description = '';
              }
            }

            // Try to extract embedded cover if we don't have a local one
            if (!localCover && metadata.common.picture && metadata.common.picture.length > 0) {
              const pic = metadata.common.picture[0];
              // We'll store a hint that this book has an embedded cover
              // The frontend or a proxy route can then extract it on demand
              // For now, let's use a special marker
              localCover = `embedded://first-chapter`;
            }
          }
      } catch (mmErr) {
        console.warn(`music-metadata failed for ${audioFiles[0].filename}: ${mmErr.message}`);
      }
    } catch (err) {
      console.warn(`Could not extract metadata for ${dirPath}: ${err.message}`);
    }
  }

  // Determine the best title for scraping and saving
  // 1. If folder name looks like a generic Ximalaya name, use album tag
  let bestTitle = albumName;
  if (albumName.includes('喜马拉雅') || albumName.length < 2 || albumName.match(/^\d+$/)) {
    if (tagAlbumTitle && tagAlbumTitle.length > 1) {
      bestTitle = tagAlbumTitle;
    }
  }

  // Check if book exists
  let book = db.prepare('SELECT * FROM books WHERE hash = ?').get(bookHash);
  
  // If book exists but has poor metadata, we'll try to update it
  const needsUpdate = book && (
    !book.description || 
    book.description.includes('[object Object]') || 
    book.description.startsWith('{') ||
    !book.cover_url ||
    !book.theme_color ||
    book.title === book.path.split('/').pop()
  );

  if (!book || needsUpdate) {
    const bookId = book ? book.id : uuidv4();
    
    // Try to scrape from Ximalaya if local metadata is basic
    let scrapedMetadata = null;
    
    // Determine the best search term for scraping
    let searchTerm = bestTitle;
    const lowerTitle = bestTitle.toLowerCase();
    if (bestTitle.match(/^\d+$/) || 
        bestTitle.includes('第') || // Likely an episode
        lowerTitle.endsWith('.mp3') || 
        lowerTitle.endsWith('.m4a') || 
        lowerTitle.endsWith('.xm') || 
        lowerTitle.endsWith('.flac')) {
      searchTerm = tagAlbumTitle || albumName;
    }

    if (searchTerm && searchTerm !== 'Unknown Book' && searchTerm.length > 1) {
      if (taskId) {
        db.prepare("UPDATE tasks SET message = ? WHERE id = ?").run(`正在刮削: ${searchTerm}`, taskId);
      }
      scrapedMetadata = await scrapeXimalaya(searchTerm);
    }

    // Determine final title: 
    let finalTitle = scrapedMetadata?.title;
    if (!finalTitle) {
      if (bestTitle.includes('第') || bestTitle.match(/\d+/) || bestTitle.length > 50) {
        finalTitle = tagAlbumTitle || albumName;
      } else {
        finalTitle = bestTitle;
      }
    }
    
    const finalAuthor = scrapedMetadata?.author || author;
    const finalNarrator = scrapedMetadata?.narrator || narrator;
    const finalDescription = scrapedMetadata?.description || description;
    const finalTags = scrapedMetadata?.tags || '';
    let finalCover = localCover || scrapedMetadata?.cover_url || '';
    
    // Decode if it's Ximalaya gibberish
    finalCover = decodeXmlyGibberish(finalCover);

    // Calculate theme color from cover
    const themeColor = await calculateThemeColor(finalCover, client);

    if (!book) {
      db.prepare(`
        INSERT INTO books (id, library_id, title, author, narrator, description, tags, cover_url, theme_color, path, hash)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
      `).run(bookId, libraryId, finalTitle, finalAuthor, finalNarrator, finalDescription, finalTags, finalCover, themeColor, dirPath, bookHash);
      book = { id: bookId, title: finalTitle, theme_color: themeColor };
    } else {
      db.prepare(`
        UPDATE books SET 
          title = ?, author = ?, narrator = ?, description = ?, tags = ?, cover_url = ?, theme_color = ?
        WHERE id = ?
      `).run(finalTitle, finalAuthor, finalNarrator, finalDescription, finalTags, finalCover, themeColor, bookId);
      book.title = finalTitle;
      book.theme_color = themeColor;
    }
  }

  // Mark book as found
  foundBooks.add(book.id);

  // Process chapters
  // Sort audio files by name to ensure correct order
  audioFiles.sort((a, b) => a.basename.localeCompare(b.basename, undefined, { numeric: true, sensitivity: 'base' }));

  console.log(`Processing ${audioFiles.length} chapters for book: ${title}`);
  console.log(`Files to process: ${audioFiles.map(f => f.basename).join(', ')}`);

  for (let i = 0; i < audioFiles.length; i++) {
    const file = audioFiles[i];
    
    // Check if chapter exists
    let existingChapter = db.prepare('SELECT id FROM chapters WHERE book_id = ? AND path = ?').get(book.id, file.filename);
    
    if (!existingChapter) {
      if (taskId && i % 10 === 0) {
        db.prepare("UPDATE tasks SET message = ? WHERE id = ?").run(`正在处理章节 (${i + 1}/${audioFiles.length}): ${file.basename}`, taskId);
      }
      
      const { title: chapterTitle, isExtra } = cleanChapterTitle(file.basename, book.title);
      let duration = 0;
      let tagTitle = '';

      const stats = await client.stat(file.filename);
      const fileSize = stats.size || 0;

      // Try to get tags for this specific chapter
      try {
        const ext = '.' + file.basename.split('.').pop().toLowerCase();
        const isXM = ext === '.xm';
        const isSmallFile = fileSize < 10 * 1024 * 1024; // 10MB
        
        if (isXM || isSmallFile) {
          try {
            console.log(`Scanning ${isXM ? 'XM' : 'regular'} file: ${file.basename} (Full Read Mode)`);
            const buffer = await client.getFileContents(file.filename, { format: 'binary' });
            
            let audioBuffer = buffer;
            if (isXM) {
              console.log(`Read ${buffer.length} bytes for ${file.basename}, decrypting...`);
              audioBuffer = await decryptXM(buffer);
              console.log(`Decrypted ${audioBuffer.length} bytes for ${file.basename}, parsing metadata...`);
            } else {
              console.log(`Read ${buffer.length} bytes for ${file.basename}, parsing metadata...`);
            }

            const metadata = await mm.parseBuffer(audioBuffer);
            
            if (metadata.format.duration) {
              duration = Math.round(metadata.format.duration);
            }
            if (metadata.common.title) {
              tagTitle = metadata.common.title;
            }
            console.log(`Finished ${isXM ? 'XM' : 'regular'} metadata for ${file.basename}: duration=${duration}, title=${tagTitle}`);
          } catch (err) {
            console.warn(`Failed to process ${isXM ? 'XM' : 'regular'} file ${file.basename}: ${err.message}`);
          }
        } else {
          // Large regular files: use tokenizer for efficiency
          console.log(`Scanning large regular file: ${file.basename}`);
          const virtualSize = fileSize;
          const meta = await getAccurateMetadata(client, file.filename, virtualSize, fileSize, false);
          if (meta) {
            if (meta.duration > 0) duration = Math.round(meta.duration);
            if (meta.title) tagTitle = meta.title;
          }
          console.log(`Finished large regular metadata for ${file.basename}: duration=${duration}, title=${tagTitle}`);
        }
        
        // Final fallback for all files if duration is still 0
        if (duration <= 0) {
           const stream = client.createReadStream(file.filename, { 
             range: { start: 0, end: Math.min(fileSize - 1, 2 * 1024 * 1024) }, // Try first 2MB
             format: 'binary'
           });
           const metadata = await mm.parseStream(stream, { 
             mimeType: ext === '.m4a' ? 'audio/mp4' : 'audio/mpeg', 
             size: fileSize 
           });
           if (metadata) {
             if (metadata.format && metadata.format.duration) {
               duration = Math.round(metadata.format.duration);
             }
             if (metadata.common && metadata.common.title) {
               tagTitle = String(metadata.common.title).trim();
             }
           }
        }
      } catch (err) {
        // Ignore tag read errors
      }
      
      // If duration is still 0, try to estimate based on file size
      if (duration <= 0) {
        try {
          const stats = await client.stat(file.filename);
          const size = stats.size || 0;
          if (size > 0) {
            const ext = file.filename.toLowerCase().split('.').pop();
            // Refined bitrate estimation
            let bitrate = 128000;
            if (ext === 'xm') bitrate = 96000;
            else if (ext === 'm4a') bitrate = 64000; // M4A often uses lower bitrate for speech
            else if (ext === 'flac') bitrate = 700000; // FLAC is much higher
            
            duration = Math.round(size / (bitrate / 8));
          }
        } catch (err) {
          duration = 0;
        }
      }

      const chapterIndex = extractChapterIndex(file.basename, i + 1);
      let finalChapterTitle = tagTitle || chapterTitle;
      
      // If we used the tag title, apply the same cleaning rules (except the file-specific ones)
      if (tagTitle) {
        const { title: cleanedTagTitle } = cleanChapterTitle(tagTitle, book.title);
        finalChapterTitle = cleanedTagTitle;
      }

      const chapterId = uuidv4();
      db.prepare(`
        INSERT INTO chapters (id, book_id, title, path, duration, chapter_index, is_extra)
        VALUES (?, ?, ?, ?, ?, ?, ?)
      `).run(chapterId, book.id, finalChapterTitle, file.filename, duration, chapterIndex, isExtra ? 1 : 0);
      
      existingChapter = { id: chapterId };
    }
    
    // Mark chapter as found
    foundChapters.add(existingChapter.id);
  }
  console.log(`Finished processing book: ${title}`);
}

module.exports = { scanLibrary };
