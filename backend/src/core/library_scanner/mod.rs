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
pub mod shared;
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
            // Read the URL from the file
            let url = match tokio::fs::read_to_string(path).await {
                Ok(content) => content.trim().to_string(),
                Err(e) => {
                    tracing::error!("无法读取 strm 文件 {}: {}", path.display(), e);
                    let t = path.file_stem().and_then(|s| s.to_str()).unwrap_or("").to_string();
                    return (String::new(), t, None, None, None, 0);
                }
            };
            
            if url.is_empty() || !url.starts_with("http") {
                tracing::warn!("strm 文件 {} 包含无效的 URL: {}", path.display(), url);
                let t = path.file_stem().and_then(|s| s.to_str()).unwrap_or("").to_string();
                return (String::new(), t, None, None, None, 0);
            }
            
            let t = path.file_stem().and_then(|s| s.to_str()).unwrap_or("").to_string();
            
            // Try to get duration using FFprobe (derive from FFmpeg path)
            let duration = if let Some(ffmpeg_path) = self.plugin_manager.get_ffmpeg_path().await {
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
                    tracing::info!("使用 FFprobe 获取 strm 文件时长: {}", url);
                
                // Add small delay to avoid overwhelming the server
                tokio::time::sleep(std::time::Duration::from_millis(200)).await;
                
                // Add User-Agent and other headers to avoid being blocked
                match tokio::process::Command::new(&ffprobe_path)
                    .arg("-v").arg("error")
                    .arg("-user_agent").arg("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
                    .arg("-headers").arg("Accept: */*")
                    .arg("-show_entries").arg("format=duration")
                    .arg("-of").arg("default=noprint_wrappers=1:nokey=1")
                    .arg(&url)
                    .output()
                    .await
                {
                    Ok(output) if output.status.success() => {
                        let duration_str = String::from_utf8_lossy(&output.stdout);
                        match duration_str.trim().parse::<f64>() {
                            Ok(dur) => {
                                let duration_secs = dur.round() as i32;
                                tracing::info!("strm 文件 {} 时长: {} 秒", t, duration_secs);
                                duration_secs
                            }
                            Err(_) => {
                                tracing::warn!("无法解析 FFprobe 输出: {}", duration_str);
                                0
                            }
                        }
                    }
                    Ok(output) => {
                        tracing::warn!("FFprobe 获取时长失败: {}", String::from_utf8_lossy(&output.stderr));
                        0
                    }
                    Err(e) => {
                        tracing::warn!("无法运行 FFprobe: {}", e);
                        0
                    }
                }
                } else {
                    tracing::warn!("未找到 FFprobe，strm 文件时长将设为 0");
                    0
                }
            } else {
                tracing::warn!("未找到 FFmpeg 插件，strm 文件时长将设为 0");
                0
            };
            
            tracing::info!("检测到 strm 文件: {}, 时长: {} 秒", t, duration);
            
            return (String::new(), t, None, None, None, duration);
        }

        // Smart metadata extraction strategy
        // 策略：优先使用格式插件（支持更多格式，对部分文件更友好）
        // 只有在插件不支持时才回退到 Symphonia
        
        let mut duration = 0i32;
        let mut album = String::new();
        let mut title = String::new();
        let mut author = None;
        let mut narrator = None;
        let mut cover_url = None;
        
        // 1. 优先尝试格式插件
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

            let params = serde_json::json!({
                "file_path": path.to_string_lossy(),
                "extract_cover": false
            });
            
            // 交由格式插件处理
            if let Ok(result) = self.plugin_manager.call_format(
                &plugin.id, 
                FormatMethod::ExtractMetadata, 
                params
            ).await {
                tracing::debug!("使用格式插件 {} 处理 {} 文件", plugin.name, ext);
                
                if let Some(t) = result.get("title").and_then(|v| v.as_str()) {
                    if !t.trim().is_empty() { title = t.to_string(); }
                }
                if let Some(a) = result.get("album").and_then(|v| v.as_str()) {
                    if !a.trim().is_empty() { album = a.to_string(); }
                }
                if let Some(au) = result.get("artist").and_then(|v| v.as_str()) {
                    if !au.trim().is_empty() { author = Some(au.to_string()); }
                }
                if let Some(aa) = result.get("album_artist").and_then(|v| v.as_str()) {
                    if !aa.trim().is_empty() { author = Some(aa.to_string()); }
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
        
        // 2. 如果插件没有处理，且是标准格式，尝试 Symphonia（仅用于完整文件）
        if !plugin_handled && is_standard {
            let file_size = tokio::fs::metadata(path).await
                .map(|m| m.len())
                .unwrap_or(0);
            
            // MP3 files: prioritize ID3 tags
            if ext == "mp3" {
                if let Ok(id3_duration) = self.get_id3_duration(path).await {
                    duration = id3_duration;
                    tracing::debug!("使用 ID3 获取 MP3 时长: {} 秒", duration);
                } else if let Ok(meta) = self.audio_streamer.read_metadata(path) {
                    duration = meta.duration.as_secs() as i32;
                    tracing::debug!("使用 Symphonia 获取 MP3 时长: {} 秒", duration);
                }
            } else {
                // Other formats: compare ID3 with file size estimation
                let id3_duration = self.get_id3_duration(path).await.unwrap_or(0);
                let estimated_duration = self.estimate_duration_by_size(file_size, &ext);
                
                if id3_duration > 0 && estimated_duration > 0 {
                    let diff_ratio = (id3_duration as f64 - estimated_duration as f64).abs() / estimated_duration as f64;
                    if diff_ratio < 0.15 {  // 差距小于15%，信任ID3
                        duration = id3_duration;
                        tracing::debug!("使用 ID3 获取 {} 时长: {} 秒 (估算: {} 秒, 差距: {:.1}%)", 
                                      ext, duration, estimated_duration, diff_ratio * 100.0);
                    } else {
                        tracing::debug!("ID3 时长差距过大 (ID3: {} 秒, 估算: {} 秒, 差距: {:.1}%), 将使用 FFprobe", 
                                      id3_duration, estimated_duration, diff_ratio * 100.0);
                    }
                }
                
                // If ID3 is unreliable or missing, use Symphonia as fallback
                if duration == 0 {
                    if let Ok(meta) = self.audio_streamer.read_metadata(path) {
                        duration = meta.duration.as_secs() as i32;
                        tracing::debug!("使用 Symphonia 获取 {} 时长: {} 秒", ext, duration);
                    }
                }
            }
            
            // 提取其他元数据（如果插件没有提供）
            if duration > 0 && (title.is_empty() || album.is_empty()) {
                if let Ok(meta) = self.audio_streamer.read_metadata(path) {
                    if title.is_empty() {
                        title = meta.title.unwrap_or_default();
                    }
                    if album.is_empty() {
                        album = meta.album.unwrap_or_default();
                    }
                 
                    // Standard metadata extraction for author/narrator
                    if author.is_none() {
                        author = meta.album_artist;
                    }
                 
                    if let Some(a) = meta.artist {
                        if !a.trim().is_empty() {
                            if author.is_none() {
                                author = Some(a.clone());
                            } else if author.as_ref() != Some(&a) && narrator.is_none() {
                                narrator = Some(a);
                            }
                        }
                    }
                 
                    if let Some(c) = meta.composer {
                        if !c.trim().is_empty() && narrator.is_none() {
                            narrator = Some(c);
                        }
                    }
                }
            }
        }
        
        // 3. 返回提取的元数据
        if !title.is_empty() || !album.is_empty() || duration > 0 {
            return (album, title, author, narrator, cover_url, duration);
        }
        
        // 4. 如果都失败了，返回空值
        (String::new(), String::new(), None, None, None, 0)
    }

    /// Get ID3 duration from audio file
    async fn get_id3_duration(&self, path: &Path) -> Result<i32> {
        use id3::TagLike;
        
        let tag = id3::Tag::read_from_path(path)
            .map_err(|e| TingError::InvalidRequest(format!("Failed to read ID3 tag: {}", e)))?;
        if let Some(duration_ms) = tag.duration() {
            Ok((duration_ms / 1000) as i32)
        } else {
            Err(TingError::InvalidRequest("No duration in ID3 tag".to_string()))
        }
    }
    
    /// Estimate duration based on file size and format
    fn estimate_duration_by_size(&self, file_size: u64, ext: &str) -> i32 {
        if file_size == 0 {
            return 0;
        }
        
        // Rough bitrate estimates for different formats (in kbps)
        let estimated_bitrate = match ext {
            "mp3" => 128,      // Common MP3 bitrate
            "m4a" | "aac" => 96,  // AAC is more efficient
            "flac" => 800,     // Lossless, much higher bitrate
            "ogg" | "opus" => 96,  // Similar to AAC
            "wma" => 128,      // Similar to MP3
            _ => 128,          // Default fallback
        };
        
        // Calculate estimated duration: file_size_bits / bitrate_bps
        let file_size_bits = file_size * 8;
        let bitrate_bps = estimated_bitrate * 1000;
        
        (file_size_bits / bitrate_bps as u64) as i32
    }

}