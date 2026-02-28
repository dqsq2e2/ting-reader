use crate::api::models::{
    BookResponse, CreateBookRequest, UpdateBookRequest,
    SearchQuery, SearchResponse, ScrapeBookRequest,
    ChapterResponse, UpdateChapterRequest,
    StatsResponse,
    MergeSuggestionResponse, MergeBooksRequest, IgnoreMergeSuggestionRequest, UpdateBookCorrectionRequest,
    BatchUpdateChaptersRequest, ScrapeDiffRequest, ScrapeDiffResponse, ScrapeApplyRequest,
    MoveChaptersRequest,
};
use crate::core::error::{Result, TingError};
use crate::db::models::Book;
use crate::core::nfo_manager::BookMetadata;
use crate::db::repository::{Repository, ChapterRepository};
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use uuid::Uuid;
use super::AppState;

/// Handler for GET /api/v1/books - List all books
pub async fn list_books(
    State(state): State<AppState>,
    Query(params): Query<std::collections::HashMap<String, String>>,
    user: crate::auth::middleware::AuthUser,
) -> Result<impl IntoResponse> {
    let search = params.get("search").cloned();
    let tag = params.get("tag").cloned();
    let library_id = params.get("library_id").cloned();
    let is_admin = user.role == "admin";

    let books = state.book_repo.find_with_filters(
        &user.id,
        is_admin,
        search,
        tag,
        library_id
    ).await?;
    
    let book_responses: Vec<BookResponse> = books.into_iter().map(BookResponse::from).collect();

    Ok(Json(book_responses))
}

/// Handler for GET /api/v1/books/:id - Get book by ID
pub async fn get_book(
    State(state): State<AppState>,
    Path(id): Path<String>,
    user: crate::auth::middleware::AuthUser,
) -> Result<impl IntoResponse> {
    let book = state.book_repo.find_by_id(&id).await?
        .ok_or_else(|| TingError::NotFound(format!("Book with id {} not found", id)))?;

    let library = state.library_repo.find_by_id(&book.library_id).await?
        .ok_or_else(|| TingError::NotFound(format!("Library with id {} not found", book.library_id)))?;

    let is_fav = state.favorite_repo.is_favorited(&user.id, &id).await?;

    let mut response = BookResponse::from(book.clone());
    response.library_type = Some(library.library_type);
    response.is_favorite = is_fav;
    
    Ok((StatusCode::OK, Json(response)))
}

/// Handler for POST /api/v1/books - Create a new book
pub async fn create_book(
    State(state): State<AppState>,
    Json(req): Json<CreateBookRequest>,
) -> Result<impl IntoResponse> {
    let book_id = Uuid::new_v4().to_string();
    let created_at = chrono::Utc::now().to_rfc3339();

    let mut theme_color = req.theme_color.clone();
    if theme_color.is_none() {
        if let Some(ref url) = req.cover_url {
            let cover_path = if url.starts_with("http://") || url.starts_with("https://") {
                url.clone()
            } else {
                let path = std::path::Path::new(url);
                if path.is_absolute() {
                    url.clone()
                } else {
                    std::path::Path::new(&req.path).join(url).to_string_lossy().to_string()
                }
            };
            
            if let Ok(Some(color)) = crate::core::color::calculate_theme_color(&cover_path).await {
                theme_color = Some(color);
            }
        }
    }

    let book = Book {
        id: book_id.clone(),
        library_id: req.library_id,
        title: req.title,
        author: req.author,
        narrator: req.narrator,
        cover_url: req.cover_url,
        theme_color,
        description: req.description,
        skip_intro: req.skip_intro,
        skip_outro: req.skip_outro,
        path: req.path,
        hash: req.hash,
        tags: req.tags,
        created_at,
        manual_corrected: 0,
        match_pattern: None,
        chapter_regex: None,
    };

    state.book_repo.create(&book).await?;

    Ok((StatusCode::CREATED, Json(BookResponse::from(book))))
}

