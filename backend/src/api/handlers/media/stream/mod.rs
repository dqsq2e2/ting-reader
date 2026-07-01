mod decrypt;
mod hls;
mod hls_serve;
mod hls_session;
mod preload;
mod strm;

use crate::api::handlers::AppState;
use crate::core::error::{Result, TingError};
use crate::db::models::{Chapter, Library};
use crate::db::repository::Repository;
use crate::plugin::manager::{FormatMethod, PluginInfo};
use axum::{
    body::Body,
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
};
pub(crate) use decrypt::create_decrypted_stream;
pub use hls::handle_hls_request;
pub use hls_serve::{get_hls_playlist, get_hls_segment, seek_hls_stream};
pub use hls_session::HlsSessionManager;
use std::process::Stdio;
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt};
use tokio::process::Command;
use tokio_util::io::ReaderStream;

/// Query parameters for stream chapter
#[derive(Debug, serde::Deserialize)]
pub struct StreamQuery {
    pub token: Option<String>,
    pub transcode: Option<String>,
    pub seek: Option<String>,
    pub download: Option<String>,
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
        _ => mime_guess::from_path(path)
            .first_or_octet_stream()
            .to_string(),
    }
}

fn is_download_query(params: &StreamQuery) -> bool {
    let Some(value) = params.download.as_deref() else {
        return false;
    };
    matches!(
        value.to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "on"
    )
}

async fn get_remote_media_reader(
    state: &AppState,
    library: &Library,
    path: &str,
    range: Option<(u64, u64)>,
) -> Result<(Box<dyn tokio::io::AsyncRead + Send + Unpin>, u64)> {
    if library.library_type == "webdav" {
        return state
            .storage_service
            .get_webdav_reader(library, path, range, state.encryption_key.as_ref())
            .await
            .map_err(|e| TingError::NotFound(format!("Remote WebDAV media not found: {}", e)));
    }

    if library.library_type == "rss" || path.starts_with("http://") || path.starts_with("https://")
    {
        return state
            .storage_service
            .get_http_reader(path, range)
            .await
            .map_err(|e| TingError::NotFound(format!("Remote media not found: {}", e)));
    }

    Err(TingError::ValidationError(format!(
        "Unsupported remote library type '{}'",
        library.library_type
    )))
}

