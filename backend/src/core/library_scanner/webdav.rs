use super::{LibraryScanner, ScanResult, ScanStatus, MetadataSource};
use crate::core::error::{Result, TingError};
use crate::core::nfo_manager::BookMetadata;
use crate::db::repository::Repository;
use crate::plugin::manager::FormatMethod;
use crate::plugin::types::PluginType;
use std::path::{Path, PathBuf};
use std::collections::{HashMap, HashSet};
use tracing::{debug, info, warn};
use uuid::Uuid;
use sha2::{Digest, Sha256};
use quick_xml::events::Event;
use quick_xml::reader::Reader;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use base64::Engine;
use id3::TagLike;

impl LibraryScanner {
    /// Scan a WebDAV library
    pub(crate) async fn scan_webdav_library(
        &self,
        library: &crate::db::models::Library,
        task_id: Option<&str>,
        scraper_config: &crate::db::models::ScraperConfig,
    ) -> Result<ScanResult> {
        if self.storage_service.is_none() {
            return Err(TingError::ConfigError("Storage service not configured for WebDAV scan".to_string()));
        }

        let mut scan_result = ScanResult::default();
        scan_result.start_time = Some(std::time::Instant::now());
        self.update_progress(task_id, "正在扫描 WebDAV 目录...".to_string()).await;

        // 1. List files recursively
        let files = self.list_webdav_files(library, task_id).await?;
        
        let supported_extensions = self.get_supported_extensions().await;
        
        // Group by directory URL (parent URL)
        // Key: Parent URL (String), Value: List of (File URL, Last Modified)
        let mut dir_groups: HashMap<String, Vec<(String, Option<chrono::DateTime<chrono::Utc>>)>> = HashMap::new();
        
        for (file_url, last_mod) in files {
            // Check extension
            if let Some(ext_pos) = file_url.rfind('.') {
                let ext = file_url[ext_pos+1..].to_lowercase();
                if supported_extensions.contains(&ext) {
                    // Get parent URL
                    if let Some(last_slash) = file_url.rfind('/') {
                        let parent = file_url[0..last_slash].to_string();
                        dir_groups.entry(parent).or_default().push((file_url, last_mod));
                    }
                }
            }
        }

        self.update_progress(task_id, format!("找到 {} 个包含音频文件的目录", dir_groups.len())).await;

        let total_groups = dir_groups.len();
        let mut processed_count = 0;

        // Pre-fetch all books for lookup and deletion handling
        let prefetched = self.prefetch_books(&library.id).await;

        let mut book_path_map: HashMap<String, (String, i32, Option<String>)> = HashMap::new();
        let mut book_hash_map: HashMap<String, (String, i32, Option<String>)> = HashMap::new();
        for (id, path, hash, manual_corrected, match_pattern) in &prefetched.all_books {
            book_path_map.insert(path.clone(), (id.clone(), *manual_corrected, match_pattern.clone()));
            book_hash_map.insert(hash.clone(), (id.clone(), *manual_corrected, match_pattern.clone()));
        }

        let mut found_book_ids: HashSet<String> = HashSet::new();
        let last_scanned = if let Some(ref date_str) = library.last_scanned_at {
            chrono::DateTime::parse_from_rfc3339(date_str)
                .map(|dt| dt.with_timezone(&chrono::Utc))
                .ok() 
        } else {
            None
        };

        for (dir_url, mut file_entries) in dir_groups {
            // Check cancellation
            self.check_cancellation(task_id).await?;

            processed_count += 1;
            // Extract directory name from URL for logging
            let decoded_dir_url = self.decode_url_path(&dir_url);
            let dir_name = decoded_dir_url.trim_end_matches('/').split('/').last().unwrap_or("Unknown");
            
            self.update_progress(task_id, format!("处理中 ({}/{}): {}", processed_count, total_groups, dir_name)).await;

            // Sort file entries naturally by URL
            file_entries.sort_by(|a, b| natord::compare(&a.0, &b.0));
            
            // Extract just URLs for processing
            let mut file_urls: Vec<String> = Vec::new();
            let mut metadata_files: Vec<String> = Vec::new();
            
            for (url, _) in file_entries.iter() {
                let ext = url.split('.').last().unwrap_or_default().to_lowercase();
                if ["json", "nfo", "jpg", "png", "jpeg", "webp"].contains(&ext.as_str()) {
                    metadata_files.push(url.clone());
                } else {
                    file_urls.push(url.clone());
                }
            }

            // Calculate directory hash for lookup
            let mut hasher = Sha256::new();
            hasher.update(dir_url.as_bytes());
            let dir_hash = format!("{:x}", hasher.finalize());

            // Optimization: Find existing book to avoid DB lookup
            let mut existing_info = book_path_map.get(&dir_url).cloned();
            if existing_info.is_none() {
                existing_info = book_hash_map.get(&dir_hash).cloned();
            }

            // Incremental Check: Skip if book exists and no files modified since last scan
            if let (Some((id, _, _)), Some(last_scan_time)) = (&existing_info, last_scanned) {
                // Check if file count changed (new files added or removed)
                let current_file_count = file_urls.len();
                let existing_chapter_count = self.chapter_repo.find_by_book(id).await
                    .map(|chapters| chapters.len())
                    .unwrap_or(0);
                
                // Determine latest modification time in this directory
                let max_mtime = file_entries.iter()
                    .filter_map(|(_, mtime)| *mtime)
                    .max();
                
                let mtime_count = file_entries.iter().filter(|(_, mtime)| mtime.is_some()).count();
                
                info!(
                    book_id = %id, 
                    url = %dir_url, 
                    max_mtime = ?max_mtime, 
                    last_scan_time = %last_scan_time,
                    total_files = file_entries.len(),
                    files_with_mtime = mtime_count,
                    current_file_count = current_file_count,
                    existing_chapter_count = existing_chapter_count,
                    "Checking if WebDAV book needs update"
                );
                
                // Skip only if:
                // 1. File count hasn't changed AND
                // 2. No files have been modified since last scan
                if current_file_count == existing_chapter_count {
                    if let Some(latest) = max_mtime {
                        if latest <= last_scan_time {
                            // Book exists and is up to date
                            scan_result.total_books += 1;
                            scan_result.books_skipped += 1;
                            found_book_ids.insert(id.clone());
                            info!(book_id = %id, url = %dir_url, "Skipping up-to-date WebDAV book");
                            continue;
                        } else {
                            info!(book_id = %id, url = %dir_url, "WebDAV book has newer files, will update");
                        }
                    } else {
                        // No modification times available, force update
                        info!(book_id = %id, url = %dir_url, "No modification times available, will process book");
                    }
                } else {
                    info!(
                        book_id = %id, 
                        url = %dir_url, 
                        "File count changed ({} -> {}), will process book", 
                        existing_chapter_count,
                        current_file_count
                    );
                }
            }

            match self.process_webdav_book(library, &dir_url, &file_urls, &metadata_files, task_id, scraper_config, existing_info).await {
                Ok((book_id, status)) => {
                    scan_result.total_books += 1;
                    match status {
                        ScanStatus::Created => scan_result.books_created += 1,
                        ScanStatus::Updated => scan_result.books_updated += 1,
                        ScanStatus::Skipped => scan_result.books_skipped += 1,
                    }
                    found_book_ids.insert(book_id.clone());
                    debug!(book_id = %book_id, url = %dir_url, status = ?status, "Processed WebDAV book directory");
                }
                Err(e) => {
                    scan_result.failed_count += 1;
                    warn!(url = %dir_url, error = %e, "Failed to process WebDAV book directory");
                    scan_result.errors.push(format!(
                        "Failed to process {}: {}",
                        dir_url,
                        e
                    ));
                }
            }

            // Periodic garbage collection
            self.plugin_manager.garbage_collect_all().await;
        }

        // 3. Handle deletions via shared helper (no path-exists check for WebDAV)
        self.handle_book_deletions(&mut scan_result, &prefetched, &found_book_ids, false).await;

        // Final garbage collection after scan
        self.plugin_manager.garbage_collect_all().await;

        Ok(scan_result)
    }

