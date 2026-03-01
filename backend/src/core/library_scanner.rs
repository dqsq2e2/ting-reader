//! Library scanner for discovering audiobooks
//!
//! This module provides functionality to scan library directories
//! and discover audiobook files, creating book and chapter records.

use crate::core::error::{Result, TingError};
use crate::db::models::{Book, Chapter};
use crate::db::repository::{BookRepository, ChapterRepository, LibraryRepository, TaskRepository, Repository};
use crate::core::services::ScraperService;
use crate::core::merge_service::MergeService;
use crate::core::text_cleaner::TextCleaner;
use crate::core::nfo_manager::{NfoManager, BookMetadata};
use crate::core::audio_streamer::AudioStreamer;
use crate::core::StorageService;
use crate::plugin::manager::{PluginManager, FormatMethod};
use crate::plugin::types::PluginType;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::collections::{HashMap, HashSet};
use walkdir::WalkDir;
use tracing::{debug, info, warn};
use uuid::Uuid;
use sha2::{Digest, Sha256};
use std::io::Read;
use quick_xml::events::Event;
use quick_xml::reader::Reader;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use base64::Engine;

/// Supported audio file extensions
// Removed hardcoded encrypted extensions. Plugins should declare their supported extensions.
const AUDIO_EXTENSIONS: &[&str] = &["mp3", "m4a", "m4b", "flac", "ogg", "wav", "opus", "wma", "aac"];

/// Standard audio extensions that can be handled by the default audio streamer
const STANDARD_EXTENSIONS: &[&str] = &["mp3", "m4a", "m4b", "flac", "ogg", "wav", "opus", "wma", "aac"];

#[derive(Debug, Clone, Copy, PartialEq)]
enum MetadataSource {
    Nfo,
    FileMetadata,
    Fallback,
}

/// Library scanner service
pub struct LibraryScanner {
    book_repo: Arc<BookRepository>,
    chapter_repo: Arc<ChapterRepository>,
    library_repo: Arc<LibraryRepository>,
    task_repo: Option<Arc<TaskRepository>>,
    text_cleaner: Arc<TextCleaner>,
    nfo_manager: Arc<NfoManager>,
    audio_streamer: Arc<AudioStreamer>,
    plugin_manager: Arc<PluginManager>,
    scraper_service: Option<Arc<ScraperService>>,
    storage_service: Option<Arc<StorageService>>,
    merge_service: Option<Arc<MergeService>>,
    encryption_key: Option<Arc<[u8; 32]>>,
    http_client: reqwest::Client,
}

impl LibraryScanner {
    /// Create a new library scanner
    pub fn new(
        book_repo: Arc<BookRepository>,
        chapter_repo: Arc<ChapterRepository>,
        library_repo: Arc<LibraryRepository>,
        text_cleaner: Arc<TextCleaner>,
        nfo_manager: Arc<NfoManager>,
        audio_streamer: Arc<AudioStreamer>,
        plugin_manager: Arc<PluginManager>,
    ) -> Self {
        Self {
            book_repo,
            chapter_repo,
            library_repo,
            task_repo: None,
            text_cleaner,
            nfo_manager,
            audio_streamer,
            plugin_manager,
            scraper_service: None,
            storage_service: None,
            merge_service: None,
            encryption_key: None,
            http_client: reqwest::Client::new(),
        }
    }

    /// Set task repository for progress reporting
    pub fn with_task_repo(mut self, task_repo: Arc<TaskRepository>) -> Self {
        self.task_repo = Some(task_repo);
        self
    }

    /// Set scraper service for metadata enhancement
    pub fn with_scraper_service(mut self, scraper_service: Arc<ScraperService>) -> Self {
        self.scraper_service = Some(scraper_service);
        self
    }

    /// Set storage service for WebDAV access
    pub fn with_storage_service(mut self, storage_service: Arc<StorageService>) -> Self {
        self.storage_service = Some(storage_service);
        self
    }

    /// Set merge service for chapter management
    pub fn with_merge_service(mut self, merge_service: Arc<MergeService>) -> Self {
        self.merge_service = Some(merge_service);
        self
    }

    /// Set encryption key for decrypting passwords
    pub fn with_encryption_key(mut self, encryption_key: Arc<[u8; 32]>) -> Self {
        self.encryption_key = Some(encryption_key);
        self
    }

    /// Update task progress if task_repo and task_id are available
    async fn update_progress(&self, task_id: Option<&str>, message: String) {
        if let (Some(repo), Some(tid)) = (&self.task_repo, task_id) {
            if let Err(e) = repo.update_progress(tid, &message).await {
                warn!("Failed to update task progress: {}", e);
            }
        }
    }

    /// Check if task has been cancelled
    async fn check_cancellation(&self, task_id: Option<&str>) -> Result<()> {
        if let (Some(repo), Some(tid)) = (&self.task_repo, task_id) {
            if let Ok(Some(task)) = repo.find_by_id(tid).await {
                if task.status == "cancelled" {
                    return Err(TingError::TaskError("Task cancelled by user".to_string()));
                }
            }
        }
        Ok(())
    }

    /// Get all supported extensions including those from plugins
    async fn get_supported_extensions(&self) -> Vec<String> {
        let mut extensions: Vec<String> = AUDIO_EXTENSIONS.iter().map(|&s| s.to_string()).collect();
        
        // Get extensions from Format plugins
        let plugins = self.plugin_manager.find_plugins_by_type(PluginType::Format).await;
        for plugin in plugins {
            // Assuming plugin metadata has supported_extensions
            // Since we can't easily change Plugin struct right now, we'll check if we can get it from somewhere else
            // Or we assume the plugin manager/loader logic handles this.
            // For this refactor, let's look at how we can get this info.
            
            // Actually, we should rely on the plugin.json metadata which should be loaded into Plugin struct.
            // Let's check Plugin struct definition in types.rs.
            if let Some(exts) = &plugin.supported_extensions {
                for ext in exts {
                    let ext_lower = ext.to_lowercase();
                    if !extensions.contains(&ext_lower) {
                        extensions.push(ext_lower);
                    }
                }
            }
        }
        
        extensions
    }

