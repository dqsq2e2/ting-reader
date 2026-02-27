//! Database models
//!
//! Data structures representing database tables

use serde::{Deserialize, Serialize};

/// Book record in the database
#[derive(Debug, Clone, Serialize, Deserialize)]
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

/// Merge suggestion record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MergeSuggestion {
    pub id: String,
    pub book_a_id: String,
    pub book_b_id: String,
    pub score: f64,
    pub reason: String,
    pub status: String, // 'pending', 'rejected', 'ignored'
    pub created_at: String,
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
    pub created_at: String,
}

/// Plugin registry record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginRecord {
    pub id: String,
    pub name: String,
    pub version: String,
    pub plugin_type: String,
    pub description: Option<String>,
    pub author: Option<String>,
    pub enabled: i32,
    pub config: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// Plugin dependency record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginDependency {
    pub plugin_id: String,
    pub dependency_id: String,
    pub version_requirement: String,
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
    pub error: Option<String>,
    pub retries: i32,
    pub max_retries: i32,
    pub created_at: String,
    pub updated_at: String,
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

/// Scraper configuration stored in Library
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
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
}