    /// List all files in a WebDAV library recursively
    async fn list_webdav_files(&self, library: &crate::db::models::Library, task_id: Option<&str>) -> Result<Vec<(String, Option<chrono::DateTime<chrono::Utc>>)>> {
        // Simple BFS or recursive traversal
        // Start from root
        let root_url = if library.root_path.starts_with('/') {
            // Combine library.url + root_path
            let base = library.url.trim_end_matches('/');
            let path = library.root_path.trim_start_matches('/');
            if path.is_empty() {
                base.to_string()
            } else {
                format!("{}/{}", base, path)
            }
        } else {
            library.url.clone()
        };

        let mut files = HashMap::new(); // Use HashMap to store URL -> LastModified
        let mut queue = std::collections::VecDeque::new();
        let mut visited_dirs = HashSet::new(); // Track visited directories to prevent cycles/re-visits

        queue.push_back(root_url.clone());
        visited_dirs.insert(root_url);

        let client = reqwest::Client::builder()
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|e| TingError::NetworkError(e.to_string()))?;
            
        let username = library.username.as_deref();
        
        // Decrypt password
        let password = if let Some(ref enc_pass) = library.password {
            if let Some(key) = &self.encryption_key {
                match crate::core::crypto::decrypt(enc_pass, key) {
                    Ok(p) => Some(p),
                    Err(_) => Some(enc_pass.clone()) // Fallback to raw if decrypt fails
                }
            } else {
                Some(enc_pass.clone())
            }
        } else {
            None
        };

        // Limit depth/count to prevent infinite loops
        let mut processed_dirs = 0;
        let max_dirs = 1000;
        let mut last_request_time = std::time::Instant::now();
        let min_request_interval = std::time::Duration::from_millis(200); // 200ms between requests

        while let Some(current_url) = queue.pop_front() {
            // Check cancellation
            self.check_cancellation(task_id).await?;

            if processed_dirs >= max_dirs {
                warn!("Max WebDAV directories limit reached");
                break;
            }
            processed_dirs += 1;
            
            // Rate limiting: ensure minimum interval between requests
            let elapsed = last_request_time.elapsed();
            if elapsed < min_request_interval {
                let sleep_time = min_request_interval - elapsed;
                tokio::time::sleep(sleep_time).await;
            }

            // PROPFIND request with browser-like headers
            let mut req = client.request(reqwest::Method::from_bytes(b"PROPFIND").unwrap(), &current_url)
                .header("Depth", "1")
                .header("Accept", "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8")
                .header("Accept-Language", "zh-CN,zh;q=0.9,en;q=0.8")
                .header("Accept-Encoding", "gzip, deflate, br")
                .header("Connection", "keep-alive");
            
            if let (Some(u), Some(p)) = (username, &password) {
                req = req.basic_auth(u, Some(p));
            }

            last_request_time = std::time::Instant::now();
            
            match req.send().await {
                Ok(res) => {
                    if res.status().is_success() || res.status().as_u16() == 207 {
                        let xml = res.text().await.unwrap_or_default();
                        let items = self.parse_webdav_response(&xml, &current_url);
                        
                        for (item_url, is_dir, last_mod) in items {
                            // Avoid re-processing current_url (PROPFIND returns self)
                            // We need to handle trailing slashes carefully
                            let item_norm = item_url.trim_end_matches('/');
                            let current_norm = current_url.trim_end_matches('/');
                            
                            if item_norm == current_norm {
                                continue;
                            }

                            if is_dir {
                                if !visited_dirs.contains(&item_url) {
                                    visited_dirs.insert(item_url.clone());
                                    queue.push_back(item_url);
                                }
                            } else {
                                // Parse last modified - try multiple formats
                                let dt = if let Some(lm) = last_mod {
                                    // Try RFC 2822 first (e.g., "Mon, 15 Aug 2005 15:52:01 +0000")
                                    chrono::DateTime::parse_from_rfc2822(&lm)
                                        .map(|dt| dt.with_timezone(&chrono::Utc))
                                        .ok()
                                        .or_else(|| {
                                            // Try RFC 3339 / ISO 8601 (e.g., "2005-08-15T15:52:01Z")
                                            chrono::DateTime::parse_from_rfc3339(&lm)
                                                .map(|dt| dt.with_timezone(&chrono::Utc))
                                                .ok()
                                        })
                                        .or_else(|| {
                                            // Log parsing failure for debugging
                                            debug!("Failed to parse WebDAV last modified time: {}", lm);
                                            None
                                        })
                                } else {
                                    None
                                };
                                files.insert(item_url, dt);
                            }
                        }
                    } else {
                        warn!("WebDAV PROPFIND failed for {}: {}", current_url, res.status());
                    }
                }
                Err(e) => {
                    warn!("WebDAV request failed for {}: {}", current_url, e);
                }
            }
        }