    /// Scan a library directory and discover audiobooks
    pub async fn scan_library(&self, library_id: &str, library_path: &str, task_id: Option<&str>) -> Result<ScanResult> {
        info!(library_id = %library_id, path = %library_path, "Starting library scan");
        self.update_progress(task_id, format!("Starting scan for library: {}", library_path)).await;
        self.check_cancellation(task_id).await?;

        // Fetch library to get configuration and type
        let library = self.library_repo.find_by_id(library_id).await?
            .ok_or_else(|| TingError::NotFound(format!("Library not found: {}", library_id)))?;
        
        let scraper_config: crate::db::models::ScraperConfig = library.scraper_config
            .as_ref()
            .and_then(|json| serde_json::from_str(json).ok())
            .unwrap_or_default();

        let last_scanned = if let Some(ref date_str) = library.last_scanned_at {
            chrono::DateTime::parse_from_rfc3339(date_str)
                .map(|dt| dt.with_timezone(&chrono::Utc))
                .ok() 
        } else {
            None
        };

        // Dispatch based on library type
        let scan_result = if library.library_type == "webdav" {
            self.scan_webdav_library(&library, task_id, &scraper_config).await?
        } else {
            // Local library scan
            let path = Path::new(library_path);
            if !path.exists() {
                return Err(TingError::NotFound(format!(
                    "Library path does not exist: {}",
                    library_path
                )));
            }

            if !path.is_dir() {
                return Err(TingError::ValidationError(format!(
                    "Library path is not a directory: {}",
                    library_path
                )));
            }

            self.scan_local_library(library_id, path, task_id, last_scanned, &scraper_config).await?
        };

        // Update library last_scanned_at
        if let Err(e) = self.library_repo.update_last_scanned(library_id).await {
            warn!("Failed to update library last_scanned_at: {}", e);
        }

        info!(
            books_created = scan_result.books_created,
            errors = scan_result.errors.len(),
            "Library scan completed"
        );
        self.update_progress(task_id, format!("Scan completed. Processed {} books.", scan_result.books_created)).await;

        // Trigger Merge Suggestions
        if let Some(merge_service) = &self.merge_service {
            self.update_progress(task_id, "Processing auto-merges...".to_string()).await;
            if let Err(e) = merge_service.process_auto_merges().await {
                warn!("Failed to process auto-merges: {}", e);
            }
            
            // Still generate suggestions for non-exact matches if needed, but user removed UI.
            // Keeping it doesn't hurt, but maybe redundant if UI is gone.
            // But API might still exist.
            // self.update_progress(task_id, "Generating merge suggestions...".to_string()).await;
            // if let Err(e) = merge_service.generate_suggestions().await {
            //    warn!("Failed to generate merge suggestions: {}", e);
            // }
        }

        Ok(scan_result)
    }

    /// Scan a local library
    async fn scan_local_library(
        &self,
        library_id: &str,
        path: &Path,
        task_id: Option<&str>,
        last_scanned: Option<chrono::DateTime<chrono::Utc>>,
        scraper_config: &crate::db::models::ScraperConfig,
    ) -> Result<ScanResult> {
        let mut scan_result = ScanResult::default();
        self.update_progress(task_id, "Scanning local directories...".to_string()).await;

        // Get all supported extensions dynamically
        let supported_extensions = self.get_supported_extensions().await;
        
        // 1. Recursively find all audio files and group them by directory
        let mut dir_groups: HashMap<PathBuf, Vec<PathBuf>> = HashMap::new();
        
        for entry in WalkDir::new(path).into_iter().filter_map(|e| e.ok()) {
            let entry_path = entry.path();
            if entry_path.is_file() {
                if let Some(ext) = entry_path.extension() {
                    let ext_str = ext.to_string_lossy().to_lowercase();
                    if supported_extensions.contains(&ext_str) {
                        if let Some(parent) = entry_path.parent() {
                            dir_groups.entry(parent.to_path_buf()).or_default().push(entry_path.to_path_buf());
                        }
                    }
                }
            }
        }

        self.update_progress(task_id, format!("Found {} directories with audio files", dir_groups.len())).await;

        // 2. Process each directory group as a book
        let total_groups = dir_groups.len();
        let mut processed_count = 0;

        // Pre-fetch manual corrected books to avoid N+1 query in process_book_directory
        // This significantly reduces memory pressure and database load during scan
        let all_books = self.book_repo.find_by_library(library_id).await.unwrap_or_default();
        let manual_corrected_books: Vec<crate::db::models::Book> = all_books.into_iter()
            .filter(|b| b.manual_corrected == 1 && b.match_pattern.is_some())
            .collect();

        for (dir, mut files) in dir_groups {
            // Check cancellation
            self.check_cancellation(task_id).await?;
            
            processed_count += 1;
            let dir_name = dir.file_name().and_then(|n| n.to_str()).unwrap_or("Unknown");
            
            self.update_progress(task_id, format!("Processing ({}/{}): {}", processed_count, total_groups, dir_name)).await;

            // Sort files by filename using natural sort order (e.g. 1, 2, 10 instead of 1, 10, 2)
            files.sort_by(|a, b| natord::compare(a.to_string_lossy().as_ref(), b.to_string_lossy().as_ref()));

            match self.process_book_directory(library_id, &dir, &files, last_scanned, task_id, scraper_config, &manual_corrected_books).await {
                Ok(book_id) => {
                    scan_result.books_created += 1;
                    debug!(book_id = %book_id, path = ?dir, "Processed book directory");
                }
                Err(e) => {
                    warn!(path = ?dir, error = %e, "Failed to process book directory");
                    scan_result.errors.push(format!(
                        "Failed to process {}: {}",
                        dir.display(),
                        e
                    ));
                }
            }

            // Periodic garbage collection to prevent memory buildup during large scans
            // Force GC after every directory to help debug memory issues with native plugins
            self.plugin_manager.garbage_collect_all().await;
        }

        // Final garbage collection after scan
        self.plugin_manager.garbage_collect_all().await;

        Ok(scan_result)
    }

