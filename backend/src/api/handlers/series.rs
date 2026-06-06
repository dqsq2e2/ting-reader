use super::AppState;
use crate::api::models::{BookResponse, CreateSeriesRequest, SeriesResponse, UpdateSeriesRequest};
use crate::core::error::{Result, TingError};
use crate::db::models::{Series, SeriesBook};
use crate::db::repository::Repository;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use uuid::Uuid;

use crate::auth::middleware::AuthUser;

/// Handler for GET /api/v1/series - List all series
pub async fn list_series(
    State(state): State<AppState>,
    user: AuthUser,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> Result<impl IntoResponse> {
    let library_id = params.get("library_id").cloned();
    let is_admin = user.role == "admin";

    let series_list = state
        .series_repo
        .find_with_filters(&user.id, is_admin, library_id)
        .await?;

    let mut response = Vec::new();
    for series in series_list {
        let mut s_res = SeriesResponse::from(series.clone());
        let books = state
            .series_repo
            .find_books_by_series_with_filters(&series.id, &user.id, is_admin)
            .await?;

        s_res.books = books
            .into_iter()
            .map(|(b, _)| BookResponse::from(b))
            .collect();
        response.push(s_res);
    }

    Ok(Json(response))
}

/// Handler for GET /api/v1/series/:id - Get series by ID
pub async fn get_series(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<String>,
) -> Result<impl IntoResponse> {
    let series = state
        .series_repo
        .find_by_id(&id)
        .await?
        .ok_or_else(|| TingError::NotFound(format!("Series with id {} not found", id)))?;

    let is_admin = user.role == "admin";

    // Check if user has access to this series
    if !state
        .series_repo
        .check_access(&series.id, &user.id, is_admin)
        .await?
    {
        return Err(TingError::PermissionDenied(
            "No access to this series".to_string(),
        ));
    }

    let mut response = SeriesResponse::from(series.clone());
    let books = state
        .series_repo
        .find_books_by_series_with_filters(&series.id, &user.id, is_admin)
        .await?;
    response.books = books
        .into_iter()
        .map(|(b, _)| BookResponse::from(b))
        .collect();

    Ok(Json(response))
}

/// Handler for POST /api/v1/series - Create a new series
pub async fn create_series(
    State(state): State<AppState>,
    user: AuthUser,
    Json(req): Json<CreateSeriesRequest>,
) -> Result<impl IntoResponse> {
    if user.role != "admin" {
        return Err(TingError::PermissionDenied(
            "Only admin can create series".to_string(),
        ));
    }

    let series_id = Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();

    // Default metadata from first book if not provided
    let mut author = req.author;
    let mut narrator = req.narrator;
    let mut cover_url = req.cover_url;
    let mut description = req.description;

    if !req.book_ids.is_empty() {
        if let Some(first_book) = state.book_repo.find_by_id(&req.book_ids[0]).await? {
            if author.is_none() {
                author = first_book.author;
            }
            if narrator.is_none() {
                narrator = first_book.narrator;
            }
            if cover_url.is_none() {
                cover_url = first_book.cover_url;
            }
            if description.is_none() {
                description = first_book.description;
            }
        }
    }

    let series = Series {
        id: series_id.clone(),
        library_id: req.library_id,
        title: req.title.trim().to_string(),
        author,
        narrator,
        cover_url,
        description,
        created_at: now.clone(),
        updated_at: now,
    };

    state.series_repo.create(&series).await?;

    // Add books
    let mut books_to_add = Vec::new();
    for book_id in &req.book_ids {
        if let Some(book) = state.book_repo.find_by_id(book_id).await? {
            books_to_add.push(book);
        }
    }

    // Sort naturally by title for new series creation
    books_to_add.sort_by(|a, b| {
        let t1 = a.title.as_deref().unwrap_or("");
        let t2 = b.title.as_deref().unwrap_or("");
        natord::compare(t1, t2)
    });

    for (idx, book) in books_to_add.iter().enumerate() {
        state
            .series_repo
            .add_book(SeriesBook {
                series_id: series_id.clone(),
                book_id: book.id.clone(),
                book_order: (idx + 1) as i32,
            })
            .await?;

        // Update metadata.json
        if let Err(e) = update_book_metadata_series(&state, &book.id).await {
            tracing::warn!("更新书籍 {} 的 metadata.json 失败: {}", book.id, e);
        }
    }

    let mut response = SeriesResponse::from(series);
    // Fetch added books to return full response
    let books = state.series_repo.find_books_by_series(&series_id).await?;
    response.books = books
        .into_iter()
        .map(|(b, _)| BookResponse::from(b))
        .collect();

    Ok((StatusCode::CREATED, Json(response)))
}

/// Handler for PUT /api/v1/series/:id - Update a series
pub async fn update_series(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<String>,
    Json(req): Json<UpdateSeriesRequest>,
) -> Result<impl IntoResponse> {
    if user.role != "admin" {
        return Err(TingError::PermissionDenied(
            "Only admin can update series".to_string(),
        ));
    }

    let existing_series = state
        .series_repo
        .find_by_id(&id)
        .await?
        .ok_or_else(|| TingError::NotFound(format!("Series with id {} not found", id)))?;

    let updated_series = Series {
        id: existing_series.id,
        library_id: existing_series.library_id,
        title: req
            .title
            .as_ref()
            .map(|t| t.trim().to_string())
            .unwrap_or(existing_series.title),
        author: if req.author.is_some() {
            req.author
        } else {
            existing_series.author
        },
        narrator: if req.narrator.is_some() {
            req.narrator
        } else {
            existing_series.narrator
        },
        cover_url: if req.cover_url.is_some() {
            req.cover_url
        } else {
            existing_series.cover_url
        },
        description: if req.description.is_some() {
            req.description
        } else {
            existing_series.description
        },
        created_at: existing_series.created_at,
        updated_at: chrono::Utc::now().to_rfc3339(),
    };

    state.series_repo.update(&updated_series).await?;

    // Update books if provided
    if let Some(book_ids) = req.book_ids {
        // We need to sync the books list.
        // Simplest way: remove all and re-add.
        // But SeriesRepository doesn't have "remove all books".
        // We can get current books, find diff.

        let current_books = state.series_repo.find_books_by_series(&id).await?;
        let current_ids: Vec<String> = current_books.iter().map(|(b, _)| b.id.clone()).collect();
        let mut affected_books = std::collections::HashSet::new();

        // Remove books that are not in new list
        for book_id in &current_ids {
            if !book_ids.contains(book_id) {
                state.series_repo.remove_book(&id, book_id).await?;
                affected_books.insert(book_id.clone());
            }
        }

        // Add/Update books
        for (idx, book_id) in book_ids.iter().enumerate() {
            state
                .series_repo
                .add_book(SeriesBook {
                    series_id: id.clone(),
                    book_id: book_id.clone(),
                    book_order: (idx + 1) as i32,
                })
                .await?;
            affected_books.insert(book_id.clone());
        }

        // Update metadata.json for all affected books
        for book_id in affected_books {
            if let Err(e) = update_book_metadata_series(&state, &book_id).await {
                tracing::warn!("更新书籍 {} 的 metadata.json 失败: {}", book_id, e);
            }
        }
    } else {
        // If only title changed, we should update metadata for all books in series
        if req.title.is_some() {
            let books = state.series_repo.find_books_by_series(&id).await?;
            for (book, _) in books {
                if let Err(e) = update_book_metadata_series(&state, &book.id).await {
                    tracing::warn!("更新书籍 {} 的 metadata.json 失败: {}", book.id, e);
                }
            }
        }
    }

    let mut response = SeriesResponse::from(updated_series);
    let books = state.series_repo.find_books_by_series(&id).await?;
    response.books = books
        .into_iter()
        .map(|(b, _)| BookResponse::from(b))
        .collect();

    Ok(Json(response))
}

/// Handler for DELETE /api/v1/series/:id - Delete a series
pub async fn delete_series(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<String>,
) -> Result<impl IntoResponse> {
    if user.role != "admin" {
        return Err(TingError::PermissionDenied(
            "Only admin can delete series".to_string(),
        ));
    }

    if state.series_repo.find_by_id(&id).await?.is_none() {
        return Err(TingError::NotFound(format!(
            "Series with id {} not found",
            id
        )));
    }

    // Get books before deletion to update metadata
    let books = state.series_repo.find_books_by_series(&id).await?;

    state.series_repo.delete(&id).await?;

    // Update metadata.json for affected books
    for (book, _) in books {
        if let Err(e) = update_book_metadata_series(&state, &book.id).await {
            tracing::warn!("更新书籍 {} 的 metadata.json 失败: {}", book.id, e);
        }
    }

    Ok(StatusCode::NO_CONTENT)
}

/// Helper function to update metadata.json for a book
async fn update_book_metadata_series(state: &AppState, book_id: &str) -> Result<()> {
    use crate::db::repository::Repository;

    if let Some(book) = state.book_repo.find_by_id(book_id).await? {
        let path = std::path::Path::new(&book.path);

        // Read existing metadata
        if let Ok(Some(mut metadata)) = crate::core::metadata_writer::read_metadata_json(path) {
            // Fetch all series for this book
            let series_list = state.series_repo.find_series_by_book(book_id).await?;
            let mut series_titles = Vec::new();

            for series in series_list {
                // Find order of this book in this series
                let formatted_title =
                    if let Ok(books) = state.series_repo.find_books_by_series(&series.id).await {
                        if let Some((_, order)) = books.iter().find(|(b, _)| b.id == book_id) {
                            format!("{} #{}", series.title, order)
                        } else {
                            series.title.clone()
                        }
                    } else {
                        series.title.clone()
                    };

                if !series_titles.contains(&formatted_title) {
                    series_titles.push(formatted_title);
                }
            }

            // Update series
            metadata.series = series_titles;

            // Write back
            if let Err(e) = crate::core::metadata_writer::write_metadata_json(path, &metadata) {
                tracing::warn!("写入 metadata.json 失败: {}", e);
            }
        }
    }
    Ok(())
}
