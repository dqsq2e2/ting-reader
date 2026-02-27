use serde::{Deserialize, Serialize};

// Progress Management API models

/// Response for progress operations
#[derive(Debug, Serialize)]
pub struct ProgressResponse {
    pub id: String,
    pub user_id: String,
    pub book_id: String,
    pub chapter_id: Option<String>,
    pub position: f64,
    pub duration: Option<f64>,
    pub updated_at: String,
    pub book_title: Option<String>,
    pub cover_url: Option<String>,
    pub library_id: Option<String>,
    pub chapter_title: Option<String>,
    pub chapter_duration: Option<i32>,
}

impl From<crate::db::models::Progress> for ProgressResponse {
    fn from(progress: crate::db::models::Progress) -> Self {
        Self {
            id: progress.id,
            user_id: progress.user_id,
            book_id: progress.book_id,
            chapter_id: progress.chapter_id,
            position: progress.position,
            duration: progress.duration,
            updated_at: progress.updated_at,
            book_title: None,
            cover_url: None,
            library_id: None,
            chapter_title: None,
            chapter_duration: None,
        }
    }
}

/// Response for recent progress list
#[derive(Debug, Serialize)]
pub struct RecentProgressResponse {
    pub progress: Vec<ProgressResponse>,
    pub total: usize,
}

/// Request body for updating progress
#[derive(Debug, Deserialize)]
pub struct UpdateProgressRequest {
    pub book_id: String,
    pub chapter_id: Option<String>,
    pub position: f64,
    pub duration: Option<f64>,
}

// Favorites Management API models

/// Response for favorite operations
#[derive(Debug, Serialize)]
pub struct FavoriteResponse {
    pub id: String,
    pub user_id: String,
    pub book_id: String,
    pub created_at: String,
}

impl From<crate::db::models::Favorite> for FavoriteResponse {
    fn from(favorite: crate::db::models::Favorite) -> Self {
        Self {
            id: favorite.id,
            user_id: favorite.user_id,
            book_id: favorite.book_id,
            created_at: favorite.created_at,
        }
    }
}

/// Response for favorites list
#[derive(Debug, Serialize)]
pub struct FavoritesListResponse {
    pub favorites: Vec<FavoriteResponse>,
    pub total: usize,
}

/// Response for add/remove favorite
#[derive(Debug, Serialize)]
pub struct FavoriteActionResponse {
    pub message: String,
}

// User Settings API models

/// Response for user settings
#[derive(Debug, Serialize)]
pub struct UserSettingsResponse {
    pub user_id: String,
    pub playback_speed: f64,
    pub theme: String,
    pub auto_play: bool,
    pub skip_intro: i32,
    pub skip_outro: i32,
    pub sleep_timer_default: i32,
    pub auto_preload: bool,
    pub auto_cache: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub widget_css: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub settings_json: Option<serde_json::Value>,
    pub updated_at: String,
}

impl From<crate::db::models::UserSettings> for UserSettingsResponse {
    fn from(settings: crate::db::models::UserSettings) -> Self {
        Self {
            user_id: settings.user_id,
            playback_speed: settings.playback_speed,
            theme: settings.theme,
            auto_play: settings.auto_play != 0,
            skip_intro: settings.skip_intro,
            skip_outro: settings.skip_outro,
            sleep_timer_default: 0, // Default value, will be filled if in settings_json
            auto_preload: true, // Default value, will be filled if in settings_json
            auto_cache: false, // Default value
            widget_css: None, // Default value, will be filled if in settings_json
            settings_json: settings.settings_json.and_then(|s| serde_json::from_str(&s).ok()),
            updated_at: settings.updated_at,
        }
    }
}

/// Request body for updating user settings
#[derive(Debug, Deserialize)]
pub struct UpdateUserSettingsRequest {
    pub playback_speed: Option<f64>,
    pub theme: Option<String>,
    pub auto_play: Option<bool>,
    pub skip_intro: Option<i32>,
    pub skip_outro: Option<i32>,
    pub sleep_timer_default: Option<i32>,
    pub auto_preload: Option<bool>,
    pub auto_cache: Option<bool>,
    pub widget_css: Option<String>,
    #[serde(flatten)]
    pub extra: std::collections::HashMap<String, serde_json::Value>,
}

// User Management API models (Admin)

/// Response for user list
#[derive(Debug, Serialize)]
pub struct UsersListResponse {
    pub users: Vec<UserInfoResponse>,
    pub total: usize,
}

/// User information response (without password)
#[derive(Debug, Serialize)]
pub struct UserInfoResponse {
    pub id: String,
    pub username: String,
    pub role: String,
    pub created_at: String,
    pub libraries_accessible: Vec<String>,
    pub books_accessible: Vec<String>,
}

impl From<crate::db::models::User> for UserInfoResponse {
    fn from(user: crate::db::models::User) -> Self {
        Self {
            id: user.id,
            username: user.username,
            role: user.role,
            created_at: user.created_at,
            libraries_accessible: Vec::new(), // To be filled by handler
            books_accessible: Vec::new(), // To be filled by handler
        }
    }
}

/// Request body for creating a user (admin)
#[derive(Debug, Deserialize)]
pub struct CreateUserRequest {
    pub username: String,
    pub password: String,
    pub role: Option<String>,
    pub libraries_accessible: Option<Vec<String>>,
    pub books_accessible: Option<Vec<String>>,
}

/// Request body for updating a user (admin)
#[derive(Debug, Deserialize)]
pub struct UpdateUserRequest {
    pub username: Option<String>,
    pub password: Option<String>,
    pub role: Option<String>,
    pub libraries_accessible: Option<Vec<String>>,
    pub books_accessible: Option<Vec<String>>,
}

/// Response for user creation/update
#[derive(Debug, Serialize)]
pub struct UserActionResponse {
    pub message: String,
    pub user: UserInfoResponse,
}
