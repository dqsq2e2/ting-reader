pub mod scrape;

pub use scrape::{apply_scrape_result, scrape_book_diff};

use super::AppState;
use crate::api::models::{
    BatchUpdateChaptersRequest, BookResponse, ChapterResponse, ChaptersPageResponse, ChaptersQuery,
    CreateBookRequest, MergeBooksRequest, MoveChaptersRequest, SearchQuery, SearchResponse,
    StatsResponse, UpdateBookCorrectionRequest, UpdateBookRequest, UpdateChapterRequest,
};
use crate::core::error::{Result, TingError};
use crate::core::nfo_manager::BookMetadata;
use crate::core::task_queue::{Priority, Task, TaskPayload};
use crate::db::models::Book;
use crate::db::repository::{ChapterRepository, Repository};
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use uuid::Uuid;

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

    let books = state
        .book_repo
        .find_with_filters(&user.id, is_admin, search, tag, library_id)
        .await?;

    let book_responses: Vec<BookResponse> = books.into_iter().map(BookResponse::from).collect();

    Ok(Json(book_responses))
}

/// Handler for GET /api/v1/books/:id - Get book by ID
pub async fn get_book(
    State(state): State<AppState>,
    Path(id): Path<String>,
    user: crate::auth::middleware::AuthUser,
) -> Result<impl IntoResponse> {
    let book = state
        .book_repo
        .find_by_id(&id)
        .await?
        .ok_or_else(|| TingError::NotFound(format!("Book with id {} not found", id)))?;

    let library = state
        .library_repo
        .find_by_id(&book.library_id)
        .await?
        .ok_or_else(|| {
            TingError::NotFound(format!("Library with id {} not found", book.library_id))
        })?;

    let is_fav = state.favorite_repo.is_favorited(&user.id, &id).await?;

    let mut response = BookResponse::from(book.clone());
    response.library_type = Some(library.library_type);
    response.is_favorite = is_fav;

    Ok((StatusCode::OK, Json(response)))
}