/// Handler for PUT /api/v1/books/:id - Update a book
pub async fn update_book(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<UpdateBookRequest>,
) -> Result<impl IntoResponse> {
    let existing_book = state
        .book_repo
        .find_by_id(&id)
        .await?
        .ok_or_else(|| TingError::NotFound(format!("Book with id {} not found", id)))?;

    let mut theme_color = req.theme_color.clone();
    let book_path_str = req.path.clone().unwrap_or(existing_book.path.clone());
    let book_path = std::path::Path::new(&book_path_str);
    
    let cover_changed = if let Some(ref new_url) = req.cover_url {
        existing_book.cover_url.as_ref() != Some(new_url)
    } else {
        false
    };
    
    if theme_color.is_none() || cover_changed {
        if let Some(ref url) = req.cover_url {
             // If cover URL is provided, always recalculate theme color if not provided
             let cover_path = if url.starts_with("http://") || url.starts_with("https://") {
                 url.clone()
             } else {
                 let path = std::path::Path::new(url);
                 if path.is_absolute() {
                     url.clone()
                 } else {
                     book_path.join(url).to_string_lossy().to_string()
                 }
             };

             match crate::core::color::calculate_theme_color(&cover_path).await {
                 Ok(Some(color)) => {
                     theme_color = Some(color);
                 },
                 Ok(None) => {
                     // Try WebDAV if local/http failed and it's a webdav library
                     if let Ok(Some(library)) = state.library_repo.find_by_id(&existing_book.library_id).await {
                         if library.library_type == "webdav" {
                             if let Ok((mut reader, _)) = state.storage_service.get_webdav_reader(
                                 &library, 
                                 url, 
                                 None, 
                                 state.encryption_key.as_ref()
                             ).await {
                                 let mut buffer = Vec::new();
                                 if tokio::io::AsyncReadExt::read_to_end(&mut reader, &mut buffer).await.is_ok() {
                                     if let Ok(Some(color)) = crate::core::color::calculate_theme_color_from_bytes(&buffer).await {
                                         theme_color = Some(color);
                                     }
                                 }
                             }
                         }
                     }
                 },
                 Err(e) => {
                     tracing::warn!("Failed to calculate theme color: {}", e);
                 }
             }
        } else {
             // If cover URL is NOT provided in request, keep existing theme color
             // UNLESS existing cover exists and theme color is missing
             theme_color = existing_book.theme_color.clone();
             if theme_color.is_none() {
                 if let Some(ref url) = existing_book.cover_url {
                     let cover_path = if url.starts_with("http://") || url.starts_with("https://") {
                         url.clone()
                     } else {
                         let path = std::path::Path::new(url);
                         if path.is_absolute() {
                             url.clone()
                         } else {
                             book_path.join(url).to_string_lossy().to_string()
                         }
                     };

                     match crate::core::color::calculate_theme_color(&cover_path).await {
                         Ok(Some(color)) => {
                             theme_color = Some(color);
                         },
                         Ok(None) => {
                             // Try WebDAV fallback for existing cover
                             if let Ok(Some(library)) = state.library_repo.find_by_id(&existing_book.library_id).await {
                                 if library.library_type == "webdav" {
                                     if let Ok((mut reader, _)) = state.storage_service.get_webdav_reader(
                                         &library, 
                                         url, 
                                         None, 
                                         state.encryption_key.as_ref()
                                     ).await {
                                         let mut buffer = Vec::new();
                                         if tokio::io::AsyncReadExt::read_to_end(&mut reader, &mut buffer).await.is_ok() {
                                             if let Ok(Some(color)) = crate::core::color::calculate_theme_color_from_bytes(&buffer).await {
                                                 theme_color = Some(color);
                                             }
                                         }
                                     }
                                 }
                             }
                         },
                         Err(_) => {}
                     }
                 }
             }
        }
    }

    let updated_book = Book {
        id: existing_book.id,
        library_id: req.library_id.unwrap_or(existing_book.library_id),
        title: req.title.or(existing_book.title),
        author: req.author.or(existing_book.author),
        narrator: req.narrator.or(existing_book.narrator),
        cover_url: req.cover_url.or(existing_book.cover_url),
        theme_color,
        description: req.description.or(existing_book.description),
        skip_intro: req.skip_intro.unwrap_or(existing_book.skip_intro),
        skip_outro: req.skip_outro.unwrap_or(existing_book.skip_outro),
        path: req.path.unwrap_or(existing_book.path),
        hash: req.hash.unwrap_or(existing_book.hash),
        tags: req.tags.or(existing_book.tags),
        created_at: existing_book.created_at,
        manual_corrected: existing_book.manual_corrected,
        match_pattern: existing_book.match_pattern,
        chapter_regex: req.chapter_regex.or(existing_book.chapter_regex),
    };

    state.book_repo.update(&updated_book).await?;

    // Check NFO writing
    if let Ok(Some(library)) = state.library_repo.find_by_id(&updated_book.library_id).await {
        if library.library_type == "local" {
            let config: crate::db::models::ScraperConfig = library.scraper_config
                .as_ref()
                .and_then(|json| serde_json::from_str(json).ok())
                .unwrap_or_default();
                
            if config.nfo_writing_enabled {
                let mut metadata = BookMetadata::new(
                    updated_book.title.clone().unwrap_or_default(),
                    "ting-reader".to_string(),
                    updated_book.id.clone(),
                    0,
                );
                metadata.author = updated_book.author.clone();
                metadata.narrator = updated_book.narrator.clone();
                metadata.intro = updated_book.description.clone();
                metadata.cover_url = updated_book.cover_url.clone();
                if let Some(tags_str) = &updated_book.tags {
                     metadata.tags.items = tags_str.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
                }
                metadata.touch(); // Update timestamp
                
                let book_path = std::path::Path::new(&updated_book.path);
                if let Err(e) = state.nfo_manager.write_book_nfo_to_dir(book_path, &metadata) {
                    tracing::warn!("Failed to write NFO for book {}: {}", updated_book.title.as_deref().unwrap_or("?"), e);
                }
            }
        }
    }

    Ok(Json(BookResponse::from(updated_book)))
}

