use crate::api::models::{
    CacheOperationResponse, CacheInfoResponse, CacheListResponse, ClearCacheResponse,
};
use crate::core::error::{Result, TingError};
use crate::db::repository::Repository;
use crate::plugin::types::{DecryptionPlan, DecryptionSegment};
use crate::plugin::manager::FormatMethod;
use axum::{
    body::Body,
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use super::AppState;
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncRead};
use tokio_util::io::ReaderStream;
use futures::StreamExt;
use base64::Engine;

/// Handler for POST /api/cache/:chapterId - Cache a chapter
pub async fn cache_chapter(
    State(state): State<AppState>,
    user: crate::auth::middleware::AuthUser,
    Path(chapter_id): Path<String>,
) -> Result<impl IntoResponse> {
    if user.role != "admin" {
        return Err(TingError::PermissionDenied("Admin access required".to_string()));
    }
    
    // Get chapter from database
    let chapter = state.chapter_repo.find_by_id(&chapter_id).await?
        .ok_or_else(|| TingError::NotFound(format!("Chapter {} not found", chapter_id)))?;
    
    // Get book and library info
    let book = state.book_repo.find_by_id(&chapter.book_id).await?
        .ok_or_else(|| TingError::NotFound(format!("Book {} not found", chapter.book_id)))?;
        
    let library = state.library_repo.find_by_id(&book.library_id).await?
        .ok_or_else(|| TingError::NotFound(format!("Library {} not found", book.library_id)))?;
    
    // Check if we should cache based on library type
    if library.library_type == "local" {
        return Ok(Json(CacheOperationResponse {
            success: true,
            message: "Local file, caching skipped".to_string(),
            cache_info: None,
        }));
    }

    // Check if already cached
    let cache_path = state.cache_manager.get_cache_path(&chapter_id);
    let cache_info = if cache_path.exists() {
        state.cache_manager.get_cache_info(&chapter_id).await
            .map_err(|e| TingError::IoError(std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to get cache info: {}", e))))?
    } else {
        // Download from WebDAV (or other remote sources)
        // We use a temp file to ensure partial downloads don't corrupt the cache
        let temp_path = cache_path.with_extension("tmp");
        
        let (mut reader, _) = state.storage_service.get_webdav_reader(
            &library, 
            &chapter.path, 
            None, 
            state.encryption_key.as_ref()
        ).await.map_err(|e| TingError::IoError(std::io::Error::new(std::io::ErrorKind::Other, e.to_string())))?;

        let mut file = tokio::fs::File::create(&temp_path).await?;
        tokio::io::copy(&mut reader, &mut file).await?;
        
        // Rename temp to final
        tokio::fs::rename(&temp_path, &cache_path).await?;
        
        // Enforce cache limits (e.g. 50 files or max_disk_usage)
        // We use 50 as default file count limit, and storage config for size limit
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

/// Handler for GET /api/cache - Get cache list
pub async fn get_cache_list(
    State(state): State<AppState>,
    user: crate::auth::middleware::AuthUser,
) -> Result<impl IntoResponse> {
    if user.role != "admin" {
        return Err(TingError::PermissionDenied("Admin access required".to_string()));
    }
    
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
            },
            Ok(None) => {
                tracing::warn!("Found orphaned cache file for chapter: {}. Deleting...", cache_info.chapter_id);
                if let Err(e) = state.cache_manager.delete_cache(&cache_info.chapter_id).await {
                     tracing::error!("Failed to delete orphaned cache {}: {}", cache_info.chapter_id, e);
                }
            },
            Err(e) => {
                tracing::error!("Failed to lookup chapter for cache {}: {}", cache_info.chapter_id, e);
            }
        }
    }
    
    let total = caches.len();
    
    Ok(Json(CacheListResponse {
        caches,
        total,
        total_size,
    }))
}

/// Handler for DELETE /api/cache/:chapterId - Delete a chapter cache
pub async fn delete_chapter_cache(
    State(state): State<AppState>,
    user: crate::auth::middleware::AuthUser,
    Path(chapter_id): Path<String>,
) -> Result<impl IntoResponse> {
    if user.role != "admin" {
        return Err(TingError::PermissionDenied("Admin access required".to_string()));
    }
    
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

/// Handler for DELETE /api/cache - Clear all caches
pub async fn clear_all_caches(
    State(state): State<AppState>,
    user: crate::auth::middleware::AuthUser,
) -> Result<impl IntoResponse> {
    if user.role != "admin" {
        return Err(TingError::PermissionDenied("Admin access required".to_string()));
    }
    
    let deleted_count = state.cache_manager.clear_all().await
        .map_err(|e| TingError::IoError(std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to clear caches: {}", e))))?;
    
    Ok(Json(ClearCacheResponse {
        success: true,
        deleted_count,
        message: format!("Cleared {} cached chapters", deleted_count),
    }))
}

/// Query parameters for proxy cover
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProxyCoverQuery {
    pub path: String,
    pub library_id: String,
    pub book_id: Option<String>,
}

/// Handler for GET /api/proxy/cover - Proxy cover image
pub async fn proxy_cover(
    State(_state): State<AppState>,
    Query(params): Query<ProxyCoverQuery>,
) -> Result<impl IntoResponse> {
    use axum::http::header;
    
    if params.path == "embedded://first-chapter" {
        return Err(TingError::NotFound("Embedded cover extraction not yet implemented".to_string()));
    }
    
    // Normalize path separators (Windows compatibility)
    let normalized_path = params.path.replace('\\', "/");
    let image_path = std::path::Path::new(&normalized_path);
    
    tracing::info!("Proxy cover request: original='{}', normalized='{}'", params.path, normalized_path);
    
    // Try to resolve path
    let final_path = if image_path.exists() {
        image_path.to_path_buf()
    } else {
        // Try relative to CWD if not found directly
        if let Ok(cwd) = std::env::current_dir() {
            let abs_path = cwd.join(image_path);
            if abs_path.exists() {
                abs_path
            } else {
                 // Try stripping './' if present
                 if normalized_path.starts_with("./") {
                     let stripped = cwd.join(&normalized_path[2..]);
                     if stripped.exists() {
                         stripped
                     } else {
                         return Err(TingError::NotFound(format!("Cover image not found: {}", params.path)));
                     }
                 } else {
                     return Err(TingError::NotFound(format!("Cover image not found: {}", params.path)));
                 }
            }
        } else {
            return Err(TingError::NotFound(format!("Cover image not found: {}", params.path)));
        }
    };
    
    let image_data = tokio::fs::read(&final_path).await?;
    
    let mime_type = mime_guess::from_path(&final_path)
        .first_or_octet_stream()
        .to_string();
    
    Ok((
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, mime_type),
            (header::ACCESS_CONTROL_ALLOW_ORIGIN, "*".to_string()),
            (header::CACHE_CONTROL, "public, max-age=31536000".to_string()),
            ("Cross-Origin-Resource-Policy".parse().unwrap(), "cross-origin".to_string()),
        ],
        image_data,
    ))
}

use std::process::Stdio;
use tokio::process::Command;

/// Query parameters for stream chapter
#[derive(Debug, serde::Deserialize)]
pub struct StreamQuery {
    pub token: Option<String>,
    pub transcode: Option<String>,
}

/// Handler for GET /api/stream/:chapterId - Stream chapter audio
pub async fn stream_chapter(
    State(state): State<AppState>,
    Path(chapter_id): Path<String>,
    Query(params): Query<StreamQuery>,
    headers: axum::http::HeaderMap,
    user: Option<crate::auth::middleware::AuthUser>,
) -> Result<impl IntoResponse> {
    use axum::http::header;
    
    if let Some(_token) = params.token {
        // Token validation would go here
    }
    
    let chapter = state.chapter_repo.find_by_id(&chapter_id).await?
        .ok_or_else(|| TingError::NotFound(format!("Chapter {} not found", chapter_id)))?;
    
    let book = state.book_repo.find_by_id(&chapter.book_id).await?
        .ok_or_else(|| TingError::NotFound(format!("Book {} not found", chapter.book_id)))?;
        
    let library = state.library_repo.find_by_id(&book.library_id).await?
        .ok_or_else(|| TingError::NotFound(format!("Library {} not found", book.library_id)))?;

    // Handle Transcoding Request
    if let Some(format) = params.transcode {
        tracing::info!("Transcoding requested: {} -> {}", chapter.path, format);

        let content_type = match format.as_str() {
            "mp3" => "audio/mpeg",
            "wav" => "audio/wav",
            _ => return Err(TingError::InvalidRequest("Unsupported transcode format".to_string())),
        };

        let ffmpeg_path = state.plugin_manager.get_ffmpeg_path().await
            .ok_or_else(|| TingError::IoError(std::io::Error::new(std::io::ErrorKind::NotFound, "FFmpeg plugin binary not found")))?;
        let cache_path = state.cache_manager.get_cache_path(&chapter_id);
        
        // 1. Try to get transcode command from plugin
        let mut plugin_command: Option<Vec<String>> = None;
        let plugin_info = state.plugin_manager.find_plugin_for_format(std::path::Path::new(&chapter.path)).await;
        
        if let Some(plugin) = plugin_info {
            let res = state.plugin_manager.call_format(
                &plugin.id,
                FormatMethod::GetStreamUrl,
                serde_json::json!({
                    "file_path": chapter.path,
                    "transcode": format
                })
            ).await;
            
            if let Ok(val) = res {
                if let Some(cmd) = val.get("command").and_then(|c| c.as_array()) {
                    let cmd_vec: Vec<String> = cmd.iter()
                        .filter_map(|v| v.as_str())
                        .map(|s| s.to_string())
                        .collect();
                    if !cmd_vec.is_empty() {
                        plugin_command = Some(cmd_vec);
                        tracing::info!("Using plugin-provided transcode command for {}", chapter.path);
                    }
                }
            }
        }

        let mut command = if let Some(cmd_vec) = plugin_command {
            let mut cmd = Command::new(&cmd_vec[0]);
            cmd.args(&cmd_vec[1..]);
            
            // If the plugin command uses "-" or "pipe:0" for input, we need to enable stdin pipe
            // The plugin should return "-" as input argument for piped input
            let use_pipe = !cache_path.exists() && library.library_type != "local";
            if use_pipe {
                cmd.stdin(Stdio::piped());
            }
            cmd
        } else {
            // Fallback to hardcoded logic
            let mut cmd = Command::new(&ffmpeg_path);
            cmd
                .arg("-y")
                .arg("-loglevel")
                .arg("error")
                .arg("-i");
                
            // Input Source
            let _use_pipe_fallback = !cache_path.exists() && library.library_type != "local";
            
            if cache_path.exists() {
                cmd.arg(cache_path.to_string_lossy().as_ref());
            } else if library.library_type == "local" {
                cmd.arg(&chapter.path);
            } else {
                // Pipe input
                cmd.arg("-");
                cmd.stdin(Stdio::piped());
            }
            
            // Explicitly set audio codec for mp3
            if format == "mp3" {
                cmd.arg("-acodec").arg("libmp3lame");
                cmd.arg("-q:a").arg("4"); // VBR Quality ~160kbps, good balance
            }

            cmd
                .arg("-f")
                .arg(&format)
                .arg("-");
            cmd
        };
            
        command.stdout(Stdio::piped());
        
        // Spawn
        let mut child = command.spawn().map_err(|e| TingError::IoError(e))?;
        
        // Handle input pipe if needed (Only if we are using the fallback pipe logic)
        let use_pipe = !cache_path.exists() && library.library_type != "local";
        if use_pipe && child.stdin.is_some() {
            if let Some(mut stdin) = child.stdin.take() {
                // Get reader
                let (mut reader, _) = state.storage_service.get_webdav_reader(&library, &chapter.path, None, state.encryption_key.as_ref()).await
                    .map_err(|e| TingError::NotFound(format!("WebDAV file not found: {}", e)))?;
                    
                tokio::spawn(async move {
                    if let Err(e) = tokio::io::copy(&mut reader, &mut stdin).await {
                         tracing::error!("Failed to pipe input to ffmpeg: {}", e);
                    }
                });
            }
        }
        
        let stdout = child.stdout.take().ok_or_else(|| TingError::IoError(std::io::Error::new(std::io::ErrorKind::Other, "Failed to capture ffmpeg stdout")))?;
        
        let stream = ReaderStream::new(stdout);
        let body = Body::from_stream(stream);
        
        return Ok((
            StatusCode::OK,
            [
                (header::CONTENT_TYPE, content_type.to_string()),
                (header::ACCESS_CONTROL_ALLOW_ORIGIN, "*".to_string()),
                ("Cross-Origin-Resource-Policy".parse().unwrap(), "cross-origin".to_string()),
            ],
            body,
        ).into_response());
    }

    // Auto Preload / Cache Logic
    if let Some(user) = user {
        // Auto-preload and auto-cache are available for all users
        if let Ok(Some(settings)) = state.settings_repo.get_by_user(&user.id).await {
            let settings_val = settings.settings_json.as_ref()
                .and_then(|s| serde_json::from_str::<serde_json::Value>(s).ok());
                
            let auto_preload = settings_val.as_ref()
                .and_then(|v| v.get("autoPreload"))
                .and_then(|v| v.as_bool())
                .unwrap_or(true);
                
            let auto_cache = settings_val.as_ref()
                .and_then(|v| v.get("autoCache"))
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
                
            if auto_preload || auto_cache {
                if let Ok(chapters) = state.chapter_repo.find_by_book(&book.id).await {
                    if let Some(pos) = chapters.iter().position(|c| c.id == chapter_id) {
                        if let Some(next_chapter) = chapters.get(pos + 1).cloned() {
                            // Spawn preload task
                            let state_clone = state.clone();
                            let next_chapter_id = next_chapter.id.clone();
                            let next_chapter_path = next_chapter.path.clone();
                            let lib_clone = library.clone();
                            let user_id = user.id.clone();
                            
                            // Cancel any previous preload task for this user
                            {
                                let mut tasks = state.active_preload_tasks.lock().await;
                                if let Some(handle) = tasks.remove(&user_id) {
                                    handle.abort();
                                    tracing::debug!("Cancelled previous preload task for user {}", user_id);
                                }
                            }
                            
                            let handle = tokio::spawn(async move {
                                // Check if already in cache BEFORE starting any heavy work
                                if auto_preload {
                                    let cache = state_clone.preload_cache.read().await;
                                    if cache.contains_key(&next_chapter_id) {
                                        tracing::debug!("Skipping auto-preload for {} - already in cache", next_chapter_id);
                                        return;
                                    }
                                }
                                
                                // Add a small delay to debounce rapid switching
                                tokio::time::sleep(std::time::Duration::from_millis(500)).await;

                                let reader_res = if lib_clone.library_type.to_lowercase() == "local" {
                                    state_clone.storage_service.get_local_reader(std::path::Path::new(&next_chapter_path), None).await
                                        .map(|(f, s)| (Box::new(f) as Box<dyn tokio::io::AsyncRead + Send + Unpin>, s))
                                } else {
                                    state_clone.storage_service.get_webdav_reader(&lib_clone, &next_chapter_path, None, state_clone.encryption_key.as_ref()).await
                                };

                                match reader_res {
                                    Ok((mut r, _)) => {
                                        // For auto_preload (memory), we need to read to buffer
                                        if auto_preload {
                                            // Double check cache before reading heavy data
                                            {
                                                let cache = state_clone.preload_cache.read().await;
                                                if cache.contains_key(&next_chapter_id) {
                                                    tracing::debug!("Skipping auto-preload for {} - already in cache (double check)", next_chapter_id);
                                                    return;
                                                }
                                            }

                                            let mut buf = Vec::new();
                                            if let Ok(_) = r.read_to_end(&mut buf).await {
                                                let bytes_data = bytes::Bytes::from(buf);
                                                
                                                // Limit preload cache size to prevent memory leaks
                                                {
                                                    let mut cache = state_clone.preload_cache.write().await;
                                                    const MAX_PRELOAD_SIZE: usize = 3;
                                                    
                                                    if cache.len() >= MAX_PRELOAD_SIZE {
                                                        // Find oldest entry to remove
                                                        let oldest_key = cache.iter()
                                                            .min_by_key(|(_, (_, time))| *time)
                                                            .map(|(k, _)| k.clone());
                                                        
                                                        if let Some(key) = oldest_key {
                                                            cache.remove(&key);
                                                            tracing::debug!("Evicted oldest preloaded chapter from memory: {}", key);
                                                        }
                                                    }
                                                    
                                                    cache.insert(next_chapter_id.clone(), (bytes_data.clone(), std::time::Instant::now()));
                                                }
                                                tracing::info!("Auto-preloaded next chapter: {}", next_chapter_id);
                                                
                                                // If auto_cache is also enabled, use the buffer to write to disk
                                                if auto_cache && lib_clone.library_type.to_lowercase() != "local" {
                                                    let cache_path = state_clone.cache_manager.get_cache_path(&next_chapter_id);
                                                    if !cache_path.exists() {
                                                        if let Ok(_) = tokio::fs::write(&cache_path, &bytes_data).await {
                                                            tracing::info!("Auto-cached next chapter (from buffer): {}", next_chapter_id);
                                                            
                                                            // Enforce limits
                                                            let config = state_clone.config.read().await;
                                                            let _ = state_clone.cache_manager.enforce_limits(50, config.storage.max_disk_usage).await;
                                                        } else {
                                                            tracing::error!("Failed to write cache file from buffer for chapter: {}", next_chapter_id);
                                                        }
                                                    }
                                                }
                                            } else {
                                                tracing::error!("Failed to read next chapter for preload: {}", next_chapter_id);
                                            }
                                        } else if auto_cache && lib_clone.library_type.to_lowercase() != "local" {
                                            // For auto_cache ONLY (disk), stream directly to file to save memory
                                            let cache_path = state_clone.cache_manager.get_cache_path(&next_chapter_id);
                                            if !cache_path.exists() {
                                                 // Create temp file first
                                                 let temp_path = cache_path.with_extension("tmp");
                                                 match tokio::fs::File::create(&temp_path).await {
                                                     Ok(file) => {
                                                         let mut writer = tokio::io::BufWriter::new(file);
                                                         match tokio::io::copy(&mut r, &mut writer).await {
                                                             Ok(_) => {
                                                                 // Rename to final path
                                                                if let Ok(_) = tokio::fs::rename(&temp_path, &cache_path).await {
                                                                    tracing::info!("Auto-cached next chapter (streamed): {}", next_chapter_id);
                                                                    
                                                                    // Enforce limits
                                                                    let config = state_clone.config.read().await;
                                                                    let _ = state_clone.cache_manager.enforce_limits(50, config.storage.max_disk_usage).await;
                                                                } else {
                                                                    tracing::error!("Failed to rename temp cache file for chapter: {}", next_chapter_id);
                                                                }
                                                             },
                                                             Err(e) => {
                                                                 tracing::error!("Failed to stream copy for auto-cache: {} - {}", next_chapter_id, e);
                                                                 let _ = tokio::fs::remove_file(&temp_path).await;
                                                             }
                                                         }
                                                     },
                                                     Err(e) => {
                                                         tracing::error!("Failed to create temp cache file: {} - {}", next_chapter_id, e);
                                                     }
                                                 }
                                            }
                                        }
                                    },
                                    Err(e) => {
                                        tracing::error!("Failed to get reader for next chapter {}: {}", next_chapter_id, e);
                                    }
                                }
                            });
                            
                            // Store the handle for cancellation
                            let mut tasks = state.active_preload_tasks.lock().await;
                            tasks.insert(user.id.clone(), handle);
                        }
                    }
                }
            }
        }
    }

    // 1. Check Preload Cache (Memory)
    {
        let mut cache = state.preload_cache.write().await;
        if let Some((data, last_access)) = cache.get_mut(&chapter_id) {
            // Update access time to implement LRU (keep frequently accessed chapters in memory)
            *last_access = std::time::Instant::now();
            
            tracing::info!(chapter_id = %chapter_id, "Serving from preload cache (memory)");
            let data = data.clone(); // Clone bytes (cheap reference count increment)
            // Drop write lock early
            drop(cache);
            
            let file_size = data.len() as u64;
            let mime_type = mime_guess::from_path(&chapter.path).first_or_octet_stream().to_string();
            
            let range_header = headers.get(header::RANGE).and_then(|v| v.to_str().ok());
            
            if let Some(range_str) = range_header {
                 if let Ok(range) = state.audio_streamer.parse_range_header(range_str, file_size) {
                     let start = range.start as usize;
                     let end = range.end as usize;
                     let content_length = (end - start) as u64;
                     let body = data[start..end].to_vec();
                     
                     return Ok((
                        StatusCode::PARTIAL_CONTENT,
                        [
                            (header::CONTENT_TYPE, mime_type),
                            (header::CONTENT_LENGTH, content_length.to_string()),
                            (header::CONTENT_RANGE, format!("bytes {}-{}/{}", start, end - 1, file_size)),
                            (header::ACCEPT_RANGES, "bytes".to_string()),
                            (header::ACCESS_CONTROL_ALLOW_ORIGIN, "*".to_string()),
                            ("Cross-Origin-Resource-Policy".parse().unwrap(), "cross-origin".to_string()),
                        ],
                        body,
                    ).into_response());
                 }
            }
            
            return Ok((
                StatusCode::OK,
                [
                    (header::CONTENT_TYPE, mime_type),
                    (header::CONTENT_LENGTH, file_size.to_string()),
                    (header::ACCEPT_RANGES, "bytes".to_string()),
                    (header::ACCESS_CONTROL_ALLOW_ORIGIN, "*".to_string()),
                    ("Cross-Origin-Resource-Policy".parse().unwrap(), "cross-origin".to_string()),
                ],
                data.to_vec(),
            ).into_response());
        }
    }
    
    // 2. Check Disk Cache
        let cache_path = state.cache_manager.get_cache_path(&chapter_id);
        if cache_path.exists() {
            tracing::info!(chapter_id = %chapter_id, "Serving from disk cache");
            
            // Check if we need to use a format plugin even for cached files (source file is cached)
            let plugin_info = state.plugin_manager.find_plugin_for_format(std::path::Path::new(&chapter.path)).await;
            
            if let Some(plugin) = plugin_info {
                // If a plugin handles this format, we use the cached file as the source for the plugin logic
                // instead of serving it directly.
                tracing::info!(chapter_id = %chapter_id, plugin = %plugin.name, "Cached file requires format plugin processing");
                
                // Fall through to the plugin handling logic below
                // We need to make sure the logic below knows to use the cache_path as source
                // This is handled by the `if cache_path.exists()` checks in the plugin block
            } else {
                let file_size = tokio::fs::metadata(&cache_path).await?.len();
                let mime_type = mime_guess::from_path(&chapter.path).first_or_octet_stream().to_string();
                
                let range_header = headers.get(header::RANGE).and_then(|v| v.to_str().ok());
                if let Some(range_str) = range_header {
                    if let Ok(range) = state.audio_streamer.parse_range_header(range_str, file_size) {
                        let content_length = range.end - range.start;
                        let mut file = tokio::fs::File::open(&cache_path).await?;
                        file.seek(std::io::SeekFrom::Start(range.start)).await?;
                        let mut buffer = vec![0u8; content_length as usize];
                        file.read_exact(&mut buffer).await?;
                        
                        return Ok((
                            StatusCode::PARTIAL_CONTENT,
                            [
                                (header::CONTENT_TYPE, mime_type.clone()),
                                (header::CONTENT_LENGTH, content_length.to_string()),
                                (header::CONTENT_RANGE, format!("bytes {}-{}/{}", range.start, range.end - 1, file_size)),
                                (header::ACCEPT_RANGES, "bytes".to_string()),
                                (header::ACCESS_CONTROL_ALLOW_ORIGIN, "*".to_string()),
                                ("Cross-Origin-Resource-Policy".parse().unwrap(), "cross-origin".to_string()),
                            ],
                            buffer,
                        ).into_response());
                    }
                }
                
                let body = tokio::fs::read(&cache_path).await?;
                return Ok((
                    StatusCode::OK,
                    [
                        (header::CONTENT_TYPE, mime_type),
                        (header::CONTENT_LENGTH, file_size.to_string()),
                        (header::ACCEPT_RANGES, "bytes".to_string()),
                        (header::ACCESS_CONTROL_ALLOW_ORIGIN, "*".to_string()),
                        ("Cross-Origin-Resource-Policy".parse().unwrap(), "cross-origin".to_string()),
                    ],
                    body,
                ).into_response());
            }
        }

    // 3. Not cached. Fetch from source.
    tracing::info!(chapter_id = %chapter_id, "Serving from source (stream)");
    
    // Determine if we need to use a format plugin
    // Instead of hardcoding extensions, we ask the plugin manager if any loaded plugin supports this extension
    let plugin_info = state.plugin_manager.find_plugin_for_format(std::path::Path::new(&chapter.path)).await;
    
    if let Some(plugin) = plugin_info {
        tracing::info!(chapter_id = %chapter_id, plugin = %plugin.name, "Processing file with format plugin");

        // 1. Read minimal header probe (10 bytes for ID3 header) to determine header size
        let probe_size = 10;
        let (mut probe_reader, _) = if cache_path.exists() {
             let (reader, size) = state.storage_service.get_local_reader(&cache_path, Some((0, probe_size))).await
                .map_err(|e| TingError::NotFound(format!("Cached file not found: {}", e)))?;
             (Box::new(reader.take(probe_size)) as Box<dyn AsyncRead + Send + Unpin>, size)
        } else if library.library_type == "local" {
            let (reader, size) = state.storage_service.get_local_reader(std::path::Path::new(&chapter.path), Some((0, probe_size))).await
                .map_err(|e| TingError::NotFound(format!("Local file not found: {}", e)))?;
            (Box::new(reader.take(probe_size)) as Box<dyn AsyncRead + Send + Unpin>, size)
        } else {
            let (reader, size) = state.storage_service.get_webdav_reader(&library, &chapter.path, Some((0, probe_size)), state.encryption_key.as_ref()).await
                .map_err(|e| TingError::NotFound(format!("WebDAV file not found: {}", e)))?;
            (Box::new(reader.take(probe_size)) as Box<dyn AsyncRead + Send + Unpin>, size)
        };

        let mut probe_bytes = Vec::new();
        probe_reader.read_to_end(&mut probe_bytes).await?;
        
        // 2. Ask plugin for required header size
        let probe_base64 = base64::engine::general_purpose::STANDARD.encode(&probe_bytes);
        let size_json = state.plugin_manager.call_format(
            &plugin.id,
            FormatMethod::GetMetadataReadSize,
            serde_json::json!({"header_base64": probe_base64})
        ).await.map_err(|e| {
            tracing::error!("Failed to get metadata read size: {}", e);
            TingError::PluginExecutionError(format!("Failed to get metadata read size: {}", e))
        })?;
        
        let header_size = size_json["size"].as_u64().unwrap_or(8192); // Default to 8KB if unknown
        tracing::info!("Required header size: {}", header_size);

        // 3. Read full header
        let (mut header_reader, total_file_size) = if cache_path.exists() {
            let (reader, size) = state.storage_service.get_local_reader(&cache_path, Some((0, header_size))).await?;
            (Box::new(reader.take(header_size)) as Box<dyn AsyncRead + Send + Unpin>, size)
        } else if library.library_type == "local" {
            let (reader, size) = state.storage_service.get_local_reader(std::path::Path::new(&chapter.path), Some((0, header_size))).await?;
            (Box::new(reader.take(header_size)) as Box<dyn AsyncRead + Send + Unpin>, size)
        } else {
            let (reader, size) = state.storage_service.get_webdav_reader(&library, &chapter.path, Some((0, header_size)), state.encryption_key.as_ref()).await?;
            (Box::new(reader.take(header_size)) as Box<dyn AsyncRead + Send + Unpin>, size)
        };
        
        let mut header_bytes = Vec::new();
        header_reader.read_to_end(&mut header_bytes).await?;

        // 4. Get Decryption Plan
        let header_base64 = base64::engine::general_purpose::STANDARD.encode(&header_bytes);
        let plan_json = state.plugin_manager.call_format(
            &plugin.id, 
            FormatMethod::GetDecryptionPlan, 
            serde_json::json!({"header_base64": header_base64})
        ).await.map_err(|e| {
            tracing::error!("Failed to get decryption plan: {}", e);
            TingError::PluginExecutionError(format!("Failed to get decryption plan: {}", e))
        })?;
        
        let plan: DecryptionPlan = serde_json::from_value(plan_json)
            .map_err(|e| TingError::SerializationError(format!("Invalid decryption plan: {}", e)))?;

        // 5. Calculate Logic Size and Range
        // First calculate total logical size to handle Range requests correctly
        let mut logic_size = 0;
        for segment in &plan.segments {
            match segment {
                DecryptionSegment::Encrypted { length, .. } => logic_size += *length as u64,
                DecryptionSegment::Plain { length, offset } => {
                    if *length <= 0 {
                        logic_size += total_file_size - offset;
                    } else {
                        logic_size += *length as u64;
                    }
                }
            }
        }
        // Override if plan provides explicit total size
        if let Some(s) = plan.total_size {
            logic_size = s;
        }

        tracing::info!("Stream prepared: logic_size={}", logic_size);
        
        // Parse Range Header
        let range_header = headers.get(header::RANGE).and_then(|v| v.to_str().ok());
        let (start, end) = if let Some(r_str) = range_header {
             if let Ok(range) = state.audio_streamer.parse_range_header(r_str, logic_size) {
                 (range.start, range.end)
             } else {
                 (0, logic_size)
             }
        } else {
            (0, logic_size)
        };
        
        let content_length = end - start;
        let mime_type = "audio/mp4"; // Decrypted XM is usually m4a/aac

        // 6. Construct Lazy Stream Chain
        let mut stream_chain: Vec<futures::stream::BoxStream<'static, std::result::Result<bytes::Bytes, std::io::Error>>> = Vec::new();
        let mut current_pos = 0;

        for segment in plan.segments {
            // Calculate segment length
            let seg_len = match segment {
                DecryptionSegment::Encrypted { length, .. } => length as u64,
                DecryptionSegment::Plain { length, offset } => {
                    if length <= 0 {
                        total_file_size - offset
                    } else {
                        length as u64
                    }
                }
            };

            let seg_start = current_pos;
            let seg_end = current_pos + seg_len;
            
            // Check intersection with requested Range
            if seg_end > start && seg_start < end {
                // Calculate overlap relative to segment
                let req_seg_start = std::cmp::max(start, seg_start);
                let req_seg_end = std::cmp::min(end, seg_end);
                
                let relative_start = req_seg_start - seg_start;
                let relative_end = req_seg_end - seg_start;
                let _length_to_read = relative_end - relative_start;

                match segment {
                    DecryptionSegment::Encrypted { offset, length, params } => {
                         // Clone state for async block
                         let state = state.clone();
                         let cache_path = cache_path.clone();
                         let library = library.clone();
                         let chapter_path = chapter.path.clone();
                         let plugin_id = plugin.id.clone();
                         let encryption_key = state.encryption_key.clone();
                         let params = params.clone();
                         
                         // Create Lazy Future for this chunk
                         let future = async move {
                             tracing::debug!("Lazy loading encrypted segment: offset={}, length={}", offset, length);
                             
                             // Safety check
                             const MAX_ENCRYPTED_SEGMENT_SIZE: u64 = 10 * 1024 * 1024; // 10MB
                             if length as u64 > MAX_ENCRYPTED_SEGMENT_SIZE {
                                 return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "Encrypted segment too large"));
                             }

                             // Download encrypted chunk (full segment)
                             // Note: We download the full encrypted segment because decryption usually depends on it.
                             // Optimization: If plugin supports partial decryption, we could optimize this.
                             let (mut reader, _) = if cache_path.exists() {
                                 let (reader, size) = state.storage_service.get_local_reader(&cache_path, Some((offset, offset + length as u64))).await
                                     .map_err(|e| std::io::Error::new(std::io::ErrorKind::NotFound, e))?;
                                 (Box::new(reader.take(length as u64)) as Box<dyn AsyncRead + Send + Unpin>, size)
                             } else if library.library_type == "local" {
                                 let (reader, size) = state.storage_service.get_local_reader(std::path::Path::new(&chapter_path), Some((offset, offset + length as u64))).await
                                     .map_err(|e| std::io::Error::new(std::io::ErrorKind::NotFound, e))?;
                                 (Box::new(reader.take(length as u64)) as Box<dyn AsyncRead + Send + Unpin>, size)
                             } else {
                                 let (reader, size) = state.storage_service.get_webdav_reader(&library, &chapter_path, Some((offset, offset + length as u64)), encryption_key.as_ref()).await
                                     .map_err(|e| std::io::Error::new(std::io::ErrorKind::NotFound, e))?;
                                 (Box::new(reader.take(length as u64)) as Box<dyn AsyncRead + Send + Unpin>, size)
                             };
                             
                             // Read and Encode
                             let chunk_base64 = {
                                 let mut chunk = Vec::with_capacity(length as usize);
                                 reader.read_to_end(&mut chunk).await?;
                                 base64::engine::general_purpose::STANDARD.encode(&chunk)
                             };
                             
                             // Decrypt
                             let result_json = state.plugin_manager.call_format(
                                 &plugin_id,
                                 FormatMethod::DecryptChunk,
                                 serde_json::json!({
                                     "data_base64": chunk_base64,
                                     "params": params
                                 })
                             ).await.map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
                             
                             let decrypted_base64 = result_json["data_base64"].as_str()
                                 .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidData, "Missing data_base64"))?;
                             
                             let decrypted = base64::engine::general_purpose::STANDARD.decode(decrypted_base64)
                                 .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
                                 
                             // Slice the result based on range
                             // Be careful with bounds
                             let slice_start = relative_start as usize;
                             let slice_end = std::cmp::min(relative_end as usize, decrypted.len());
                             
                             if slice_start >= decrypted.len() {
                                 return Ok(bytes::Bytes::new());
                             }
                             
                             Ok(bytes::Bytes::from(decrypted[slice_start..slice_end].to_vec()))
                         };
                         
                         // Wrap future in a stream that yields once
                         stream_chain.push(futures::stream::once(future).boxed());
                    },
                    DecryptionSegment::Plain { offset, .. } => {
                        // Plain segment: use ReaderStream
                        let read_start = offset + relative_start;
                        let read_end = offset + relative_end;
                        
                        // We need to construct the stream here, but ReaderStream creation is async (opening file)
                        // Wait, storage_service.get_... is async.
                        // We need to wrap this in a future too?
                        // Yes, stream::once(async { ... }) or stream::unfold?
                        // Actually, ReaderStream expects an AsyncRead.
                        // So we need an async block to OPEN the file, then return a stream.
                        
                        let state = state.clone();
                        let cache_path = cache_path.clone();
                        let library = library.clone();
                        let chapter_path = chapter.path.clone();
                        let encryption_key = state.encryption_key.clone();
                        
                        let future = async move {
                             let (reader, _) = if cache_path.exists() {
                                 let (reader, size) = state.storage_service.get_local_reader(&cache_path, Some((read_start, read_end))).await
                                     .map_err(|e| std::io::Error::new(std::io::ErrorKind::NotFound, e))?;
                                 (Box::new(reader.take(read_end - read_start)) as Box<dyn AsyncRead + Send + Unpin>, size)
                             } else if library.library_type == "local" {
                                 let (reader, size) = state.storage_service.get_local_reader(std::path::Path::new(&chapter_path), Some((read_start, read_end))).await
                                     .map_err(|e| std::io::Error::new(std::io::ErrorKind::NotFound, e))?;
                                 (Box::new(reader.take(read_end - read_start)) as Box<dyn AsyncRead + Send + Unpin>, size)
                             } else {
                                 let (reader, size) = state.storage_service.get_webdav_reader(&library, &chapter_path, Some((read_start, read_end)), encryption_key.as_ref()).await
                                     .map_err(|e| std::io::Error::new(std::io::ErrorKind::NotFound, e))?;
                                 (Box::new(reader.take(read_end - read_start)) as Box<dyn AsyncRead + Send + Unpin>, size)
                             };
                             
                             // Convert reader to stream
                             let stream = ReaderStream::new(reader)
                                 .map(|res| res.map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e)));
                                 
                             Ok(stream)
                        };
                        
                        // We have a Future that returns a Stream.
                        // We want a Stream that yields Bytes.
                        // stream::once(future) -> Stream<Item = Result<Stream<Bytes>>>
                        // We need to flatten this.
                        // stream::once(future).map_ok(|s| s).try_flatten()
                        
                        let stream = futures::stream::once(future)
                             .map(|res| match res {
                                 Ok(s) => s.boxed(),
                                 Err(e) => futures::stream::iter(vec![Err(e)]).boxed(),
                             })
                             .flatten();
                             
                        stream_chain.push(stream.boxed());
                    }
                }
            }
            current_pos += seg_len;
        }

        let final_stream = futures::stream::iter(stream_chain).flatten();
        let body = Body::from_stream(final_stream);

        // If no range header was requested, return 200 OK with chunked encoding (no Content-Length)
        // This avoids "unexpected end of stream" errors if our logic_size prediction is slightly off (e.g. padding)
        if range_header.is_none() {
             return Ok((
                StatusCode::OK,
                [
                    (header::CONTENT_TYPE, mime_type.to_string()),
                    (header::ACCEPT_RANGES, "bytes".to_string()),
                    (header::ACCESS_CONTROL_ALLOW_ORIGIN, "*".to_string()),
                    ("Cross-Origin-Resource-Policy".parse().unwrap(), "cross-origin".to_string()),
                ],
                body,
            ).into_response());
        }

        return Ok((
            StatusCode::PARTIAL_CONTENT,
            [
                (header::CONTENT_TYPE, mime_type.to_string()),
                (header::CONTENT_LENGTH, content_length.to_string()),
                (header::CONTENT_RANGE, format!("bytes {}-{}/{}", start, end - 1, logic_size)),
                (header::ACCEPT_RANGES, "bytes".to_string()),
                (header::ACCESS_CONTROL_ALLOW_ORIGIN, "*".to_string()),
                ("Cross-Origin-Resource-Policy".parse().unwrap(), "cross-origin".to_string()),
            ],
            body,
        ).into_response());

    }
    
    // Non-encrypted: Stream directly
    let range_header = headers.get(header::RANGE).and_then(|v| v.to_str().ok());
    
    // Parse range header manually since we don't have file size yet
    // We only support simple ranges "bytes=start-end" or "bytes=start-" for now
    let range = if let Some(r) = range_header {
        let r_str = r.replace("bytes=", "");
        let parts: Vec<&str> = r_str.split('-').collect();
        let start = parts[0].parse::<u64>().unwrap_or(0);
        let end = if parts.len() > 1 && !parts[1].is_empty() {
            parts[1].parse::<u64>().unwrap_or(0)
        } else {
            0
        };
        // storage_service expects (start, end) where end=0 means "until end of file"
        // But for get_local_reader, we need to handle limiting manually
        if end > 0 { Some((start, end + 1)) } else { Some((start, 0)) }
    } else {
        None
    };

    let (mut reader, total_size) = if library.library_type == "local" {
        let (f, size) = state.storage_service.get_local_reader(std::path::Path::new(&chapter.path), range).await
             .map_err(|e| TingError::NotFound(format!("Local file not found: {}", e)))?;
        (Box::new(f) as Box<dyn tokio::io::AsyncRead + Send + Unpin>, size)
    } else {
        state.storage_service.get_webdav_reader(&library, &chapter.path, range, state.encryption_key.as_ref()).await
             .map_err(|e| TingError::NotFound(format!("WebDAV file not found: {}", e)))?
    };

    // Calculate actual content length and range for response headers
    let start = range.map(|r| r.0).unwrap_or(0);
    let end = if let Some(r) = range {
        if r.1 > 0 { std::cmp::min(r.1, total_size) } else { total_size }
    } else {
        total_size
    };
    
    let content_length = end.saturating_sub(start);
    
    // For local files, we need to limit the reader if a specific end was requested
    if library.library_type == "local" && content_length < (total_size - start) {
        reader = Box::new(reader.take(content_length));
    }

    // Convert AsyncRead to Stream
    let stream = ReaderStream::new(reader);
    let body = Body::from_stream(stream);

    let mime_type = mime_guess::from_path(&chapter.path).first_or_octet_stream().to_string();

    if range_header.is_some() {
        let content_range = format!("bytes {}-{}/{}", start, end.saturating_sub(1), total_size);
        
        Ok((
            StatusCode::PARTIAL_CONTENT,
            [
                (header::CONTENT_TYPE, mime_type),
                (header::CONTENT_LENGTH, content_length.to_string()),
                (header::CONTENT_RANGE, content_range),
                (header::ACCEPT_RANGES, "bytes".to_string()),
                (header::ACCESS_CONTROL_ALLOW_ORIGIN, "*".to_string()),
                ("Cross-Origin-Resource-Policy".parse().unwrap(), "cross-origin".to_string()),
            ],
            body,
        ).into_response())
    } else {
        Ok((
            StatusCode::OK,
            [
                (header::CONTENT_TYPE, mime_type),
                (header::CONTENT_LENGTH, total_size.to_string()),
                (header::ACCEPT_RANGES, "bytes".to_string()),
                (header::ACCESS_CONTROL_ALLOW_ORIGIN, "*".to_string()),
                ("Cross-Origin-Resource-Policy".parse().unwrap(), "cross-origin".to_string()),
            ],
            body,
        ).into_response())
    }
}
