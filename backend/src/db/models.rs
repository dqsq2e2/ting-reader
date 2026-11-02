//! Database models
//!
//! Data structures representing database tables

use serde::{Deserialize, Serialize};

/// Book record in the database
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Book {
    pub id: String,
    pub library_id: String,
    pub title: Option<String>,
    pub author: Option<String>,
    pub narrator: Option<String>,
    pub cover_url: Option<String>,
    pub theme_color: Option<String>,
    pub description: Option<String>,
    pub skip_intro: i32,
    pub skip_outro: i32,
    pub path: String,
    pub hash: String,
    pub tags: Option<String>,
    pub genre: Option<String>,
    pub year: Option<i32>,
    pub created_at: String,
    // V5 Migration
    #[serde(default)]
    pub manual_corrected: i32, // 0 or 1
    #[serde(default)]
    pub match_pattern: Option<String>,
    // V6 Migration
    #[serde(default)]
    pub chapter_regex: Option<String>,
}

/// User library access record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserLibraryAccess {
    pub user_id: String,
    pub library_id: String,
    pub created_at: String,
}

/// User book access record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserBookAccess {
    pub user_id: String,
    pub book_id: String,
    pub created_at: String,
}

/// Chapter record in the database
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Chapter {
    pub id: String,
    pub book_id: String,
    pub title: Option<String>,
    pub path: String,
    pub duration: Option<i32>,
    pub chapter_index: Option<i32>,
    pub is_extra: i32, // 0 or 1
    pub hash: Option<String>,
    // V7
    #[serde(default)]
    pub manual_corrected: i32, // 0 or 1
    // V31: milliseconds inside `path` for chapters read from an embedded
    // chapter table. Both values stay empty for one-file-per-chapter books.
    #[serde(default)]
    pub start_time: Option<i64>,
    #[serde(default)]
    pub end_time: Option<i64>,
    pub created_at: String,
}

/// Task record in the database
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskRecord {
    pub id: String,
    pub task_type: String,
    pub status: String,
    pub payload: Option<String>,
    pub message: Option<String>,
    pub message_key: Option<String>,
    pub message_params: Option<String>,
    pub error: Option<String>,
    pub retries: i32,
    pub max_retries: i32,
    pub created_at: String,
    pub updated_at: String,
}

impl TaskRecord {
    /// The existing TEXT column stores an error envelope. Historical rows
    /// contain only display text, so their original classification is unknown.
    pub fn failure(&self) -> Option<crate::core::app::error::ErrorResponse> {
        use crate::core::app::error::{ErrorResponse, TaskErrorKind, TingError};
        self.error.as_ref().map(|stored| {
            serde_json::from_str(stored).unwrap_or_else(|_| {
                ErrorResponse::from_error_with_trace_id(
                    &TingError::task(TaskErrorKind::Unclassified, "task.history", stored.clone()),
                    format!("legacy-task-{}", self.id),
                )
            })
        })
    }

    pub fn encode_failure(
        error: Option<&crate::core::app::error::ErrorResponse>,
    ) -> crate::core::app::error::Result<Option<String>> {
        use crate::core::app::error::{CodecErrorKind, TingError};
        error
            .map(serde_json::to_string)
            .transpose()
            .map_err(|error| {
                TingError::serialization(
                    CodecErrorKind::InvalidData,
                    "task.persist_error",
                    "Failed to serialize task error".into(),
                )
                .with_source(error.into())
            })
    }
}

/// User record in the database
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: String,
    pub username: String,
    pub password_hash: String,
    pub role: String,
    pub created_at: String,
}

/// Progress record in the database
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Progress {
    pub id: String,
    pub user_id: String,
    pub book_id: String,
    pub chapter_id: Option<String>,
    pub position: f64,
    pub duration: Option<f64>,
    pub updated_at: String,
}

/// Favorite record in the database
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Favorite {
    pub id: String,
    pub user_id: String,
    pub book_id: String,
    pub created_at: String,
}

/// User settings record in the database
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserSettings {
    pub user_id: String,
    pub playback_speed: f64,
    pub theme: String,
    pub auto_play: i32,
    pub skip_intro: i32,
    pub skip_outro: i32,
    pub settings_json: Option<String>,
    pub updated_at: String,
}