/// Handler for POST /api/v1/books/:id/scrape - Scrape and update book details
pub async fn scrape_book(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<ScrapeBookRequest>,
) -> Result<impl IntoResponse> {
    let existing_book = state
        .book_repo
        .find_by_id(&id)
        .await?
        .ok_or_else(|| TingError::NotFound(format!("Book with id {} not found", id)))?;

    let (source, external_id) = match (req.source, req.external_id) {
        (Some(s), Some(e)) => (s, e),
        _ => return Err(TingError::ValidationError("Source and External ID are required for now".to_string())),
    };

    let detail = state.scraper_service.get_detail(&source, &external_id).await?;

    let mut updated_book = existing_book.clone();
    
    updated_book.title = Some(detail.title);
    updated_book.author = Some(detail.author);

    if let Some(narrator) = detail.narrator {
        updated_book.narrator = Some(narrator);
    }
    
    if !detail.intro.is_empty() {
        updated_book.description = Some(detail.intro);
    }
    
    if !detail.tags.is_empty() {
        updated_book.tags = Some(detail.tags.join(","));
    }

    let old_cover = existing_book.cover_url.clone();
    let new_cover = detail.cover_url;
    
    if let Some(url) = new_cover {
        updated_book.cover_url = Some(url.clone());
    }

    let should_calculate_color = updated_book.cover_url != old_cover || updated_book.theme_color.is_none();
    
    if should_calculate_color {
        if let Some(ref url) = updated_book.cover_url {
             let cover_path = if url.starts_with("http://") || url.starts_with("https://") {
                 url.clone()
             } else {
                 let book_path = std::path::Path::new(&updated_book.path);
                 if std::path::Path::new(&url).is_absolute() {
                     url.clone()
                 } else {
                     book_path.join(&url).to_string_lossy().to_string()
                 }
             };

             match crate::core::color::calculate_theme_color(&cover_path).await {
                 Ok(Some(color)) => {
                     updated_book.theme_color = Some(color);
                 },
                 Ok(None) => {
                     if let Ok(Some(library)) = state.library_repo.find_by_id(&updated_book.library_id).await {
                         if library.library_type == "webdav" {
                             if let Ok((mut reader, _)) = state.storage_service.get_webdav_reader(
                                 &library, 
                                 &url, 
                                 None, 
                                 state.encryption_key.as_ref()
                             ).await {
                                 let mut buffer = Vec::new();
                                 if tokio::io::AsyncReadExt::read_to_end(&mut reader, &mut buffer).await.is_ok() {
                                     if let Ok(Some(color)) = crate::core::color::calculate_theme_color_from_bytes(&buffer).await {
                                         updated_book.theme_color = Some(color);
                                     }
                                 }
                             }
                         }
                     }
                 },
                 Err(_) => {}
             }
        }
    }

    state.book_repo.update(&updated_book).await?;

    // Check NFO writing
    if let Ok(Some(library)) = state.library_repo.find_by_id(&updated_book.library_id).await {
        if library.library_type == "local" {
            let config: crate::db::models::ScraperConfig = library.scraper_config
                .as_ref()
                .and_then(|json| serde_json::from_str(json).ok())
                .unwrap_or_default();
                
            if config.nfo_writing_enabled {
                let mut metadata = BookMetadata::new(
                    updated_book.title.clone().unwrap_or_default(),
                    "ting-reader".to_string(),
                    updated_book.id.clone(),
                    0,
                );
                metadata.author = updated_book.author.clone();
                metadata.narrator = updated_book.narrator.clone();
                metadata.intro = updated_book.description.clone();
                metadata.cover_url = updated_book.cover_url.clone();
                if let Some(tags_str) = &updated_book.tags {
                     metadata.tags.items = tags_str.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
                }
                metadata.touch();
                
                let book_path = std::path::Path::new(&updated_book.path);
                if let Err(e) = state.nfo_manager.write_book_nfo_to_dir(book_path, &metadata) {
                    tracing::warn!("Failed to write NFO for book {}: {}", updated_book.title.as_deref().unwrap_or("?"), e);
                }
            }
        }
    }

    Ok(Json(BookResponse::from(updated_book)))
}

