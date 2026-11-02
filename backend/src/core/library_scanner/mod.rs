//! Library scanner for discovering audiobooks
//!
//! This module provides functionality to scan library directories
//! and discover audiobook files, creating book and chapter records.

use crate::core::StorageService;
use crate::core::app::error::{InputErrorKind, LookupErrorKind, TaskErrorKind};
use crate::core::app::error::{Result, TingError};
use crate::core::audio::AudioStreamer;
use crate::core::books::ScraperService;
use crate::core::books::merge_service::MergeService;
use crate::core::books::nfo_manager::NfoManager;
use crate::core::books::text_cleaner::TextCleaner;
use crate::db::repository::{
    BookRepository, ChapterRepository, LibraryRepository, LibraryScanStateRepository, Repository,
    SeriesRepository, TaskRepository,
};
use crate::plugin::manager::PluginManager;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tracing::{info, warn};

pub mod local;
pub mod rss;
pub mod scheduler;
pub mod shared;
pub mod watcher;
pub mod webdav;

#[cfg(test)]
#[path = "../../../tests/unit/core/library_scanner/fixtures.rs"]
mod test_support;

/// Supported audio file extensions
// Removed hardcoded encrypted extensions. Plugins should declare their supported extensions.
pub const AUDIO_EXTENSIONS: &[&str] = &[
    "mp3", "m4a", "m4b", "flac", "ogg", "wav", "opus", "wma", "aac", "strm",
];

/// Standard audio extensions that can be handled by the default audio streamer
pub const STANDARD_EXTENSIONS: &[&str] = &[
    "mp3", "m4a", "m4b", "flac", "ogg", "wav", "opus", "wma", "aac", "strm",
];

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MetadataSource {
    Nfo,
    FileMetadata,
    Scraper,
    Fallback,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScanMode {
    Incremental,
    Full,
}

impl From<&str> for ScanMode {
    fn from(value: &str) -> Self {
        match value.trim().to_ascii_lowercase().as_str() {
            "full" | "force" | "rescan" => Self::Full,
            _ => Self::Incremental,
        }
    }
}

impl ScanMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Incremental => "incremental",
            Self::Full => "full",
        }
    }

    pub fn is_full(&self) -> bool {
        matches!(self, Self::Full)
    }
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
    pub changed_book_ids: HashSet<String>,
    pub failed_count: usize,
    pub errors: Vec<String>,
    pub start_time: Option<std::time::Instant>,
    pub end_time: Option<std::time::Instant>,
}

pub(crate) struct LocalLibraryScanContext<'a> {
    library_id: &'a str,
    path: &'a Path,
    task_id: Option<&'a str>,
    last_scanned: Option<chrono::DateTime<chrono::Utc>>,
    scraper_config: &'a crate::db::models::ScraperConfig,
    mode: ScanMode,
    scan_paths: Option<&'a [PathBuf]>,
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

pub struct LibraryScannerDependencies {
    pub book_repo: Arc<BookRepository>,
    pub chapter_repo: Arc<ChapterRepository>,
    pub library_repo: Arc<LibraryRepository>,
    pub series_repo: Arc<SeriesRepository>,
    pub text_cleaner: Arc<TextCleaner>,
    pub nfo_manager: Arc<NfoManager>,
    pub audio_streamer: Arc<AudioStreamer>,
    pub plugin_manager: Arc<PluginManager>,
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
    pub(crate) scan_state_repo: Arc<LibraryScanStateRepository>,
    pub(crate) scraper_service: Option<Arc<ScraperService>>,
    pub(crate) storage_service: Option<Arc<StorageService>>,
    pub(crate) merge_service: Option<Arc<MergeService>>,
    pub(crate) encryption_key: Option<Arc<[u8; 32]>>,
    pub(crate) http_client: reqwest::Client,
}