    /// Scan a WebDAV library
    async fn scan_webdav_library(
        &self,
        library: &crate::db::models::Library,
        task_id: Option<&str>,
        scraper_config: &crate::db::models::ScraperConfig,
    ) -> Result<ScanResult> {
        if self.storage_service.is_none() {
            return Err(TingError::ConfigError("Storage service not configured for WebDAV scan".to_string()));
        }

        let mut scan_result = ScanResult::default();
        self.update_progress(task_id, "Scanning WebDAV directories...".to_string()).await;

        // 1. List files recursively
        // We need to implement a WebDAV walker
        let files = self.list_webdav_files(library, task_id).await?;
        
        let supported_extensions = self.get_supported_extensions().await;
        
        // Group by directory URL (parent URL)
        // Key: Parent URL (String), Value: List of File URLs (String)
        let mut dir_groups: HashMap<String, Vec<String>> = HashMap::new();
        
        for file_url in files {
            // Check extension
            if let Some(ext_pos) = file_url.rfind('.') {
                let ext = file_url[ext_pos+1..].to_lowercase();
                if supported_extensions.contains(&ext) {
                    // Get parent URL
                    if let Some(last_slash) = file_url.rfind('/') {
                        let parent = file_url[0..last_slash].to_string();
                        dir_groups.entry(parent).or_default().push(file_url);
                    }
                }
            }
        }

        self.update_progress(task_id, format!("Found {} directories with audio files", dir_groups.len())).await;

        let total_groups = dir_groups.len();
        let mut processed_count = 0;

        for (dir_url, mut file_urls) in dir_groups {
            // Check cancellation
            self.check_cancellation(task_id).await?;

            processed_count += 1;
            // Extract directory name from URL for logging
            let decoded_dir_url = self.decode_url_path(&dir_url);
            let dir_name = decoded_dir_url.trim_end_matches('/').split('/').last().unwrap_or("Unknown");
            
            self.update_progress(task_id, format!("Processing ({}/{}): {}", processed_count, total_groups, dir_name)).await;

            // Sort file URLs naturally (handles 1, 2, 10 correctly)
            file_urls.sort_by(|a, b| natord::compare(a, b));

            match self.process_webdav_book(library, &dir_url, &file_urls, task_id, scraper_config).await {
                Ok(book_id) => {
                    scan_result.books_created += 1;
                    debug!(book_id = %book_id, url = %dir_url, "Processed WebDAV book directory");
                }
                Err(e) => {
                    warn!(url = %dir_url, error = %e, "Failed to process WebDAV book directory");
                    scan_result.errors.push(format!(
                        "Failed to process {}: {}",
                        dir_url,
                        e
                    ));
                }
            }

            // Periodic garbage collection
            // Force GC after every directory to help debug memory issues with native plugins
            self.plugin_manager.garbage_collect_all().await;
        }

        // Final garbage collection after scan
        self.plugin_manager.garbage_collect_all().await;

        Ok(scan_result)
    }