/// Library record in the database (compatible with Node.js version)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Library {
    pub id: String,
    pub name: String,
    pub library_type: String,
    pub url: String,
    pub username: Option<String>,
    pub password: Option<String>,
    pub root_path: String,
    pub last_scanned_at: Option<String>,
    pub created_at: String,
    pub scraper_config: Option<String>,
}

impl Library {
    pub fn can_write_metadata_files(&self) -> bool {
        self.library_type == "local"
            || self.library_type == "webdav"
                && self
                    .scraper_config
                    .as_deref()
                    .and_then(|json| serde_json::from_str::<ScraperConfig>(json).ok())
                    .is_some_and(|config| config.webdav_metadata_writing_enabled)
    }
}

/// Scraper configuration stored in Library
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScraperConfig {
    /// Default list of scraper sources in order of priority
    #[serde(default)]
    pub default_sources: Vec<String>,
    /// Specific sources for author
    pub author_sources: Option<Vec<String>>,
    /// Specific sources for narrator
    pub narrator_sources: Option<Vec<String>>,
    /// Specific sources for cover image
    pub cover_sources: Option<Vec<String>>,
    /// Specific sources for introduction/description
    pub intro_sources: Option<Vec<String>>,
    /// Specific sources for tags
    pub tags_sources: Option<Vec<String>>,
    /// Whether to write metadata to NFO files
    #[serde(default)]
    pub nfo_writing_enabled: bool,
    /// Whether to write metadata to metadata.json files
    #[serde(default)]
    pub metadata_writing_enabled: bool,
    /// Synchronize edited sidecars and file covers to the WebDAV book folder.
    #[serde(default)]
    pub webdav_metadata_writing_enabled: bool,
    /// Priority order for metadata sources
    /// Values: "filename", "local_metadata" (nfo/json), "audio_metadata", "scraper"
    #[serde(default = "default_metadata_priority")]
    pub metadata_priority: Vec<String>,
    /// Whether to extract cover image from audio files
    #[serde(default = "default_extract_audio_cover")]
    pub extract_audio_cover: bool,
    /// Whether to detect and group extra chapters separately
    #[serde(default = "default_extract_extra_chapters")]
    pub extract_extra_chapters: bool,
    /// Automatically create and link series inferred from directory names.
    /// Explicit series declarations in metadata.json are always imported.
    #[serde(default = "default_auto_generate_series")]
    pub auto_generate_series: bool,
    /// Whether to disable the directory watcher for this library
    #[serde(default)]
    pub disable_watcher: bool,
    /// Cloud drive mode: when enabled, adjust scanning behavior for WebDAV/local libraries
    #[serde(default)]
    pub cloud_mode: bool,
    /// Whether periodic incremental synchronization is enabled for remote libraries.
    #[serde(default)]
    pub scheduled_sync_enabled: bool,
    /// Periodic synchronization interval: hourly, daily, weekly, or monthly.
    #[serde(default = "default_scheduled_sync_interval")]
    pub scheduled_sync_interval: String,
}

impl ScraperConfig {
    pub fn metadata_sources(&self) -> impl DoubleEndedIterator<Item = &str> {
        std::iter::once("filename")
            .filter(|_| {
                !self.metadata_priority.is_empty()
                    && !self
                        .metadata_priority
                        .iter()
                        .any(|source| source == "filename")
            })
            .chain(self.metadata_priority.iter().map(String::as_str))
            .chain(
                DEFAULT_METADATA_PRIORITY
                    .into_iter()
                    .filter(|_| self.metadata_priority.is_empty()),
            )
    }

