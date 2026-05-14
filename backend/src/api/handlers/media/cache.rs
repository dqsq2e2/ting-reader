//! Chapter cache management handlers (admin-only)

use crate::api::models::{
    CacheOperationResponse, CacheInfoResponse, CacheListResponse, ClearCacheResponse,
};
use crate::api::require_admin;
use crate::core::error::{Result, TingError};
use crate::db::repository::Repository;
use axum::{
    extract::{Path, State},
    Json,
};
use crate::api::handlers::AppState;

/// POST /api/cache/:chapterId - Cache a chapter from remote to local
pub async fn cache_chapter(
    State(state): State<AppState>,
    user: crate::auth::middleware::AuthUser,
    Path(chapter_id): Path<String>,
) -> Result<impl axum::response::IntoResponse> {
    require_admin(&user)?;

    let chapter = state.chapter_repo.find_by_id(&chapter_id).await?
        .ok_or_else(|| TingError::NotFound(format!("Chapter {} not found", chapter_id)))?;

    let book = state.book_repo.find_by_id(&chapter.book_id).await?
        .ok_or_else(|| TingError::NotFound(format!("Book {} not found", chapter.book_id)))?;

    let library = state.library_repo.find_by_id(&book.library_id).await?
        .ok_or_else(|| TingError::NotFound(format!("Library {} not found", book.library_id)))?;

    if library.library_type == "local" {
        return Ok(Json(CacheOperationResponse {
            success: true,
            message: "Local file, caching skipped".to_string(),
            cache_info: None,
        }));
    }

    let cache_path = state.cache_manager.get_cache_path(&chapter_id);
    let cache_info = if cache_path.exists() {
        state.cache_manager.get_cache_info(&chapter_id).await
            .map_err(|e| TingError::IoError(std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to get cache info: {}", e))))?
    } else {
        let temp_path = cache_path.with_extension("tmp");
        let (mut reader, _) = state.storage_service.get_webdav_reader(
            &library, &chapter.path, None, state.encryption_key.as_ref(),
        ).await.map_err(|e| TingError::IoError(std::io::Error::new(std::io::ErrorKind::Other, e.to_string())))?;

        let mut file = tokio::fs::File::create(&temp_path).await?;
        tokio::io::copy(&mut reader, &mut file).await?;
        tokio::fs::rename(&temp_path, &cache_path).await?;

        let config = state.config.read().await;
        let _ = state.cache_manager.enforce_limits(50, config.storage.max_disk_usage).await;

        state.cache_manager.get_cache_info(&chapter_id).await
            .map_err(|e| TingError::IoError(std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to get cache info: {}", e))))?
    };

    let created_at = cache_info.created_at
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| chrono::DateTime::from_timestamp(d.as_secs() as i64, 0))
        .flatten()
        .map(|dt| dt.to_rfc3339());

    Ok(Json(CacheOperationResponse {
        success: true,
        message: format!("Chapter {} cached successfully", chapter_id),
        cache_info: Some(CacheInfoResponse {
            chapter_id: cache_info.chapter_id,
            book_id: Some(chapter.book_id),
            book_title: book.title,
            chapter_title: chapter.title,
            file_size: cache_info.file_size,
            created_at,
            cover_url: book.cover_url,
        }),
    }))
}

/// GET /api/cache - Get cache list with book metadata
pub async fn get_cache_list(
    State(state): State<AppState>,
    user: crate::auth::middleware::AuthUser,
) -> Result<impl axum::response::IntoResponse> {
    require_admin(&user)?;

    let cached_chapters = state.cache_manager.list_cached().await
        .map_err(|e| TingError::IoError(std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to list caches: {}", e))))?;

    let mut caches = Vec::new();
    let mut total_size = 0;

    for cache_info in cached_chapters {
        match state.chapter_repo.find_by_id(&cache_info.chapter_id).await {
            Ok(Some(chapter)) => {
                let book = state.book_repo.find_by_id(&chapter.book_id).await.ok().flatten();
                let created_at = cache_info.created_at
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| chrono::DateTime::from_timestamp(d.as_secs() as i64, 0))
                    .flatten()
                    .map(|dt| dt.to_rfc3339());

                caches.push(CacheInfoResponse {
                    chapter_id: cache_info.chapter_id,
                    book_id: Some(chapter.book_id),
                    book_title: book.as_ref().and_then(|b| b.title.clone()),
                    chapter_title: Some(chapter.title.unwrap_or_default()),
                    file_size: cache_info.file_size,
                    created_at,
                    cover_url: book.as_ref().and_then(|b| b.cover_url.clone()),
                });
                total_size += cache_info.file_size;
            }
            Ok(None) => {
                tracing::warn!("Orphaned cache found for chapter {}. Removing...", cache_info.chapter_id);
                let _ = state.cache_manager.delete_cache(&cache_info.chapter_id).await;
            }
            Err(e) => {
                tracing::error!("Failed to lookup chapter for cache {}: {}", cache_info.chapter_id, e);
            }
        }
    }

    let total = caches.len();
    Ok(Json(CacheListResponse { caches, total, total_size }))
}

/// DELETE /api/cache/:chapterId - Delete a single chapter cache
pub async fn delete_chapter_cache(
    State(state): State<AppState>,
    user: crate::auth::middleware::AuthUser,
    Path(chapter_id): Path<String>,
) -> Result<impl axum::response::IntoResponse> {
    require_admin(&user)?;

    state.cache_manager.delete_cache(&chapter_id).await
        .map_err(|e| match e {
            crate::cache::CacheError::NotFound(_) => TingError::NotFound(format!("Cache for chapter {} not found", chapter_id)),
            _ => TingError::IoError(std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to delete cache: {}", e))),
        })?;

    Ok(Json(CacheOperationResponse {
        success: true,
        message: format!("Cache for chapter {} deleted successfully", chapter_id),
        cache_info: None,
    }))
}

/// DELETE /api/cache - Clear all caches
pub async fn clear_all_caches(
    State(state): State<AppState>,
    user: crate::auth::middleware::AuthUser,
) -> Result<impl axum::response::IntoResponse> {
    require_admin(&user)?;

    let deleted_count = state.cache_manager.clear_all().await
        .map_err(|e| TingError::IoError(std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to clear caches: {}", e))))?;

    Ok(Json(ClearCacheResponse {
        success: true,
        deleted_count,
        message: format!("Cleared {} cached chapters", deleted_count),
    }))
}