async fn transcode_plugin_stream(
    state: &AppState,
    chapter: &Chapter,
    library: &Library,
    plugin: &PluginInfo,
    ffmpeg_path: &str,
    format: &str,
    content_type: &str,
    seek: Option<&str>,
) -> Result<axum::response::Response> {
    let (plugin_stream, _, _, _, _, _, _) =
        create_decrypted_stream(state, chapter, library, plugin, None).await?;

    let mut cmd = Command::new(ffmpeg_path);
    cmd.arg("-y").arg("-loglevel").arg("error");
    if let Some(seek_time) = seek {
        cmd.arg("-ss").arg(seek_time);
    }
    cmd.arg("-i").arg("pipe:0");

    if format == "mp3" {
        cmd.arg("-fflags")
            .arg("+genpts+igndts")
            .arg("-avoid_negative_ts")
            .arg("make_zero")
            .arg("-acodec")
            .arg("libmp3lame")
            .arg("-b:a")
            .arg("128k")
            .arg("-ac")
            .arg("2")
            .arg("-ar")
            .arg("44100")
            .arg("-vn")
            .arg("-map")
            .arg("0:a:0")
            .arg("-f")
            .arg("mp3");
    } else if format == "wav" {
        cmd.arg("-vn").arg("-map").arg("0:a:0").arg("-f").arg("wav");
    } else {
        cmd.arg("-f").arg(format);
    }

    cmd.arg("pipe:1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    tracing::info!(
        chapter_id = %chapter.id,
        plugin = %plugin.name,
        format = %format,
        "Using format plugin decoded stream for transcoded output"
    );

    let mut child = cmd.spawn().map_err(TingError::IoError)?;

    let mut stdin = child.stdin.take().ok_or_else(|| {
        TingError::IoError(std::io::Error::new(
            std::io::ErrorKind::Other,
            "Failed to capture ffmpeg stdin",
        ))
    })?;

    tokio::spawn(async move {
        use futures::StreamExt;

        let mut stream = plugin_stream;
        while let Some(chunk) = stream.next().await {
            match chunk {
                Ok(bytes) => {
                    if let Err(error) = stdin.write_all(&bytes).await {
                        tracing::debug!(
                            "Plugin decoded stream write to FFmpeg interrupted: {}",
                            error
                        );
                        break;
                    }
                }
                Err(error) => {
                    tracing::error!(
                        error = %error,
                        message_key = "media.plugin_stream.read_failed",
                        message_params = %serde_json::json!({ "error": error.to_string() }),
                        "Plugin decode stream read failed"
                    );
                    break;
                }
            }
        }
        let _ = stdin.shutdown().await;
    });

    if let Some(mut stderr) = child.stderr.take() {
        tokio::spawn(async move {
            let mut buffer = String::new();
            if stderr.read_to_string(&mut buffer).await.is_ok() && !buffer.is_empty() {
                tracing::warn!("FFmpeg stderr: {}", buffer);
            }
        });
    }

    let stdout = child.stdout.take().ok_or_else(|| {
        TingError::IoError(std::io::Error::new(
            std::io::ErrorKind::Other,
            "Failed to capture ffmpeg stdout",
        ))
    })?;

    let stream = ReaderStream::new(stdout);
    let body = Body::from_stream(stream);
    use axum::http::header;
    Ok((
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, content_type.to_string()),
            (header::ACCESS_CONTROL_ALLOW_ORIGIN, "*".to_string()),
            (
                "Cross-Origin-Resource-Policy".parse().unwrap(),
                "cross-origin".to_string(),
            ),
        ],
        body,
    )
        .into_response())
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

    if let Some(_token) = &params.token {
        // Token validation would go here
    }

    let is_head_request = method == axum::http::Method::HEAD;

    let chapter = state
        .chapter_repo
        .find_by_id(&chapter_id)
        .await?
        .ok_or_else(|| TingError::NotFound(format!("Chapter {} not found", chapter_id)))?;

    let book = state
        .book_repo
        .find_by_id(&chapter.book_id)
        .await?
        .ok_or_else(|| TingError::NotFound(format!("Book {} not found", chapter.book_id)))?;

    let library = state
        .library_repo
        .find_by_id(&book.library_id)
        .await?
        .ok_or_else(|| TingError::NotFound(format!("Library {} not found", book.library_id)))?;

    let ext = std::path::Path::new(&chapter.path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    let is_download_request = is_download_query(&params);

    // Handle .strm files (URL Redirect or Proxy)
    if ext == "strm" {
        return strm::handle_strm_stream(
            state,
            &chapter_id,
            chapter,
            book,
            library,
            &params,
            &headers,
        )
        .await;
    }
    // Handle HLS Transcoding Request
    if let Some(format) = &params.transcode {
        if format == "hls" {
            tracing::info!("Requested HLS transcoding: {}", chapter.path);
            return handle_hls_request(
                state,
                chapter,
                book,
                library,
                ext == "strm",
                params.seek.clone(),
            )
            .await;
        }
    }

    // Handle Transcoding Request
    if let Some(format) = &params.transcode {
        tracing::info!("Requested transcoding: {} -> {}", chapter.path, format);

        let content_type = match format.as_str() {
            "mp3" => "audio/mpeg",
            "wav" => "audio/wav",
            _ => {
                return Err(TingError::InvalidRequest(
                    "Unsupported transcode format".to_string(),
                ))
            }
        };

        let ffmpeg_path = state
            .plugin_manager
            .get_ffmpeg_path()
            .await
            .ok_or_else(|| {
                TingError::IoError(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    "FFmpeg plugin binary not found",
                ))
            })?;
        let cache_path = state.cache_manager.get_cache_path(&chapter_id);
        let plugin_info = state
            .plugin_manager
            .find_plugin_for_format(std::path::Path::new(&chapter.path))
            .await;

        // Check if we can use direct URL transcoding (for WebDAV or cached files).
        // Plugin-backed formats must be decoded/decrypted before FFmpeg sees them.
        let can_use_direct_url =
            library.library_type != "local" && !cache_path.exists() && plugin_info.is_none();

        if can_use_direct_url {
            // WebDAV files: Use direct URL transcoding (same as STRM)
            // Build the WebDAV URL with authentication
            let mut webdav_url =
                if chapter.path.starts_with("http://") || chapter.path.starts_with("https://") {
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
                        let mut segments = url
                            .path_segments_mut()
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
                let decrypted_password =
                    crate::core::crypto::decrypt(password, state.encryption_key.as_ref())
                        .unwrap_or_else(|_| password.clone());
                webdav_url.set_username(username).ok();
                webdav_url.set_password(Some(&decrypted_password)).ok();
            }

            let webdav_url_str = webdav_url.to_string();

            tracing::info!("Transcoding from direct URL: {}", webdav_url_str);

            let ffmpeg_tools = state
                .plugin_manager
                .get_ffmpeg_tool_paths()
                .await
                .ok_or_else(|| {
                    TingError::IoError(std::io::Error::new(
                        std::io::ErrorKind::NotFound,
                        "FFmpeg plugin binaries not found",
                    ))
                })?;

            // Get duration using FFprobe

            // Add delay to avoid overwhelming the server
            tokio::time::sleep(std::time::Duration::from_millis(300)).await;

            let duration_output = Command::new(&ffmpeg_tools.ffprobe)
                .arg("-v")
                .arg("error")
                .arg("-show_entries")
                .arg("format=duration")
                .arg("-of")
                .arg("default=noprint_wrappers=1:nokey=1")
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
                tracing::info!("Audio duration: {:.2} seconds", dur);

                // Update chapter duration in database if significantly different
                if let Ok(Some(mut chapter_record)) =
                    state.chapter_repo.find_by_id(&chapter_id).await
                {
                    let db_duration = chapter_record.duration.unwrap_or(0);
                    let new_duration = dur.round() as i32;
                    if (db_duration - new_duration).abs() > 2 {
                        tracing::info!(
                            "Updated chapter duration: {} -> {} seconds",
                            db_duration,
                            new_duration
                        );
                        chapter_record.duration = Some(new_duration);
                        let _ = state.chapter_repo.update(&chapter_record).await;
                    }
                }
            }

            // Build FFmpeg command to transcode directly from URL
            let mut cmd = Command::new(&ffmpeg_tools.ffmpeg);
            cmd.arg("-y").arg("-loglevel").arg("warning");

            // Add seek parameter if present (must be before -i for input seeking)
            if let Some(seek_time) = &params.seek {
                cmd.arg("-ss").arg(seek_time);
                tracing::info!("Seeking to position: {}", seek_time);
            }

            // Use URL as input directly (FFmpeg will handle HTTP/HTTPS)
            cmd.arg("-i").arg(&webdav_url_str);

            // Add transcoding parameters
            if format == "mp3" {
                cmd.arg("-fflags")
                    .arg("+genpts+igndts")
                    .arg("-avoid_negative_ts")
                    .arg("make_zero")
                    .arg("-acodec")
                    .arg("libmp3lame")
                    .arg("-b:a")
                    .arg("128k")
                    .arg("-ac")
                    .arg("2")
                    .arg("-ar")
                    .arg("44100")
                    .arg("-vn")
                    .arg("-map")
                    .arg("0:a:0")
                    .arg("-f")
                    .arg("mp3");
            } else if format == "wav" {
                cmd.arg("-vn").arg("-map").arg("0:a:0").arg("-f").arg("wav");
            }

            cmd.arg("pipe:1"); // Output to stdout

            cmd.stdout(Stdio::piped());
            cmd.stderr(Stdio::piped());

            tracing::info!("Starting FFmpeg process reading directly from URL...");

            // Spawn FFmpeg process
            let mut child = cmd.spawn().map_err(|e| TingError::IoError(e))?;

            let stdout = child.stdout.take().ok_or_else(|| {
                TingError::IoError(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "Failed to capture ffmpeg stdout",
                ))
            })?;

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
                        (
                            "Cross-Origin-Resource-Policy".parse().unwrap(),
                            "cross-origin".to_string(),
                        ),
                        ("X-Audio-Duration".parse().unwrap(), dur.to_string()),
                    ],
                    body,
                )
                    .into_response());
            } else {
                return Ok((
                    StatusCode::OK,
                    [
                        (header::CONTENT_TYPE, content_type.to_string()),
                        (header::ACCESS_CONTROL_ALLOW_ORIGIN, "*".to_string()),
                        (
                            "Cross-Origin-Resource-Policy".parse().unwrap(),
                            "cross-origin".to_string(),
                        ),
                    ],
                    body,
                )
                    .into_response());
            }
        }

        // Fallback: Use plugin or pipe-based transcoding for local/cached files
        // 1. Try to get transcode command from plugin
        let mut plugin_command: Option<Vec<String>> = None;

        if let Some(plugin) = &plugin_info {
            let res = state
                .plugin_manager
                .call_format(
                    &plugin.id,
                    FormatMethod::GetStreamUrl,
                    serde_json::json!({
                        "file_path": chapter.path,
                        "transcode": format,
                        "seek": params.seek,
                        "download": is_download_request
                    }),
                )
                .await;

            if let Ok(val) = res {
                if let Some(cmd) = val.get("command").and_then(|c| c.as_array()) {
                    let cmd_vec: Vec<String> = cmd
                        .iter()
                        .filter_map(|v| v.as_str())
                        .map(|s| s.to_string())
                        .collect();
                    if !cmd_vec.is_empty() {
                        plugin_command = Some(cmd_vec);
                        tracing::info!(
                            "Using plugin-provided transcode command for {}",
                            chapter.path
                        );
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
                    let (mut reader, _) =
                        get_remote_media_reader(&state, &library, &chapter.path, None).await?;

                    tokio::spawn(async move {
                        if let Err(e) = tokio::io::copy(&mut reader, &mut stdin).await {
                            tracing::error!(
                                error = %e,
                                message_key = "media.ffmpeg.pipe_failed",
                                message_params = %serde_json::json!({ "error": e.to_string() }),
                                "Failed to pipe input to FFmpeg"
                            );
                        }
                    });
                }
            }

            let stdout = child.stdout.take().ok_or_else(|| {
                TingError::IoError(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "Failed to capture ffmpeg stdout",
                ))
            })?;

            let stream = ReaderStream::new(stdout);
            let body = Body::from_stream(stream);

            return Ok((
                StatusCode::OK,
                [
                    (header::CONTENT_TYPE, content_type.to_string()),
                    (header::ACCESS_CONTROL_ALLOW_ORIGIN, "*".to_string()),
                    (
                        "Cross-Origin-Resource-Policy".parse().unwrap(),
                        "cross-origin".to_string(),
                    ),
                ],
                body,
            )
                .into_response());
        } else {
            if let Some(plugin) = &plugin_info {
                return transcode_plugin_stream(
                    &state,
                    &chapter,
                    &library,
                    plugin,
                    &ffmpeg_path,
                    format,
                    content_type,
                    params.seek.as_deref(),
                )
                .await;
            }

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
                cmd.arg("-fflags")
                    .arg("+genpts+igndts")
                    .arg("-avoid_negative_ts")
                    .arg("make_zero")
                    .arg("-acodec")
                    .arg("libmp3lame")
                    .arg("-b:a")
                    .arg("128k")
                    .arg("-ac")
                    .arg("2")
                    .arg("-ar")
                    .arg("44100")
                    .arg("-vn")
                    .arg("-map")
                    .arg("0:a:0");
            }

            cmd.arg("-f").arg(&format).arg("-");

            cmd.stdout(Stdio::piped());

            let mut child = cmd.spawn().map_err(|e| TingError::IoError(e))?;

            // Handle input pipe if needed (Only if we are using the fallback pipe logic)
            let use_pipe = !cache_path.exists() && library.library_type != "local";
            if use_pipe && child.stdin.is_some() {
                if let Some(mut stdin) = child.stdin.take() {
                    // Get reader
                    let (mut reader, _) =
                        get_remote_media_reader(&state, &library, &chapter.path, None).await?;

                    tokio::spawn(async move {
                        if let Err(e) = tokio::io::copy(&mut reader, &mut stdin).await {
                            tracing::error!(
                                error = %e,
                                message_key = "media.ffmpeg.pipe_failed",
                                message_params = %serde_json::json!({ "error": e.to_string() }),
                                "Failed to pipe input to FFmpeg"
                            );
                        }
                    });
                }
            }

            let stdout = child.stdout.take().ok_or_else(|| {
                TingError::IoError(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "Failed to capture ffmpeg stdout",
                ))
            })?;

            let stream = ReaderStream::new(stdout);
            let body = Body::from_stream(stream);

            return Ok((
                StatusCode::OK,
                [
                    (header::CONTENT_TYPE, content_type.to_string()),
                    (header::ACCESS_CONTROL_ALLOW_ORIGIN, "*".to_string()),
                    (
                        "Cross-Origin-Resource-Policy".parse().unwrap(),
                        "cross-origin".to_string(),
                    ),
                ],
                body,
            )
                .into_response());
        }
    }

    // ... existing code ...

    preload::maybe_spawn_auto_preload(&state, user.as_ref(), &book, &chapter_id, &library).await;
    // 1. Check Preload Cache (Memory)
    {
        let mut cache = state.preload_cache.write().await;
        if let Some((data, last_access)) = cache.get_mut(&chapter_id) {
            // Check if we need to use a format plugin even for cached files (source file is cached)
            let plugin_info = state
                .plugin_manager
                .find_plugin_for_format(std::path::Path::new(&chapter.path))
                .await;

            if plugin_info.is_some() {
                // If a plugin handles this format, we CANNOT use the preload cache directly if it contains encrypted data.
                // The current preload implementation stores raw bytes.
                // TODO: Implement decrypted preload cache or handle decryption here.
                // For now, skip preload cache for plugin-handled files to avoid sending encrypted data to client.
                tracing::info!(chapter_id = %chapter_id, "Skipping preload cache for plugin-processed file");
            } else {
                // Update access time to implement LRU (keep frequently accessed chapters in memory)
                *last_access = std::time::Instant::now();

                tracing::debug!(target: "media", chapter_id = %chapter_id, "Serving from preload cache (memory)");
                let data = data.clone(); // Clone bytes (cheap reference count increment)
                                         // Drop write lock early
                drop(cache);

                let file_size = data.len() as u64;
                let mime_type = stream_mime_type_from_path(&chapter.path);

                let range_header = headers.get(header::RANGE).and_then(|v| v.to_str().ok());

                if let Some(range_str) = range_header {
                    if let Ok(range) = state
                        .audio_streamer
                        .parse_range_header(range_str, file_size)
                    {
                        let start = range.start as usize;
                        let end = range.end as usize;
                        let content_length = (end - start) as u64;
                        let body = data[start..end].to_vec();

                        return Ok((
                            StatusCode::PARTIAL_CONTENT,
                            [
                                (header::CONTENT_TYPE, mime_type),
                                // (header::CONTENT_LENGTH, content_length.to_string()), // Removed to allow chunked transfer encoding for encrypted streams
                                (
                                    header::CONTENT_RANGE,
                                    format!("bytes {}-{}/{}", start, end - 1, file_size),
                                ),
                                (header::CONTENT_LENGTH, content_length.to_string()),
                                (header::ACCEPT_RANGES, "bytes".to_string()),
                                (header::ACCESS_CONTROL_ALLOW_ORIGIN, "*".to_string()),
                                (
                                    "Cross-Origin-Resource-Policy".parse().unwrap(),
                                    "cross-origin".to_string(),
                                ),
                            ],
                            body,
                        )
                            .into_response());
                    }
                }

                return Ok((
                    StatusCode::OK,
                    [
                        (header::CONTENT_TYPE, mime_type),
                        (header::CONTENT_LENGTH, file_size.to_string()),
                        (header::ACCEPT_RANGES, "bytes".to_string()),
                        (header::ACCESS_CONTROL_ALLOW_ORIGIN, "*".to_string()),
                        (
                            "Cross-Origin-Resource-Policy".parse().unwrap(),
                            "cross-origin".to_string(),
                        ),
                    ],
                    data.to_vec(),
                )
                    .into_response());
            }
        }
    }

    // 2. Check Disk Cache
    let cache_path = state.cache_manager.get_cache_path(&chapter_id);
    if cache_path.exists() {
        tracing::debug!(target: "media", chapter_id = %chapter_id, "Serving from disk cache");

        // Check if we need to use a format plugin even for cached files (source file is cached)
        let plugin_info = state
            .plugin_manager
            .find_plugin_for_format(std::path::Path::new(&chapter.path))
            .await;

        if let Some(plugin) = plugin_info {
            // If a plugin handles this format, we use the cached file as the source for the plugin logic
            // instead of serving it directly.
            tracing::info!(chapter_id = %chapter_id, plugin = %plugin.name, "Cached file requires format plugin processing");

            // Fall through to the plugin handling logic below
            // We need to make sure the logic below knows to use the cache_path as source
            // This is handled by the `if cache_path.exists()` checks in the plugin block
        } else {
            let file_size = tokio::fs::metadata(&cache_path).await?.len();
            let mime_type = stream_mime_type_from_path(&chapter.path);

            let range_header = headers.get(header::RANGE).and_then(|v| v.to_str().ok());
            if let Some(range_str) = range_header {
                if let Ok(range) = state
                    .audio_streamer
                    .parse_range_header(range_str, file_size)
                {
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
                            (
                                header::CONTENT_RANGE,
                                format!("bytes {}-{}/{}", range.start, range.end - 1, file_size),
                            ),
                            (header::ACCEPT_RANGES, "bytes".to_string()),
                            (header::ACCESS_CONTROL_ALLOW_ORIGIN, "*".to_string()),
                            (
                                "Cross-Origin-Resource-Policy".parse().unwrap(),
                                "cross-origin".to_string(),
                            ),
                        ],
                        buffer,
                    )
                        .into_response());
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
                    (
                        "Cross-Origin-Resource-Policy".parse().unwrap(),
                        "cross-origin".to_string(),
                    ),
                ],
                body,
            )
                .into_response());
        }
    }

    // 3. Not cached. Fetch from source.
    tracing::debug!(target: "media", chapter_id = %chapter_id, "Serving from source stream");

    // Determine if we need to use a format plugin
    // Instead of hardcoding extensions, we ask the plugin manager if any loaded plugin supports this extension
    let plugin_info = state
        .plugin_manager
        .find_plugin_for_format(std::path::Path::new(&chapter.path))
        .await;

    if let Some(plugin) = plugin_info {
        tracing::info!(chapter_id = %chapter_id, plugin = %plugin.name, "Processing file with format plugin");

        let range_header = headers.get(header::RANGE).and_then(|v| v.to_str().ok());
        let (stream, mime_type, output_extension, content_length, start, end, logic_size) =
            create_decrypted_stream(
                &state,
                &chapter,
                &library,
                &plugin,
                range_header.map(|s| s.to_string()),
            )
            .await?;
        let download_extension = output_extension.unwrap_or_else(|| {
            if mime_type.contains("mpeg") || mime_type.contains("mp3") {
                "mp3".to_string()
            } else if mime_type.contains("flac") {
                "flac".to_string()
            } else if mime_type.contains("ogg") {
                "ogg".to_string()
            } else if mime_type.contains("wav") {
                "wav".to_string()
            } else {
                "m4a".to_string()
            }
        });

        let body = Body::from_stream(stream);

        if range_header.is_some() {
            let end_inclusive = if end > 0 { end.saturating_sub(1) } else { 0 };
            return Ok((
                StatusCode::PARTIAL_CONTENT,
                [
                    (header::CONTENT_TYPE, mime_type.to_string()),
                    (header::CONTENT_LENGTH, content_length.to_string()),
                    (
                        header::CONTENT_RANGE,
                        format!("bytes {}-{}/{}", start, end_inclusive, logic_size),
                    ),
                    (header::ACCEPT_RANGES, "bytes".to_string()),
                    (
                        "X-Download-Extension".parse().unwrap(),
                        download_extension.clone(),
                    ),
                    (header::ACCESS_CONTROL_ALLOW_ORIGIN, "*".to_string()),
                    (
                        "Cross-Origin-Resource-Policy".parse().unwrap(),
                        "cross-origin".to_string(),
                    ),
                ],
                if is_head_request { Body::empty() } else { body },
            )
                .into_response());
        } else {
            return Ok((
                StatusCode::OK,
                [
                    (header::CONTENT_TYPE, mime_type.to_string()),
                    (header::CONTENT_LENGTH, content_length.to_string()),
                    (header::ACCEPT_RANGES, "bytes".to_string()),
                    ("X-Download-Extension".parse().unwrap(), download_extension),
                    (header::ACCESS_CONTROL_ALLOW_ORIGIN, "*".to_string()),
                    (
                        "Cross-Origin-Resource-Policy".parse().unwrap(),
                        "cross-origin".to_string(),
                    ),
                ],
                if is_head_request { Body::empty() } else { body },
            )
                .into_response());
        }
    }

    // Non-encrypted: Stream directly
    let range_header = headers.get(header::RANGE).and_then(|v| v.to_str().ok());

    // Local files know their size, so use the shared parser to support suffix ranges
    // like "bytes=-500". WebDAV keeps the lightweight open-ended parser because
    // the upstream server owns the final range handling.
    let range = if let Some(r) = range_header {
        if library.library_type == "local" {
            let file_size = tokio::fs::metadata(std::path::Path::new(&chapter.path))
                .await?
                .len();
            let parsed = state.audio_streamer.parse_range_header(r, file_size)?;
            Some((parsed.start, parsed.end))
        } else {
            let r_str = r.replace("bytes=", "");
            let parts: Vec<&str> = r_str.split('-').collect();
            let start = parts[0].parse::<u64>().unwrap_or(0);
            let end = if parts.len() > 1 && !parts[1].is_empty() {
                parts[1].parse::<u64>().unwrap_or(0)
            } else {
                0
            };
            // storage_service expects (start, end) where end=0 means "until end of file"
            if end > 0 {
                Some((start, end + 1))
            } else {
                Some((start, 0))
            }
        }
    } else {
        None
    };

    let (mut reader, total_size) = if library.library_type == "local" {
        let (f, size) = state
            .storage_service
            .get_local_reader(std::path::Path::new(&chapter.path), range)
            .await
            .map_err(|e| TingError::NotFound(format!("Local file not found: {}", e)))?;
        (
            Box::new(f) as Box<dyn tokio::io::AsyncRead + Send + Unpin>,
            size,
        )
    } else {
        get_remote_media_reader(&state, &library, &chapter.path, range).await?
    };

    // Calculate actual content length and range for response headers
    let start = range.map(|r| r.0).unwrap_or(0);
    let end = if let Some(r) = range {
        if r.1 > 0 {
            std::cmp::min(r.1, total_size)
        } else {
            total_size
        }
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
                (
                    "Cross-Origin-Resource-Policy".parse().unwrap(),
                    "cross-origin".to_string(),
                ),
            ],
            if is_head_request { Body::empty() } else { body },
        )
            .into_response())
    } else {
        Ok((
            StatusCode::OK,
            [
                (header::CONTENT_TYPE, mime_type),
                (header::CONTENT_LENGTH, total_size.to_string()),
                (header::ACCEPT_RANGES, "bytes".to_string()),
                (header::ACCESS_CONTROL_ALLOW_ORIGIN, "*".to_string()),
                (
                    "Cross-Origin-Resource-Policy".parse().unwrap(),
                    "cross-origin".to_string(),
                ),
            ],
            if is_head_request { Body::empty() } else { body },
        )
            .into_response())
    }
}