        Ok(files.into_iter().collect())
    }

    fn parse_webdav_response(&self, xml: &str, base_url: &str) -> Vec<(String, bool, Option<String>)> {
        let mut items = Vec::new();
        let mut reader = Reader::from_str(xml);
        reader.trim_text(true);

        let mut in_response = false;
        let mut current_href = String::new();
        let mut is_collection = false;
        let mut current_last_mod = None;
        let mut buf = Vec::new();

        // Simple state machine
        // Structure: <response> <href>...</href> ... <resourcetype><collection/></resourcetype> <getlastmodified>...</getlastmodified> ... </response>
        
        loop {
            match reader.read_event_into(&mut buf) {
                Ok(Event::Start(e)) => {
                    match e.name().as_ref() {
                        b"D:response" | b"d:response" | b"response" => {
                            in_response = true;
                            current_href.clear();
                            is_collection = false;
                            current_last_mod = None;
                        }
                        b"D:href" | b"d:href" | b"href" => {
                            if in_response {
                                if let Ok(txt) = reader.read_text(e.name()) {
                                    current_href = txt.to_string();
                                }
                            }
                        }
                        b"D:collection" | b"d:collection" | b"collection" => {
                            if in_response {
                                is_collection = true;
                            }
                        }
                        b"D:getlastmodified" | b"d:getlastmodified" | b"getlastmodified" => {
                            if in_response {
                                if let Ok(txt) = reader.read_text(e.name()) {
                                    current_last_mod = Some(txt.to_string());
                                }
                            }
                        }
                        _ => {}
                    }
                }
                Ok(Event::Empty(e)) => {
                    match e.name().as_ref() {
                        b"D:collection" | b"d:collection" | b"collection" => {
                            if in_response {
                                is_collection = true;
                            }
                        }
                        _ => {}
                    }
                }
                Ok(Event::End(e)) => {
                    match e.name().as_ref() {
                        b"D:response" | b"d:response" | b"response" => {
                            if in_response && !current_href.is_empty() {
                                // Resolve href to full URL
                                // href might be relative or absolute path
                                let full_url = self.resolve_webdav_url(base_url, &current_href);
                                items.push((full_url, is_collection, current_last_mod.clone()));
                            }
                            in_response = false;
                        }
                        _ => {}
                    }
                }
                Ok(Event::Eof) => break,
                Err(_) => break,
                _ => {}
            }
            buf.clear();
        }

        items
    }

    fn resolve_webdav_url(&self, base_request_url: &str, href: &str) -> String {
        // href typically looks like "/remote.php/webdav/folder/file.mp3"
        // base_request_url looks like "https://host/remote.php/webdav/folder"
        
        // We need to construct the full URL.
        // If href is already a full URL, return it.
        if href.starts_with("http") {
            return href.to_string();
        }

        // Parse base URL to get scheme and host
        if let Ok(base) = url::Url::parse(base_request_url) {
            if let Ok(joined) = base.join(href) {
                return joined.to_string();
            }
        }
        
        // Fallback simple join
        href.to_string()
    }

    fn decode_url_path(&self, url: &str) -> String {
        match urlencoding::decode(url) {
            Ok(s) => s.into_owned(),
            Err(_) => {
                // If standard decode fails (e.g. invalid UTF-8 from GBK), 
                // we try to decode manually to bytes and then use lossy conversion.
                let mut bytes = Vec::new();
                let input_bytes = url.as_bytes();
                let mut i = 0;
                
                while i < input_bytes.len() {
                    if input_bytes[i] == b'%' && i + 2 < input_bytes.len() {
                        if let Ok(slice) = std::str::from_utf8(&input_bytes[i+1..i+3]) {
                            if let Ok(b) = u8::from_str_radix(slice, 16) {
                                bytes.push(b);
                                i += 3;
                                continue;
                            }
                        }
                    }
                    bytes.push(input_bytes[i]);
                    i += 1;
                }
                String::from_utf8_lossy(&bytes).into_owned()
            }
        }
    }

    async fn process_webdav_book(
        &self,
        library: &crate::db::models::Library,
        dir_url: &str,
        file_urls: &[String],
        metadata_files: &[String],
        _task_id: Option<&str>,
        scraper_config: &crate::db::models::ScraperConfig,
        existing_info: Option<(String, i32, Option<String>)>,
    ) -> Result<(String, ScanStatus)> {
        // Derive title from directory name
        // Decode URL to handle percent-encoded characters (e.g. Chinese)
        let decoded_url = self.decode_url_path(dir_url);
        let dir_name_title = decoded_url.trim_end_matches('/').split('/').last().unwrap_or("Unknown Book").to_string();
        let (cleaned_dir_name, _) = self.text_cleaner.clean_chapter_title(&dir_name_title, None);
        
        // No local path, use URL as path
        // We use the original URL as path to ensure connectivity, but StorageService needs to handle it correctly
        let path = dir_url.to_string();

        // Check if book exists
        let mut hasher = Sha256::new();
        hasher.update(path.as_bytes());
        let path_hash = format!("{:x}", hasher.finalize());
        let book_hash = path_hash.clone();
        
        // Prepare temp directory for WebDAV book metadata and cover
        // Structure: temp/{book_hash}/
        let temp_book_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
            .join("temp").join(&book_hash);
        if !temp_book_dir.exists() {
            std::fs::create_dir_all(&temp_book_dir).ok();
        }
        
        // Extended metadata fields for WebDAV (to be written to metadata.json)
        let mut subtitle: Option<String> = None;
        let mut published_year: Option<String> = None;
        let mut published_date: Option<String> = None;
        let mut publisher: Option<String> = None;
        let mut isbn: Option<String> = None;
        let mut asin: Option<String> = None;
        let mut language: Option<String> = None;
        let mut explicit: bool = false;
        let mut abridged: bool = false;
        let mut json_tags: Vec<String> = Vec::new();
        let mut json_series: Vec<String> = Vec::new();
        
        // Extract metadata from WebDAV files (try multiple files if needed)
        let (mut meta_album, mut _meta_chapter_title, mut meta_author, mut meta_narrator, mut meta_cover_url, _meta_duration) = if !file_urls.is_empty() {
            // 尝试多个文件，直到找到完整的元数据（包括封面）
            let mut album = String::new();
            let mut title = String::new();
            let mut author = None;
            let mut narrator = None;
            let mut cover_url = None;
            let mut duration = 0;
            
            let max_files = std::cmp::min(file_urls.len(), 3); // 最多尝试 3 个文件
            
            for (index, file_url) in file_urls.iter().take(max_files).enumerate() {
                let (a, t, au, n, c, d) = self.extract_webdav_metadata(
                    library, 
                    file_url, 
                    if index == 0 { Some(&temp_book_dir) } else { None }, // 只在第一个文件时保存封面到目录
                    scraper_config.extract_audio_cover
                ).await;
                
                if index == 0 {
                    tracing::debug!("从第一个 WebDAV 文件提取元数据");
                } else {
                    tracing::debug!("从第 {} 个 WebDAV 文件补充元数据", index + 1);
                }
                
                // 只在第一个文件或缺失时提取基本元数据
                if index == 0 || album.is_empty() {
                    if !a.is_empty() { album = a; }
                }
                if index == 0 || title.is_empty() {
                    if !t.is_empty() { title = t; }
                }
                if index == 0 || author.is_none() {
                    if au.is_some() { author = au; }
                }
                if index == 0 || narrator.is_none() {
                    if n.is_some() { narrator = n; }
                }
                if index == 0 {
                    duration = d;
                }
                
                // 封面：如果还没有找到，继续尝试
                if scraper_config.extract_audio_cover && cover_url.is_none() {
                    if c.is_some() {
                        cover_url = c;
                        if index > 0 {
                            tracing::info!("从第 {} 个 WebDAV 文件中找到封面", index + 1);
                        }
                    }
                }
                
                // 如果已经找到了所有需要的元数据，就停止
                let has_basic_metadata = !album.is_empty() || author.is_some();
                let has_cover_if_needed = !scraper_config.extract_audio_cover || cover_url.is_some();
                
                if has_basic_metadata && has_cover_if_needed {
                    tracing::debug!("已找到完整 WebDAV 元数据，停止尝试其他文件");
                    break;
                }
            }
            
            (album, title, author, narrator, cover_url, duration)
        } else {
             (String::new(), String::new(), None, None, None, 0)
        };

        // Try to fetch and parse metadata.json and book.nfo from WebDAV
        // We do this by downloading them to temp_book_dir
        if let Some(storage) = &self.storage_service {
             // Decryption key
             let key = self.encryption_key.as_deref().unwrap_or(&[0u8; 32]);
             
             for meta_url in metadata_files {
                 let filename = meta_url.split('/').last().unwrap_or_default();
                 if filename == "metadata.json" || filename == "book.nfo" {
                     let temp_path = temp_book_dir.join(filename);
                     if let Ok((mut reader, _)) = storage.get_webdav_reader(library, meta_url, None, key).await {
                         if let Ok(mut file) = tokio::fs::File::create(&temp_path).await {
                             let _ = tokio::io::copy(&mut reader, &mut file).await;
                         }
                     }
                 }
             }
        }

        // Read metadata.json if downloaded
        let mut json_chapters: Option<Vec<crate::core::metadata_writer::AudiobookshelfChapter>> = None;
        if let Ok(Some(json_meta)) = crate::core::metadata_writer::read_metadata_json(&temp_book_dir) {
             if let Some(t) = json_meta.title { meta_album = t; }
             if !json_meta.authors.is_empty() { meta_author = Some(json_meta.authors[0].clone()); }
             if !json_meta.narrators.is_empty() { meta_narrator = Some(json_meta.narrators[0].clone()); }
             if !json_meta.series.is_empty() { json_series = json_meta.series; }
             if !json_meta.tags.is_empty() { json_tags = json_meta.tags; }
             
             // Store chapters for later use
             if !json_meta.chapters.is_empty() {
                 json_chapters = Some(json_meta.chapters);
             }
             
             // Extended
             subtitle = json_meta.subtitle;
             published_year = json_meta.published_year;
             published_date = json_meta.published_date;
             publisher = json_meta.publisher;
             isbn = json_meta.isbn;
             asin = json_meta.asin;
             language = json_meta.language;
             explicit = json_meta.explicit;
             abridged = json_meta.abridged;
        }

        // Read book.nfo if downloaded (merge, lower priority than json usually, but let's check)
        // If metadata.json was present, we prefer it.
        // If not, we check nfo.
        let nfo_path = temp_book_dir.join("book.nfo");
        if nfo_path.exists() {
             if let Ok(nfo_meta) = self.nfo_manager.read_book_nfo(&nfo_path) {
                 if meta_album.is_empty() && !nfo_meta.title.is_empty() { meta_album = nfo_meta.title; }
                 if meta_author.is_none() && !nfo_meta.author.is_none() { meta_author = nfo_meta.author; }
                 if meta_narrator.is_none() && !nfo_meta.narrator.is_none() { meta_narrator = nfo_meta.narrator; }
                 if meta_cover_url.is_none() && !nfo_meta.cover_url.is_none() { meta_cover_url = nfo_meta.cover_url; }
             }
        }
        
        // Also check if there's a local cover image directly in the webdav folder
        // For webdav, we downloaded metadata_files, let's see if there is a cover
        for meta_url in metadata_files {
            let filename = meta_url.split('/').last().unwrap_or_default().to_lowercase();
            if ["cover.jpg", "cover.png", "cover.jpeg", "cover.webp", "folder.jpg"].contains(&filename.as_str()) {
                // We don't download it yet, just store the URL so the frontend can access it via WebDAV proxy or similar.
                // Wait, cover_url needs to be accessible. WebDAV urls are not directly accessible by frontend unless proxied.
                // Actually, our API serves cover_url directly if it's a URL or path.
                // We can set it to the meta_url.
                if meta_cover_url.is_none() {
                    meta_cover_url = Some(meta_url.clone());
                }
                break;
            }
        }

        let mut book_title;
        let source;

        // Title Selection Logic: Priority Local Metadata > ID3 > Fallback
        if scraper_config.use_filename_as_title {
            book_title = cleaned_dir_name.clone();
            source = MetadataSource::Fallback;
        } else if !meta_album.trim().is_empty() && !meta_album.to_lowercase().starts_with("track") {
            // Priority 1/2: metadata.json or ID3 (already merged above)
            // Bugfix: Ignore generic "Track XX" titles from ID3 metadata
            book_title = meta_album.clone();
            source = MetadataSource::FileMetadata;
        } else {
            book_title = cleaned_dir_name.clone();
            source = MetadataSource::Fallback;
        }
        
        // Clean the book title (whether from ID3 or Directory)
        book_title = self.text_cleaner.clean_filename(&book_title);

        let (book_id, manual_corrected) = if let Some((ref id, mc, _)) = existing_info {
            (id.clone(), mc == 1)
        } else if let Ok(Some(book)) = self.book_repo.find_by_hash(&path_hash).await {
            (book.id, book.manual_corrected == 1)
        } else {
            (Uuid::new_v4().to_string(), false)
        };

        // Create or Update book
        let mut book = crate::db::models::Book {
            id: book_id.clone(),
            library_id: library.id.clone(),
            title: Some(book_title.clone()),
            author: meta_author.or(Some("Unknown".to_string())),
            narrator: meta_narrator,
            cover_url: meta_cover_url,
            description: None,
            created_at: chrono::Utc::now().to_rfc3339(),
            path: path.clone(),
            hash: path_hash.clone(),
            theme_color: None,
            skip_intro: 0,
            skip_outro: 0,
            tags: None,
            genre: None,
            year: published_year.as_ref().and_then(|y| y.parse::<i32>().ok()),
            manual_corrected: if manual_corrected { 1 } else { 0 },
            match_pattern: None,
            chapter_regex: None,
        };

        // If manual corrected, we should preserve existing fields.
        // We need to fetch the existing book to do that properly if we are updating.
        if manual_corrected {
            if let Ok(Some(existing_book)) = self.book_repo.find_by_id(&book_id).await {
                book.title = existing_book.title;
                book.author = existing_book.author;
                book.narrator = existing_book.narrator;
                book.description = existing_book.description;
                book.tags = existing_book.tags;
                book.cover_url = existing_book.cover_url;
                book.theme_color = existing_book.theme_color;
                book.chapter_regex = existing_book.chapter_regex;
            }
        }

        // Run scraper if enabled and NOT manual corrected
        if !manual_corrected {
            if let Some(scraper_service) = &self.scraper_service {
                 match scraper_service.scrape_book_metadata(&book_title, scraper_config).await {
                    Ok(detail) => {
                        if !detail.title.is_empty() {
                            // Overwrite if ID3 is empty OR if we are using Fallback source (Directory Name)
                            // Requirement: "If using directory name as book name, then scraped data > ID3 data"
                            if source == MetadataSource::Fallback || meta_album.trim().is_empty() {
                                book.title = Some(detail.title);
                            }
                        }
                        
                        if !detail.author.is_empty() {
                            // Overwrite if Fallback source (Directory Name) OR if current is Unknown/None
                            if source == MetadataSource::Fallback || book.author.as_deref() == Some("Unknown") || book.author.is_none() {
                                book.author = Some(detail.author);
                            }
                        }
                        
                        if !detail.intro.is_empty() {
                            if source == MetadataSource::Fallback || book.description.is_none() {
                                book.description = Some(detail.intro);
                            }
                        }
                        
                        if detail.cover_url.is_some() {
                            if source == MetadataSource::Fallback || book.cover_url.is_none() {
                                book.cover_url = detail.cover_url;
                            }
                        }
                        
                        if detail.narrator.is_some() {
                            if source == MetadataSource::Fallback || book.narrator.is_none() {
                                book.narrator = detail.narrator;
                            }
                        }
                        
                        if !detail.tags.is_empty() {
                            if source == MetadataSource::Fallback || book.tags.is_none() {
                                book.tags = Some(detail.tags.join(","));
                            }
                        }
                        
                        // Capture extended metadata for metadata.json
                        if detail.subtitle.is_some() { subtitle = detail.subtitle; }
                        if detail.published_year.is_some() { published_year = detail.published_year; }
                        if detail.published_date.is_some() { published_date = detail.published_date; }
                        if detail.publisher.is_some() { publisher = detail.publisher; }
                        if detail.isbn.is_some() { isbn = detail.isbn; }
                        if detail.asin.is_some() { asin = detail.asin; }
                        if detail.language.is_some() { language = detail.language; }
                        if detail.explicit { explicit = true; }
                        if detail.abridged { abridged = true; }
                    },
                    Err(e) => {
                        warn!("Scraper failed for WebDAV book {}: {}", book_title, e);
                    }
                }
            }
        }

        // Calculate theme color if cover exists
        // If cover is from scraper (http), we fetch it.
        // If cover is local (relative), we fetch it from WebDAV.
        // Currently scraper returns HTTP URLs usually.
        // But if we want to support cover.jpg in WebDAV folder:
        // We need to implement find_cover_image for WebDAV.
        
        // For now, if scraper provided cover_url, we try to calculate color.
        if !manual_corrected {
            if let Some(ref url) = book.cover_url {
                let cover_path = if url.starts_with("//") {
                    format!("https:{}", url)
                } else {
                    url.clone()
                };
                if let Ok(Some(color)) = crate::core::color::calculate_theme_color_with_client(&cover_path, &self.http_client).await {
                    book.theme_color = Some(color);
                }
            }
        }

        let mut status = ScanStatus::Created;
        // Check if existing book (by ID check above)
        if existing_info.is_some() {
             if !manual_corrected {
                 // Preserve chapter_regex from existing book if not set in metadata
                 if book.chapter_regex.is_none() {
                     if let Ok(Some(existing)) = self.book_repo.find_by_id(&book_id).await {
                         book.chapter_regex = existing.chapter_regex;
                     }
                 }
                 self.book_repo.update(&book).await?;
                 status = ScanStatus::Updated;
             } else {
                 status = ScanStatus::Skipped;
             }
        } else if let Ok(Some(existing)) = self.book_repo.find_by_id(&book_id).await {
             if !manual_corrected {
                 // Preserve chapter_regex from existing book if not set in metadata
                 if book.chapter_regex.is_none() {
                     book.chapter_regex = existing.chapter_regex;
                 }
                 self.book_repo.update(&book).await?;
                 status = ScanStatus::Updated;
             } else {
                 status = ScanStatus::Skipped;
             }
        } else {
             self.book_repo.create(&book).await?;
        }

        // Create chapters
        let mut main_counter = 0;
        let mut extra_counter = 0;

        // Fetch book to check for regex rule
        let regex_pattern = if manual_corrected {
            self.book_repo.find_by_id(&book_id).await?.and_then(|b| b.chapter_regex)
        } else {
            book.chapter_regex.clone()
        };

        let chapter_regex = regex_pattern.and_then(|p| regex::Regex::new(&p).ok());
        
        // Track processed chapter IDs to find deleted ones
        let mut processed_chapter_ids = HashSet::new();
        let mut chapters_changed = false;
        
        // Check if we can use JSON chapters
        let use_json_chapters = if let Some(ref chapters) = json_chapters {
            if chapters.len() == file_urls.len() {
                info!("Using metadata.json chapters for WebDAV book: {}", book_title);
                true
            } else {
                if !chapters.is_empty() {
                    warn!("metadata.json chapter count ({}) does not match file count ({}) for WebDAV book {}. Ignoring JSON chapters.", 
                          chapters.len(), file_urls.len(), book_title);
                }
                false
            }
        } else {
            false
        };

        for (index, file_url) in file_urls.iter().enumerate() {
            // Decode filename for title
            let decoded_file_url = self.decode_url_path(file_url);
            let filename = decoded_file_url.split('/').last().unwrap_or("chapter").to_string();
            
            // Regex extraction
            let mut regex_idx = None;
            let mut regex_title = None;
            
            if let Some(re) = &chapter_regex {
                if let Some(caps) = re.captures(&filename) {
                        if let Some(m) = caps.get(1) {
                            if let Ok(idx) = m.as_str().parse::<i32>() {
                                regex_idx = Some(idx);
                            }
                        }
                        if let Some(m) = caps.get(2) {
                            regex_title = Some(m.as_str().to_string());
                        }
                }
            }

            // Check if chapter exists to avoid duplicates
            let mut ch_hasher = Sha256::new();
            ch_hasher.update(file_url.as_bytes());
            let ch_hash = format!("{:x}", ch_hasher.finalize());
            
            // Extract metadata from WebDAV file (download header chunk)
            // Optimization: Skip metadata extraction if chapter exists and not forced?
            // Extract metadata or use JSON chapters
            let (meta_title, meta_duration) = if use_json_chapters {
                if let Some(ref chapters) = json_chapters {
                    if index < chapters.len() {
                        let chapter = &chapters[index];
                        let duration = ((chapter.end - chapter.start).round() as i32).max(0);
                        (chapter.title.clone(), duration)
                    } else {
                        // Fallback (should not happen)
                        let (_, t, _, _, _, d) = self.extract_webdav_metadata(library, file_url, None, scraper_config.extract_audio_cover).await;
                        (t, d)
                    }
                } else {
                    // Fallback (should not happen)
                    let (_, t, _, _, _, d) = self.extract_webdav_metadata(library, file_url, None, scraper_config.extract_audio_cover).await;
                    (t, d)
                }
            } else {
                // Extract from WebDAV file
                let (_, t, _, _, _, d) = self.extract_webdav_metadata(library, file_url, None, scraper_config.extract_audio_cover).await;
                (t, d)
            };
            
            // Determine Title
            let raw_title = if use_json_chapters {
                // Use JSON title directly
                meta_title
            } else if let Some(rt) = regex_title {
                rt
            } else if scraper_config.use_filename_as_title {
                filename
            } else if !meta_title.trim().is_empty() && !meta_title.to_lowercase().starts_with("track") {
                meta_title
            } else {
                filename
            };
            
            // Clean Title (skip if using JSON)
            let (final_title, is_extra) = if use_json_chapters {
                (raw_title, false)
            } else {
                self.text_cleaner.clean_chapter_title(&raw_title, book.title.as_deref())
            };
            
            let counter_idx = if is_extra {
                 extra_counter += 1;
                 extra_counter
            } else {
                 main_counter += 1;
                 main_counter
            };

            let chapter_idx = regex_idx.unwrap_or(counter_idx);

            let chapter = crate::db::models::Chapter {
                id: Uuid::new_v4().to_string(),
                book_id: book_id.clone(),
                title: Some(final_title),
                path: file_url.clone(),
                duration: Some(meta_duration),
                chapter_index: Some(chapter_idx),
                is_extra: if is_extra { 1 } else { 0 },
                hash: Some(ch_hash.clone()),
                created_at: chrono::Utc::now().to_rfc3339(),
                manual_corrected: 0,
            };

            // Check if chapter exists by hash (Deduplication)
            if let Ok(Some(mut existing)) = self.chapter_repo.find_by_hash(&ch_hash).await {
                // Update existing chapter
                // Check Lock
                if existing.manual_corrected == 0 {
                    existing.title = chapter.title;
                    existing.chapter_index = chapter.chapter_index;
                }
                existing.duration = chapter.duration;
                existing.book_id = book_id.clone(); // Ensure it belongs to this book
                self.chapter_repo.update(&existing).await?;
                processed_chapter_ids.insert(existing.id.clone());
                chapters_changed = true;
            } else {
                self.chapter_repo.create(&chapter).await?;
                processed_chapter_ids.insert(chapter.id.clone());
                chapters_changed = true;
            }
        }
        
        // Handle deleted chapters
        if let Ok(existing_chapters) = self.chapter_repo.find_by_book(&book_id).await {
            for ch in existing_chapters {
                if !processed_chapter_ids.contains(&ch.id) {
                    info!("Removing missing chapter from DB: {:?}", ch.path);
                    if let Err(e) = self.chapter_repo.delete(&ch.id).await {
                        warn!("Failed to delete missing chapter {}: {}", ch.id, e);
                    } else {
                        chapters_changed = true;
                    }
                }
            }
        }
        
        // Process Series
        if !json_series.is_empty() {
            for series_title_raw in json_series {
                let series_title_raw = series_title_raw.trim();
                if series_title_raw.is_empty() { continue; }
                
                // Parse series title and optional sequence number
                let mut series_title = series_title_raw.to_string();
                let mut explicit_order = None;
                
                if let Some(idx) = series_title_raw.rfind(" #") {
                    let (name_part, num_part) = series_title_raw.split_at(idx);
                    let num_str = num_part[2..].trim();
                    if let Ok(order) = num_str.parse::<i32>() {
                        series_title = name_part.trim().to_string();
                        explicit_order = Some(order);
                    }
                }
                
                // Find or create series atomically (globally across all libraries to handle concurrent syncs and multiple libraries)
                let new_series = crate::db::models::Series {
                    id: Uuid::new_v4().to_string(),
                    library_id: library.id.clone(),
                    title: series_title.clone(),
                    author: book.author.clone(), // Initial author from first found book
                    narrator: book.narrator.clone(),
                    cover_url: book.cover_url.clone(),
                    description: None,
                    created_at: chrono::Utc::now().to_rfc3339(),
                    updated_at: chrono::Utc::now().to_rfc3339(),
                };
                let series = self.series_repo.find_or_create_by_title(new_series).await?;
                
                // Link book to series if not already linked
                let books = self.series_repo.find_books_by_series(&series.id).await?;
                if let Some((_, current_order)) = books.iter().find(|(b, _)| b.id == book_id) {
                     // Already linked, update order if explicit order changed
                     if let Some(o) = explicit_order {
                         if *current_order != o {
                             self.series_repo.add_book(crate::db::models::SeriesBook {
                                 series_id: series.id.clone(),
                                 book_id: book_id.clone(),
                                 book_order: o,
                             }).await?;
                         }
                     }
                } else {
                     // Not linked, insert it
                     let order = if let Some(o) = explicit_order {
                         o
                     } else {
                         books.len() as i32 + 1
                     };
                     
                     self.series_repo.add_book(crate::db::models::SeriesBook {
                         series_id: series.id.clone(),
                         book_id: book_id.clone(),
                         book_order: order,
                     }).await?;

                     // If no explicit order, resort all books in series by natural order of title
                     if explicit_order.is_none() {
                         let mut all_books = self.series_repo.find_books_by_series(&series.id).await?;
                         all_books.sort_by(|a, b| {
                             let t1 = a.0.title.as_deref().unwrap_or("");
                             let t2 = b.0.title.as_deref().unwrap_or("");
                             natord::compare(t1, t2)
                         });
                         
                         let new_orders: Vec<(String, i32)> = all_books.into_iter()
                             .enumerate()
                             .map(|(i, (b, _))| (b.id, (i + 1) as i32))
                             .collect();
                             
                         self.series_repo.update_book_orders(&series.id, new_orders).await?;
                     }
                     
                     // DO NOT update series metadata based on subsequent books to avoid instability
                     // Series metadata should only be set on creation or manual update
                }
            }
        }

        // Fetch all chapters to generate metadata.json correctly with cumulative times
        let chapters = self.chapter_repo.find_by_book(&book_id).await?;
        let mut sorted_chapters = chapters;
        sorted_chapters.sort_by(|a, b| {
            a.chapter_index.unwrap_or(0).cmp(&b.chapter_index.unwrap_or(0))
                .then_with(|| natord::compare(a.title.as_deref().unwrap_or(""), b.title.as_deref().unwrap_or("")))
        });

        let mut abs_chapters = Vec::new();
        let mut current_time = 0.0;
        for (idx, ch) in sorted_chapters.iter().enumerate() {
            let duration = ch.duration.unwrap_or(0) as f64;
            abs_chapters.push(crate::core::metadata_writer::AudiobookshelfChapter {
                id: idx as u32,
                start: current_time,
                end: current_time + duration,
                title: ch.title.clone().unwrap_or_default(),
            });
            current_time += duration;
        }
        
        // Write metadata.json to temp dir for WebDAV book
        if scraper_config.metadata_writing_enabled {
            // Try to preserve existing tags from temp dir if metadata.json exists
            if let Ok(Some(existing_meta)) = crate::core::metadata_writer::read_metadata_json(&temp_book_dir) {
                if !existing_meta.tags.is_empty() {
                    json_tags = existing_meta.tags;
                }
            }

            let extended_meta = crate::core::metadata_writer::ExtendedMetadata {
                subtitle: subtitle.clone(),
                published_year,
                published_date,
                publisher,
                isbn,
                asin,
                language,
                explicit,
                abridged,
                tags: json_tags,
            };
            
            // Get series for this book
            let series_list = self.series_repo.find_series_by_book(&book_id).await.unwrap_or_default();
            let mut series_titles = Vec::new();
            for series in series_list {
                let formatted_title = if let Ok(books) = self.series_repo.find_books_by_series(&series.id).await {
                    if let Some((_, order)) = books.iter().find(|(b, _)| b.id == book_id) {
                        format!("{} #{}", series.title, order)
                    } else {
                        series.title.clone()
                    }
                } else {
                    series.title.clone()
                };
                
                if !series_titles.contains(&formatted_title) {
                    series_titles.push(formatted_title);
                }
            }
            
            let metadata_json = crate::core::metadata_writer::AudiobookshelfMetadata::new(
                &book,
                abs_chapters,
                extended_meta,
                series_titles
            );
            
            if let Err(e) = crate::core::metadata_writer::write_metadata_json(&temp_book_dir, &metadata_json) {
                warn!("Failed to write metadata.json for WebDAV book {}: {}", book_title, e);
            }
        }

        // Write NFO to temp dir for WebDAV book
        if scraper_config.nfo_writing_enabled {
            let mut metadata = BookMetadata::new(
                book.title.clone().unwrap_or_default(),
                "ting-reader".to_string(),
                book.id.clone(),
                0,
            );
            metadata.author = book.author.clone();
            metadata.narrator = book.narrator.clone();
            metadata.intro = book.description.clone();
            metadata.cover_url = book.cover_url.clone();
            metadata.subtitle = subtitle; // Pass subtitle to NFO if available
            
            if let Some(tags_str) = &book.tags {
                 metadata.tags.items = tags_str.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
            }
            
            if let Err(e) = self.nfo_manager.write_book_nfo_to_dir(&temp_book_dir, &metadata) {
                warn!("Failed to write NFO for WebDAV book {}: {}", book.title.unwrap_or_default(), e);
            }
        }
        
        let final_status = match status {
            ScanStatus::Created => ScanStatus::Created,
            _ => if chapters_changed { ScanStatus::Updated } else { status }
        };
        
        Ok((book_id, final_status))
    }

    async fn extract_webdav_metadata(
        &self,
        library: &crate::db::models::Library,
        file_url: &str,
        cover_target_dir: Option<&Path>,
        extract_cover: bool,
    ) -> (String, String, Option<String>, Option<String>, Option<String>, i32) {
        // Returns: (album, title, author, narrator, cover_url, duration)
        if let Some(storage) = &self.storage_service {
            // Determine temp file path
            let ext = Path::new(file_url).extension().and_then(|e| e.to_str()).unwrap_or("tmp");
            let temp_dir = std::env::temp_dir();
            let temp_filename = format!("ting_scan_{}.{}", Uuid::new_v4(), ext);
            let temp_path = temp_dir.join(&temp_filename);
            
            // Decryption key
            let key = self.encryption_key.as_deref().unwrap_or(&[0u8; 32]);
            
            // 1. Probe Header
            // We need enough bytes to detect ID3v2 header and size.
            // ID3v2 header is 10 bytes. Size is encoded in bytes 6-9 (Synchsafe integer).
            // Let's probe 64KB first, usually enough for metadata, but maybe not cover.
            let probe_size = 64 * 1024; 
            let mut required_size = probe_size as u64; // Default fallback
            let mut probe_data = Vec::with_capacity(probe_size);
            
            if let Ok((mut reader, _)) = storage.get_webdav_reader(library, file_url, Some((0, probe_size as u64)), key).await {
                let mut buf = vec![0u8; probe_size];
                if let Ok(n) = reader.read(&mut buf).await {
                    probe_data.extend_from_slice(&buf[..n]);
                }
            }
            
            if !probe_data.is_empty() {
                // Check for ID3v2 header
                if probe_data.len() >= 10 && &probe_data[0..3] == b"ID3" {
                    // Parse ID3v2 size
                    // Size is 4 bytes (6-9), each byte uses 7 bits (MSB is 0)
                    let size_bytes = &probe_data[6..10];
                    let tag_size = ((size_bytes[0] as u32) << 21) |
                                   ((size_bytes[1] as u32) << 14) |
                                   ((size_bytes[2] as u32) << 7) |
                                   (size_bytes[3] as u32);
                    
                    // Total size = Header (10) + Tag Size + Footer (10, optional but we ignore for read size)
                    // We need to download at least this much to get full ID3 tag including cover
                    let total_id3_size = 10 + tag_size as u64;
                    if total_id3_size > required_size {
                        required_size = total_id3_size;
                        debug!("Detected ID3v2 tag size: {} bytes", required_size);
                    }
                }

                // Ask plugins for required size (e.g. for encrypted formats)
                let plugins = self.plugin_manager.find_plugins_by_type(PluginType::Format).await;
                for plugin in plugins {
                    let params = serde_json::json!({
                        "header_base64": base64::engine::general_purpose::STANDARD.encode(&probe_data)
                    });
                    
                    if let Ok(result) = self.plugin_manager.call_format(&plugin.id, FormatMethod::GetMetadataReadSize, params).await {
                        if let Some(size) = result.get("size").and_then(|v| v.as_u64()) {
                             if size > required_size {
                                 required_size = size;
                                 debug!("Plugin {} requested {} bytes for metadata", plugin.name, required_size);
                             }
                        }
                    }
                }
            }
            
            // 2. Download required data
            if let Ok(mut file) = tokio::fs::File::create(&temp_path).await {
                // Write probe data
                if file.write_all(&probe_data).await.is_ok() {
                    // Download rest if needed
                    if required_size > probe_data.len() as u64 {
                        let start = probe_data.len() as u64;
                        let end = required_size;
                        
                        if let Ok((mut reader, _)) = storage.get_webdav_reader(library, file_url, Some((start, end)), key).await {
                            let _ = tokio::io::copy(&mut reader, &mut file).await;
                        }
                    }
                    
                    // Extract metadata
                    let mut album = String::new();
                    let mut title = String::new();
                    let mut author = None;
                    let mut narrator = None;
                    let mut duration = 0;
                    let mut cover_url = None;
                    
                    let ext = file_url.split('.').last().unwrap_or("").to_lowercase();
                    let mut use_ffprobe = false;
                    
                    // 策略：优先使用格式插件处理所有格式（包括 MP3/M4A/WMA/FLAC 等）
                    // 格式插件使用 lofty 等库，对部分文件支持更好，不会报 "end of stream" 错误
                    
                    // 1. 查找支持该格式的插件
                    let plugins = self.plugin_manager.find_plugins_by_type(PluginType::Format).await;
                    let mut plugin_handled = false;
                    
                    for plugin in plugins {
                        // 检查插件是否声明支持该扩展名
                        let supports_ext = plugin.supported_extensions.as_ref()
                            .map(|exts| exts.iter().any(|e| e.eq_ignore_ascii_case(&ext)))
                            .unwrap_or(false);
                        
                        if !supports_ext {
                            continue;
                        }
                        
                        // 交由格式插件处理
                        let params = serde_json::json!({
                            "file_path": temp_path.to_string_lossy(),
                            "extract_cover": extract_cover
                        });
                        
                        if let Ok(result) = self.plugin_manager.call_format(&plugin.id, FormatMethod::ExtractMetadata, params).await {
                            tracing::debug!("使用格式插件 {} 处理 {} 文件", plugin.name, ext);
                            
                            // 提取元数据
                            if let Some(a) = result.get("album").and_then(|v| v.as_str()) {
                                if !a.trim().is_empty() { album = a.to_string(); }
                            }
                            if let Some(t) = result.get("title").and_then(|v| v.as_str()) {
                                if !t.trim().is_empty() { title = t.to_string(); }
                            }
                            if let Some(au) = result.get("author").and_then(|v| v.as_str()) {
                                if !au.trim().is_empty() { author = Some(au.to_string()); }
                            }
                            if let Some(n) = result.get("narrator").and_then(|v| v.as_str()) {
                                if !n.trim().is_empty() { narrator = Some(n.to_string()); }
                            }
                            if let Some(dur) = result.get("duration").and_then(|v| v.as_f64()) {
                                duration = dur.round() as i32;
                                if duration > 0 {
                                    tracing::debug!("格式插件 {} 获取到时长: {} 秒", plugin.name, duration);
                                }
                            }
                            if let Some(c) = result.get("cover_url").and_then(|v| v.as_str()) {
                                if !c.trim().is_empty() { cover_url = Some(c.to_string()); }
                            }
                            
                            plugin_handled = true;
                            break;
                        }
                    }
                    
                    // 2. 如果没有插件处理，且是 MP3 文件，尝试使用 ID3 库（对部分文件支持好）
                    if !plugin_handled && ext == "mp3" {
                        if let Ok(tag) = id3::Tag::read_from_path(&temp_path) {
                            debug!("使用 ID3 库处理 MP3 文件");
                            if let Some(t) = tag.album() { 
                                if !t.trim().is_empty() { album = t.to_string(); }
                            }
                            if let Some(t) = tag.title() { 
                                if !t.trim().is_empty() { title = t.to_string(); }
                            }
                            
                            // Author logic: Album Artist > Artist
                            if let Some(t) = tag.album_artist() { 
                                if !t.trim().is_empty() { author = Some(t.to_string()); }
                            }
                            
                            if let Some(t) = tag.artist() {
                                 if !t.trim().is_empty() {
                                     if author.is_none() {
                                         author = Some(t.to_string());
                                     } else if author.as_deref() != Some(t) {
                                         narrator = Some(t.to_string());
                                     }
                                 }
                            }
                            
                            if let Some(d) = tag.duration() {
                                duration = (d / 1000) as i32;
                            }
                        }
                    }
                    
                    // 注意：不再调用 extract_chapter_metadata，因为它使用 Symphonia
                    // 对部分文件会报 "end of stream" 错误
                    
                    // 3. 验证时长合理性：与文件大小估算比对
                    if duration > 0 {
                        // 获取文件大小（通过之前的 WebDAV 请求已经获取）
                        // 我们可以发起一个 HEAD 请求或者使用 Range 请求获取
                        if let Ok((_, file_size)) = storage.get_webdav_reader(library, file_url, Some((0, 1)), key).await {
                            if file_size > 0 {
                                // 根据文件大小和格式估算时长
                                let estimated_duration = self.estimate_duration_by_size(file_size, &ext);
                                
                                if estimated_duration > 0 {
                                    let diff_ratio = (duration as f64 - estimated_duration as f64).abs() / estimated_duration as f64;
                                    
                                    if diff_ratio > 0.15 {
                                        // 差距超过15%，时长可能不准确，需要用 FFprobe 验证
                                        tracing::warn!(
                                            "WebDAV 文件 {} 时长差距过大 (探测: {} 秒, 估算: {} 秒, 差距: {:.1}%), 将使用 FFprobe",
                                            file_url, duration, estimated_duration, diff_ratio * 100.0
                                        );
                                        use_ffprobe = true;
                                    } else {
                                        tracing::debug!(
                                            "WebDAV 文件 {} 时长验证通过 (探测: {} 秒, 估算: {} 秒, 差距: {:.1}%)",
                                            file_url, duration, estimated_duration, diff_ratio * 100.0
                                        );
                                    }
                                }
                            }
                        }
                    } else {
                        // 无法从部分文件中获取时长，需要 FFprobe
                        use_ffprobe = true;
                        tracing::debug!("无法从部分文件获取 {} 时长，将使用 FFprobe", ext);
                    }
                    
                    // 4. Use FFprobe when needed (fallback or validation)
                    if use_ffprobe {
                        if let Some(ffmpeg_path) = self.plugin_manager.get_ffmpeg_path().await {
                            let ffprobe_path = {
                                let ffmpeg_dir = std::path::Path::new(&ffmpeg_path).parent();
                                if let Some(dir) = ffmpeg_dir {
                                    // ⭐ 跨平台：Windows 使用 ffprobe.exe，Linux/Mac 使用 ffprobe
                                    let ffprobe_name = if cfg!(target_os = "windows") {
                                        "ffprobe.exe"
                                    } else {
                                        "ffprobe"
                                    };
                                    
                                    let probe = dir.join(ffprobe_name);
                                    if probe.exists() {
                                        Some(probe.to_string_lossy().to_string())
                                    } else {
                                        None
                                    }
                                } else {
                                    None
                                }
                            };
                        
                            if let Some(ffprobe_path) = ffprobe_path {
                                // Add small delay before FFprobe to avoid overwhelming the server
                                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                            
                            // Build WebDAV URL with authentication
                            let webdav_url = if file_url.starts_with("http://") || file_url.starts_with("https://") {
                                url::Url::parse(file_url).ok()
                            } else {
                                None
                            };
                            
                            if let Some(mut url) = webdav_url {
                                // Add authentication to URL if present
                                if let (Some(username), Some(password)) = (&library.username, &library.password) {
                                    let decrypted_password = crate::core::crypto::decrypt(password, key)
                                        .unwrap_or_else(|_| password.clone());
                                    url.set_username(username).ok();
                                    url.set_password(Some(&decrypted_password)).ok();
                                }
                                
                                let url_str = url.to_string();
                                
                                match tokio::process::Command::new(&ffprobe_path)
                                    .arg("-v").arg("error")
                                    .arg("-show_entries").arg("format=duration")
                                    .arg("-of").arg("default=noprint_wrappers=1:nokey=1")
                                    .arg(&url_str)
                                    .output()
                                    .await
                                {
                                    Ok(output) if output.status.success() => {
                                        let duration_str = String::from_utf8_lossy(&output.stdout);
                                        if let Ok(dur) = duration_str.trim().parse::<f64>() {
                                            duration = dur.round() as i32;
                                            debug!("FFprobe 获取 WebDAV 文件时长: {} 秒 ({})", duration, ext);
                                        }
                                    }
                                    Ok(output) => {
                                        debug!("FFprobe 获取 WebDAV 时长失败: {}", String::from_utf8_lossy(&output.stderr));
                                    }
                                    Err(e) => {
                                        debug!("无法运行 FFprobe: {}", e);
                                    }
                                }
                            }
                            }
                        }
                    }
                    
                    if !extract_cover {
                        cover_url = None;
                    }

                    // Manually extract cover here since we have the temp file
                    let mut final_cover_url = cover_url;
                    
                    if extract_cover && final_cover_url.is_none() {
                         // Decide target directory
                         let (target_dir, use_hash_name) = if let Some(dir) = cover_target_dir {
                             (dir.to_path_buf(), false) // Use fixed name "cover.ext" inside dir
                         } else {
                             // Fallback to old behavior: temp/covers/{hash}.ext
                             let cache_dir = Path::new("./temp/covers");
                             if !cache_dir.exists() {
                                 let _ = std::fs::create_dir_all(cache_dir);
                             }
                             (cache_dir.to_path_buf(), true)
                         };
                         
                         // Ensure directory exists
                         if !target_dir.exists() {
                             let _ = std::fs::create_dir_all(&target_dir);
                         }
                         
                         // Check if cover file already exists (for non-hash mode)
                         if !use_hash_name {
                             let cover_extensions = ["jpg", "jpeg", "png", "webp", "gif"];
                             for ext in &cover_extensions {
                                 let cover_path = target_dir.join(format!("cover.{}", ext));
                                 if cover_path.exists() {
                                     debug!("Cover file already exists at {:?}, skipping extraction", cover_path);
                                     final_cover_url = Some(cover_path.to_string_lossy().replace('\\', "/"));
                                     break;
                                 }
                             }
                         }
                         
                         // Only extract if we didn't find an existing cover
                         if final_cover_url.is_none() {
                             // First try plugin-based extraction (supports M4A, etc.)
                             let ext = temp_path.extension()
                                 .and_then(|e| e.to_str())
                                 .unwrap_or("")
                                 .to_lowercase();
                             
                             let plugins = self.plugin_manager.find_plugins_by_type(PluginType::Format).await;
                             for plugin in plugins {
                                 let supports_ext = plugin.supported_extensions.as_ref()
                                     .map(|exts| exts.iter().any(|e| e.eq_ignore_ascii_case(&ext)))
                                     .unwrap_or(false);
                                 if !supports_ext { continue; }
                                 
                                 let params = serde_json::json!({ 
                                     "file_path": temp_path.to_string_lossy(), 
                                     "extract_cover": true 
                                 });
                                 
                                 if let Ok(result) = self.plugin_manager.call_format(&plugin.id, FormatMethod::ExtractMetadata, params).await {
                                     if let Some(c) = result.get("cover_url").and_then(|v| v.as_str()) {
                                         if !c.trim().is_empty() {
                                             // Plugin returned a cover path, use it
                                             final_cover_url = Some(c.to_string());
                                             break;
                                         }
                                     }
                                 }
                             }
                             
                             // Fallback to ID3 extraction (for MP3)
                             if final_cover_url.is_none() {
                                 if let Ok(tag) = id3::Tag::read_from_path(&temp_path) {
                                     if let Some(picture) = tag.pictures().next() {
                                         let ext = match picture.mime_type.as_str() {
                                             "image/png" => "png",
                                             "image/webp" => "webp",
                                             "image/gif" => "gif",
                                             _ => "jpg",
                                         };
                                         
                                         let target_path = if use_hash_name {
                                             // Generate hash from parent URL
                                             let parent_url = if let Some(idx) = file_url.rfind('/') {
                                                 &file_url[..idx]
                                             } else {
                                                 file_url
                                             };
                                             let mut hasher = Sha256::new();
                                             hasher.update(parent_url.as_bytes());
                                             let book_hash = format!("{:x}", hasher.finalize());
                                             target_dir.join(format!("{}.{}", book_hash, ext))
                                         } else {
                                             target_dir.join(format!("cover.{}", ext))
                                         };
                                         
                                         // Only write if not exists
                                         if !target_path.exists() {
                                             if std::fs::write(&target_path, &picture.data).is_ok() {
                                                 debug!("Saved WebDAV cover from ID3 to {:?}", target_path);
                                             }
                                         }
                                         final_cover_url = Some(target_path.to_string_lossy().replace('\\', "/"));
                                     }
                                 }
                             }
                         }
                    }

                    // Cleanup
                    let _ = tokio::fs::remove_file(&temp_path).await;
                    
                    return (album, title, author, narrator, final_cover_url, duration);
                }
            }
            
            // Ensure cleanup on failure
            if temp_path.exists() {
                 let _ = tokio::fs::remove_file(&temp_path).await;
            }
        }
        
        (String::new(), String::new(), None, None, None, 0)
    }
}
