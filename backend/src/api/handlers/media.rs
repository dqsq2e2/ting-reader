use crate::api::models::{
    CacheOperationResponse, CacheInfoResponse, CacheListResponse, ClearCacheResponse,
};
use crate::core::error::{Result, TingError};
use crate::db::repository::Repository;
use crate::db::models::{Chapter, Library};
use crate::plugin::types::{DecryptionPlan, DecryptionSegment};
use crate::plugin::manager::{FormatMethod, PluginInfo};
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
use std::process::Stdio;
use tokio::process::Command;

/// Query parameters for stream chapter
#[derive(Debug, serde::Deserialize)]
pub struct StreamQuery {
    pub token: Option<String>,
    pub transcode: Option<String>,
    pub seek: Option<String>,
}

fn stream_mime_type_from_path(path: &str) -> String {
    let ext = std::path::Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();

    match ext.as_str() {
        // WebKit prefers audio/mp4 for m4a/mp4 audio streams.
        "m4a" | "mp4" => "audio/mp4".to_string(),
        "mp3" => "audio/mpeg".to_string(),
        "aac" => "audio/aac".to_string(),
        "flac" => "audio/flac".to_string(),
        "ogg" => "audio/ogg".to_string(),
        "opus" => "audio/opus".to_string(),
        "wav" => "audio/wav".to_string(),
        _ => mime_guess::from_path(path).first_or_octet_stream().to_string(),
    }
}

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
                tracing::warn!("发现章节 {} 的孤立缓存文件。正在删除...", cache_info.chapter_id);
                if let Err(e) = state.cache_manager.delete_cache(&cache_info.chapter_id).await {
                     tracing::error!("删除孤立缓存 {} 失败: {}", cache_info.chapter_id, e);
                }
            },
            Err(e) => {
                tracing::error!("查找缓存 {} 的章节失败: {}", cache_info.chapter_id, e);
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

    // 如果是外部 URL（包含 http），并且带有 hash referer (由前端编码后传递过来的)
    if params.path.starts_with("http") {
        let mut target_url = params.path.clone();
        let mut referer = "".to_string();

        // 尝试解析 #referer=
        if let Some(idx) = target_url.find("#referer=") {
            referer = target_url[idx + 9..].to_string();
            target_url = target_url[..idx].to_string();
        }

        tracing::info!("代理外部图片请求: {}, referer: {}", target_url, referer);

        let client = reqwest::Client::new();
        let mut req = client.get(&target_url)
            .header(reqwest::header::USER_AGENT, "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36");

        if !referer.is_empty() {
            req = req.header(reqwest::header::REFERER, referer);
        }

        match req.send().await {
            Ok(resp) => {
                if resp.status().is_success() {
                    let content_type = resp.headers().get(reqwest::header::CONTENT_TYPE)
                        .and_then(|v| v.to_str().ok())
                        .unwrap_or("application/octet-stream")
                        .to_string();
                    
                    let bytes = resp.bytes().await.map_err(|e| TingError::IoError(std::io::Error::new(std::io::ErrorKind::Other, e)))?;
                    
                    return Ok((
                        StatusCode::OK,
                        [
                            (header::CONTENT_TYPE, content_type),
                            (header::ACCESS_CONTROL_ALLOW_ORIGIN, "*".to_string()),
                            (header::CACHE_CONTROL, "public, max-age=31536000".to_string()),
                            ("Cross-Origin-Resource-Policy".parse().unwrap(), "cross-origin".to_string()),
                        ],
                        bytes.to_vec(),
                    ).into_response());
                } else {
                    return Err(TingError::NotFound(format!("Failed to fetch external cover: HTTP {}", resp.status())));
                }
            }
            Err(e) => {
                return Err(TingError::NotFound(format!("Failed to fetch external cover: {}", e)));
            }
        }
    }
    
    // Normalize path separators (Windows compatibility)
    let normalized_path = params.path.replace('\\', "/");
    let image_path = std::path::Path::new(&normalized_path);
    
    tracing::info!("代理封面请求：原始='{}', 归一化='{}'", params.path, normalized_path);
    
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

/// Handler for GET /api/stream/:chapterId - Stream chapter audio
pub async fn stream_chapter(
    State(state): State<AppState>,
    Path(chapter_id): Path<String>,
    Query(params): Query<StreamQuery>,
    method: axum::http::Method,
    headers: axum::http::HeaderMap,
    user: Option<crate::auth::middleware::AuthUser>,
) -> Result<impl IntoResponse> {
    use axum::http::header;
    
    if let Some(_token) = params.token {
        // Token validation would go here
    }

    let is_head_request = method == axum::http::Method::HEAD;

    
    let chapter = state.chapter_repo.find_by_id(&chapter_id).await?
        .ok_or_else(|| TingError::NotFound(format!("Chapter {} not found", chapter_id)))?;
    
    let book = state.book_repo.find_by_id(&chapter.book_id).await?
        .ok_or_else(|| TingError::NotFound(format!("Book {} not found", chapter.book_id)))?;
        
    let library = state.library_repo.find_by_id(&book.library_id).await?
        .ok_or_else(|| TingError::NotFound(format!("Library {} not found", book.library_id)))?;

    if !is_head_request {
        let username = user.as_ref().map(|u| u.username.clone()).unwrap_or_else(|| "匿名用户".to_string());
        let book_title = book.title.clone().unwrap_or_default();
        let chapter_title = chapter.title.clone().unwrap_or_default();
        tracing::info!(
            target: "audit::playback",
            "用户 '{}' 开始播放书籍 '{}' 的章节 '{}'", username, book_title, chapter_title
        );
    }

    // Handle .strm files (URL Redirect)
    let ext = std::path::Path::new(&chapter.path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
        
    if ext == "strm" {
        // Read the URL from the file
        let url = std::fs::read_to_string(&chapter.path).unwrap_or_default().trim().to_string();
        if url.is_empty() || !url.starts_with("http") {
            return Err(TingError::InvalidRequest("Invalid strm file content".to_string()));
        }
        
        // Return 302 Redirect
        return Ok((
            StatusCode::FOUND,
            [(header::LOCATION, url)],
            Body::empty()
        ).into_response());
    }

    // Handle Transcoding Request
    if let Some(format) = &params.transcode {
        tracing::info!("请求转码: {} -> {}", chapter.path, format);

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
        
        if let Some(plugin) = &plugin_info {
            let res = state.plugin_manager.call_format(
                &plugin.id,
                FormatMethod::GetStreamUrl,
                serde_json::json!({
                    "file_path": chapter.path,
                    "transcode": format,
                    "seek": params.seek
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
                        tracing::info!("对 {} 使用插件提供的转码命令", chapter.path);
                    }
                }
            }
        }

        if let Some(cmd_vec) = plugin_command {
            let mut cmd = Command::new(&cmd_vec[0]);
            cmd.args(&cmd_vec[1..]);
            
            // If the plugin command uses "-" or "pipe:0" for input, we need to enable stdin pipe
            // The plugin should return "-" as input argument for piped input
            let use_pipe = !cache_path.exists() && library.library_type != "local";
            if use_pipe {
                cmd.stdin(Stdio::piped());
            }
            
            cmd.stdout(Stdio::piped());
            
            // Spawn
            let mut child = cmd.spawn().map_err(|e| TingError::IoError(e))?;
            
            // Handle input pipe if needed (Only if we are using the fallback pipe logic)
            if use_pipe && child.stdin.is_some() {
                if let Some(mut stdin) = child.stdin.take() {
                    // Get reader
                    let (mut reader, _) = state.storage_service.get_webdav_reader(&library, &chapter.path, None, state.encryption_key.as_ref()).await
                        .map_err(|e| TingError::NotFound(format!("WebDAV file not found: {}", e)))?;
                        
                    tokio::spawn(async move {
                        if let Err(e) = tokio::io::copy(&mut reader, &mut stdin).await {
                             tracing::error!("无法将输入通过管道传输到 ffmpeg: {}", e);
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
        } else {
            let mut goto_standard_stream = false;
            if let Some(plugin) = &plugin_info {
                 let has_ffmpeg_utils = plugin.dependencies.iter().any(|d| d.plugin_name == "ffmpeg-utils");
                 if !has_ffmpeg_utils {
                     // Skip FFmpeg transcoding for plugins that don't depend on it (e.g. xm-format)
                     // We will fall through to the standard streaming logic which uses the plugin to decrypt/decode.
                     tracing::info!("跳过插件 '{}' 的 FFmpeg 转码（原生支持）", plugin.name);
                     goto_standard_stream = true;
                 }
            }
            
            if !goto_standard_stream {
                // Fallback to hardcoded logic
                let mut cmd = Command::new(&ffmpeg_path);
                cmd.arg("-y").arg("-loglevel").arg("error");

                if let Some(seek_time) = &params.seek {
                    cmd.arg("-ss").arg(seek_time);
                }

                cmd.arg("-i");
                    
                // Input Source
                if cache_path.exists() {
                    cmd.arg(cache_path.to_string_lossy().as_ref());
                } else if library.library_type == "local" {
                    cmd.arg(&chapter.path);
                } else {
                    // Pipe input
                    cmd.arg("-");
                    cmd.stdin(Stdio::piped());
                }
                
                if format == "mp3" {
                    cmd.arg("-acodec").arg("libmp3lame")
                       .arg("-b:a").arg("128k")
                       .arg("-ac").arg("2")
                       .arg("-ar").arg("44100")
                       .arg("-vn")
                       .arg("-map").arg("0:a:0");
                }

                cmd.arg("-f").arg(&format).arg("-");
                
                cmd.stdout(Stdio::piped());
                
                let mut child = cmd.spawn().map_err(|e| TingError::IoError(e))?;
                
                // Handle input pipe if needed (Only if we are using the fallback pipe logic)
                let use_pipe = !cache_path.exists() && library.library_type != "local";
                if use_pipe && child.stdin.is_some() {
                    if let Some(mut stdin) = child.stdin.take() {
                        // Get reader
                        let (mut reader, _) = state.storage_service.get_webdav_reader(&library, &chapter.path, None, state.encryption_key.as_ref()).await
                            .map_err(|e| TingError::NotFound(format!("WebDAV file not found: {}", e)))?;
                            
                        tokio::spawn(async move {
                            if let Err(e) = tokio::io::copy(&mut reader, &mut stdin).await {
                                 tracing::error!("无法将输入通过管道传输到 ffmpeg: {}", e);
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
        }
    }
    
    // ... existing code ...


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
                                    tracing::debug!("已取消用户 {} 之前的预加载任务", user_id);
                                }
                            }
                            
                            let handle = tokio::spawn(async move {
                                // Check if already in cache BEFORE starting any heavy work
                                if auto_preload {
                                    let cache = state_clone.preload_cache.read().await;
                                    if cache.contains_key(&next_chapter_id) {
                                        tracing::debug!("跳过 {} 的自动预加载 - 已在缓存中", next_chapter_id);
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
                                                    tracing::debug!("跳过 {} 的自动预加载 - 已在缓存中 (二次检查)", next_chapter_id);
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
                                                            tracing::debug!("已从内存中驱逐最旧的预加载章节: {}", key);
                                                        }
                                                    }
                                                    
                                                    cache.insert(next_chapter_id.clone(), (bytes_data.clone(), std::time::Instant::now()));
                                                }
                                                tracing::info!("已自动预加载下一章: {}", next_chapter_id);
                                                
                                                // If auto_cache is also enabled, use the buffer to write to disk
                                                if auto_cache && lib_clone.library_type.to_lowercase() != "local" {
                                                    let cache_path = state_clone.cache_manager.get_cache_path(&next_chapter_id);
                                                    if !cache_path.exists() {
                                                        // Use temp file to ensure atomicity and prevent race conditions
                                                        let temp_path = cache_path.with_extension("tmp");
                                                        if let Ok(_) = tokio::fs::write(&temp_path, &bytes_data).await {
                                                            if let Ok(_) = tokio::fs::rename(&temp_path, &cache_path).await {
                                                                tracing::info!("已自动缓存下一章 (从缓冲区): {}", next_chapter_id);
                                                                
                                                                // Enforce limits
                                                                let config = state_clone.config.read().await;
                                                                let _ = state_clone.cache_manager.enforce_limits(50, config.storage.max_disk_usage).await;
                                                            } else {
                                                                tracing::error!("重命名章节 {} 的临时缓存文件失败", next_chapter_id);
                                                                let _ = tokio::fs::remove_file(&temp_path).await;
                                                            }
                                                        } else {
                                                            tracing::error!("从缓冲区为章节 {} 写入临时缓存文件失败", next_chapter_id);
                                                        }
                                                    }
                                                }
                                            } else {
                                                tracing::error!("读取下一章预加载失败: {}", next_chapter_id);
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
                                                                    tracing::info!("已自动缓存下一章 (流式): {}", next_chapter_id);
                                                                    
                                                                    // Enforce limits
                                                                    let config = state_clone.config.read().await;
                                                                    let _ = state_clone.cache_manager.enforce_limits(50, config.storage.max_disk_usage).await;
                                                                } else {
                                                                    tracing::error!("重命名章节 {} 的临时缓存文件失败", next_chapter_id);
                                                                }
                                                             },
                                                             Err(e) => {
                                                                 tracing::error!("自动缓存的流复制失败: {} - {}", next_chapter_id, e);
                                                                 let _ = tokio::fs::remove_file(&temp_path).await;
                                                             }
                                                         }
                                                     },
                                                     Err(e) => {
                                                         tracing::error!("创建临时缓存文件失败: {} - {}", next_chapter_id, e);
                                                     }
                                                 }
                                            }
                                        }
                                    },
                                    Err(e) => {
                                        tracing::error!("获取下一章 {} 的读取器失败: {}", next_chapter_id, e);
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
            // Check if we need to use a format plugin even for cached files (source file is cached)
            let plugin_info = state.plugin_manager.find_plugin_for_format(std::path::Path::new(&chapter.path)).await;
            
            if plugin_info.is_some() {
                // If a plugin handles this format, we CANNOT use the preload cache directly if it contains encrypted data.
                // The current preload implementation stores raw bytes.
                // TODO: Implement decrypted preload cache or handle decryption here.
                // For now, skip preload cache for plugin-handled files to avoid sending encrypted data to client.
                tracing::info!(chapter_id = %chapter_id, "跳过插件处理文件的预加载缓存");
            } else {
                // Update access time to implement LRU (keep frequently accessed chapters in memory)
                *last_access = std::time::Instant::now();
                
                tracing::debug!(target: "media", chapter_id = %chapter_id, "从预加载缓存 (内存) 提供服务");
                let data = data.clone(); // Clone bytes (cheap reference count increment)
                // Drop write lock early
                drop(cache);
                
                let file_size = data.len() as u64;
                let mime_type = stream_mime_type_from_path(&chapter.path);
                
                let range_header = headers.get(header::RANGE).and_then(|v| v.to_str().ok());
                
                if let Some(range_str) = range_header {
                     if let Ok(range) = state.audio_streamer.parse_range_header(range_str, file_size) {
                         let start = range.start as usize;
                         let end = range.end as usize;
                         let _content_length = (end - start) as u64;
                         let body = data[start..end].to_vec();
                         
                         return Ok((
                            StatusCode::PARTIAL_CONTENT,
                            [
                                (header::CONTENT_TYPE, mime_type),
                                // (header::CONTENT_LENGTH, content_length.to_string()), // Removed to allow chunked transfer encoding for encrypted streams
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
    }
    
    // 2. Check Disk Cache
        let cache_path = state.cache_manager.get_cache_path(&chapter_id);
        if cache_path.exists() {
            tracing::debug!(target: "media", chapter_id = %chapter_id, "从磁盘缓存提供服务");
            
            // Check if we need to use a format plugin even for cached files (source file is cached)
            let plugin_info = state.plugin_manager.find_plugin_for_format(std::path::Path::new(&chapter.path)).await;
            
            if let Some(plugin) = plugin_info {
                // If a plugin handles this format, we use the cached file as the source for the plugin logic
                // instead of serving it directly.
                tracing::info!(chapter_id = %chapter_id, plugin = %plugin.name, "缓存文件需要格式插件处理");
                
                // Fall through to the plugin handling logic below
                // We need to make sure the logic below knows to use the cache_path as source
                // This is handled by the `if cache_path.exists()` checks in the plugin block
            } else {
                let file_size = tokio::fs::metadata(&cache_path).await?.len();
                let mime_type = stream_mime_type_from_path(&chapter.path);
                
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

                let file = tokio::fs::File::open(&cache_path).await?;
                let stream = ReaderStream::new(file);
                let body = Body::from_stream(stream);
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
    tracing::debug!(target: "media", chapter_id = %chapter_id, "从源 (流) 提供服务");
    
    // Determine if we need to use a format plugin
    // Instead of hardcoding extensions, we ask the plugin manager if any loaded plugin supports this extension
    let plugin_info = state.plugin_manager.find_plugin_for_format(std::path::Path::new(&chapter.path)).await;
    
    if let Some(plugin) = plugin_info {
        tracing::info!(chapter_id = %chapter_id, plugin = %plugin.name, "使用格式插件处理文件");

        let range_header = headers.get(header::RANGE).and_then(|v| v.to_str().ok());
        let (stream, mime_type, content_length, start, end, logic_size) = create_decrypted_stream(
            &state, &chapter, &library, &plugin, range_header.map(|s| s.to_string())
        ).await?;

    let body = Body::from_stream(stream);
    
    if range_header.is_some() {
        let end_inclusive = if end > 0 { end.saturating_sub(1) } else { 0 };
        return Ok((
            StatusCode::PARTIAL_CONTENT,
            [
                (header::CONTENT_TYPE, mime_type.to_string()),
                (header::CONTENT_LENGTH, content_length.to_string()),
                (header::CONTENT_RANGE, format!("bytes {}-{}/{}", start, end_inclusive, logic_size)),
                (header::ACCEPT_RANGES, "bytes".to_string()),
                (header::ACCESS_CONTROL_ALLOW_ORIGIN, "*".to_string()),
                ("Cross-Origin-Resource-Policy".parse().unwrap(), "cross-origin".to_string()),
            ],
            if is_head_request { Body::empty() } else { body },
        ).into_response());
    } else {
             return Ok((
                StatusCode::OK,
                [
                    (header::CONTENT_TYPE, mime_type.to_string()),
                    (header::CONTENT_LENGTH, content_length.to_string()),
                    (header::ACCEPT_RANGES, "bytes".to_string()),
                    (header::ACCESS_CONTROL_ALLOW_ORIGIN, "*".to_string()),
                    ("Cross-Origin-Resource-Policy".parse().unwrap(), "cross-origin".to_string()),
                ],
                if is_head_request { Body::empty() } else { body },
            ).into_response());
        }
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

    let mime_type = stream_mime_type_from_path(&chapter.path);

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
            if is_head_request { Body::empty() } else { body },
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
            if is_head_request { Body::empty() } else { body },
        ).into_response())
    }
}

/// Helper to create a decrypted stream for a file using the specified plugin
async fn create_decrypted_stream(
    state: &AppState,
    chapter: &Chapter,
    library: &Library,
    plugin: &PluginInfo,
    range_header: Option<String>,
) -> Result<(futures::stream::BoxStream<'static, std::io::Result<bytes::Bytes>>, String, u64, u64, u64, u64)> {
    use base64::Engine;
    use tokio::io::AsyncReadExt;
    
    let cache_path = state.cache_manager.get_cache_path(&chapter.id);
    
    // 1. Read minimal header probe
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
    probe_reader.read_to_end(&mut probe_bytes).await.map_err(TingError::IoError)?;
    
    // 2. Ask plugin for required header size
    let probe_base64 = base64::engine::general_purpose::STANDARD.encode(&probe_bytes);
    let size_json = state.plugin_manager.call_format(
        &plugin.id,
        FormatMethod::GetMetadataReadSize,
        serde_json::json!({"header_base64": probe_base64})
    ).await.map_err(|e| {
        tracing::error!("获取元数据读取大小失败: {}", e);
        TingError::PluginExecutionError(format!("获取元数据读取大小失败: {}", e))
    })?;
    
    let header_size = size_json["size"].as_u64().unwrap_or(8192);

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
    header_reader.read_to_end(&mut header_bytes).await.map_err(TingError::IoError)?;

    // 4. Get Decryption Plan
    let header_base64 = base64::engine::general_purpose::STANDARD.encode(&header_bytes);
    let plan_json = state.plugin_manager.call_format(
        &plugin.id, 
        FormatMethod::GetDecryptionPlan, 
        serde_json::json!({"header_base64": header_base64})
    ).await.map_err(|e| {
        tracing::error!("获取解密计划失败: {}", e);
        TingError::PluginExecutionError(format!("获取解密计划失败: {}", e))
    })?;
    
    let plan: DecryptionPlan = serde_json::from_value(plan_json)
        .map_err(|e| TingError::SerializationError(format!("Invalid decryption plan: {}", e)))?;

    let mime_type = "audio/mp4".to_string();

    // 5. Calculate Logic Size and Resolve Encrypted Segments
    let mut resolved_segments = Vec::new();
    let mut logic_size = 0;

    for segment in plan.segments {
        match segment {
            DecryptionSegment::Encrypted { offset, length, params } => {
                // Fetch and decrypt eagerly
                let (mut reader, _) = if cache_path.exists() {
                    let (reader, _) = state.storage_service.get_local_reader(&cache_path, Some((offset, offset + length as u64))).await
                        .map_err(|e| TingError::NotFound(format!("Cached file not found: {}", e)))?;
                    (Box::new(reader.take(length as u64)) as Box<dyn AsyncRead + Send + Unpin>, 0)
                } else if library.library_type == "local" {
                    let (reader, _) = state.storage_service.get_local_reader(std::path::Path::new(&chapter.path), Some((offset, offset + length as u64))).await
                        .map_err(|e| TingError::NotFound(format!("Local file not found: {}", e)))?;
                    (Box::new(reader.take(length as u64)) as Box<dyn AsyncRead + Send + Unpin>, 0)
                } else {
                    let (reader, _) = state.storage_service.get_webdav_reader(&library, &chapter.path, Some((offset, offset + length as u64)), state.encryption_key.as_ref()).await
                        .map_err(|e| TingError::NotFound(format!("WebDAV file not found: {}", e)))?;
                    (Box::new(reader.take(length as u64)) as Box<dyn AsyncRead + Send + Unpin>, 0)
                };
                
                let mut encrypted_data = Vec::with_capacity(length as usize);
                reader.read_to_end(&mut encrypted_data).await.map_err(TingError::IoError)?;
                
                let chunk_base64 = base64::engine::general_purpose::STANDARD.encode(&encrypted_data);
                
                let result_json = state.plugin_manager.call_format(
                    &plugin.id,
                    FormatMethod::DecryptChunk,
                    serde_json::json!({
                        "data_base64": chunk_base64,
                        "params": params
                    })
                ).await.map_err(|e| TingError::PluginExecutionError(e.to_string()))?;
                
                let decrypted_base64 = result_json["data_base64"].as_str()
                    .ok_or_else(|| TingError::PluginExecutionError("Missing data_base64".to_string()))?;
                    
                let decrypted = base64::engine::general_purpose::STANDARD.decode(decrypted_base64)
                    .map_err(|e| TingError::PluginExecutionError(e.to_string()))?;
                    
                let dec_len = decrypted.len() as u64;
                resolved_segments.push((bytes::Bytes::from(decrypted), None, dec_len));
                logic_size += dec_len;
            },
            DecryptionSegment::Plain { length, offset } => {
                let p_len = if length <= 0 {
                    total_file_size.saturating_sub(offset)
                } else {
                    length as u64
                };
                resolved_segments.push((bytes::Bytes::new(), Some(offset), p_len));
                logic_size += p_len;
            }
        }
    }
    
    if let Some(s) = plan.total_size {
        logic_size = s;
    }
    
    // Parse Range
    let (start, end) = if let Some(r_str) = range_header {
         if let Ok(range) = state.audio_streamer.parse_range_header(&r_str, logic_size) {
             (range.start, range.end)
         } else {
             (0, logic_size)
         }
    } else {
        (0, logic_size)
    };
    
    let content_length = end.saturating_sub(start);

    // 6. Construct Lazy Stream Chain
    let mut stream_chain: Vec<futures::stream::BoxStream<'static, std::result::Result<bytes::Bytes, std::io::Error>>> = Vec::new();
    let mut current_pos = 0;

    for (data, plain_offset, seg_len) in resolved_segments {
        let seg_start = current_pos;
        let seg_end = current_pos + seg_len;
        
        if seg_end > start && seg_start < end {
            let req_seg_start = std::cmp::max(start, seg_start);
            let req_seg_end = std::cmp::min(end, seg_end);
            
            let relative_start = req_seg_start - seg_start;
            let relative_end = req_seg_end - seg_start;

            if let Some(offset) = plain_offset {
                let read_start = offset + relative_start;
                let read_end = offset + relative_end;
                
                let state = state.clone();
                let cache_path = cache_path.clone();
                let library = library.clone();
                let chapter_path = chapter.path.clone();
                let encryption_key = state.encryption_key.clone();
                
                let future = async move {
                     let (reader, _) = if cache_path.exists() {
                         let (reader, _) = state.storage_service.get_local_reader(&cache_path, Some((read_start, read_end))).await
                             .map_err(|e| std::io::Error::new(std::io::ErrorKind::NotFound, e))?;
                         (Box::new(reader.take(read_end - read_start)) as Box<dyn AsyncRead + Send + Unpin>, 0)
                     } else if library.library_type == "local" {
                         let (reader, _) = state.storage_service.get_local_reader(std::path::Path::new(&chapter_path), Some((read_start, read_end))).await
                             .map_err(|e| std::io::Error::new(std::io::ErrorKind::NotFound, e))?;
                         (Box::new(reader.take(read_end - read_start)) as Box<dyn AsyncRead + Send + Unpin>, 0)
                     } else {
                         let (reader, _) = state.storage_service.get_webdav_reader(&library, &chapter_path, Some((read_start, read_end)), encryption_key.as_ref()).await
                             .map_err(|e| std::io::Error::new(std::io::ErrorKind::NotFound, e))?;
                         (Box::new(reader.take(read_end - read_start)) as Box<dyn AsyncRead + Send + Unpin>, 0)
                     };
                     
                     let stream = ReaderStream::new(reader)
                         .map(|res| res.map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e)));
                         
                     Ok(stream)
                };
                
                let stream = futures::stream::once(future)
                     .map(|res| match res {
                         Ok(s) => s.boxed(),
                         Err(e) => futures::stream::iter(vec![Err(e)]).boxed(),
                     })
                     .flatten();
                     
                stream_chain.push(stream.boxed());
            } else {
                let slice_start = relative_start as usize;
                let slice_end = std::cmp::min(data.len(), relative_end as usize);
                let slice = data.slice(slice_start..slice_end);
                let future = async move { Ok(slice) };
                stream_chain.push(futures::stream::once(future).boxed());
            }
        }
        
        current_pos += seg_len;
    }

    let stream = futures::stream::iter(stream_chain).flatten();
    
    // Wrap with padding to ensure Content-Length is satisfied
    // This is crucial for browsers (Chrome/Edge) to support seeking
    // even if the decrypted size is slightly smaller than calculated logic_size
    let padded_stream = PaddedStream {
        inner: Box::pin(stream),
        remaining_pad: content_length,
    };
    
    Ok((Box::pin(padded_stream), mime_type, content_length, start, end, logic_size))
}

struct PaddedStream {
    inner: futures::stream::BoxStream<'static, std::io::Result<bytes::Bytes>>,
    remaining_pad: u64,
}

impl futures::Stream for PaddedStream {
    type Item = std::io::Result<bytes::Bytes>;

    fn poll_next(mut self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<Option<Self::Item>> {
        match self.inner.as_mut().poll_next(cx) {
            std::task::Poll::Ready(Some(Ok(bytes))) => {
                let len = bytes.len() as u64;
                if len > 0 {
                    if self.remaining_pad >= len {
                        self.remaining_pad -= len;
                    } else {
                        self.remaining_pad = 0;
                    }
                }
                std::task::Poll::Ready(Some(Ok(bytes)))
            }
            std::task::Poll::Ready(Some(Err(e))) => std::task::Poll::Ready(Some(Err(e))),
            std::task::Poll::Ready(None) => {
                if self.remaining_pad > 0 {
                    // Pad with zeros
                    let chunk_size = std::cmp::min(self.remaining_pad, 8192);
                    self.remaining_pad -= chunk_size;
                    std::task::Poll::Ready(Some(Ok(bytes::Bytes::from(vec![0u8; chunk_size as usize]))))
                } else {
                    std::task::Poll::Ready(None)
                }
            }
            std::task::Poll::Pending => std::task::Poll::Pending,
        }
    }
}
