//! Library scanner for discovering audiobooks
//!
//! This module provides functionality to scan library directories
//! and discover audiobook files, creating book and chapter records.

use crate::core::error::{Result, TingError};
use crate::db::repository::{BookRepository, ChapterRepository, LibraryRepository, TaskRepository, SeriesRepository, Repository};
use crate::core::services::ScraperService;
use crate::core::merge_service::MergeService;
use crate::core::text_cleaner::TextCleaner;
use crate::core::nfo_manager::NfoManager;
use crate::core::audio_streamer::AudioStreamer;
use crate::core::StorageService;
use crate::plugin::manager::{PluginManager, FormatMethod};
use crate::plugin::types::PluginType;
use std::path::Path;
use std::sync::Arc;
use tracing::{info, warn};

pub mod local;
pub mod webdav;

/// Supported audio file extensions
// Removed hardcoded encrypted extensions. Plugins should declare their supported extensions.
pub const AUDIO_EXTENSIONS: &[&str] = &["mp3", "m4a", "m4b", "flac", "ogg", "wav", "opus", "wma", "aac", "strm"];

/// Standard audio extensions that can be handled by the default audio streamer
pub const STANDARD_EXTENSIONS: &[&str] = &["mp3", "m4a", "m4b", "flac", "ogg", "wav", "opus", "wma", "aac", "strm"];

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MetadataSource {
    Nfo,
    FileMetadata,
    Fallback,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ScanStatus {
    Created,
    Updated,
    Skipped,
}

/// Result of a library scan operation
#[derive(Debug, Default)]
pub struct ScanResult {
    pub total_books: usize,
    pub books_created: usize,
    pub books_updated: usize,
    pub books_skipped: usize,
    pub books_deleted: usize,
    pub failed_count: usize,
    pub errors: Vec<String>,
    pub start_time: Option<std::time::Instant>,
    pub end_time: Option<std::time::Instant>,
}

impl ScanResult {
    pub fn duration(&self) -> std::time::Duration {
        if let (Some(start), Some(end)) = (self.start_time, self.end_time) {
            end.duration_since(start)
        } else {
            std::time::Duration::default()
        }
    }
}

/// Library scanner service
pub struct LibraryScanner {
    pub(crate) book_repo: Arc<BookRepository>,
    pub(crate) chapter_repo: Arc<ChapterRepository>,
    pub(crate) library_repo: Arc<LibraryRepository>,
    pub(crate) series_repo: Arc<SeriesRepository>,
    pub(crate) task_repo: Option<Arc<TaskRepository>>,
    pub(crate) text_cleaner: Arc<TextCleaner>,
    pub(crate) nfo_manager: Arc<NfoManager>,
    pub(crate) audio_streamer: Arc<AudioStreamer>,
    pub(crate) plugin_manager: Arc<PluginManager>,
    pub(crate) scraper_service: Option<Arc<ScraperService>>,
    pub(crate) storage_service: Option<Arc<StorageService>>,
    pub(crate) merge_service: Option<Arc<MergeService>>,
    pub(crate) encryption_key: Option<Arc<[u8; 32]>>,
    pub(crate) http_client: reqwest::Client,
}

impl LibraryScanner {
    /// Create a new library scanner
    pub fn new(
        book_repo: Arc<BookRepository>,
        chapter_repo: Arc<ChapterRepository>,
        library_repo: Arc<LibraryRepository>,
        series_repo: Arc<SeriesRepository>,
        text_cleaner: Arc<TextCleaner>,
        nfo_manager: Arc<NfoManager>,
        audio_streamer: Arc<AudioStreamer>,
        plugin_manager: Arc<PluginManager>,
    ) -> Self {
        Self {
            book_repo,
            chapter_repo,
            library_repo,
            series_repo,
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
    pub(crate) async fn update_progress(&self, task_id: Option<&str>, message: String) {
        if let (Some(repo), Some(tid)) = (&self.task_repo, task_id) {
            if let Err(e) = repo.update_progress(tid, &message).await {
                warn!("Failed to update task progress: {}", e);
            }
        }
    }

    /// Check if task has been cancelled
    pub(crate) async fn check_cancellation(&self, task_id: Option<&str>) -> Result<()> {
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
    pub(crate) async fn get_supported_extensions(&self) -> Vec<String> {
        let mut extensions: Vec<String> = AUDIO_EXTENSIONS.iter().map(|&s| s.to_string()).collect();
        
        // Get extensions from Format plugins
        let plugins = self.plugin_manager.find_plugins_by_type(PluginType::Format).await;
        for plugin in plugins {
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
        info!(target: "audit::scan", "开始扫描存储库: {} (ID: {})", library_path, library_id);
        self.update_progress(task_id, format!("开始扫描存储库: {}", library_path)).await;
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
            target: "audit::scan",
            "存储库 '{}' 扫描完成：共 {} 本书，新增 {} 本，更新 {} 本，错误 {} 个",
            library_path, scan_result.total_books, scan_result.books_created, scan_result.books_updated, scan_result.errors.len()
        );
        self.update_progress(task_id, format!("扫描完成。处理了 {} 本书。", scan_result.books_created + scan_result.books_updated)).await;

        // Trigger Merge Suggestions
        if let Some(merge_service) = &self.merge_service {
            self.update_progress(task_id, "正在处理自动合并...".to_string()).await;
            if let Err(e) = merge_service.process_auto_merges().await {
                warn!("Failed to process auto-merges: {}", e);
            }
        }

        Ok(scan_result)
    }

    pub(crate) async fn extract_chapter_metadata(&self, path: &Path) -> (String, String, Option<String>, Option<String>, Option<String>, i32) {
        // Returns: (album, title, author, narrator, cover_url, duration)
        
        // Try NFO
        let nfo_path = path.with_extension("nfo");
        if let Ok(meta) = self.nfo_manager.read_chapter_nfo(&nfo_path) {
            return (String::new(), meta.title, None, None, None, meta.duration.unwrap_or(0) as i32);
        }

        // Check if it is a standard audio file
        let ext = path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
        let is_standard = STANDARD_EXTENSIONS.contains(&ext.as_str());

        // Handle .strm files explicitly
        if ext == "strm" {
            // strm files are URL references, not actual audio files
            // They should not download content for metadata extraction
            // Instead, rely on companion metadata files (NFO/JSON) or manual input
            
            // Use filename as title, duration 0 (will be updated on first play)
            let t = path.file_stem().and_then(|s| s.to_str()).unwrap_or("").to_string();
            
            tracing::info!("检测到 strm 文件: {}, 使用文件名作为标题", t);
            tracing::info!("提示: strm 文件的完整元数据应通过 NFO 文件或 metadata.json 提供");
            
            return (String::new(), t, None, None, None, 0);
        }

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
                "file_path": path.to_string_lossy(),
                "extract_cover": false
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

}
