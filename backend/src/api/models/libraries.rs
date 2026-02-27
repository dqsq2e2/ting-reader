use serde::{Deserialize, Serialize};

// Library Management API models

/// Response for library operations (compatible with Node.js version)
#[derive(Debug, Serialize)]
pub struct LibraryResponse {
    pub id: String,
    pub name: String,
    pub library_type: String,
    pub url: String,
    pub username: Option<String>,
    pub root_path: String,
    pub last_scanned_at: Option<String>,
    pub created_at: String,
    pub scraper_config: Option<serde_json::Value>,
}

impl From<crate::db::models::Library> for LibraryResponse {
    fn from(library: crate::db::models::Library) -> Self {
        Self {
            id: library.id,
            name: library.name,
            library_type: library.library_type,
            url: library.url,
            username: library.username,
            root_path: library.root_path,
            last_scanned_at: library.last_scanned_at,
            created_at: library.created_at,
            scraper_config: library.scraper_config.and_then(|s| serde_json::from_str(&s).ok()),
        }
    }
}

/// Response for libraries list
#[derive(Debug, Serialize)]
pub struct LibrariesListResponse {
    pub libraries: Vec<LibraryResponse>,
    pub total: usize,
}

/// Request body for creating a library
/// 
/// Accepts frontend format: path (for local), webdav_url, webdav_username, webdav_password
#[derive(Debug, Deserialize)]
pub struct CreateLibraryRequest {
    pub name: String,
    pub library_type: String,
    /// Local path (for local libraries) - frontend sends this as "path"
    pub path: Option<String>,
    /// WebDAV URL (for WebDAV libraries) - frontend sends this as "webdav_url"
    pub webdav_url: Option<String>,
    /// WebDAV username - frontend sends this as "webdav_username"
    pub webdav_username: Option<String>,
    /// WebDAV password - frontend sends this as "webdav_password"
    pub webdav_password: Option<String>,
    pub description: Option<String>,
    pub enabled: Option<bool>,
    /// Root path (mainly for WebDAV)
    pub root_path: Option<String>,
    /// Scraper configuration
    pub scraper_config: Option<serde_json::Value>,
}

/// Request body for updating a library
/// 
/// Accepts frontend format: path, webdav_url, webdav_username, webdav_password
#[derive(Debug, Deserialize)]
pub struct UpdateLibraryRequest {
    pub name: Option<String>,
    pub library_type: Option<String>,
    /// Local path (for local libraries)
    pub path: Option<String>,
    /// WebDAV URL (for WebDAV libraries)
    pub webdav_url: Option<String>,
    /// WebDAV username
    pub webdav_username: Option<String>,
    /// WebDAV password
    pub webdav_password: Option<String>,
    pub description: Option<String>,
    pub enabled: Option<bool>,
    /// Root path (mainly for WebDAV)
    pub root_path: Option<String>,
    /// Scraper configuration
    pub scraper_config: Option<serde_json::Value>,
}

/// Response for library scan
#[derive(Debug, Serialize)]
pub struct LibraryScanResponse {
    pub task_id: String,
    pub status: String,
    pub message: String,
}

/// Folder information
#[derive(Debug, Serialize)]
pub struct FolderInfo {
    pub name: String,
    pub path: String,
    pub is_directory: bool,
}

// Cache management models

/// Information about a cached chapter
#[derive(Debug, Serialize)]
pub struct CacheInfoResponse {
    pub chapter_id: String,
    pub book_id: Option<String>,
    pub book_title: Option<String>,
    pub chapter_title: Option<String>,
    pub file_size: u64,
    pub created_at: Option<String>,
    pub cover_url: Option<String>,
}

/// Response for cache list
#[derive(Debug, Serialize)]
pub struct CacheListResponse {
    pub caches: Vec<CacheInfoResponse>,
    pub total: usize,
    pub total_size: u64,
}

/// Response for cache operations
#[derive(Debug, Serialize)]
pub struct CacheOperationResponse {
    pub success: bool,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_info: Option<CacheInfoResponse>,
}

/// Response for clear all caches
#[derive(Debug, Serialize)]
pub struct ClearCacheResponse {
    pub success: bool,
    pub deleted_count: usize,
    pub message: String,
}