/// Handler for POST /api/v1/books - Create a new book
pub async fn create_book(
    State(state): State<AppState>,
    user: crate::auth::middleware::AuthUser,
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
                    std::path::Path::new(&req.path)
                        .join(url)
                        .to_string_lossy()
                        .to_string()
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
        genre: None,
        year: None,
        created_at,
        manual_corrected: 0,
        match_pattern: None,
        chapter_regex: None,
    };

    state.book_repo.create(&book).await?;

    let library = state
        .library_repo
        .find_by_id(&book.library_id)
        .await
        .ok()
        .flatten();
    let book_title = book.title.clone().unwrap_or_else(|| "Unknown".to_string());
    let library_name = library.as_ref().map(|item| item.name.clone());

    tracing::info!(
        target: "audit::library",
        message_key = "book.created",
        message_params = %serde_json::json!({
            "actor": user.username.as_str(),
            "book_id": book.id.as_str(),
            "book_title": book_title.as_str(),
            "library_id": book.library_id.as_str(),
            "library_name": library_name.as_deref().unwrap_or(""),
        }),
        actor_id = %user.id,
        actor = %user.username,
        book_id = %book.id,
        book_title = %book_title,
        library_id = %book.library_id,
        library_name = %library_name.as_deref().unwrap_or(""),
        "Book created"
    );

    crate::core::notifications::dispatch_application_event(
        state.notification_repo.clone(),
        state.plugin_manager.clone(),
        crate::core::notifications::NotificationEventPayload::new(
            "book.created",
            "作品入库",
            format!("作品 {} 已入库", book_title),
            serde_json::json!({
                "actor_id": user.id,
                "actor": user.username,
                "book_id": book.id,
                "book_title": book_title,
                "author": book.author,
                "narrator": book.narrator,
                "library_id": book.library_id,
                "library_name": library_name,
            }),
        ),
    );

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
                }
                Ok(None) => {
                    // Try WebDAV if local/http failed and it's a webdav library
                    if let Ok(Some(library)) = state
                        .library_repo
                        .find_by_id(&existing_book.library_id)
                        .await
                    {
                        if library.library_type == "webdav" {
                            if let Ok((mut reader, _)) = state
                                .storage_service
                                .get_webdav_reader(
                                    &library,
                                    url,
                                    None,
                                    state.encryption_key.as_ref(),
                                )
                                .await
                            {
                                let mut buffer = Vec::new();
                                if tokio::io::AsyncReadExt::read_to_end(&mut reader, &mut buffer)
                                    .await
                                    .is_ok()
                                {
                                    if let Ok(Some(color)) =
                                        crate::core::color::calculate_theme_color_from_bytes(
                                            &buffer,
                                        )
                                        .await
                                    {
                                        theme_color = Some(color);
                                    }
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!(
                        error = %e,
                        message_key = "book.theme_color.calculate_failed",
                        message_params = %serde_json::json!({ "error": e.to_string() }),
                        "Book theme color calculation failed"
                    );
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
                        }
                        Ok(None) => {
                            // Try WebDAV fallback for existing cover
                            if let Ok(Some(library)) = state
                                .library_repo
                                .find_by_id(&existing_book.library_id)
                                .await
                            {
                                if library.library_type == "webdav" {
                                    if let Ok((mut reader, _)) = state
                                        .storage_service
                                        .get_webdav_reader(
                                            &library,
                                            url,
                                            None,
                                            state.encryption_key.as_ref(),
                                        )
                                        .await
                                    {
                                        let mut buffer = Vec::new();
                                        if tokio::io::AsyncReadExt::read_to_end(
                                            &mut reader,
                                            &mut buffer,
                                        )
                                        .await
                                        .is_ok()
                                        {
                                            if let Ok(Some(color)) = crate::core::color::calculate_theme_color_from_bytes(&buffer).await {
                                                 theme_color = Some(color);
                                             }
                                        }
                                    }
                                }
                            }
                        }
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
        genre: req.genre.or(existing_book.genre),
        year: req.year.or(existing_book.year),
        created_at: existing_book.created_at,
        manual_corrected: existing_book.manual_corrected,
        match_pattern: existing_book.match_pattern,
        chapter_regex: req.chapter_regex.or(existing_book.chapter_regex),
    };

    state.book_repo.update(&updated_book).await?;

    // Check NFO writing
    if let Ok(Some(library)) = state
        .library_repo
        .find_by_id(&updated_book.library_id)
        .await
    {
        let config: crate::db::models::ScraperConfig = library
            .scraper_config
            .as_ref()
            .and_then(|json| serde_json::from_str(json).ok())
            .unwrap_or_default();

        // Handle NFO writing (Local & WebDAV)
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
                metadata.tags.items = tags_str
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect();
            }
            metadata.touch(); // Update timestamp

            // Determine path
            let target_dir = if library.library_type == "webdav" {
                // WebDAV uses hash-based temp dir
                let mut hasher = sha2::Sha256::new();
                use sha2::Digest;
                hasher.update(updated_book.path.as_bytes()); // updated_book.path is the WebDAV URL
                let book_hash = format!("{:x}", hasher.finalize());
                let temp_book_dir = std::env::current_dir()
                    .unwrap_or_else(|_| std::path::PathBuf::from("."))
                    .join("temp")
                    .join(&book_hash);
                if !temp_book_dir.exists() {
                    std::fs::create_dir_all(&temp_book_dir).ok();
                }
                temp_book_dir
            } else {
                std::path::PathBuf::from(&updated_book.path)
            };

            if let Err(e) = state
                .nfo_manager
                .write_book_nfo_to_dir(&target_dir, &metadata)
            {
                tracing::warn!(
                    book_title = %updated_book.title.as_deref().unwrap_or("?"),
                    error = %e,
                    message_key = "metadata.nfo.write_failed",
                    message_params = %serde_json::json!({
                        "book_title": updated_book.title.as_deref().unwrap_or("?"),
                        "error": e.to_string(),
                    }),
                    "Failed to write NFO"
                );
            }
        }

        // Handle metadata.json writing
        if config.metadata_writing_enabled {
            // Read existing metadata.json to preserve extended fields
            let target_dir = if library.library_type == "webdav" {
                let mut hasher = sha2::Sha256::new();
                use sha2::Digest;
                hasher.update(updated_book.path.as_bytes());
                let book_hash = format!("{:x}", hasher.finalize());
                let temp_book_dir = std::env::current_dir()
                    .unwrap_or_else(|_| std::path::PathBuf::from("."))
                    .join("temp")
                    .join(&book_hash);
                if !temp_book_dir.exists() {
                    std::fs::create_dir_all(&temp_book_dir).ok();
                }
                temp_book_dir
            } else {
                std::path::PathBuf::from(&updated_book.path)
            };

            let mut metadata_json = crate::core::metadata_writer::read_metadata_json(&target_dir)
                .unwrap_or(None)
                .unwrap_or_default();

            // Update fields from book record
            metadata_json.title = updated_book.title.clone();
            metadata_json.authors = updated_book
                .author
                .clone()
                .map(|s| vec![s])
                .unwrap_or_default();
            metadata_json.narrators = updated_book
                .narrator
                .clone()
                .map(|s| vec![s])
                .unwrap_or_default();
            metadata_json.description = updated_book.description.clone();
            metadata_json.genres = updated_book
                .genre
                .clone()
                .map(|s| s.split(',').map(|t| t.trim().to_string()).collect())
                .unwrap_or_default();
            metadata_json.tags = updated_book
                .tags
                .clone()
                .map(|s| s.split(',').map(|t| t.trim().to_string()).collect())
                .unwrap_or_default();
            metadata_json.published_year = updated_book.year.map(|y| y.to_string());

            // Sync chapters from DB
            let chapter_repo = ChapterRepository::new(state.book_repo.db().clone());
            if let Ok(chapters) = chapter_repo.find_by_book(&updated_book.id).await {
                let mut sorted_chapters = chapters;
                sorted_chapters.sort_by(|a, b| {
                    a.chapter_index
                        .unwrap_or(0)
                        .cmp(&b.chapter_index.unwrap_or(0))
                        .then_with(|| {
                            natord::compare(
                                a.title.as_deref().unwrap_or(""),
                                b.title.as_deref().unwrap_or(""),
                            )
                        })
                });

                let mut abs_chapters = Vec::new();
                let mut current_time = 0.0;
                for (idx, ch) in sorted_chapters.iter().enumerate() {
                    let duration = ch.duration.unwrap_or(0) as f64;
                    abs_chapters.push(crate::core::metadata_writer::AudiobookshelfChapter {
                        id: idx as u32,
                        start: current_time,
                        end: current_time + duration,
                        title: ch.title.clone().unwrap_or_default(),
                    });
                    current_time += duration;
                }
                metadata_json.chapters = abs_chapters;
            }

            // Sync series from DB
            let series_list = state
                .series_repo
                .find_series_by_book(&updated_book.id)
                .await
                .unwrap_or_default();
            let mut series_titles = Vec::new();
            for series in series_list {
                let formatted_title = if let Ok(books) =
                    state.series_repo.find_books_by_series(&series.id).await
                {
                    if let Some((_, order)) = books.iter().find(|(b, _)| b.id == updated_book.id) {
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
            metadata_json.series = series_titles;

            // Subtitle is now in metadata.json but not in Book struct, so we preserve what was read.
            // If request had extended fields (not supported in UpdateBookRequest yet), we would update them here.

            if let Err(e) =
                crate::core::metadata_writer::write_metadata_json(&target_dir, &metadata_json)
            {
                tracing::error!(
                    target: "audit::metadata",
                    book_title = %updated_book.title.as_deref().unwrap_or("?"),
                    error = %e,
                    message_key = "metadata.json.write_failed",
                    message_params = %serde_json::json!({
                        "book_title": updated_book.title.as_deref().unwrap_or("?"),
                        "error": e.to_string(),
                    }),
                    "Failed to write metadata.json"
                );
            }
        }
    }

    Ok(Json(BookResponse::from(updated_book)))
}

/// Handler for DELETE /api/v1/books/:id - Delete a book
pub async fn delete_book(
    State(state): State<AppState>,
    Path(id): Path<String>,
    user: crate::auth::middleware::AuthUser,
) -> Result<impl IntoResponse> {
    let book = state
        .book_repo
        .find_by_id(&id)
        .await?
        .ok_or_else(|| TingError::NotFound(format!("Book with id {} not found", id)))?;

    // Cleanup cover if cached (WebDAV temp/cache covers)
    if let Some(cover_url) = &book.cover_url {
        let path_str = cover_url.replace('\\', "/");
        if path_str.contains("/temp/covers/") || path_str.contains("/storage/cache/covers/") {
            let path = std::path::Path::new(&path_str);
            if path.exists() {
                if let Err(e) = std::fs::remove_file(path) {
                    tracing::warn!(
                        path = %cover_url,
                        error = %e,
                        message_key = "library.cover_cache.delete_failed",
                        message_params = %serde_json::json!({
                            "path": cover_url,
                            "error": e.to_string(),
                        }),
                        "Failed to delete cover cache"
                    );
                } else {
                    tracing::info!("Deleted orphan cover cache: {}", cover_url);
                }
            }
        }
    }

    state.book_repo.delete(&id).await?;

    let library = state
        .library_repo
        .find_by_id(&book.library_id)
        .await
        .ok()
        .flatten();
    let book_title = book.title.clone().unwrap_or_else(|| "Unknown".to_string());
    let library_name = library.as_ref().map(|item| item.name.clone());

    tracing::info!(
        target: "audit::library",
        message_key = "book.deleted",
        message_params = %serde_json::json!({
            "actor": user.username.as_str(),
            "book_id": book.id.as_str(),
            "book_title": book_title.as_str(),
            "library_id": book.library_id.as_str(),
            "library_name": library_name.as_deref().unwrap_or(""),
        }),
        actor_id = %user.id,
        actor = %user.username,
        book_id = %book.id,
        book_title = %book_title,
        library_id = %book.library_id,
        library_name = %library_name.as_deref().unwrap_or(""),
        "Book deleted"
    );

    crate::core::notifications::dispatch_application_event(
        state.notification_repo.clone(),
        state.plugin_manager.clone(),
        crate::core::notifications::NotificationEventPayload::new(
            "book.deleted",
            "删除作品",
            format!("作品 {} 已删除", book_title),
            serde_json::json!({
                "actor_id": user.id,
                "actor": user.username,
                "book_id": book.id,
                "book_title": book_title,
                "author": book.author,
                "narrator": book.narrator,
                "library_id": book.library_id,
                "library_name": library_name,
            }),
        ),
    );

    Ok(StatusCode::NO_CONTENT)
}

/// Handler for GET /api/v1/search - Search for books
pub async fn search_books(
    State(state): State<AppState>,
    Query(query): Query<SearchQuery>,
) -> Result<impl IntoResponse> {
    let result = state
        .scraper_service
        .search(
            &query.q,
            None,
            None,
            query.source.as_deref(),
            query.page,
            query.page_size,
        )
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
    Query(query): Query<ChaptersQuery>,
    user: crate::auth::middleware::AuthUser,
) -> Result<impl IntoResponse> {
    if state.book_repo.find_by_id(&book_id).await?.is_none() {
        return Err(TingError::NotFound(format!(
            "Book with id {} not found",
            book_id
        )));
    }

    let chapter_repo = ChapterRepository::new(state.book_repo.db().clone());

    let wants_paged = query.offset.is_some()
        || query.limit.is_some()
        || query.chapter_type.is_some()
        || query.order.is_some()
        || query.target_chapter_id.is_some();

    if wants_paged {
        let limit = query.limit.unwrap_or(100).clamp(1, 500);
        let requested_type = query.chapter_type.as_deref().unwrap_or("main");
        let mut chapter_type = match requested_type {
            "extra" => "extra",
            "all" => "all",
            _ => "main",
        };
        let mut is_extra = match chapter_type {
            "extra" => Some(1),
            "all" => None,
            _ => Some(0),
        };
        let order = if query.order.as_deref() == Some("desc") {
            "desc"
        } else {
            "asc"
        };
        let descending = order == "desc";

        let mut offset = query.offset.unwrap_or(0);
        if let Some(target_chapter_id) = query.target_chapter_id.as_deref() {
            let lookup_is_extra = if query.chapter_type.is_some() && requested_type != "all" {
                is_extra
            } else {
                None
            };
            if let Some((target_offset, resolved_is_extra)) = chapter_repo
                .page_offset_for_chapter(
                    &book_id,
                    target_chapter_id,
                    lookup_is_extra,
                    limit,
                    descending,
                )
                .await?
            {
                offset = target_offset;
                if query.chapter_type.as_deref().unwrap_or("main") != "all" {
                    is_extra = Some(resolved_is_extra);
                    chapter_type = if resolved_is_extra == 0 {
                        "main"
                    } else {
                        "extra"
                    };
                }
            }
        }

        let counts = chapter_repo.count_by_book(&book_id).await?;
        let filtered_total = match is_extra {
            Some(0) => counts.main,
            Some(_) => counts.extra,
            None => counts.total,
        };
        if offset >= filtered_total && filtered_total > 0 {
            offset = ((filtered_total - 1) / limit) * limit;
        }

        let chapters_with_progress = chapter_repo
            .find_by_book_with_progress_page(
                &book_id, &user.id, is_extra, offset, limit, descending,
            )
            .await?;

        let chapter_responses: Vec<ChapterResponse> = chapters_with_progress
            .into_iter()
            .map(|(chapter, pos, updated)| {
                let mut response = ChapterResponse::from(chapter);
                response.progress_position = pos;
                response.progress_updated_at = updated;
                response
            })
            .collect();

        return Ok(Json(ChaptersPageResponse {
            chapters: chapter_responses,
            total: counts.total,
            main_total: counts.main,
            extra_total: counts.extra,
            offset,
            limit,
            chapter_type: chapter_type.to_string(),
            order: order.to_string(),
        })
        .into_response());
    }

    let chapters_with_progress = chapter_repo
        .find_by_book_with_progress(&book_id, &user.id)
        .await?;

    let chapter_responses: Vec<ChapterResponse> = chapters_with_progress
        .into_iter()
        .map(|(chapter, pos, updated)| {
            let mut response = ChapterResponse::from(chapter);
            response.progress_position = pos;
            response.progress_updated_at = updated;
            response
        })
        .collect();

    Ok(Json(chapter_responses).into_response())
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

    // Regenerate metadata.json if enabled
    let book = state
        .book_repo
        .find_by_id(&updated_chapter.book_id)
        .await?
        .ok_or_else(|| {
            TingError::NotFound(format!(
                "Book with id {} not found",
                updated_chapter.book_id
            ))
        })?;

    if let Ok(Some(library)) = state.library_repo.find_by_id(&book.library_id).await {
        let config: crate::db::models::ScraperConfig = library
            .scraper_config
            .as_ref()
            .and_then(|json| serde_json::from_str(json).ok())
            .unwrap_or_default();

        if config.metadata_writing_enabled {
            // Determine path
            let target_dir = if library.library_type == "webdav" {
                let mut hasher = sha2::Sha256::new();
                use sha2::Digest;
                hasher.update(book.path.as_bytes());
                let book_hash = format!("{:x}", hasher.finalize());
                let temp_book_dir = std::env::current_dir()
                    .unwrap_or_else(|_| std::path::PathBuf::from("."))
                    .join("temp")
                    .join(&book_hash);
                if !temp_book_dir.exists() {
                    std::fs::create_dir_all(&temp_book_dir).ok();
                }
                temp_book_dir
            } else {
                std::path::PathBuf::from(&book.path)
            };

            let mut metadata_json = crate::core::metadata_writer::read_metadata_json(&target_dir)
                .unwrap_or(None)
                .unwrap_or_default();

            // Sync chapters from DB
            if let Ok(chapters) = chapter_repo.find_by_book(&book.id).await {
                let mut sorted_chapters = chapters;
                sorted_chapters.sort_by(|a, b| {
                    a.chapter_index
                        .unwrap_or(0)
                        .cmp(&b.chapter_index.unwrap_or(0))
                        .then_with(|| {
                            natord::compare(
                                a.title.as_deref().unwrap_or(""),
                                b.title.as_deref().unwrap_or(""),
                            )
                        })
                });

                let mut abs_chapters = Vec::new();
                let mut current_time = 0.0;
                for (idx, ch) in sorted_chapters.iter().enumerate() {
                    let duration = ch.duration.unwrap_or(0) as f64;
                    abs_chapters.push(crate::core::metadata_writer::AudiobookshelfChapter {
                        id: idx as u32,
                        start: current_time,
                        end: current_time + duration,
                        title: ch.title.clone().unwrap_or_default(),
                    });
                    current_time += duration;
                }
                metadata_json.chapters = abs_chapters;
            }

            // Sync series from DB
            let series_list = state
                .series_repo
                .find_series_by_book(&book.id)
                .await
                .unwrap_or_default();
            let mut series_titles = Vec::new();
            for series in series_list {
                let formatted_title =
                    if let Ok(books) = state.series_repo.find_books_by_series(&series.id).await {
                        if let Some((_, order)) = books.iter().find(|(b, _)| b.id == book.id) {
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
            metadata_json.series = series_titles;

            if let Err(e) =
                crate::core::metadata_writer::write_metadata_json(&target_dir, &metadata_json)
            {
                tracing::error!(
                    target: "audit::metadata",
                    chapter_title = %updated_chapter.title.as_deref().unwrap_or("?"),
                    error = %e,
                    message_key = "metadata.json.write_failed",
                    message_params = %serde_json::json!({
                        "chapter_title": updated_chapter.title.as_deref().unwrap_or("?"),
                        "error": e.to_string(),
                    }),
                    "Failed to write metadata.json"
                );
            }
        }
    }

    Ok(Json(ChapterResponse::from(updated_chapter)))
}

/// Handler for GET /api/v1/tags - Get all unique tags
pub async fn get_tags(
    State(state): State<AppState>,
    user: crate::auth::middleware::AuthUser,
) -> Result<impl IntoResponse> {
    let is_admin = user.role == "admin";

    // Use find_with_filters to respect user permissions
    let books = state
        .book_repo
        .find_with_filters(
            &user.id, is_admin, None, // search
            None, // tag
            None, // library_id
        )
        .await?;

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
    let last_scan_time = libraries
        .iter()
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

/// Handler for POST /api/v1/books/merge - Merge two books
pub async fn merge_books(
    State(state): State<AppState>,
    user: crate::auth::middleware::AuthUser,
    Json(req): Json<MergeBooksRequest>,
) -> Result<impl IntoResponse> {
    if user.role != "admin" {
        return Err(TingError::PermissionDenied(
            "Admin access required".to_string(),
        ));
    }

    let result = state
        .merge_service
        .merge_books(&req.source_book_id, &req.target_book_id)
        .await?;

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
        return Err(TingError::PermissionDenied(
            "Admin access required".to_string(),
        ));
    }

    if state.book_repo.find_by_id(&id).await?.is_none() {
        return Err(TingError::NotFound(format!(
            "Book with id {} not found",
            id
        )));
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

    // Regenerate metadata.json if enabled
    let book = state
        .book_repo
        .find_by_id(&id)
        .await?
        .ok_or_else(|| TingError::NotFound(format!("Book with id {} not found", id)))?;

    if let Ok(Some(library)) = state.library_repo.find_by_id(&book.library_id).await {
        let config: crate::db::models::ScraperConfig = library
            .scraper_config
            .as_ref()
            .and_then(|json| serde_json::from_str(json).ok())
            .unwrap_or_default();

        if config.metadata_writing_enabled {
            // Determine path
            let target_dir = if library.library_type == "webdav" {
                let mut hasher = sha2::Sha256::new();
                use sha2::Digest;
                hasher.update(book.path.as_bytes());
                let book_hash = format!("{:x}", hasher.finalize());
                let temp_book_dir = std::env::current_dir()
                    .unwrap_or_else(|_| std::path::PathBuf::from("."))
                    .join("temp")
                    .join(&book_hash);
                if !temp_book_dir.exists() {
                    std::fs::create_dir_all(&temp_book_dir).ok();
                }
                temp_book_dir
            } else {
                std::path::PathBuf::from(&book.path)
            };

            let mut metadata_json = crate::core::metadata_writer::read_metadata_json(&target_dir)
                .unwrap_or(None)
                .unwrap_or_default();

            // Sync chapters from DB
            if let Ok(chapters) = chapter_repo.find_by_book(&book.id).await {
                let mut sorted_chapters = chapters;
                sorted_chapters.sort_by(|a, b| {
                    a.chapter_index
                        .unwrap_or(0)
                        .cmp(&b.chapter_index.unwrap_or(0))
                        .then_with(|| {
                            natord::compare(
                                a.title.as_deref().unwrap_or(""),
                                b.title.as_deref().unwrap_or(""),
                            )
                        })
                });

                let mut abs_chapters = Vec::new();
                let mut current_time = 0.0;
                for (idx, ch) in sorted_chapters.iter().enumerate() {
                    let duration = ch.duration.unwrap_or(0) as f64;
                    abs_chapters.push(crate::core::metadata_writer::AudiobookshelfChapter {
                        id: idx as u32,
                        start: current_time,
                        end: current_time + duration,
                        title: ch.title.clone().unwrap_or_default(),
                    });
                    current_time += duration;
                }
                metadata_json.chapters = abs_chapters;
            }

            // Sync series from DB
            let series_list = state
                .series_repo
                .find_series_by_book(&book.id)
                .await
                .unwrap_or_default();
            let mut series_titles = Vec::new();
            for series in series_list {
                if let Ok(books) = state.series_repo.find_books_by_series(&series.id).await {
                    if let Some((_, order)) = books.iter().find(|(b, _)| b.id == book.id) {
                        series_titles.push(format!("{} #{}", series.title, order));
                    } else {
                        series_titles.push(series.title);
                    }
                } else {
                    series_titles.push(series.title);
                }
            }
            metadata_json.series = series_titles;

            if let Err(e) =
                crate::core::metadata_writer::write_metadata_json(&target_dir, &metadata_json)
            {
                tracing::error!(
                    target: "audit::metadata",
                    book_title = %book.title.as_deref().unwrap_or("?"),
                    error = %e,
                    message_key = "metadata.json.write_failed",
                    message_params = %serde_json::json!({
                        "book_title": book.title.as_deref().unwrap_or("?"),
                        "error": e.to_string(),
                    }),
                    "Failed to write metadata.json"
                );
            }
        }
    }

    Ok(Json(serde_json::json!({
        "message": "Chapters updated successfully"
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
        return Err(TingError::PermissionDenied(
            "Admin access required".to_string(),
        ));
    }

    state
        .merge_service
        .update_manual_correction(&id, req.manual_corrected, req.match_pattern)
        .await?;

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
        return Err(TingError::PermissionDenied(
            "Admin access required".to_string(),
        ));
    }

    state
        .merge_service
        .move_chapters(&req.target_book_id, req.chapter_ids)
        .await?;

    Ok(Json(serde_json::json!({
        "message": "Chapters moved successfully"
    })))
}

/// Handler for POST /api/v1/books/:id/write-metadata - Write metadata to audio files
pub async fn write_book_metadata_to_files(
    State(state): State<AppState>,
    Path(id): Path<String>,
    user: crate::auth::middleware::AuthUser,
) -> Result<impl IntoResponse> {
    if user.role != "admin" {
        return Err(TingError::PermissionDenied(
            "Admin access required".to_string(),
        ));
    }

    let book = state
        .book_repo
        .find_by_id(&id)
        .await?
        .ok_or_else(|| TingError::NotFound(format!("Book with id {} not found", id)))?;

    // Create task
    let task = Task::new(
        format!("写入元数据: {}", book.title.unwrap_or_default()),
        Priority::Normal,
        TaskPayload::Custom {
            task_type: "write_metadata".to_string(),
            data: serde_json::json!({
                "book_id": id
            }),
        },
    );

    let task_id = state.task_queue.submit(task).await?;

    Ok(Json(serde_json::json!({
        "message": "Metadata write task submitted",
        "task_id": task_id
    })))
}
