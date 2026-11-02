//! Chapter listing, edits and batch operations.

use crate::api::models::{
    BatchUpdateChaptersRequest, ChapterResponse, ChaptersPageResponse, ChaptersQuery,
    MoveChaptersRequest, UpdateChapterRequest,
};
use crate::api::state::AppState;
use crate::core::app::error::LookupErrorKind;
use crate::core::app::error::{Result, TingError};
use crate::db::repository::{ChapterRepository, Repository};
use axum::{
    Json,
    extract::{Path, Query, State},
    response::IntoResponse,
};

/// Handler for GET /api/v1/books/:id/chapters - Get chapters for a book
pub async fn get_book_chapters(
    State(state): State<AppState>,
    Path(book_id): Path<String>,
    Query(query): Query<ChaptersQuery>,
    user: crate::auth::middleware::AuthUser,
) -> Result<impl IntoResponse> {
    let book = state.book_repo.find_by_id(&book_id).await?.ok_or_else(|| {
        TingError::not_found(
            LookupErrorKind::Book,
            "get_book_chapters::not_found",
            format!("Book with id {} not found", book_id),
        )
    })?;

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
        let mut source_total = None;
        if let Some(library) = state.library_repo.find_by_id(&book.library_id).await?
            && let Some(page) = state
                .scraper_service
                .load_source_chapter_page(&state.book_repo, &book, &library, offset, limit)
                .await?
        {
            source_total = page.total;
        }
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
        let total = source_total.unwrap_or(counts.total);
        let main_total = source_total.unwrap_or(counts.main);
        let filtered_total = match is_extra {
            Some(0) => main_total,
            Some(_) => counts.extra,
            None => total,
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
            total,
            main_total,
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

    let existing_chapter = chapter_repo.find_by_id(&id).await?.ok_or_else(|| {
        TingError::not_found(
            LookupErrorKind::Chapter,
            "update_chapter::not_found",
            format!("Chapter with id {} not found", id),
        )
    })?;

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
        start_time: existing_chapter.start_time,
        end_time: existing_chapter.end_time,
    };

    chapter_repo.update(&updated_chapter).await?;

    // Regenerate metadata.json if enabled
    let book = state
        .book_repo
        .find_by_id(&updated_chapter.book_id)
        .await?
        .ok_or_else(|| {
            TingError::not_found(
                LookupErrorKind::Chapter,
                "update_chapter::not_found",
                format!("Book with id {} not found", updated_chapter.book_id),
            )
        })?;

    if state
        .library_repo
        .find_by_id(&book.library_id)
        .await?
        .is_some_and(|library| library.library_type == "webdav")
    {
        state
            .merge_service
            .update_manual_correction(&book.id, true, book.title.as_deref().map(regex::escape))
            .await?;
    }
    state
        .metadata_service()
        .sync(
            &book,
            crate::core::books::metadata::MetadataChange::Chapters,
            &Default::default(),
        )
        .await?;

    Ok(Json(ChapterResponse::from(updated_chapter)))
}

/// Handler for POST /api/v1/books/:id/chapters/batch - Batch update chapters
pub async fn batch_update_chapters(
    State(state): State<AppState>,
    Path(id): Path<String>,
    user: crate::auth::middleware::AuthUser,
    Json(req): Json<BatchUpdateChaptersRequest>,
) -> Result<impl IntoResponse> {
    crate::api::require_admin(&user)?;

    if state.book_repo.find_by_id(&id).await?.is_none() {
        return Err(TingError::not_found(
            LookupErrorKind::Book,
            "batch_update_chapters::not_found",
            format!("Book with id {} not found", id),
        ));
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
    let book = state.book_repo.find_by_id(&id).await?.ok_or_else(|| {
        TingError::not_found(
            LookupErrorKind::Book,
            "batch_update_chapters::not_found",
            format!("Book with id {} not found", id),
        )
    })?;

    if state
        .library_repo
        .find_by_id(&book.library_id)
        .await?
        .is_some_and(|library| library.library_type == "webdav")
    {
        state
            .merge_service
            .update_manual_correction(&book.id, true, book.title.as_deref().map(regex::escape))
            .await?;
    }
    state
        .metadata_service()
        .sync(
            &book,
            crate::core::books::metadata::MetadataChange::Chapters,
            &Default::default(),
        )
        .await?;

    Ok(Json(serde_json::json!({
        "message": "Chapters updated successfully"
    })))
}

/// Handler for POST /api/v1/books/chapters/move - Move chapters to another book
pub async fn move_chapters(
    State(state): State<AppState>,
    user: crate::auth::middleware::AuthUser,
    Json(req): Json<MoveChaptersRequest>,
) -> Result<impl IntoResponse> {
    crate::api::require_admin(&user)?;

    state
        .merge_service
        .move_chapters(&req.target_book_id, req.chapter_ids)
        .await?;

    Ok(Json(serde_json::json!({
        "message": "Chapters moved successfully"
    })))
}