    /// Select a book/chapter title by the same priority policy for both scanners.
    /// The path name remains the fallback when no configured source has a title.
    pub fn select_title<'a>(
        &self,
        filename: &'a str,
        local: Option<&'a str>,
        audio: Option<&'a str>,
        scraper: Option<&'a str>,
    ) -> (&'a str, &'static str) {
        for source in self.metadata_sources() {
            let (value, source) = match source {
                "filename" => (Some(filename), "filename"),
                "local_metadata" => (local, "local_metadata"),
                "audio_metadata" => (
                    audio.filter(|title| !title.trim().to_ascii_lowercase().starts_with("track")),
                    "audio_metadata",
                ),
                "scraper" => (scraper, "scraper"),
                _ => continue,
            };
            if let Some(title) = value.filter(|title| !title.trim().is_empty()) {
                return (title, source);
            }
        }
        (filename, "filename")
    }

    pub fn metadata_rank(&self, source: &str) -> usize {
        self.metadata_sources()
            .position(|candidate| candidate == source)
            .unwrap_or(usize::MAX - 1)
    }

    pub fn scraper_sources(&self) -> impl Iterator<Item = &str> {
        [
            Some(&self.default_sources),
            self.author_sources.as_ref(),
            self.narrator_sources.as_ref(),
            self.cover_sources.as_ref(),
            self.intro_sources.as_ref(),
            self.tags_sources.as_ref(),
        ]
        .into_iter()
        .flatten()
        .flat_map(|sources| sources.iter())
        .map(|source| source.trim())
        .filter(|source| !source.is_empty())
    }

    pub fn has_scraper_sources(&self) -> bool {
        self.scraper_sources().next().is_some()
    }
}

impl Default for ScraperConfig {
    fn default() -> Self {
        Self {
            default_sources: Vec::new(),
            author_sources: None,
            narrator_sources: None,
            cover_sources: None,
            intro_sources: None,
            tags_sources: None,
            nfo_writing_enabled: false,
            metadata_writing_enabled: false,
            webdav_metadata_writing_enabled: false,
            metadata_priority: default_metadata_priority(),
            extract_audio_cover: default_extract_audio_cover(),
            extract_extra_chapters: default_extract_extra_chapters(),
            auto_generate_series: default_auto_generate_series(),
            disable_watcher: false,
            cloud_mode: false,
            scheduled_sync_enabled: false,
            scheduled_sync_interval: default_scheduled_sync_interval(),
        }
    }
}

fn default_extract_audio_cover() -> bool {
    true
}

fn default_extract_extra_chapters() -> bool {
    true
}

fn default_auto_generate_series() -> bool {
    true
}

const DEFAULT_METADATA_PRIORITY: [&str; 4] =
    ["filename", "local_metadata", "audio_metadata", "scraper"];

/// Upgrade persisted or incoming legacy settings without dropping plugin fields.
pub(crate) fn normalize_metadata_priority(config: &mut serde_json::Value) {
    let Some(config) = config.as_object_mut() else {
        return;
    };
    let prefer_filename = config
        .remove("use_filename_as_title")
        .and_then(|value| value.as_bool())
        .unwrap_or(true);
    let mut priority: Vec<serde_json::Value> = config
        .get("metadata_priority")
        .and_then(|value| value.as_array())
        .filter(|values| !values.is_empty())
        .cloned()
        .unwrap_or_else(|| {
            DEFAULT_METADATA_PRIORITY[1..]
                .iter()
                .map(|value| (*value).into())
                .collect()
        });
    if !priority.iter().any(|value| value == "filename") {
        if prefer_filename {
            priority.insert(0, "filename".into());
        } else {
            priority.push("filename".into());
        }
    }
    config.insert("metadata_priority".into(), priority.into());
}

fn default_metadata_priority() -> Vec<String> {
    DEFAULT_METADATA_PRIORITY
        .into_iter()
        .map(str::to_owned)
        .collect()
}

fn default_scheduled_sync_interval() -> String {
    "daily".to_string()
}

/// Series record in the database
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Series {
    pub id: String,
    pub library_id: String,
    pub title: String,
    pub author: Option<String>,
    pub narrator: Option<String>,
    pub cover_url: Option<String>,
    pub description: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// Series book link record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SeriesBook {
    pub series_id: String,
    pub book_id: String,
    pub book_order: i32,
}

/// User playlist record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Playlist {
    pub id: String,
    pub user_id: String,
    pub title: String,
    pub description: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// Playlist book link record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaylistBook {
    pub playlist_id: String,
    pub book_id: String,
    pub book_order: i32,
}

/// Playlist item link record. Items can be individual books or whole series.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaylistItem {
    pub playlist_id: String,
    pub item_type: String,
    pub item_id: String,
    pub item_order: i32,
}

/// Webhook notification configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationWebhook {
    pub id: String,
    pub name: String,
    pub url: String,
    pub enabled: i32,
    pub events: String,
    pub secret: Option<String>,
    pub headers: String,
    pub body_template: String,
    pub created_at: String,
    pub updated_at: String,
}