impl LibraryScanner {
    fn metadata_service(&self) -> crate::core::books::metadata::BookMetadataService<'_> {
        use crate::core::books::metadata::BookMetadataService;
        BookMetadataService {
            chapters: &self.chapter_repo,
            series: &self.series_repo,
            libraries: &self.library_repo,
            nfo: &self.nfo_manager,
            // Automatic scanning must never write to the remote drive.
            remote: None,
        }
    }

    /// Create a new library scanner
    pub fn new(dependencies: LibraryScannerDependencies) -> Self {
        let LibraryScannerDependencies {
            book_repo,
            chapter_repo,
            library_repo,
            series_repo,
            text_cleaner,
            nfo_manager,
            audio_streamer,
            plugin_manager,
        } = dependencies;
        let scan_state_repo = Arc::new(LibraryScanStateRepository::new(book_repo.db().clone()));
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
            scan_state_repo,
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

    /// Update task progress with a frontend-localizable key.
    pub(crate) async fn update_progress_key(
        &self,
        task_id: Option<&str>,
        message_key: &str,
        message_params: serde_json::Value,
    ) {
        if let (Some(repo), Some(tid)) = (&self.task_repo, task_id)
            && let Err(e) = repo
                .update_progress_key(tid, message_key, message_params)
                .await
        {
            warn!("Failed to update task progress: {}", e);
        }
    }

    /// Check if task has been cancelled
    pub(crate) async fn check_cancellation(&self, task_id: Option<&str>) -> Result<()> {
        if let (Some(repo), Some(tid)) = (&self.task_repo, task_id)
            && let Ok(Some(task)) = repo.find_by_id(tid).await
            && task.status == "cancelled"
        {
            return Err(TingError::task(
                TaskErrorKind::InvalidState,
                "check_cancellation::task",
                "Task cancelled by user".to_string(),
            ));
        }
        Ok(())
    }

    /// Get all supported extensions including those from plugins
    pub(crate) async fn get_supported_extensions(&self) -> Vec<String> {
        let mut extensions: Vec<String> = AUDIO_EXTENSIONS.iter().map(|&s| s.to_string()).collect();

        // Get extensions from format_handler capabilities.
        let plugins = self
            .plugin_manager
            .find_plugins_by_capability_kind("format_handler")
            .await;
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
    pub async fn scan_library(
        &self,
        library_id: &str,
        library_path: &str,
        mode: ScanMode,
        task_id: Option<&str>,
    ) -> Result<ScanResult> {
        self.scan_library_scoped(library_id, library_path, mode, task_id, None)
            .await
    }

    /// Scan a library, optionally limiting local-library discovery to a set of
    /// directories supplied by the filesystem watcher. Full scans always ignore
    /// the scope and walk the complete library.
    pub async fn scan_library_scoped(
        &self,
        library_id: &str,
        library_path: &str,
        mode: ScanMode,
        task_id: Option<&str>,
        scan_paths: Option<Vec<std::path::PathBuf>>,
    ) -> Result<ScanResult> {
        info!(
            target: "audit::scan",
            message_key = "scan.started",
            message_params = %serde_json::json!({
                "path": library_path,
                "library_id": library_id,
                "mode": mode.as_str(),
            }),
            scan_mode = %mode.as_str(),
            library_id = %library_id,
            path = %library_path,
            "Library scan started"
        );
        self.update_progress_key(
            task_id,
            "scan.started",
            serde_json::json!({
                "path": library_path,
                "library_id": library_id,
                "mode": mode.as_str(),
            }),
        )
        .await;
        self.check_cancellation(task_id).await?;

        // Fetch library to get configuration and type
        let library = self
            .library_repo
            .find_by_id(library_id)
            .await?
            .ok_or_else(|| {
                TingError::not_found(
                    LookupErrorKind::Library,
                    "scan_library_scoped::not_found",
                    format!("Library not found: {}", library_id),
                )
            })?;

        if library.library_type == "source" {
            return Err(TingError::validation(
                crate::core::app::error::InputErrorKind::UnsupportedValue,
                "scan_library.book_source",
                "Managed book source libraries cannot be scanned as folders".into(),
            ));
        }
        let scraper_config: crate::db::models::ScraperConfig = library
            .scraper_config
            .as_ref()
            .and_then(|json| serde_json::from_str(json).ok())
            .unwrap_or_default();

        let dirty_states = if library.library_type == "local" {
            self.scan_state_repo
                .find_by_library_kind(library_id, "local_dirty")
                .await
                .unwrap_or_default()
        } else {
            Default::default()
        };
        let library_root = Path::new(library_path);
        let consumed_local_dirty: Vec<(String, String)> = dirty_states
            .values()
            .filter(|state| {
                mode.is_full() || Path::new(&state.entry_path).starts_with(library_root)
            })
            .map(|state| (state.entry_path.clone(), state.fingerprint.clone()))
            .collect();
        let effective_scan_paths = if library.library_type == "local" && !mode.is_full() {
            let mut paths = scan_paths.unwrap_or_default();
            for state in dirty_states.values() {
                let dirty_path = std::path::PathBuf::from(&state.entry_path);
                if dirty_path.starts_with(library_root) {
                    paths.push(dirty_path);
                }
            }

            let has_baseline = self
                .scan_state_repo
                .find(library_id, library_path, "local_baseline")
                .await?
                .is_some()
                || !self
                    .scan_state_repo
                    .find_by_library_kind(library_id, "local_file")
                    .await?
                    .is_empty();

            if paths.is_empty() && !scraper_config.disable_watcher && has_baseline {
                Some(Vec::new())
            } else if paths.is_empty() {
                None
            } else {
                Some(paths)
            }
        } else {
            scan_paths
        };

        let last_scanned = if mode.is_full() {
            None
        } else if let Some(ref date_str) = library.last_scanned_at {
            chrono::DateTime::parse_from_rfc3339(date_str)
                .map(|dt| dt.with_timezone(&chrono::Utc))
                .ok()
        } else {
            None
        };

        // Dispatch based on library type
        let scan_result = if library.library_type == "webdav" {
            self.scan_webdav_library(&library, task_id, &scraper_config, mode)
                .await?
        } else if library.library_type == "rss" {
            self.scan_rss_library(&library, task_id, mode).await?
        } else {
            // Local library scan
            let path = Path::new(library_path);
            if !path.exists() {
                return Err(TingError::not_found(
                    LookupErrorKind::Library,
                    "scan_library_scoped::not_found",
                    format!("Library path does not exist: {}", library_path),
                ));
            }

            if !path.is_dir() {
                return Err(TingError::validation(
                    InputErrorKind::InvalidPath,
                    "scan_library_scoped::validation",
                    format!("Library path is not a directory: {}", library_path),
                ));
            }

            if effective_scan_paths
                .as_ref()
                .is_some_and(|paths| paths.is_empty())
            {
                let total_books = self
                    .book_repo
                    .find_all_minimal_by_library(library_id)
                    .await?
                    .len();
                ScanResult {
                    total_books,
                    books_skipped: total_books,
                    start_time: Some(std::time::Instant::now()),
                    end_time: Some(std::time::Instant::now()),
                    ..Default::default()
                }
            } else {
                self.scan_local_library(LocalLibraryScanContext {
                    library_id,
                    path,
                    task_id,
                    last_scanned,
                    scraper_config: &scraper_config,
                    mode,
                    scan_paths: effective_scan_paths.as_deref(),
                })
                .await?
            }
        };

        if !consumed_local_dirty.is_empty() {
            self.scan_state_repo
                .delete_matching(library_id, "local_dirty", consumed_local_dirty)
                .await?;
        }

        // Update library last_scanned_at
        if let Err(e) = self.library_repo.update_last_scanned(library_id).await {
            warn!("Failed to update library last_scanned_at: {}", e);
        }

        info!(
            target: "audit::scan",
            message_key = "scan.completed",
            message_params = %serde_json::json!({
                "path": library_path,
                "total": scan_result.total_books,
                "created": scan_result.books_created,
                "updated": scan_result.books_updated,
                "deleted": scan_result.books_deleted,
                "errors": scan_result.errors.len(),
            }),
            path = %library_path,
            total_books = scan_result.total_books,
            books_created = scan_result.books_created,
            books_updated = scan_result.books_updated,
            books_deleted = scan_result.books_deleted,
            errors = scan_result.errors.len(),
            "Library scan completed"
        );
        self.update_progress_key(
            task_id,
            "scan.completed",
            serde_json::json!({
                "path": library_path,
                "total": scan_result.total_books,
                "created": scan_result.books_created,
                "updated": scan_result.books_updated,
                "deleted": scan_result.books_deleted,
                "errors": scan_result.errors.len(),
            }),
        )
        .await;

        // Trigger Merge Suggestions
        if !scan_result.changed_book_ids.is_empty()
            && scan_result.failed_count == 0
            && let Some(merge_service) = &self.merge_service
        {
            self.update_progress_key(task_id, "scan.auto_merge.processing", serde_json::json!({}))
                .await;
            if let Err(e) = merge_service
                .process_auto_merges_for_library_books(library_id, &scan_result.changed_book_ids)
                .await
            {
                warn!("Failed to process auto-merges: {}", e);
            }
        }

        Ok(scan_result)
    }

    pub(crate) async fn extract_chapter_metadata(
        &self,
        path: &Path,
        cloud_mode: bool,
    ) -> (
        String,
        String,
        Option<String>,
        Option<String>,
        Option<String>,
        i32,
    ) {
        // Returns: (album, title, author, narrator, cover_url, duration)

        let ext = path
            .extension()
            .map(|e| e.to_string_lossy().to_lowercase())
            .unwrap_or_default();
        let is_standard = STANDARD_EXTENSIONS.contains(&ext.as_str());

        // A sidecar without a duration still needs the source file's duration.
        let nfo_path = path.with_extension("nfo");
        if let Ok(meta) = self.nfo_manager.read_chapter_nfo(&nfo_path) {
            let mut duration = meta.duration.unwrap_or(0) as i32;
            if duration <= 0 && is_standard && ext != "strm" {
                duration = self.probe_local_audio_duration(path).await;
            }
            return (String::new(), meta.title, None, None, None, duration);
        }

        // Handle .strm files explicitly
        if ext == "strm" {
            let t = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_string();

            // In cloud/drive mode for local libraries, we do not probe remote URLs or durations
            if cloud_mode {
                tracing::info!(
                    "Cloud mode enabled, skipping metadata extraction for strm file: {}",
                    path.display()
                );
                return (String::new(), t, None, None, None, 0);
            }

            // strm files are URL references, not actual audio files
            // Read the URL from the file
            let url = match tokio::fs::read_to_string(path).await {
                Ok(content) => content.trim().to_string(),
                Err(e) => {
                    tracing::error!(
                        path = %path.display(),
                        error = %e,
                        message_key = "strm.file.read_failed",
                        message_params = %serde_json::json!({
                            "path": path.display().to_string(),
                            "error": e.to_string(),
                        }),
                        "Failed to read strm file"
                    );
                    return (String::new(), t, None, None, None, 0);
                }
            };

            if url.is_empty() || !url.starts_with("http") {
                tracing::warn!(
                    path = %path.display(),
                    url = %url,
                    message_key = "strm.url.invalid",
                    message_params = %serde_json::json!({
                        "path": path.display().to_string(),
                    }),
                    "strm file contains invalid URL"
                );
                return (String::new(), t, None, None, None, 0);
            }

            // Use the FFprobe binary shipped beside the server.
            let duration = if let Ok(mut ffprobe) =
                crate::core::audio::AudioService::ffprobe_command()
            {
                tracing::info!("Using FFprobe to get strm file duration: {}", url);

                // Add small delay to avoid overwhelming the server
                tokio::time::sleep(std::time::Duration::from_millis(200)).await;

                // Add User-Agent and other headers to avoid being blocked
                match ffprobe
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
                                tracing::info!("strm file {} duration: {} seconds", t, duration_secs);
                                duration_secs
                            }
                            Err(_) => {
                                tracing::warn!(
                                    output = %duration_str,
                                    message_key = "ffprobe.output_parse_failed",
                                    "Failed to parse FFprobe output"
                                );
                                0
                            }
                        }
                    }
                    Ok(output) => {
                        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                        tracing::warn!(
                            error = %stderr,
                            message_key = "ffprobe.duration_failed",
                            message_params = %serde_json::json!({ "error": stderr }),
                            "FFprobe duration detection failed"
                        );
                        0
                    }
                    Err(e) => {
                        tracing::warn!(
                            error = %e,
                            message_key = "ffprobe.run_failed",
                            message_params = %serde_json::json!({ "error": e.to_string() }),
                            "Failed to run FFprobe"
                        );
                        0
                    }
                }
            } else {
                tracing::warn!(
                    message_key = "ffprobe.missing",
                    "FFprobe not found; duration will be set to zero"
                );
                0
            };

            tracing::info!("Detected strm file: {}, duration: {} seconds", t, duration);

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
        let mut plugin_handled = false;
        if let Ok(Some(extracted)) = self
            .plugin_manager
            .extract_local_format_metadata(path, false)
            .await
            && let Ok(result) = extracted.into_scanner_json(None)
        {
            tracing::debug!(
                "Using format plugin {} to process {} file",
                "format_handler",
                ext
            );

            if let Some(t) = result.get("title").and_then(|v| v.as_str())
                && !t.trim().is_empty()
            {
                title = t.to_string();
            }
            if let Some(a) = result.get("album").and_then(|v| v.as_str())
                && !a.trim().is_empty()
            {
                album = a.to_string();
            }
            if let Some(au) = result.get("artist").and_then(|v| v.as_str())
                && !au.trim().is_empty()
            {
                author = Some(au.to_string());
            }
            if let Some(aa) = result.get("album_artist").and_then(|v| v.as_str())
                && !aa.trim().is_empty()
            {
                author = Some(aa.to_string());
            }
            if let Some(n) = result.get("narrator").and_then(|v| v.as_str())
                && !n.trim().is_empty()
            {
                narrator = Some(n.to_string());
            }
            if let Some(dur) = result.get("duration").and_then(|v| v.as_f64()) {
                duration = dur.round() as i32;
                if duration > 0 {
                    tracing::debug!(
                        "Format plugin {} detected duration: {} seconds",
                        "format_handler",
                        duration
                    );
                }
            }
            if let Some(c) = result.get("cover_url").and_then(|v| v.as_str())
                && !c.trim().is_empty()
            {
                cover_url = Some(c.to_string());
            }

            plugin_handled = true;
        }

        if !plugin_handled && is_standard {
            match crate::core::audio::AudioService::read_file_metadata(
                &crate::core::audio::metadata::AudioInput::local(path),
                None,
            )
            .await
            {
                Ok(metadata) => {
                    (author, narrator) = metadata.author_narrator();
                    album = metadata.album.unwrap_or_default();
                    title = metadata.title.unwrap_or_default();
                    duration = metadata.duration.round() as i32;
                }
                Err(error) => {
                    tracing::warn!(path = %path.display(), error = %error, "Audio metadata read failed");
                    if let Ok(metadata) = self.audio_streamer.read_metadata(path) {
                        duration = metadata.duration.as_secs() as i32;
                        title = metadata.title.unwrap_or_default();
                        album = metadata.album.unwrap_or_default();
                        author = metadata.album_artist.or(metadata.artist);
                        narrator = metadata.composer;
                    }
                }
            }
        }

        // Standard containers such as ASF/WMA are not all handled by Symphonia.
        // Keep the bundled FFprobe fallback that provided their duration before.
        if duration <= 0 && is_standard {
            duration = self.probe_local_audio_duration(path).await;
        }

        // 3. 返回提取的元数据
        if !title.is_empty() || !album.is_empty() || duration > 0 {
            return (album, title, author, narrator, cover_url, duration);
        }

        // 4. 如果都失败了，返回空值
        (String::new(), String::new(), None, None, None, 0)
    }

    async fn probe_local_audio_duration(&self, path: &Path) -> i32 {
        match crate::core::audio::AudioService::probe_duration(path).await {
            Ok(Some(duration)) => {
                tracing::debug!(
                    path = %path.display(),
                    duration,
                    "Read local audio duration with bundled FFprobe"
                );
                duration.round() as i32
            }
            Ok(None) => 0,
            Err(error) => {
                tracing::warn!(
                    path = %path.display(),
                    error = %error,
                    "Could not read local audio duration with bundled FFprobe"
                );
                0
            }
        }
    }
}
