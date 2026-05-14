mod decrypt;

pub(crate) use decrypt::create_decrypted_stream;
use crate::core::error::{Result, TingError};
use crate::db::repository::Repository;
use crate::plugin::manager::FormatMethod;
use axum::{
    body::Body,
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
};
use crate::api::handlers::AppState;
use tokio::io::{AsyncReadExt, AsyncSeekExt};
use tokio_util::io::ReaderStream;
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

    // Handle .strm files (URL Redirect or Proxy)
    let ext = std::path::Path::new(&chapter.path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
        
    if ext == "strm" {
        // Read the URL from the file
        let url = if library.library_type == "local" {
            std::fs::read_to_string(&chapter.path)
                .map_err(|e| TingError::IoError(e))?
                .trim()
                .to_string()
        } else {
            // WebDAV library
            let (mut reader, _) = state.storage_service.get_webdav_reader(
                &library, 
                &chapter.path, 
                None, 
                state.encryption_key.as_ref()
            ).await.map_err(|e| TingError::NotFound(format!("Failed to read strm file: {}", e)))?;
            
            let mut content = String::new();
            reader.read_to_string(&mut content).await
                .map_err(|e| TingError::IoError(e))?;
            content.trim().to_string()
        };
        
        if url.is_empty() || !url.starts_with("http") {
            return Err(TingError::InvalidRequest(format!("Invalid strm file content: '{}'", url)));
        }
        
        tracing::info!("处理 strm 文件: {}", url);
        
        // Handle Transcoding Request for .strm files
        // Some strm URLs point to formats that browsers can't play (WMA, APE, etc.)
        // Frontend will automatically request transcoding when playback fails
        if let Some(format) = &params.transcode {
            tracing::info!("对 strm URL 进行转码: {} -> {}", url, format);
            
            let content_type = match format.as_str() {
                "mp3" => "audio/mpeg",
                "wav" => "audio/wav",
                _ => return Err(TingError::InvalidRequest("Unsupported transcode format".to_string())),
            };
            
            // Get FFmpeg path and derive FFprobe path (same directory)
            let ffmpeg_path = state.plugin_manager.get_ffmpeg_path().await
                .ok_or_else(|| TingError::IoError(std::io::Error::new(
                    std::io::ErrorKind::NotFound, 
                    "FFmpeg plugin binary not found"
                )))?;
            
            let ffprobe_path = {
                let ffmpeg_dir = std::path::Path::new(&ffmpeg_path).parent()
                    .ok_or_else(|| TingError::IoError(std::io::Error::new(
                        std::io::ErrorKind::NotFound,
                        "Cannot determine FFmpeg directory"
                    )))?;
                
                // ⭐ 跨平台：Windows 使用 ffprobe.exe，Linux/Mac 使用 ffprobe
                let ffprobe_name = if cfg!(target_os = "windows") {
                    "ffprobe.exe"
                } else {
                    "ffprobe"
                };
                
                ffmpeg_dir.join(ffprobe_name).to_string_lossy().to_string()
            };
            
            // 优先使用数据库中的时长，避免重复调用 FFprobe
            let duration_seconds = if let Some(db_duration) = chapter.duration {
                if db_duration > 0 {
                    tracing::debug!("使用数据库中的时长: {} 秒", db_duration);
                    Some(db_duration as f64)
                } else {
                    None
                }
            } else {
                None
            };
            
            // 只有在数据库中没有时长时才使用 FFprobe
            let duration_seconds = if duration_seconds.is_none() {
                tracing::info!("数据库中无时长信息，使用 FFprobe 获取音频时长...");
                
                // Add delay to avoid overwhelming the server
                tokio::time::sleep(std::time::Duration::from_millis(300)).await;
                
                let duration_output = Command::new(&ffprobe_path)
                    .arg("-v").arg("error")
                    .arg("-show_entries").arg("format=duration")
                    .arg("-of").arg("default=noprint_wrappers=1:nokey=1")
                    .arg(&url)
                    .output()
                    .await;
                
                if let Ok(output) = duration_output {
                    if output.status.success() {
                        let duration_str = String::from_utf8_lossy(&output.stdout);
                        let dur = duration_str.trim().parse::<f64>().ok();
                        
                        if let Some(d) = dur {
                            tracing::info!("FFprobe 获取到时长: {:.2} 秒", d);
                            
                            // 更新数据库中的时长
                            if let Ok(Some(mut chapter_record)) = state.chapter_repo.find_by_id(&chapter_id).await {
                                let new_duration = d.round() as i32;
                                tracing::info!("更新章节时长到数据库: {} 秒", new_duration);
                                chapter_record.duration = Some(new_duration);
                                let _ = state.chapter_repo.update(&chapter_record).await;
                            }
                        }
                        
                        dur
                    } else {
                        tracing::warn!("FFprobe 获取时长失败: {}", String::from_utf8_lossy(&output.stderr));
                        None
                    }
                } else {
                    tracing::warn!("无法运行 FFprobe");
                    None
                }
            } else {
                duration_seconds
            };
            
            tracing::info!("使用 FFmpeg 直接从 URL 读取: {}", ffmpeg_path);
            
            // Build FFmpeg command to transcode directly from URL
            // This allows seeking support
            let mut cmd = Command::new(&ffmpeg_path);
            cmd.arg("-y")
               .arg("-loglevel").arg("warning");
            
            // Add seek parameter if present (must be before -i for input seeking)
            if let Some(seek_time) = &params.seek {
                cmd.arg("-ss").arg(seek_time);
                tracing::info!("Seek 到位置: {}", seek_time);
            }
            
            // Use URL as input directly (FFmpeg will handle HTTP/HTTPS)
            cmd.arg("-i").arg(&url);
            
            // Add transcoding parameters
            if format == "mp3" {
                cmd.arg("-acodec").arg("libmp3lame")
                   .arg("-b:a").arg("128k")
                   .arg("-ac").arg("2")
                   .arg("-ar").arg("44100")
                   .arg("-vn")
                   .arg("-map").arg("0:a:0")
                   .arg("-f").arg("mp3");
            } else if format == "wav" {
                cmd.arg("-vn")
                   .arg("-map").arg("0:a:0")
                   .arg("-f").arg("wav");
            }
            
            cmd.arg("pipe:1");  // Output to stdout
            
            cmd.stdout(Stdio::piped());
            cmd.stderr(Stdio::piped());
            
            tracing::info!("启动 FFmpeg 进程（直接从 URL 读取）...");
            
            // Spawn FFmpeg process
            let mut child = cmd.spawn()
                .map_err(|e| TingError::IoError(e))?;
            
            let stdout = child.stdout.take()
                .ok_or_else(|| TingError::IoError(std::io::Error::new(
                    std::io::ErrorKind::Other, 
                    "Failed to capture ffmpeg stdout"
                )))?;
            
            let stderr = child.stderr.take();
            
            // Log FFmpeg errors
            if let Some(mut stderr) = stderr {
                tokio::spawn(async move {
                    let mut buffer = String::new();
                    use tokio::io::AsyncReadExt;
                    if let Ok(_) = stderr.read_to_string(&mut buffer).await {
                        if !buffer.is_empty() {
                            tracing::warn!("FFmpeg stderr: {}", buffer);
                        }
                    }
                });
            }
            
            // Create streaming response from FFmpeg stdout
            let stream = ReaderStream::new(stdout);
            let body = Body::from_stream(stream);
            
            // Build response with duration header if available
            if let Some(dur) = duration_seconds {
                return Ok((
                    StatusCode::OK,
                    [
                        (header::CONTENT_TYPE, content_type.to_string()),
                        (header::ACCESS_CONTROL_ALLOW_ORIGIN, "*".to_string()),
                        ("Cross-Origin-Resource-Policy".parse().unwrap(), "cross-origin".to_string()),
                        ("X-Audio-Duration".parse().unwrap(), dur.to_string()),
                    ],
                    body,
                ).into_response());
            } else {
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
        
        // Check if URL contains authentication (username:password@)
        // If it does, we need to proxy the request to avoid CORS issues
        let has_auth = url.contains("://") && url.split("://").nth(1).map(|s| s.contains('@')).unwrap_or(false);
        
        if has_auth {
            // Proxy the request through our server to strip authentication from URL
            tracing::info!("代理包含认证信息的 strm URL");
            
            let range_header = headers.get(header::RANGE).and_then(|v| v.to_str().ok());
            
            // Build request with authentication and browser-like headers
            let client = reqwest::Client::builder()
                .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .unwrap_or_else(|_| reqwest::Client::new());
            let mut req = client.get(&url)
                .header("Accept", "*/*")
                .header("Accept-Language", "zh-CN,zh;q=0.9,en;q=0.8")
                .header("Accept-Encoding", "gzip, deflate, br")
                .header("Connection", "keep-alive");
            
            // Forward range header if present (use string literal to avoid type conflicts)
            if let Some(range) = range_header {
                req = req.header("range", range);
            }
            
            // Make the request
            let response = req.send().await
                .map_err(|e| TingError::IoError(std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to fetch strm URL: {}", e))))?;
            
            let status = response.status();
            
            // Use string literals to avoid type conflicts between axum::http and reqwest::http
            let content_type = response.headers()
                .get("content-type")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("audio/mpeg")
                .to_string();
            
            let content_length = response.headers()
                .get("content-length")
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.parse::<u64>().ok());
            
            let content_range = response.headers()
                .get("content-range")
                .and_then(|v| v.to_str().ok())
                .map(|v| v.to_string());
            
            // Stream the response
            let stream = response.bytes_stream();
            let body = Body::from_stream(stream);
            
            // Build response with proper status code
            let response_status = if status == reqwest::StatusCode::PARTIAL_CONTENT {
                StatusCode::PARTIAL_CONTENT
            } else {
                StatusCode::OK
            };
            
            let response_builder = (
                response_status,
                [
                    (header::CONTENT_TYPE, content_type),
                    (header::ACCEPT_RANGES, "bytes".to_string()),
                    (header::ACCESS_CONTROL_ALLOW_ORIGIN, "*".to_string()),
                    ("Cross-Origin-Resource-Policy".parse().unwrap(), "cross-origin".to_string()),
                ],
            );
            
            // Add optional headers
            if let Some(cl) = content_length {
                if let Some(cr) = content_range {
                    return Ok((
                        response_builder.0,
                        [
                            response_builder.1[0].clone(),
                            response_builder.1[1].clone(),
                            response_builder.1[2].clone(),
                            response_builder.1[3].clone(),
                            (header::CONTENT_LENGTH, cl.to_string()),
                            (header::CONTENT_RANGE, cr),
                        ],
                        body,
                    ).into_response());
                } else {
                    return Ok((
                        response_builder.0,
                        [
                            response_builder.1[0].clone(),
                            response_builder.1[1].clone(),
                            response_builder.1[2].clone(),
                            response_builder.1[3].clone(),
                            (header::CONTENT_LENGTH, cl.to_string()),
                        ],
                        body,
                    ).into_response());
                }
            } else if let Some(cr) = content_range {
                return Ok((
                    response_builder.0,
                    [
                        response_builder.1[0].clone(),
                        response_builder.1[1].clone(),
                        response_builder.1[2].clone(),
                        response_builder.1[3].clone(),
                        (header::CONTENT_RANGE, cr),
                    ],
                    body,
                ).into_response());
            } else {
                return Ok((
                    response_builder.0,
                    response_builder.1,
                    body,
                ).into_response());
            }
        } else {
            // No authentication in URL, safe to redirect
            tracing::info!("重定向到 strm URL (无认证信息)");
            return Ok((
                StatusCode::FOUND,
                [(header::LOCATION, url)],
                Body::empty()
            ).into_response());
        }
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
        
        // Check if we can use direct URL transcoding (for WebDAV or cached files)
        let can_use_direct_url = library.library_type != "local" && !cache_path.exists();
        
        if can_use_direct_url {
            // WebDAV files: Use direct URL transcoding (same as STRM)
            // Build the WebDAV URL with authentication
            let mut webdav_url = if chapter.path.starts_with("http://") || chapter.path.starts_with("https://") {
                // Parse existing URL
                url::Url::parse(&chapter.path)
                    .map_err(|e| TingError::ValidationError(e.to_string()))?
            } else {
                // Construct URL from library config
                let base_url = url::Url::parse(&library.url)
                    .map_err(|e| TingError::ValidationError(e.to_string()))?;
                let mut url = base_url.clone();
                
                let root = library.root_path.as_str();
                let root = if root.is_empty() { "/" } else { root };
                let root_trimmed = root.trim_matches('/');
                let rel_trimmed = chapter.path.trim_matches('/');
                let full_path_str = if root_trimmed.is_empty() {
                    rel_trimmed.to_string()
                } else {
                    format!("{}/{}", root_trimmed, rel_trimmed)
                };
                
                let decoded_path = urlencoding::decode(&full_path_str)
                    .map_err(|e| TingError::ValidationError(e.to_string()))?;
                
                {
                    let mut segments = url.path_segments_mut()
                        .map_err(|_| TingError::ValidationError("Invalid URL".to_string()))?;
                    for segment in decoded_path.split('/') {
                        if !segment.is_empty() {
                            segments.push(segment);
                        }
                    }
                }
                
                url
            };
            
            // Add authentication to URL if present
            if let (Some(username), Some(password)) = (&library.username, &library.password) {
                let decrypted_password = crate::core::crypto::decrypt(password, state.encryption_key.as_ref())
                    .unwrap_or_else(|_| password.clone());
                webdav_url.set_username(username).ok();
                webdav_url.set_password(Some(&decrypted_password)).ok();
            }
            
            let webdav_url_str = webdav_url.to_string();
            
            tracing::info!("使用直接 URL 转码: {}", webdav_url_str);
            
            // Get FFmpeg path and derive FFprobe path (same directory)
            let ffmpeg_path = state.plugin_manager.get_ffmpeg_path().await
                .ok_or_else(|| TingError::IoError(std::io::Error::new(
                    std::io::ErrorKind::NotFound, 
                    "FFmpeg plugin binary not found"
                )))?;
            
            let ffprobe_path = {
                let ffmpeg_dir = std::path::Path::new(&ffmpeg_path).parent()
                    .ok_or_else(|| TingError::IoError(std::io::Error::new(
                        std::io::ErrorKind::NotFound,
                        "Cannot determine FFmpeg directory"
                    )))?;
                
                // ⭐ 跨平台：Windows 使用 ffprobe.exe，Linux/Mac 使用 ffprobe
                let ffprobe_name = if cfg!(target_os = "windows") {
                    "ffprobe.exe"
                } else {
                    "ffprobe"
                };
                
                ffmpeg_dir.join(ffprobe_name).to_string_lossy().to_string()
            };
            
            // Get duration using FFprobe
            
            // Add delay to avoid overwhelming the server
            tokio::time::sleep(std::time::Duration::from_millis(300)).await;
            
            let duration_output = Command::new(&ffprobe_path)
                .arg("-v").arg("error")
                .arg("-show_entries").arg("format=duration")
                .arg("-of").arg("default=noprint_wrappers=1:nokey=1")
                .arg(&webdav_url_str)
                .output()
                .await;
            
            let duration_seconds = if let Ok(output) = duration_output {
                if output.status.success() {
                    let duration_str = String::from_utf8_lossy(&output.stdout);
                    duration_str.trim().parse::<f64>().ok()
                } else {
                    None
                }
            } else {
                None
            };
            
            if let Some(dur) = duration_seconds {
                tracing::info!("音频时长: {:.2} 秒", dur);
                
                // Update chapter duration in database if significantly different
                if let Ok(Some(mut chapter_record)) = state.chapter_repo.find_by_id(&chapter_id).await {
                    let db_duration = chapter_record.duration.unwrap_or(0);
                    let new_duration = dur.round() as i32;
                    if (db_duration - new_duration).abs() > 2 {
                        tracing::info!("更新章节时长: {} -> {} 秒", db_duration, new_duration);
                        chapter_record.duration = Some(new_duration);
                        let _ = state.chapter_repo.update(&chapter_record).await;
                    }
                }
            }
            
            // Build FFmpeg command to transcode directly from URL
            let mut cmd = Command::new(&ffmpeg_path);
            cmd.arg("-y")
               .arg("-loglevel").arg("warning");
            
            // Add seek parameter if present (must be before -i for input seeking)
            if let Some(seek_time) = &params.seek {
                cmd.arg("-ss").arg(seek_time);
                tracing::info!("Seek 到位置: {}", seek_time);
            }
            
            // Use URL as input directly (FFmpeg will handle HTTP/HTTPS)
            cmd.arg("-i").arg(&webdav_url_str);
            
            // Add transcoding parameters
            if format == "mp3" {
                cmd.arg("-acodec").arg("libmp3lame")
                   .arg("-b:a").arg("128k")
                   .arg("-ac").arg("2")
                   .arg("-ar").arg("44100")
                   .arg("-vn")
                   .arg("-map").arg("0:a:0")
                   .arg("-f").arg("mp3");
            } else if format == "wav" {
                cmd.arg("-vn")
                   .arg("-map").arg("0:a:0")
                   .arg("-f").arg("wav");
            }
            
            cmd.arg("pipe:1");  // Output to stdout
            
            cmd.stdout(Stdio::piped());
            cmd.stderr(Stdio::piped());
            
            tracing::info!("启动 FFmpeg 进程（直接从 URL 读取）...");
            
            // Spawn FFmpeg process
            let mut child = cmd.spawn()
                .map_err(|e| TingError::IoError(e))?;
            
            let stdout = child.stdout.take()
                .ok_or_else(|| TingError::IoError(std::io::Error::new(
                    std::io::ErrorKind::Other, 
                    "Failed to capture ffmpeg stdout"
                )))?;
            
            let stderr = child.stderr.take();
            
            // Log FFmpeg errors
            if let Some(mut stderr) = stderr {
                tokio::spawn(async move {
                    let mut buffer = String::new();
                    use tokio::io::AsyncReadExt;
                    if let Ok(_) = stderr.read_to_string(&mut buffer).await {
                        if !buffer.is_empty() {
                            tracing::warn!("FFmpeg stderr: {}", buffer);
                        }
                    }
                });
            }
            
            // Create streaming response from FFmpeg stdout
            let stream = ReaderStream::new(stdout);
            let body = Body::from_stream(stream);
            
            // Build response with duration header if available
            if let Some(dur) = duration_seconds {
                return Ok((
                    StatusCode::OK,
                    [
                        (header::CONTENT_TYPE, content_type.to_string()),
                        (header::ACCESS_CONTROL_ALLOW_ORIGIN, "*".to_string()),
                        ("Cross-Origin-Resource-Policy".parse().unwrap(), "cross-origin".to_string()),
                        ("X-Audio-Duration".parse().unwrap(), dur.to_string()),
                    ],
                    body,
                ).into_response());
            } else {
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
        
        // Fallback: Use plugin or pipe-based transcoding for local/cached files
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