/// Handler for DELETE /api/v1/books/:id - Delete a book
pub async fn delete_book(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse> {
    if state.book_repo.find_by_id(&id).await?.is_none() {
        return Err(TingError::NotFound(format!("Book with id {} not found", id)));
    }

    state.book_repo.delete(&id).await?;

    Ok(StatusCode::NO_CONTENT)
}

/// Handler for GET /api/v1/search - Search for books
pub async fn search_books(
    State(state): State<AppState>,
    Query(query): Query<SearchQuery>,
) -> Result<impl IntoResponse> {
    let result = state
        .scraper_service
        .search(&query.q, query.source.as_deref(), query.page, query.page_size)
        .await?;

    Ok(Json(SearchResponse {
        items: result.items,
        total: result.total,
        page: result.page,
        page_size: result.page_size,
    }))
}

/// Handler for GET /api/v1/books/:id/chapters - Get chapters for a book
pub async fn get_book_chapters(
    State(state): State<AppState>,
    Path(book_id): Path<String>,
    user: crate::auth::middleware::AuthUser,
) -> Result<impl IntoResponse> {
    if state.book_repo.find_by_id(&book_id).await?.is_none() {
        return Err(TingError::NotFound(format!("Book with id {} not found", book_id)));
    }

    let chapter_repo = ChapterRepository::new(state.book_repo.db().clone());
    let chapters_with_progress = chapter_repo.find_by_book_with_progress(&book_id, &user.id).await?;
    
    let chapter_responses: Vec<ChapterResponse> = chapters_with_progress
        .into_iter()
        .map(|(chapter, pos, updated)| {
            let mut response = ChapterResponse::from(chapter);
            response.progress_position = pos;
            response.progress_updated_at = updated;
            response
        })
        .collect();

    Ok(Json(chapter_responses))
}

/// Handler for PATCH /api/v1/chapters/:id - Update a chapter
pub async fn update_chapter(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<UpdateChapterRequest>,
) -> Result<impl IntoResponse> {
    let chapter_repo = ChapterRepository::new(state.book_repo.db().clone());

    let existing_chapter = chapter_repo
        .find_by_id(&id)
        .await?
        .ok_or_else(|| TingError::NotFound(format!("Chapter with id {} not found", id)))?;

    let updated_chapter = crate::db::models::Chapter {
        id: existing_chapter.id,
        book_id: existing_chapter.book_id,
        title: req.title.or(existing_chapter.title),
        path: req.path.unwrap_or(existing_chapter.path),
        duration: req.duration.or(existing_chapter.duration),
        chapter_index: req.chapter_index.or(existing_chapter.chapter_index),
        is_extra: req.is_extra.unwrap_or(existing_chapter.is_extra),
        hash: existing_chapter.hash,
        created_at: existing_chapter.created_at,
        manual_corrected: existing_chapter.manual_corrected,
    };

    chapter_repo.update(&updated_chapter).await?;

    Ok(Json(ChapterResponse::from(updated_chapter)))
}

/// Handler for GET /api/v1/tags - Get all unique tags
pub async fn get_tags(State(state): State<AppState>) -> Result<impl IntoResponse> {
    let books = state.book_repo.find_all().await?;

    let mut tags_set = std::collections::HashSet::new();
    for book in books {
        if let Some(tags_str) = book.tags {
            for tag in tags_str.split(',') {
                let trimmed = tag.trim();
                if !trimmed.is_empty() {
                    tags_set.insert(trimmed.to_string());
                }
            }
        }
    }

    let mut tags: Vec<String> = tags_set.into_iter().collect();
    tags.sort();

    Ok(Json(tags))
}

/// Handler for GET /api/v1/stats - Get system statistics
pub async fn get_stats(State(state): State<AppState>) -> Result<impl IntoResponse> {
    let (total_books, total_chapters, total_duration) = state.book_repo.get_stats().await?;

    let libraries = state.library_repo.find_all().await?;
    let last_scan_time = libraries.iter()
        .filter_map(|l| l.last_scanned_at.as_ref())
        .max()
        .cloned();

    let stats = StatsResponse {
        total_books,
        total_chapters,
        total_duration,
        last_scan_time,
    };

    Ok(Json(stats))
}

// ============================================================================
// Merge System Handlers
// ============================================================================

/// Handler for GET /api/v1/books/merge/suggestions - Get merge suggestions
pub async fn get_merge_suggestions(
    State(state): State<AppState>,
    user: crate::auth::middleware::AuthUser,
) -> Result<impl IntoResponse> {
    if user.role != "admin" {
        return Err(TingError::PermissionDenied("Admin access required".to_string()));
    }

    let suggestions = state.merge_service.find_suggestions(0.85).await?;
    
    let mut response = Vec::new();
    for suggestion in suggestions {
        let source_book = state.book_repo.find_by_id(&suggestion.book_a_id).await?;
        let target_book = state.book_repo.find_by_id(&suggestion.book_b_id).await?;
        
        if let (Some(source), Some(target)) = (source_book, target_book) {
            response.push(MergeSuggestionResponse {
                id: suggestion.id,
                source_book_id: source.id,
                target_book_id: target.id,
                source_title: source.title.unwrap_or_default(),
                target_title: target.title.unwrap_or_default(),
                source_author: source.author,
                target_author: target.author,
                score: suggestion.score,
                reason: suggestion.reason,
                status: suggestion.status,
                created_at: suggestion.created_at,
            });
        }
    }

    Ok(Json(response))
}

/// Handler for POST /api/v1/books/merge - Merge two books
pub async fn merge_books(
    State(state): State<AppState>,
    user: crate::auth::middleware::AuthUser,
    Json(req): Json<MergeBooksRequest>,
) -> Result<impl IntoResponse> {
    if user.role != "admin" {
        return Err(TingError::PermissionDenied("Admin access required".to_string()));
    }

    let result = state.merge_service.merge_books(&req.source_book_id, &req.target_book_id).await?;

    Ok(Json(serde_json::json!({
        "message": "Books merged successfully",
        "result": result
    })))
}

/// Handler for POST /api/v1/books/:id/chapters/batch - Batch update chapters
pub async fn batch_update_chapters(
    State(state): State<AppState>,
    Path(id): Path<String>,
    user: crate::auth::middleware::AuthUser,
    Json(req): Json<BatchUpdateChaptersRequest>,
) -> Result<impl IntoResponse> {
    if user.role != "admin" {
        return Err(TingError::PermissionDenied("Admin access required".to_string()));
    }

    if state.book_repo.find_by_id(&id).await?.is_none() {
        return Err(TingError::NotFound(format!("Book with id {} not found", id)));
    }

    let chapter_repo = ChapterRepository::new(state.book_repo.db().clone());

    for update in req.updates {
        if let Some(mut chapter) = chapter_repo.find_by_id(&update.id).await? {
            // Verify chapter belongs to book
            if chapter.book_id != id {
                continue;
            }

            if let Some(title) = update.title {
                chapter.title = Some(title);
            }
            if let Some(idx) = update.chapter_index {
                chapter.chapter_index = Some(idx);
            }
            if let Some(is_extra) = update.is_extra {
                chapter.is_extra = is_extra;
            }

            // Mark as manually corrected so scanner won't overwrite
            chapter.manual_corrected = 1;

            chapter_repo.update(&chapter).await?;
        }
    }

    Ok(Json(serde_json::json!({
        "message": "Chapters updated successfully"
    })))
}

use crate::db::models::ScraperConfig;

/// Handler for POST /api/v1/books/:id/scrape-diff - Get scrape diff
pub async fn scrape_book_diff(
    State(state): State<AppState>,
    Path(id): Path<String>,
    user: crate::auth::middleware::AuthUser,
    Json(req): Json<ScrapeDiffRequest>,
) -> Result<impl IntoResponse> {
    if user.role != "admin" {
        return Err(TingError::PermissionDenied("Admin access required".to_string()));
    }

    let existing_book = state.book_repo.find_by_id(&id).await?
        .ok_or_else(|| TingError::NotFound(format!("Book with id {} not found", id)))?;

    // 1. Get Library Config
    let library = state.library_repo.find_by_id(&existing_book.library_id).await?
        .ok_or_else(|| TingError::NotFound(format!("Library with id {} not found", existing_book.library_id)))?;
    
    let config: ScraperConfig = if let Some(json) = &library.scraper_config {
        serde_json::from_str(json).unwrap_or_default()
    } else {
        ScraperConfig::default()
    };

    // 2. Determine Primary Source (from Default List or First Enabled)
    let sources = state.scraper_service.get_sources().await;
    
    // Find first enabled source from config defaults, or just first enabled system-wide
    let mut primary_source_id = None;
    for source_id in &config.default_sources {
        if let Some(s) = sources.iter().find(|s| s.id == *source_id && s.enabled) {
            primary_source_id = Some(s.id.clone());
            break;
        }
    }
    
    if primary_source_id.is_none() {
        primary_source_id = sources.iter().find(|s| s.enabled).map(|s| s.id.clone());
    }
    
    let primary_source_id = match primary_source_id {
        Some(s) => s,
        None => return Err(TingError::PluginNotFound("No active scraper plugins found".to_string())),
    };

    // 3. Search and Get Detail from Primary Source
    // Use scraper service search to find match by title
    let search_results = state.scraper_service.search(&req.query, Some(&primary_source_id), 1, 1).await?;
    
    if search_results.items.is_empty() {
        return Err(TingError::NotFound("No matching book found in scrapers".to_string()));
    }
    
    let best_match = &search_results.items[0];
    // We use search result directly as it now contains full metadata
    let mut detail = crate::plugin::scraper::BookDetail {
        id: best_match.id.clone(),
        title: best_match.title.clone(),
        author: best_match.author.clone(),
        narrator: best_match.narrator.clone(),
        cover_url: best_match.cover_url.clone(),
        intro: best_match.intro.clone().unwrap_or_default(),
        tags: best_match.tags.clone(),
        chapter_count: best_match.chapter_count.unwrap_or(0),
        duration: best_match.duration,
    };
    
    // 4. Handle Specific Field Sources (Merge Strategy)
    // Helper to fetch and merge specific field
    // We assume the search query is the same (Book Title) for other sources too.
    
    // Check Author Source
    if let Some(srcs) = &config.author_sources {
        if !srcs.is_empty() && srcs[0] != primary_source_id {
             if let Some(s_id) = sources.iter().find(|s| s.id == srcs[0] && s.enabled).map(|s| s.id.clone()) {
                 if let Ok(res) = state.scraper_service.search(&req.query, Some(&s_id), 1, 1).await {
                     if !res.items.is_empty() {
                         let item = &res.items[0];
                         if !item.author.is_empty() { detail.author = item.author.clone(); }
                     }
                 }
             }
        }
    }

    // Check Narrator Source
    if let Some(srcs) = &config.narrator_sources {
        if !srcs.is_empty() && srcs[0] != primary_source_id {
             if let Some(s_id) = sources.iter().find(|s| s.id == srcs[0] && s.enabled).map(|s| s.id.clone()) {
                 if let Ok(res) = state.scraper_service.search(&req.query, Some(&s_id), 1, 1).await {
                     if !res.items.is_empty() {
                         let item = &res.items[0];
                         if item.narrator.is_some() { detail.narrator = item.narrator.clone(); }
                     }
                 }
             }
        }
    }

    // Check Cover Source
    if let Some(srcs) = &config.cover_sources {
        if !srcs.is_empty() && srcs[0] != primary_source_id {
             if let Some(s_id) = sources.iter().find(|s| s.id == srcs[0] && s.enabled).map(|s| s.id.clone()) {
                 if let Ok(res) = state.scraper_service.search(&req.query, Some(&s_id), 1, 1).await {
                     if !res.items.is_empty() {
                         let item = &res.items[0];
                         if item.cover_url.is_some() { detail.cover_url = item.cover_url.clone(); }
                     }
                 }
             }
        }
    }

    // Check Intro Source
    if let Some(srcs) = &config.intro_sources {
        if !srcs.is_empty() && srcs[0] != primary_source_id {
             if let Some(s_id) = sources.iter().find(|s| s.id == srcs[0] && s.enabled).map(|s| s.id.clone()) {
                 if let Ok(res) = state.scraper_service.search(&req.query, Some(&s_id), 1, 1).await {
                     if !res.items.is_empty() {
                         let item = &res.items[0];
                         if let Some(intro) = &item.intro {
                             if !intro.is_empty() { detail.intro = intro.clone(); }
                         }
                     }
                 }
             }
        }
    }

    // Check Tags Source
    if let Some(srcs) = &config.tags_sources {
        if !srcs.is_empty() && srcs[0] != primary_source_id {
             if let Some(s_id) = sources.iter().find(|s| s.id == srcs[0] && s.enabled).map(|s| s.id.clone()) {
                 if let Ok(res) = state.scraper_service.search(&req.query, Some(&s_id), 1, 1).await {
                     if !res.items.is_empty() {
                         let item = &res.items[0];
                         if !item.tags.is_empty() { detail.tags = item.tags.clone(); }
                     }
                 }
             }
        }
    }
    
    // Construct ScrapeMetadata for current book
    let current_meta = crate::api::models::books::ScrapeMetadata {
        title: existing_book.title.clone().unwrap_or_default(),
        author: existing_book.author.clone().unwrap_or_default(),
        narrator: existing_book.narrator.clone().unwrap_or_default(),
        description: existing_book.description.clone().unwrap_or_default(),
        cover_url: existing_book.cover_url.clone(),
        tags: existing_book.tags.map(|s| s.split(',').map(|t| t.trim().to_string()).filter(|t| !t.is_empty()).collect()),
    };
    
    // Construct ScrapeMetadata for scraped detail
    let clean_cover_url = detail.cover_url.clone();

    let scraped_meta = crate::api::models::books::ScrapeMetadata {
        title: detail.title.clone(),
        author: detail.author.clone(),
        narrator: detail.narrator.clone().unwrap_or_default(),
        description: detail.intro.clone(),
        cover_url: clean_cover_url,
        tags: if detail.tags.is_empty() { None } else { Some(detail.tags.clone()) },
    };
    
    // Chapter changes (Not implemented yet, returning empty list)
    // To implement this, we need ScraperService to return chapter list
    let chapter_changes = Vec::new();

    Ok(Json(ScrapeDiffResponse {
        current: current_meta,
        scraped: scraped_meta,
        chapter_changes,
    }))
}

/// Handler for POST /api/v1/books/:id/scrape-apply - Apply scrape result
pub async fn apply_scrape_result(
    State(state): State<AppState>,
    Path(id): Path<String>,
    user: crate::auth::middleware::AuthUser,
    Json(req): Json<ScrapeApplyRequest>,
) -> Result<impl IntoResponse> {
    if user.role != "admin" {
        return Err(TingError::PermissionDenied("Admin access required".to_string()));
    }

    let mut book = state.book_repo.find_by_id(&id).await?
        .ok_or_else(|| TingError::NotFound(format!("Book with id {} not found", id)))?;

    if req.apply_metadata {
        let detail = &req.metadata;
        
        if !detail.title.is_empty() { book.title = Some(detail.title.clone()); }
        if !detail.author.is_empty() { book.author = Some(detail.author.clone()); }
        if let Some(n) = &detail.narrator { book.narrator = Some(n.clone()); }
        if !detail.intro.is_empty() { book.description = Some(detail.intro.clone()); }
        if !detail.tags.is_empty() { book.tags = Some(detail.tags.join(",")); }
        if let Some(url) = &detail.cover_url { 
            book.cover_url = Some(url.clone());
            
            // Recalculate theme color for new cover
            match crate::core::color::calculate_theme_color(url).await {
                Ok(Some(color)) => {
                    tracing::info!("Updated theme color for book {}: {}", book.id, color);
                    book.theme_color = Some(color);
                },
                Ok(None) => {
                     // Try WebDAV if local/http failed and it's a webdav library
                     if let Ok(Some(library)) = state.library_repo.find_by_id(&book.library_id).await {
                         if library.library_type == "webdav" {
                             if let Ok((mut reader, _)) = state.storage_service.get_webdav_reader(
                                 &library, 
                                 url, 
                                 None, 
                                 state.encryption_key.as_ref()
                             ).await {
                                 let mut buffer = Vec::new();
                                 if tokio::io::AsyncReadExt::read_to_end(&mut reader, &mut buffer).await.is_ok() {
                                     if let Ok(Some(color)) = crate::core::color::calculate_theme_color_from_bytes(&buffer).await {
                                         book.theme_color = Some(color);
                                     }
                                 }
                             }
                         }
                     }
                },
                Err(e) => {
                    tracing::warn!("Failed to calculate theme color: {}", e);
                }
            }
        }
        
        // Set manual corrected
        book.manual_corrected = 1; // 1 for true
        book.match_pattern = Some(regex::escape(&detail.title)); // Set default match pattern to exact title
        
        state.book_repo.update(&book).await?;

        // Check NFO writing
        if let Ok(Some(library)) = state.library_repo.find_by_id(&book.library_id).await {
            if library.library_type == "local" {
                let config: crate::db::models::ScraperConfig = library.scraper_config
                    .as_ref()
                    .and_then(|json| serde_json::from_str(json).ok())
                    .unwrap_or_default();
                    
                if config.nfo_writing_enabled {
                    let mut metadata = BookMetadata::new(
                        book.title.clone().unwrap_or_default(),
                        "ting-reader".to_string(),
                        book.id.clone(),
                        0,
                    );
                    metadata.author = book.author.clone();
                    metadata.narrator = book.narrator.clone();
                    metadata.intro = book.description.clone();
                    metadata.cover_url = book.cover_url.clone();
                    if let Some(tags_str) = &book.tags {
                         metadata.tags.items = tags_str.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
                    }
                    metadata.touch();
                    
                    let book_path = std::path::Path::new(&book.path);
                    if let Err(e) = state.nfo_manager.write_book_nfo_to_dir(book_path, &metadata) {
                        tracing::warn!("Failed to write NFO for book {}: {}", book.title.as_deref().unwrap_or("?"), e);
                    }
                }
            }
        }
    }
    
    // Handle chapter updates if any (req.apply_chapters)
    // Since we don't have scraped chapters yet, we skip this for now.
    
    Ok(Json(BookResponse::from(book)))
}

/// Handler for POST /api/v1/books/merge/ignore - Ignore a merge suggestion
pub async fn ignore_merge_suggestion(
    State(state): State<AppState>,
    user: crate::auth::middleware::AuthUser,
    Json(req): Json<IgnoreMergeSuggestionRequest>,
) -> Result<impl IntoResponse> {
    if user.role != "admin" {
        return Err(TingError::PermissionDenied("Admin access required".to_string()));
    }

    state.merge_service.ignore_suggestion(&req.suggestion_id).await?;

    Ok(Json(serde_json::json!({
        "message": "Suggestion ignored successfully"
    })))
}

/// Handler for PUT /api/v1/books/:id/correction - Update book correction status
pub async fn update_book_correction(
    State(state): State<AppState>,
    Path(id): Path<String>,
    user: crate::auth::middleware::AuthUser,
    Json(req): Json<UpdateBookCorrectionRequest>,
) -> Result<impl IntoResponse> {
    if user.role != "admin" {
        return Err(TingError::PermissionDenied("Admin access required".to_string()));
    }

    state.merge_service.update_manual_correction(&id, req.manual_corrected, req.match_pattern).await?;

    Ok(Json(serde_json::json!({
        "message": "Book correction status updated successfully"
    })))
}

/// Handler for POST /api/v1/books/chapters/move - Move chapters to another book
pub async fn move_chapters(
    State(state): State<AppState>,
    user: crate::auth::middleware::AuthUser,
    Json(req): Json<MoveChaptersRequest>,
) -> Result<impl IntoResponse> {
    if user.role != "admin" {
        return Err(TingError::PermissionDenied("Admin access required".to_string()));
    }

    state.merge_service.move_chapters(&req.target_book_id, req.chapter_ids).await?;

    Ok(Json(serde_json::json!({
        "message": "Chapters moved successfully"
    })))
}