    /// List all files in a WebDAV library recursively
    async fn list_webdav_files(&self, library: &crate::db::models::Library, task_id: Option<&str>) -> Result<Vec<String>> {
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

        let mut files = HashSet::new(); // Use HashSet to prevent duplicates
        let mut queue = std::collections::VecDeque::new();
        let mut visited_dirs = HashSet::new(); // Track visited directories to prevent cycles/re-visits

        queue.push_back(root_url.clone());
        visited_dirs.insert(root_url);

        let client = reqwest::Client::new();
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

        while let Some(current_url) = queue.pop_front() {
            // Check cancellation
            self.check_cancellation(task_id).await?;

            if processed_dirs >= max_dirs {
                warn!("Max WebDAV directories limit reached");
                break;
            }
            processed_dirs += 1;

            // PROPFIND request
            let mut req = client.request(reqwest::Method::from_bytes(b"PROPFIND").unwrap(), &current_url)
                .header("Depth", "1");
            
            if let (Some(u), Some(p)) = (username, &password) {
                req = req.basic_auth(u, Some(p));
            }

            match req.send().await {
                Ok(res) => {
                    if res.status().is_success() || res.status().as_u16() == 207 {
                        let xml = res.text().await.unwrap_or_default();
                        let items = self.parse_webdav_response(&xml, &current_url);
                        
                        for (item_url, is_dir) in items {
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
                                files.insert(item_url);
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

    /// Parse WebDAV XML response
    fn parse_webdav_response(&self, xml: &str, base_url: &str) -> Vec<(String, bool)> {
        let mut items = Vec::new();
        let mut reader = Reader::from_str(xml);
        reader.trim_text(true);

        let mut in_response = false;
        let mut current_href = String::new();
        let mut is_collection = false;
        let mut buf = Vec::new();

        // Simple state machine
        // Structure: <response> <href>...</href> ... <resourcetype><collection/></resourcetype> ... </response>
        
        // This is a simplified parser. For production, better use deserialization structs.
        // But manual parsing is robust enough for simple listing.
        
        loop {
            match reader.read_event_into(&mut buf) {
                Ok(Event::Start(e)) => {
                    match e.name().as_ref() {
                        b"D:response" | b"d:response" | b"response" => {
                            in_response = true;
                            current_href.clear();
                            is_collection = false;
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
                                items.push((full_url, is_collection));
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
        _task_id: Option<&str>,
        scraper_config: &crate::db::models::ScraperConfig,
    ) -> Result<String> {
        // Derive title from directory name
        // Decode URL to handle percent-encoded characters (e.g. Chinese)
        let decoded_url = self.decode_url_path(dir_url);
        let title = decoded_url.trim_end_matches('/').split('/').last().unwrap_or("Unknown Book").to_string();
        
        // No local path, use URL as path
        // We use the original URL as path to ensure connectivity, but StorageService needs to handle it correctly
        let path = dir_url.to_string();

        // Check if book exists
        let mut hasher = Sha256::new();
        hasher.update(path.as_bytes());
        let path_hash = format!("{:x}", hasher.finalize());
        
        // Extract metadata from WebDAV file (first file)
        let (meta_album, _meta_chapter_title, meta_author, meta_narrator, meta_cover_url, _meta_duration) = if !file_urls.is_empty() {
            self.extract_webdav_metadata(library, &file_urls[0]).await
        } else {
             (String::new(), String::new(), None, None, None, 0)
        };
        
        let mut book_title = title.clone();
        if !meta_album.trim().is_empty() {
            // If the title from metadata is actually an album title, we might use it.
            if !meta_album.trim().is_empty() {
                book_title = meta_album.clone();
            }
        }

        let existing_book = self.book_repo.find_by_hash(&path_hash).await?;
        let book_id = if let Some(book) = existing_book {
            book.id
        } else {
            Uuid::new_v4().to_string()
        };

        // Create or Update book
        let mut book = crate::db::models::Book {
            id: book_id.clone(),
            library_id: library.id.clone(),
            title: Some(book_title),
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
            manual_corrected: 0,
            match_pattern: None,
            chapter_regex: None,
        };

        // Run scraper if enabled
        if let Some(scraper_service) = &self.scraper_service {
             match scraper_service.scrape_book_metadata(&title, scraper_config).await {
                Ok(detail) => {
                    // Only overwrite if we don't have ID3 metadata (meta_album is empty)
                    if !detail.title.is_empty() {
                        if meta_album.trim().is_empty() {
                            book.title = Some(detail.title);
                        }
                    }
                    
                    if !detail.author.is_empty() {
                        // Only overwrite if current is Unknown (meaning no ID3 author found)
                        if book.author.as_deref() == Some("Unknown") || book.author.is_none() {
                            book.author = Some(detail.author);
                        }
                    }
                    
                    if !detail.intro.is_empty() {
                        book.description = Some(detail.intro);
                    }
                    
                    if detail.cover_url.is_some() && book.cover_url.is_none() {
                        book.cover_url = detail.cover_url;
                    }
                    
                    if detail.narrator.is_some() && book.narrator.is_none() {
                        book.narrator = detail.narrator;
                    }
                    
                    if !detail.tags.is_empty() {
                        book.tags = Some(detail.tags.join(","));
                    }
                },
                Err(e) => {
                    warn!("Scraper failed for WebDAV book {}: {}", title, e);
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
        if let Some(ref url) = book.cover_url {
            if let Ok(Some(color)) = crate::core::color::calculate_theme_color_with_client(url, &self.http_client).await {
                book.theme_color = Some(color);
            }
        }

        if let Ok(Some(existing)) = self.book_repo.find_by_id(&book_id).await {
             if existing.manual_corrected == 0 {
                 self.book_repo.update(&book).await?;
             }
        } else {
             self.book_repo.create(&book).await?;
        }

        // Create chapters
        for (index, file_url) in file_urls.iter().enumerate() {
            // Decode filename for title
            let decoded_file_url = self.decode_url_path(file_url);
            let filename = decoded_file_url.split('/').last().unwrap_or("chapter").to_string();
            
            // Check if chapter exists to avoid duplicates
            let mut ch_hasher = Sha256::new();
            ch_hasher.update(file_url.as_bytes());
            let ch_hash = format!("{:x}", ch_hasher.finalize());
            
            // Extract metadata from WebDAV file (download header chunk)
            let (_, meta_title, _, _, _, meta_duration) = self.extract_webdav_metadata(library, file_url).await;
            
            // Determine Title
            let raw_title = if !meta_title.trim().is_empty() {
                meta_title
            } else {
                filename
            };
            
            // Clean Title
            let (final_title, is_extra) = self.text_cleaner.clean_chapter_title(&raw_title, book.title.as_deref());
            
            let chapter = crate::db::models::Chapter {
                id: Uuid::new_v4().to_string(),
                book_id: book_id.clone(),
                title: Some(final_title),
                path: file_url.clone(),
                duration: Some(meta_duration),
                chapter_index: Some((index + 1) as i32),
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
            } else {
                self.chapter_repo.create(&chapter).await?;
            }
        }
        
        Ok(book_id)
    }

    /// Process a directory containing audio files as a book
    async fn process_book_directory(
        &self,
        library_id: &str,
        dir: &Path,
        files: &[PathBuf],
        last_scanned: Option<chrono::DateTime<chrono::Utc>>,
        task_id: Option<&str>,
        scraper_config: &crate::db::models::ScraperConfig,
        manual_corrected_books: &[crate::db::models::Book],
    ) -> Result<String> {
        // 0. Check New Chapter Protection (Manual Correction)
        // If this directory matches a pattern of a manually corrected book, add chapters to that book.
        
        for book in manual_corrected_books {
            if let Some(pattern) = &book.match_pattern {
                if !pattern.is_empty() {
                    // Check regex match against directory name
                    let dir_name = dir.file_name().and_then(|n| n.to_str()).unwrap_or("");
                    
                    if let Ok(re) = regex::Regex::new(pattern) {
                        if re.is_match(dir_name) {
                            info!("New Chapter Protection: Merging {} into existing book {}", dir_name, book.title.as_deref().unwrap_or("?"));
                            
                            // Process chapters for this existing book
                            self.process_chapters(&book.id, files, last_scanned, task_id).await?;
                            return Ok(book.id.clone());
                        }
                    }
                }
            }
        }

        // 1. Check if Book Exists (by Title+Author or Hash)
        // We need to decide whether to update metadata or not.
        
        let mut existing_book_id = None;
        let mut is_manual_corrected = false;

        // Try to find existing book first to check manual_corrected status
        // Heuristic: Extract potential Title/Author from directory to find existing book
        // But extracting metadata is heavy.
        // Let's rely on Hash first? Or quick Directory name check?
        // Let's do a quick metadata extraction (Title only) from directory name
        let dir_name = dir.file_name().and_then(|n| n.to_str()).unwrap_or("Unknown Book");
        let (quick_title, _) = self.text_cleaner.clean_chapter_title(dir_name, None);
        
        // Try finding by hash first (most reliable for exact folder match)
        let book_hash = self.generate_book_hash(dir);
        if let Ok(Some(book)) = self.book_repo.find_by_hash(&book_hash).await {
            existing_book_id = Some(book.id.clone());
            is_manual_corrected = book.manual_corrected == 1;
        }

        // If not found by hash, we might find by Title later, but for now we proceed.

        // 2. Extract Metadata & Scrape (ONLY if not manually corrected)
        let (title, author, narrator, description, tags, cover_url, theme_color) = if is_manual_corrected {
            // If manually corrected, we DON'T extract/scrape. We use existing values.
            // But we need values to create the book object if we were to update it?
            // Actually, if it exists, we just update what?
            // If it exists and is corrected, we should NOT update metadata.
            // But we need to process chapters.
            
            // We just need the ID to proceed to `process_chapters`.
            // But we might want to update `path` or `library_id` if moved.
            // Let's fetch the book to get its current state.
            if let Some(id) = &existing_book_id {
                if let Some(book) = self.book_repo.find_by_id(id).await? {
                    (
                        book.title.unwrap_or(quick_title),
                        book.author,
                        book.narrator,
                        book.description,
                        book.tags,
                        book.cover_url,
                        book.theme_color
                    )
                } else {
                    // Should not happen
                    (quick_title, None, None, None, None, None, None)
                }
            } else {
                (quick_title, None, None, None, None, None, None)
            }
        } else {
            // Not corrected (or New), proceed with extraction
            let (t, a, n, d, tg, c, source) = self.extract_metadata(dir, files).await;
            let mut title = t;
            let mut author = a;
            let mut narrator = n;
            let mut description = d;
            let mut tags = tg;
            let mut cover_url = c;

            // Default to "Unknown" if author is missing
            if author.is_none() {
                author = Some("Unknown".to_string());
            }

            // Enhanced Metadata with Scraper
            if let Some(scraper) = &self.scraper_service {
                // Double check if we found a book by Title+Author now that we have them
                if existing_book_id.is_none() {
                     if let Some(ref a) = author {
                        if let Ok(Some(existing_book)) = self.book_repo.find_by_title_and_author(&title, a).await {
                            existing_book_id = Some(existing_book.id.clone());
                            if existing_book.manual_corrected == 1 {
                                // Found it, and it's corrected! Abort scraping.
                                // Use existing metadata.
                                title = existing_book.title.unwrap_or(title);
                                author = existing_book.author;
                                narrator = existing_book.narrator;
                                description = existing_book.description;
                                tags = existing_book.tags;
                                cover_url = existing_book.cover_url;
                                // Skip scraping
                            }
                        }
                    }
                }

                // If still not corrected (or new), scrape.
                // We check existing_book again because we might have just found it.
                let should_scrape = if let Some(id) = &existing_book_id {
                     if let Ok(Some(b)) = self.book_repo.find_by_id(id).await {
                         b.manual_corrected == 0
                     } else { true }
                } else {
                    true
                };

                if should_scrape && description.is_none() {
                    let author_display = author.as_deref().unwrap_or("Unknown");
                    debug!("Attempting to scrape metadata for: {} - {}", title, author_display);
                    self.update_progress(task_id, format!("Scraping metadata for: {} - {}", title, author_display)).await;
                    
                    // Use only Title as search query
                    let query = title.clone();
                    
                    match scraper.scrape_book_metadata(&query, scraper_config).await {
                        Ok(detail) => {
                            info!("Scraper found metadata for: {}", title);
                            // Only update if we have better info and not manually corrected (which is checked outside)
                            // We prefer scraper info over local extraction for these fields if scraper provides them
                            
                            if !detail.intro.is_empty() { description = Some(detail.intro); }
                            if !detail.tags.is_empty() { 
                                tags = Some(detail.tags.join(",")); 
                            }
                            
                            // Update cover if scraper has one AND local cover is missing (Prioritize ID3/Local)
                            if detail.cover_url.is_some() && cover_url.is_none() { 
                                cover_url = detail.cover_url; 
                            }
                            
                            // Update narrator if scraper has one AND local narrator is missing/fallback
                            if detail.narrator.is_some() { 
                                if source == MetadataSource::Fallback || narrator.is_none() {
                                    narrator = detail.narrator; 
                                }
                            }
                            
                            // For author, we update if it's Unknown OR if scraper provides one AND local source is fallback
                            if !detail.author.is_empty() {
                                if source == MetadataSource::Fallback || author.is_none() || author.as_deref() == Some("Unknown") {
                                    author = Some(detail.author);
                                }
                            }
                        },
                        Err(e) => debug!("Scraper failed: {}", e),
                    }
                }
            }
            
            // Calculate theme color
            let mut theme_color = None;
            if let Some(ref url) = cover_url {
                let cover_path = if url.starts_with("http") {
                    url.clone()
                } else {
                    dir.join(url).to_string_lossy().to_string()
                };

                match crate::core::color::calculate_theme_color_with_client(&cover_path, &self.http_client).await {
                    Ok(Some(color)) => theme_color = Some(color),
                    Ok(None) => {},
                    Err(e) => warn!("Failed to calculate theme color for {}: {}", title, e),
                }
            }
            
            (title, author, narrator, description, tags, cover_url, theme_color)
        };

        // 3. Create or Update Book Record
        let final_book_id = if let Some(id) = existing_book_id {
            let mut book = self.book_repo.find_by_id(&id).await?.ok_or_else(|| TingError::NotFound("Book not found".to_string()))?;
            
            // Always update location info
            book.library_id = library_id.to_string();
            book.path = dir.to_string_lossy().to_string();
            
            // Only update metadata if NOT manually corrected
            if book.manual_corrected == 0 {
                // We overwrite with new scanned values because if they changed on disk (or scraper config changed), we want to reflect that.
                // Unless we want to be more granular. But "manual_corrected" is the big switch.
                
                if book.author != author { book.author = author.clone(); }
                if book.narrator != narrator { book.narrator = narrator.clone(); }
                
                if book.cover_url != cover_url {
                    book.cover_url = cover_url.clone();
                    book.theme_color = theme_color.clone();
                } else if book.theme_color.is_none() && theme_color.is_some() {
                    book.theme_color = theme_color.clone();
                }
                
                // For description and tags, we also update if we have new values
                if description.is_some() { book.description = description.clone(); }
                if tags.is_some() { book.tags = tags.clone(); }
                
                self.book_repo.update(&book).await?;
            } else {
                // Just update path/lib if needed
                self.book_repo.update(&book).await?;
            }
            
            id
        } else {
            // Create new book
            let new_id = Uuid::new_v4().to_string();
            let book = Book {
                id: new_id.clone(),
                library_id: library_id.to_string(),
                title: Some(title.clone()),
                author: author.clone(),
                narrator,
                cover_url,
                theme_color,
                description,
                skip_intro: 0,
                skip_outro: 0,
                path: dir.to_string_lossy().to_string(),
                hash: book_hash.clone(),
                tags,
                created_at: chrono::Utc::now().to_rfc3339(),
                manual_corrected: 0,
                match_pattern: None,
                chapter_regex: None,
            };
            self.book_repo.create(&book).await?;
            new_id
        };

        // Write NFO if enabled and book is not manual corrected (or we want to update it anyway?)
        // If manual_corrected is true, we might still want to sync to NFO if user enabled it.
        // Yes, NFO should reflect DB state.
        if scraper_config.nfo_writing_enabled {
            if let Ok(Some(book)) = self.book_repo.find_by_id(&final_book_id).await {
                // Construct metadata
                let mut metadata = BookMetadata::new(
                    book.title.clone().unwrap_or_default(),
                    "ting-reader".to_string(),
                    book.id.clone(),
                    0, // Chapter count will be updated later or we can't know yet easily without counting files
                );
                metadata.author = book.author.clone();
                metadata.narrator = book.narrator.clone();
                metadata.intro = book.description.clone();
                metadata.cover_url = book.cover_url.clone();
                if let Some(tags_str) = &book.tags {
                     metadata.tags.items = tags_str.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
                }
                
                // Write NFO
                // For local library, path is dir.
                // We assume process_book_directory handles local paths.
                let book_path = std::path::Path::new(&book.path);
                if let Err(e) = self.nfo_manager.write_book_nfo_to_dir(book_path, &metadata) {
                    warn!("Failed to write NFO for book {}: {}", book.title.unwrap_or_default(), e);
                }
            }
        }

        // 4. Process Chapters (Incremental & Deduplication)
        self.process_chapters(&final_book_id, files, last_scanned, task_id).await?;

        Ok(final_book_id)
    }

    async fn extract_metadata(&self, dir: &Path, files: &[PathBuf]) -> (String, Option<String>, Option<String>, Option<String>, Option<String>, Option<String>, MetadataSource) {
        // Try NFO
        let nfo_path = dir.join("book.nfo");
        if let Ok(meta) = self.nfo_manager.read_book_nfo(&nfo_path) {
            return (
                meta.title,
                meta.author,
                meta.narrator,
                meta.intro,
                Some(meta.tags.items.join(",")),
                meta.cover_url,
                MetadataSource::Nfo
            );
        }

        // Try Audio Metadata from first file
        let mut title = String::new();
        let mut author = None;
        let mut narrator = None;
        let mut cover_url_from_plugin = None;
        let mut source = MetadataSource::Fallback;

        if !files.is_empty() {
            let file_path = &files[0];
            
            // First try standard audio streamer (Symphonia)
            // But for encrypted files (like XM, NCM), standard streamer might fail or return garbage
            let ext = file_path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
            let is_standard = STANDARD_EXTENSIONS.contains(&ext.as_str());
            
            if is_standard {
                if let Ok(meta) = self.audio_streamer.read_metadata(file_path) {
                    if let Some(t) = meta.album {
                        if !t.trim().is_empty() {
                            title = t;
                            source = MetadataSource::FileMetadata;
                        }
                    }
                    
                    // Logic for Author/Narrator extraction
                    // Priority: AlbumArtist > Artist
                    if let Some(aa) = meta.album_artist {
                         if !aa.trim().is_empty() {
                             author = Some(aa);
                             source = MetadataSource::FileMetadata;
                         }
                    }
                    
                    if let Some(a) = meta.artist {
                        if !a.trim().is_empty() {
                            if author.is_none() {
                                author = Some(a.clone());
                                source = MetadataSource::FileMetadata;
                            } else if author.as_ref() != Some(&a) {
                                // If Author is already set (e.g. from AlbumArtist) and Artist is different,
                                // Artist is likely the Narrator or Contributing Artist.
                                narrator = Some(a);
                            }
                        }
                    }
                    
                    if let Some(c) = meta.composer {
                        if !c.trim().is_empty() {
                            if narrator.is_none() {
                                narrator = Some(c);
                            }
                        }
                    }
                }
            }

            // If standard failed or it's an encrypted file, try plugins
            if title.is_empty() || !is_standard {
                // Try format plugins
                // Find a format plugin that supports this file
                let plugins = self.plugin_manager.find_plugins_by_type(PluginType::Format).await;
                for plugin in plugins {
                    // Check if plugin supports this extension
                    let supports_ext = plugin.supported_extensions.as_ref()
                        .map(|exts| exts.iter().any(|e| e.eq_ignore_ascii_case(&ext)))
                        .unwrap_or(false);
                    
                    if !supports_ext {
                        continue;
                    }

                    // Call 'extract_metadata' on the plugin
                    let params = serde_json::json!({
                        "file_path": file_path.to_string_lossy()
                    });
                    
                    if let Ok(result) = self.plugin_manager.call_format(
                        &plugin.id, 
                        FormatMethod::ExtractMetadata, 
                        params
                    ).await {
                        // Parse result
                        if let Some(t) = result.get("album").and_then(|v| v.as_str()) {
                            if !t.trim().is_empty() {
                                title = t.to_string();
                                source = MetadataSource::FileMetadata;
                            }
                        }
                        
                        // Handle Artist/Narrator based on Plugin ID
                        // For 'xm-format', the 'artist' field is actually the Narrator
                        if let Some(a) = result.get("artist").and_then(|v| v.as_str()) {
                            if !a.trim().is_empty() {
                                author = Some(a.to_string());
                                source = MetadataSource::FileMetadata;
                            }
                        }
                        
                        // If plugin explicitly returns narrator (future proof)
                        if let Some(n) = result.get("narrator").and_then(|v| v.as_str()) {
                            if !n.trim().is_empty() {
                                narrator = Some(n.to_string());
                            }
                        }

                        if let Some(c) = result.get("cover_url").and_then(|v| v.as_str()) {
                            if !c.trim().is_empty() {
                                cover_url_from_plugin = Some(c.to_string());
                            }
                        }
                        
                        // If we found metadata, break
                        if !title.is_empty() {
                            break;
                        }
                    }
                }
            }
        }

        // Fallback: Directory Name
        if title.is_empty() {
            let dir_name = dir.file_name().and_then(|n| n.to_str()).unwrap_or("Unknown Book");
            let (cleaned, _) = self.text_cleaner.clean_chapter_title(dir_name, None);
            title = cleaned;
            source = MetadataSource::Fallback;
        }

        // Fallback Author from "Author - Title" pattern
        if author.is_none() {
             let dir_name = dir.file_name().and_then(|n| n.to_str()).unwrap_or("");
             if dir_name.contains(" - ") {
                 let parts: Vec<&str> = dir_name.split(" - ").collect();
                 if parts.len() >= 2 {
                     author = Some(parts[0].trim().to_string());
                 }
             }
        }

        let cover_url = self.find_cover_image(dir).or(cover_url_from_plugin);

        (title, author, narrator, None, None, cover_url, source)
    }

    async fn process_chapters(
        &self, 
        book_id: &str, 
        files: &[PathBuf], 
        last_scanned: Option<chrono::DateTime<chrono::Utc>>,
        task_id: Option<&str>
    ) -> Result<()> {
        let total_files = files.len();
        
        // Fetch book to check for regex rule
        let book = self.book_repo.find_by_id(book_id).await?
            .ok_or_else(|| TingError::NotFound("Book not found".to_string()))?;
            
        let chapter_regex = if let Some(pattern) = &book.chapter_regex {
            regex::Regex::new(pattern).ok()
        } else {
            None
        };

        // Pre-fetch existing chapters to support efficient incremental scanning
        // Map Path -> Chapter
        let existing_chapters = self.chapter_repo.find_by_book(book_id).await?;
        let mut chapter_map: HashMap<PathBuf, Chapter> = HashMap::new();
        for ch in existing_chapters {
            let p = PathBuf::from(&ch.path);
            chapter_map.insert(p, ch);
        }

        for (index, file_path) in files.iter().enumerate() {
            if index % 5 == 0 {
                // Check cancellation and log progress
                self.check_cancellation(task_id).await?;
                self.update_progress(task_id, format!("Processing chapter {}/{}", index + 1, total_files)).await;
            }

            // Incremental Scan Logic
            // Check if file exists in DB
            let mut existing_chapter = chapter_map.get(file_path).cloned();
            
            // Check if file has changed
            let is_modified = if let Some(last_scan) = last_scanned {
                if let Ok(metadata) = std::fs::metadata(file_path) {
                    if let Ok(mtime) = metadata.modified() {
                        let mtime_utc: chrono::DateTime<chrono::Utc> = mtime.into();
                        mtime_utc > last_scan
                    } else {
                        true // Can't read mtime, force check
                    }
                } else {
                    true // Can't read metadata, force check
                }
            } else {
                true // No last scan, force check
            };

            // Optimization: If chapter exists and file is not modified, skip processing!
            if let Some(ref ch) = existing_chapter {
                if !is_modified {
                    // Update index if needed (e.g. reordering files), but skip hashing/metadata
                    // Also respect manual_corrected if we were to update anything else
                    let new_index = (index + 1) as i32;
                    if ch.chapter_index != Some(new_index) {
                         // Only update index
                         // Check manual_corrected? 
                         // Usually index is structural, but if user manually ordered chapters, we might break it.
                         // But we sort files by name.
                         // If user manually corrected, we probably shouldn't touch index either?
                         // "manual_corrected" on chapter usually means Title correction.
                         // But let's respect it for index too if set.
                         if ch.manual_corrected == 0 {
                             let mut updated_ch = ch.clone();
                             updated_ch.chapter_index = Some(new_index);
                             self.chapter_repo.update(&updated_ch).await?;
                         }
                    }
                    continue;
                }
            }

            // If we are here, either it's a new file OR it's modified.
            
            // Calculate content-based hash
            let file_hash = self.calculate_file_hash(file_path)?;

            // Check if chapter exists by Hash (Global Deduplication)
            // But we must be careful: if we already found it by Path, we know it's that chapter.
            // If we found by Path, but Hash changed, it's an update.
            // If we didn't find by Path, we check Hash to see if it's a move/rename.
            
            if existing_chapter.is_none() {
                if let Ok(Some(ch)) = self.chapter_repo.find_by_hash(&file_hash).await {
                    // Found by hash (Rename/Move case)
                    // But we are processing a specific book_id here.
                    // If the found chapter belongs to another book, we might be stealing it?
                    // Or it's a duplicate file (e.g. same intro file in multiple books).
                    // If it's the same book, we treat it as the "existing chapter".
                    if ch.book_id == book_id {
                        existing_chapter = Some(ch);
                    }
                    // If different book, we create a new chapter record (duplicate content allowed across books)
                }
            }

            // Extract metadata
            let (_, mut title, _, _, _, duration) = self.extract_chapter_metadata(file_path).await;
            let mut chapter_idx = (index + 1) as i32;
            
            // If regex rule exists, try to apply it to filename
            if let Some(re) = &chapter_regex {
                if let Some(filename) = file_path.file_stem().and_then(|s| s.to_str()) {
                    if let Some(caps) = re.captures(filename) {
                        // Capture Group 1: Index (Optional)
                        if let Some(m) = caps.get(1) {
                            if let Ok(idx) = m.as_str().parse::<i32>() {
                                chapter_idx = idx;
                            }
                        }
                        
                        // Capture Group 2: Title (Optional)
                        if let Some(m) = caps.get(2) {
                            title = m.as_str().to_string();
                        }
                    }
                }
            }
            
            // Apply text cleaner to title, regardless of source
            // If title is empty, use filename stem
            let raw_title = if title.is_empty() {
                file_path.file_stem().and_then(|s| s.to_str()).unwrap_or("Unknown").to_string()
            } else {
                title
            };
            
            let (final_title, is_extra) = self.text_cleaner.clean_chapter_title(&raw_title, book.title.as_deref());

            if let Some(mut ch) = existing_chapter {
                // Update Existing
                // Check Lock
                if ch.manual_corrected == 0 {
                    ch.title = Some(final_title);
                    ch.chapter_index = Some(chapter_idx);
                    ch.is_extra = if is_extra { 1 } else { 0 };
                }
                // Always update duration/path/hash if file changed
                ch.path = file_path.to_string_lossy().to_string();
                ch.duration = Some(duration);
                ch.hash = Some(file_hash);
                ch.book_id = book_id.to_string();
                
                self.chapter_repo.update(&ch).await?;
            } else {
                // Create New
                let chapter = Chapter {
                    id: Uuid::new_v4().to_string(),
                    book_id: book_id.to_string(),
                    title: Some(final_title),
                    path: file_path.to_string_lossy().to_string(),
                    duration: Some(duration),
                    chapter_index: Some(chapter_idx),
                    is_extra: if is_extra { 1 } else { 0 },
                    hash: Some(file_hash),
                    created_at: chrono::Utc::now().to_rfc3339(),
                    manual_corrected: 0,
                };
                
                 match self.chapter_repo.create(&chapter).await {
                     Ok(_) => {},
                     Err(e) => warn!("Failed to create chapter: {}", e),
                 }
            }
        }
        Ok(())
    }

    async fn extract_webdav_metadata(
        &self,
        library: &crate::db::models::Library,
        file_url: &str,
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
            
            // 1. Probe Header (512 bytes) to determine required size
            let probe_size = 512;
            let mut required_size = 2 * 1024 * 1024; // Default 2MB fallback
            let mut probe_data = Vec::with_capacity(probe_size);
            
            if let Ok((mut reader, _)) = storage.get_webdav_reader(library, file_url, Some((0, probe_size as u64)), key).await {
                let mut buf = vec![0u8; probe_size];
                if let Ok(n) = reader.read(&mut buf).await {
                    probe_data.extend_from_slice(&buf[..n]);
                }
            }
            
            if !probe_data.is_empty() {
                // Ask plugins for required size
                let plugins = self.plugin_manager.find_plugins_by_type(PluginType::Format).await;
                for plugin in plugins {
                    let params = serde_json::json!({
                        "header_base64": base64::engine::general_purpose::STANDARD.encode(&probe_data)
                    });
                    
                    if let Ok(result) = self.plugin_manager.call_format(&plugin.id, FormatMethod::GetMetadataReadSize, params).await {
                        if let Some(size) = result.get("size").and_then(|v| v.as_u64()) {
                             if size > 0 {
                                 required_size = size as usize;
                                 debug!("Plugin {} requested {} bytes for metadata", plugin.name, required_size);
                                 break;
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
                    if required_size > probe_data.len() {
                        let start = probe_data.len() as u64;
                        let end = required_size as u64;
                        // Avoid requesting past EOF if file is small (though WebDAV usually handles it)
                        // But we don't know total size.
                        // However, if ID3 says size is X, file MUST be at least X.
                        if let Ok((mut reader, _)) = storage.get_webdav_reader(library, file_url, Some((start, end)), key).await {
                            let _ = tokio::io::copy(&mut reader, &mut file).await;
                        }
                    }
                    
                    // Extract metadata using existing logic
                    let result = self.extract_chapter_metadata(&temp_path).await;
                    
                    // Cleanup
                    let _ = tokio::fs::remove_file(&temp_path).await;
                    
                    return result;
                }
            }
            
            // Ensure cleanup on failure
            if temp_path.exists() {
                 let _ = tokio::fs::remove_file(&temp_path).await;
            }
        }
        
        (String::new(), String::new(), None, None, None, 0)
    }

    async fn extract_chapter_metadata(&self, path: &Path) -> (String, String, Option<String>, Option<String>, Option<String>, i32) {
        // Returns: (album, title, author, narrator, cover_url, duration)
        
        // Try NFO
        let nfo_path = path.with_extension("nfo");
        if let Ok(meta) = self.nfo_manager.read_chapter_nfo(&nfo_path) {
            return (String::new(), meta.title, None, None, None, meta.duration.unwrap_or(0) as i32);
        }

        // Check if it is a standard audio file
        let ext = path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
        let is_standard = STANDARD_EXTENSIONS.contains(&ext.as_str());

        // Try Audio (Skip for non-standard/encrypted files as standard reader fails)
        if is_standard {
            if let Ok(meta) = self.audio_streamer.read_metadata(path) {
                 let t = meta.title.unwrap_or_default();
                 let album = meta.album.unwrap_or_default();
                 let d = meta.duration.as_secs() as i32;
                 
                 // Standard metadata extraction for author/narrator
                 let mut author = meta.album_artist;
                 let mut narrator = None;
                 
                 if let Some(a) = meta.artist {
                     if !a.trim().is_empty() {
                         if author.is_none() {
                             author = Some(a.clone());
                         } else if author.as_ref() != Some(&a) {
                             narrator = Some(a);
                         }
                     }
                 }
                 
                 if let Some(c) = meta.composer {
                     if !c.trim().is_empty() && narrator.is_none() {
                         narrator = Some(c);
                     }
                 }
                 
                 return (album, t, author, narrator, None, d);
            }
        }
        
        // Try Plugins (Force for non-standard/encrypted files)
        let plugins = self.plugin_manager.find_plugins_by_type(PluginType::Format).await;
        for plugin in plugins {
            // Check if plugin supports this extension
            let supports_ext = plugin.supported_extensions.as_ref()
                .map(|exts| exts.iter().any(|e| e.eq_ignore_ascii_case(&ext)))
                .unwrap_or(false);
            
            if !supports_ext {
                continue;
            }

            let params = serde_json::json!({
                "file_path": path.to_string_lossy()
            });
            
            // Try to extract metadata using plugin
            if let Ok(result) = self.plugin_manager.call_format(
                &plugin.id, 
                FormatMethod::ExtractMetadata, 
                params
            ).await {
                let t = result.get("title")
                    .and_then(|v| v.as_str())
                    .unwrap_or("").to_string();
                    
                let album = result.get("album")
                    .and_then(|v| v.as_str())
                    .unwrap_or("").to_string();

                // For XM files, duration might be 0 from plugin if not decrypted
                // But at least we get the title
                let d = result.get("duration").and_then(|v| v.as_f64()).unwrap_or(0.0) as i32;
                
                let mut author = None;
                let mut narrator = None;
                let mut cover_url = None;
                
                if let Some(a) = result.get("artist").and_then(|v| v.as_str()) {
                    if !a.trim().is_empty() {
                        author = Some(a.to_string());
                    }
                }
                
                if let Some(n) = result.get("narrator").and_then(|v| v.as_str()) {
                    if !n.trim().is_empty() {
                        narrator = Some(n.to_string());
                    }
                }
                
                if let Some(c) = result.get("cover_url").and_then(|v| v.as_str()) {
                    if !c.trim().is_empty() {
                        cover_url = Some(c.to_string());
                    }
                }
                
                if !t.is_empty() || !album.is_empty() || d > 0 {
                    return (album, t, author, narrator, cover_url, d);
                }
            }
        }

        (String::new(), String::new(), None, None, None, 0)
    }

    fn find_cover_image(&self, dir: &Path) -> Option<String> {
        let cover_names = ["cover.jpg", "cover.png", "cover.jpeg", "folder.jpg", "folder.png"];
        for name in cover_names {
            let path = dir.join(name);
            if path.exists() {
                return Some(path.to_string_lossy().to_string());
            }
        }
        if let Ok(mut entries) = std::fs::read_dir(dir) {
             while let Some(Ok(entry)) = entries.next() {
                 let path = entry.path();
                 if path.is_file() {
                     if let Some(ext) = path.extension() {
                         let ext_str = ext.to_string_lossy().to_lowercase();
                         if ["jpg", "jpeg", "png", "webp"].contains(&ext_str.as_str()) {
                             return Some(path.to_string_lossy().to_string());
                         }
                     }
                 }
             }
        }
        None
    }

    fn generate_book_hash(&self, audiobook_dir: &Path) -> String {
        let path_str = audiobook_dir.to_string_lossy();
        let mut hasher = Sha256::new();
        hasher.update(path_str.as_bytes());
        format!("{:x}", hasher.finalize())
    }

    fn calculate_file_hash(&self, path: &Path) -> Result<String> {
        let mut file = std::fs::File::open(path).map_err(|e| TingError::IoError(e))?;
        let metadata = file.metadata().map_err(|e| TingError::IoError(e))?;
        let len = metadata.len();
        
        let mut buffer = vec![0; 16384]; // 16KB
        let n = file.read(&mut buffer).map_err(|e| TingError::IoError(e))?;
        
        let mut hasher = Sha256::new();
        hasher.update(&buffer[..n]);
        hasher.update(len.to_le_bytes());
        // Also include filename to distinguish different chapters with same content/size (unlikely but possible)
        if let Some(name) = path.file_name() {
             hasher.update(name.to_string_lossy().as_bytes());
        }

        Ok(format!("{:x}", hasher.finalize()))
    }
}

/// Result of a library scan operation
#[derive(Debug, Default)]
pub struct ScanResult {
    pub books_created: usize,
    pub errors: Vec<String>,
}
