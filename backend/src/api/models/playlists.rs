use crate::api::models::{BookResponse, SeriesResponse};
use crate::db::models::Playlist;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize)]
pub struct PlaylistItemRequest {
    pub item_type: String,
    pub item_id: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreatePlaylistRequest {
    pub title: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub book_ids: Vec<String>,
    #[serde(default)]
    pub items: Option<Vec<PlaylistItemRequest>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UpdatePlaylistRequest {
    pub title: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub book_ids: Option<Vec<String>>,
    #[serde(default)]
    pub items: Option<Vec<PlaylistItemRequest>>,
}

#[derive(Debug, Serialize)]
pub struct PlaylistItemResponse {
    pub item_type: String,
    pub item_id: String,
    pub order: i32,
    pub book: Option<BookResponse>,
    pub series: Option<SeriesResponse>,
}

#[derive(Debug, Serialize)]
pub struct PlaylistResponse {
    pub id: String,
    pub user_id: String,
    pub title: String,
    pub description: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub book_ids: Vec<String>,
    pub books: Vec<BookResponse>,
    pub items: Vec<PlaylistItemResponse>,
}

impl From<Playlist> for PlaylistResponse {
    fn from(playlist: Playlist) -> Self {
        Self {
            id: playlist.id,
            user_id: playlist.user_id,
            title: playlist.title,
            description: playlist.description,
            created_at: playlist.created_at,
            updated_at: playlist.updated_at,
            book_ids: Vec::new(),
            books: Vec::new(),
            items: Vec::new(),
        }
    }
}
