use super::AppState;
use crate::api::models::{
    BookResponse, CreatePlaylistRequest, PlaylistItemRequest, PlaylistItemResponse,
    PlaylistResponse, SeriesResponse, UpdatePlaylistRequest,
};
use crate::auth::middleware::AuthUser;
use crate::core::error::{Result, TingError};
use crate::db::models::{Playlist, PlaylistItem};
use crate::db::repository::Repository;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use uuid::Uuid;

pub async fn list_playlists(
    State(state): State<AppState>,
    user: AuthUser,
) -> Result<impl IntoResponse> {
    let is_admin = user.role == "admin";
    let playlists = state.playlist_repo.find_by_user(&user.id).await?;
    let mut response = Vec::new();

    for playlist in playlists {
        response.push(build_playlist_response(&state, playlist, &user.id, is_admin).await?);
    }

    Ok(Json(response))
}

pub async fn get_playlist(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<String>,
) -> Result<impl IntoResponse> {
    let playlist = state
        .playlist_repo
        .find_by_user_and_id(&id, &user.id)
        .await?
        .ok_or_else(|| TingError::NotFound(format!("Playlist with id {} not found", id)))?;

    let response =
        build_playlist_response(&state, playlist, &user.id, user.role == "admin").await?;
    Ok(Json(response))
}

pub async fn create_playlist(
    State(state): State<AppState>,
    user: AuthUser,
    Json(req): Json<CreatePlaylistRequest>,
) -> Result<impl IntoResponse> {
    let title = req.title.trim();
    if title.is_empty() {
        return Err(TingError::InvalidRequest(
            "Playlist title cannot be empty".to_string(),
        ));
    }

    let now = chrono::Utc::now().to_rfc3339();
    let playlist = Playlist {
        id: Uuid::new_v4().to_string(),
        user_id: user.id.clone(),
        title: title.to_string(),
        description: req.description.map(|value| value.trim().to_string()),
        created_at: now.clone(),
        updated_at: now,
    };

    state.playlist_repo.create(&playlist).await?;
    let items = normalize_playlist_items(req.items, req.book_ids)?;
    state
        .playlist_repo
        .replace_items(&playlist.id, &user.id, user.role == "admin", items)
        .await?;

    let response =
        build_playlist_response(&state, playlist, &user.id, user.role == "admin").await?;
    Ok((StatusCode::CREATED, Json(response)))
}

pub async fn update_playlist(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<String>,
    Json(req): Json<UpdatePlaylistRequest>,
) -> Result<impl IntoResponse> {
    let existing = state
        .playlist_repo
        .find_by_user_and_id(&id, &user.id)
        .await?
        .ok_or_else(|| TingError::NotFound(format!("Playlist with id {} not found", id)))?;

    let title = req
        .title
        .as_ref()
        .map(|value| value.trim().to_string())
        .unwrap_or(existing.title.clone());

    if title.is_empty() {
        return Err(TingError::InvalidRequest(
            "Playlist title cannot be empty".to_string(),
        ));
    }

    let updated = Playlist {
        id: existing.id.clone(),
        user_id: existing.user_id,
        title,
        description: if req.description.is_some() {
            req.description.map(|value| value.trim().to_string())
        } else {
            existing.description
        },
        created_at: existing.created_at,
        updated_at: chrono::Utc::now().to_rfc3339(),
    };

    state.playlist_repo.update(&updated).await?;

    if req.items.is_some() || req.book_ids.is_some() {
        let items = normalize_playlist_items(req.items, req.book_ids.unwrap_or_default())?;
        state
            .playlist_repo
            .replace_items(&id, &user.id, user.role == "admin", items)
            .await?;
    }

    let refreshed = state
        .playlist_repo
        .find_by_user_and_id(&id, &user.id)
        .await?
        .unwrap_or(updated);
    let response =
        build_playlist_response(&state, refreshed, &user.id, user.role == "admin").await?;
    Ok(Json(response))
}

pub async fn delete_playlist(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<String>,
) -> Result<impl IntoResponse> {
    let existing = state
        .playlist_repo
        .find_by_user_and_id(&id, &user.id)
        .await?;

    if existing.is_none() {
        return Err(TingError::NotFound(format!(
            "Playlist with id {} not found",
            id
        )));
    }

    state.playlist_repo.delete(&id).await?;
    Ok(StatusCode::NO_CONTENT)
}

fn normalize_playlist_items(
    items: Option<Vec<PlaylistItemRequest>>,
    fallback_book_ids: Vec<String>,
) -> Result<Vec<PlaylistItem>> {
    let raw_items = items.unwrap_or_else(|| {
        fallback_book_ids
            .into_iter()
            .map(|book_id| PlaylistItemRequest {
                item_type: "book".to_string(),
                item_id: book_id,
            })
            .collect()
    });

    let mut normalized = Vec::new();
    for (idx, item) in raw_items.into_iter().enumerate() {
        let item_type = item.item_type.trim().to_lowercase();
        let item_id = item.item_id.trim().to_string();

        if item_id.is_empty() || !matches!(item_type.as_str(), "book" | "series") {
            return Err(TingError::InvalidRequest(
                "Playlist items must be books or series with a valid id".to_string(),
            ));
        }

        normalized.push(PlaylistItem {
            playlist_id: String::new(),
            item_type,
            item_id,
            item_order: (idx + 1) as i32,
        });
    }

    Ok(normalized)
}

async fn build_playlist_response(
    state: &AppState,
    playlist: Playlist,
    user_id: &str,
    is_admin: bool,
) -> Result<PlaylistResponse> {
    let mut response = PlaylistResponse::from(playlist.clone());
    let items = state
        .playlist_repo
        .find_items_by_playlist(&playlist.id)
        .await?;

    for item in items {
        match item.item_type.as_str() {
            "book" => {
                if let Some(book) = state.book_repo.find_by_id(&item.item_id).await? {
                    if !state
                        .book_repo
                        .check_access(&book.id, user_id, is_admin)
                        .await?
                    {
                        continue;
                    }

                    let book_response = BookResponse::from(book);
                    response.book_ids.push(book_response.id.clone());
                    response.books.push(book_response.clone());
                    response.items.push(PlaylistItemResponse {
                        item_type: "book".to_string(),
                        item_id: item.item_id,
                        order: item.item_order,
                        book: Some(book_response),
                        series: None,
                    });
                }
            }
            "series" => {
                if let Some(series) = state.series_repo.find_by_id(&item.item_id).await? {
                    if !is_admin
                        && !state
                            .series_repo
                            .check_access(&series.id, user_id, is_admin)
                            .await?
                    {
                        continue;
                    }

                    let mut series_response = SeriesResponse::from(series.clone());
                    let series_books = state
                        .series_repo
                        .find_books_by_series_with_filters(&series.id, user_id, is_admin)
                        .await?;

                    for (book, _) in series_books {
                        let book_response = BookResponse::from(book);
                        response.book_ids.push(book_response.id.clone());
                        response.books.push(book_response.clone());
                        series_response.books.push(book_response);
                    }

                    response.items.push(PlaylistItemResponse {
                        item_type: "series".to_string(),
                        item_id: item.item_id,
                        order: item.item_order,
                        book: None,
                        series: Some(series_response),
                    });
                }
            }
            _ => {}
        }
    }

    Ok(response)
}
