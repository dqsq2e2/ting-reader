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
    Path(chapter_id): Path<String>,
) -> Result<impl IntoResponse> {
    
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
) -> Result<impl IntoResponse> {
    
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
    Path(chapter_id): Path<String>,
) -> Result<impl IntoResponse> {
    
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
) -> Result<impl IntoResponse> {
    
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
    
    let image_path = std::path::Path::new(&params.path);
    
    if !image_path.exists() {
        return Err(TingError::NotFound(format!("Cover image not found: {}", params.path)));
    }
    
    let image_data = tokio::fs::read(image_path).await?;
    
    let mime_type = mime_guess::from_path(image_path)
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

/// Query parameters for stream chapter
#[derive(Debug, serde::Deserialize)]
pub struct StreamQuery {
    pub token: Option<String>,
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

    // Auto Preload / Cache Logic
    if let Some(user) = user {
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
                            let state_clone = state.clone();
                            let next_chapter_id = next_chapter.id.clone();
                            let next_chapter_path = next_chapter.path.clone();
                            let lib_clone = library.clone();
                            
                            tokio::spawn(async move {
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
                                            let mut buf = Vec::new();
                                            if let Ok(_) = r.read_to_end(&mut buf).await {
                                                state_clone.preload_cache.write().await.insert(next_chapter_id.clone(), bytes::Bytes::from(buf.clone()));
                                                tracing::info!("Auto-preloaded next chapter: {}", next_chapter_id);
                                                
                                                // If auto_cache is also enabled, use the buffer to write to disk
                                                if auto_cache && lib_clone.library_type.to_lowercase() != "local" {
                                                    let cache_path = state_clone.cache_manager.get_cache_path(&next_chapter_id);
                                                    if !cache_path.exists() {
                                                        if let Ok(_) = tokio::fs::write(&cache_path, &buf).await {
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
                        }
                    }
                }
            }
        }
    }

    // 1. Check Preload Cache (Memory)
    {
        let cache = state.preload_cache.read().await;
        if let Some(data) = cache.get(&chapter_id) {
            tracing::info!(chapter_id = %chapter_id, "Serving from preload cache (memory)");
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

        // 5. Process Segments
        // We assume a simple structure: [Encrypted] [Plain]
        let mut decrypted_header = Vec::new();
        let mut plain_segment_offset = 0;
        
        for segment in plan.segments {
            match segment {
                DecryptionSegment::Encrypted { offset, length, params } => {
                    tracing::debug!("Decrypting segment: offset={}, length={}", offset, length);
                    // Download encrypted chunk
                    let (mut reader, _) = if cache_path.exists() {
                        let (reader, size) = state.storage_service.get_local_reader(&cache_path, Some((offset, offset + length as u64))).await?;
                        (Box::new(reader.take(length as u64)) as Box<dyn AsyncRead + Send + Unpin>, size)
                    } else if library.library_type == "local" {
                        let (reader, size) = state.storage_service.get_local_reader(std::path::Path::new(&chapter.path), Some((offset, offset + length as u64))).await?;
                        (Box::new(reader.take(length as u64)) as Box<dyn AsyncRead + Send + Unpin>, size)
                    } else {
                        let (reader, size) = state.storage_service.get_webdav_reader(&library, &chapter.path, Some((offset, offset + length as u64)), state.encryption_key.as_ref()).await?;
                        (Box::new(reader.take(length as u64)) as Box<dyn AsyncRead + Send + Unpin>, size)
                    };
                    
                    let mut chunk = Vec::new();
                    reader.read_to_end(&mut chunk).await?;
                    
                    // Decrypt in memory
                    let chunk_base64 = base64::engine::general_purpose::STANDARD.encode(&chunk);
                    let result_json = state.plugin_manager.call_format(
                        &plugin.id,
                        FormatMethod::DecryptChunk,
                        serde_json::json!({
                            "data_base64": chunk_base64,
                            "params": params
                        })
                    ).await.map_err(|e| {
                        tracing::error!("Decryption chunk call failed: {}", e);
                        TingError::PluginExecutionError(format!("Decryption failed: {}", e))
                    })?;
                    
                    let decrypted_base64 = result_json["data_base64"].as_str()
                        .ok_or_else(|| TingError::PluginExecutionError("Missing data_base64 in response".to_string()))?;
                    let decrypted = base64::engine::general_purpose::STANDARD.decode(decrypted_base64)
                        .map_err(|e| TingError::SerializationError(format!("Invalid base64: {}", e)))?;
                        
                    decrypted_header.extend(decrypted);
                },
                DecryptionSegment::Plain { offset, .. } => {
                    plain_segment_offset = offset;
                    // We only support one plain segment at the end for now
                    break;
                }
            }
        }

        // 6. Calculate Logic Size and Range
        let logic_size = decrypted_header.len() as u64 + (total_file_size - plain_segment_offset);
        tracing::info!("Stream prepared: logic_size={}, decrypted_header_len={}, plain_offset={}", logic_size, decrypted_header.len(), plain_segment_offset);
        
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

        // 7. Construct Response Stream
        // Header part
        let header_len = decrypted_header.len() as u64;
        let mut stream_chain = Vec::new();
        
        if start < header_len {
            let h_start = start as usize;
            let h_end = std::cmp::min(end, header_len) as usize;
            if h_start < h_end {
                let data = bytes::Bytes::from(decrypted_header[h_start..h_end].to_vec());
                stream_chain.push(futures::stream::iter(vec![Ok::<bytes::Bytes, std::io::Error>(data)]).boxed());
            }
        }
        
        // Body part
        if end > header_len {
            let req_start = plain_segment_offset + (std::cmp::max(start, header_len) - header_len);
            let req_end = plain_segment_offset + (end - header_len);
            
            tracing::debug!("Streaming body: req_start={}, req_end={}", req_start, req_end);

            let (reader, _) = if cache_path.exists() {
                let (reader, size) = state.storage_service.get_local_reader(&cache_path, Some((req_start, req_end))).await?;
                (Box::new(reader.take(req_end - req_start)) as Box<dyn AsyncRead + Send + Unpin>, size)
            } else if library.library_type == "local" {
                let (reader, size) = state.storage_service.get_local_reader(std::path::Path::new(&chapter.path), Some((req_start, req_end))).await?;
                (Box::new(reader.take(req_end - req_start)) as Box<dyn AsyncRead + Send + Unpin>, size)
            } else {
                let (reader, size) = state.storage_service.get_webdav_reader(&library, &chapter.path, Some((req_start, req_end)), state.encryption_key.as_ref()).await?;
                (Box::new(reader.take(req_end - req_start)) as Box<dyn AsyncRead + Send + Unpin>, size)
            };
            
            // Convert AsyncRead to Stream
            let stream = ReaderStream::new(reader).map(|res| res.map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e)));
            stream_chain.push(stream.boxed());
        }
        
        let final_stream = futures::stream::iter(stream_chain).flatten();
        let body = Body::from_stream(final_stream);

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
    let range = if let Some(r) = range_header {
        let r_str = r.replace("bytes=", "");
        let parts: Vec<&str> = r_str.split('-').collect();
        let start = parts[0].parse::<u64>().unwrap_or(0);
        let end = if parts.len() > 1 && !parts[1].is_empty() {
            parts[1].parse::<u64>().unwrap_or(0)
        } else {
            0
        };
        if end > 0 { Some((start, end + 1)) } else { Some((start, 0)) }
    } else {
        None
    };

    let (mut reader, content_length) = if library.library_type == "local" {
        let (f, size) = state.storage_service.get_local_reader(std::path::Path::new(&chapter.path), range).await
             .map_err(|e| TingError::NotFound(format!("Local file not found: {}", e)))?;
        (Box::new(f) as Box<dyn tokio::io::AsyncRead + Send + Unpin>, size)
    } else {
        state.storage_service.get_webdav_reader(&library, &chapter.path, range, state.encryption_key.as_ref()).await
             .map_err(|e| TingError::NotFound(format!("WebDAV file not found: {}", e)))?
    };

    let mut body = Vec::new();
    reader.read_to_end(&mut body).await?;
    
    Ok((
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, "audio/mpeg".to_string()),
            (header::CONTENT_LENGTH, content_length.to_string()),
            (header::ACCEPT_RANGES, "bytes".to_string()),
            (header::ACCESS_CONTROL_ALLOW_ORIGIN, "*".to_string()),
            ("Cross-Origin-Resource-Policy".parse().unwrap(), "cross-origin".to_string()),
        ],
        body,
    ).into_response())
}
