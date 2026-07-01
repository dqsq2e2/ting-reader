use super::{handle_hls_request, StreamQuery};
use crate::api::handlers::AppState;
use crate::core::error::{Result, TingError};
use crate::db::models::{Book, Chapter, Library};
use crate::db::repository::Repository;
use axum::{
    body::Body,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use std::process::Stdio;
use tokio::io::AsyncReadExt;
use tokio::process::Command;
use tokio_util::io::ReaderStream;

pub(super) async fn handle_strm_stream(
    state: AppState,
    chapter_id: &str,
    chapter: Chapter,
    book: Book,
    library: Library,
    params: &StreamQuery,
    headers: &axum::http::HeaderMap,
) -> Result<Response> {
    use axum::http::header;
    // Read the URL from the file
    let url = if library.library_type == "local" {
        std::fs::read_to_string(&chapter.path)
            .map_err(|e| TingError::IoError(e))?
            .trim()
            .to_string()
    } else if library.library_type == "webdav" {
        // WebDAV library
        let (mut reader, _) = state
            .storage_service
            .get_webdav_reader(&library, &chapter.path, None, state.encryption_key.as_ref())
            .await
            .map_err(|e| TingError::NotFound(format!("Failed to read strm file: {}", e)))?;

        let mut content = String::new();
        reader
            .read_to_string(&mut content)
            .await
            .map_err(|e| TingError::IoError(e))?;
        content.trim().to_string()
    } else {
        let (mut reader, _) = state
            .storage_service
            .get_http_reader(&chapter.path, None)
            .await
            .map_err(|e| TingError::NotFound(format!("Failed to read strm URL: {}", e)))?;

        let mut content = String::new();
        reader
            .read_to_string(&mut content)
            .await
            .map_err(|e| TingError::IoError(e))?;
        content.trim().to_string()
    };

    if url.is_empty() || !url.starts_with("http") {
        return Err(TingError::InvalidRequest(format!(
            "Invalid strm file content: '{}'",
            url
        )));
    }

    tracing::info!("Handling strm file: {}", url);

    // Handle Transcoding Request for .strm files
    // Frontend will request transcoding via &transcode=mp3 when playback fails
    // Android app uses &transcode=hls which is handled by the general HLS handler below
    if let Some(format) = &params.transcode {
        // HLS transcoding for strm files is handled by the general HLS handler
        if format == "hls" {
            tracing::info!("strm file requested HLS transcoding; delegating to HLS handler");
            return handle_hls_request(
                state,
                chapter,
                book,
                library,
                true, // is_strm
                params.seek.clone(),
            )
            .await;
        }

        tracing::info!("Transcoding strm URL: {} -> {}", url, format);

        let content_type = match format.as_str() {
            "mp3" => "audio/mpeg",
            "wav" => "audio/wav",
            _ => {
                return Err(TingError::InvalidRequest(
                    "Unsupported transcode format".to_string(),
                ))
            }
        };

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

        // 优先使用数据库中的时长，避免重复调用 FFprobe
        let duration_seconds = if let Some(db_duration) = chapter.duration {
            if db_duration > 0 {
                tracing::debug!("Using database duration: {} seconds", db_duration);
                Some(db_duration as f64)
            } else {
                None
            }
        } else {
            None
        };

        // 只有在数据库中没有时长时才使用 FFprobe
        let duration_seconds = if duration_seconds.is_none() {
            tracing::info!("No duration in database; using FFprobe to get audio duration...");

            // Add delay to avoid overwhelming the server
            tokio::time::sleep(std::time::Duration::from_millis(300)).await;

            let duration_output = Command::new(&ffmpeg_tools.ffprobe)
                .arg("-v")
                .arg("error")
                .arg("-show_entries")
                .arg("format=duration")
                .arg("-of")
                .arg("default=noprint_wrappers=1:nokey=1")
                .arg(&url)
                .output()
                .await;

            if let Ok(output) = duration_output {
                if output.status.success() {
                    let duration_str = String::from_utf8_lossy(&output.stdout);
                    let dur = duration_str.trim().parse::<f64>().ok();

                    if let Some(d) = dur {
                        tracing::info!("FFprobe detected duration: {:.2} seconds", d);

                        // 更新数据库中的时长
                        if let Ok(Some(mut chapter_record)) =
                            state.chapter_repo.find_by_id(&chapter_id).await
                        {
                            let new_duration = d.round() as i32;
                            tracing::info!(
                                "Updated chapter duration in database: {} seconds",
                                new_duration
                            );
                            chapter_record.duration = Some(new_duration);
                            let _ = state.chapter_repo.update(&chapter_record).await;
                        }
                    }

                    dur
                } else {
                    let error = String::from_utf8_lossy(&output.stderr);
                    tracing::warn!(
                        message_key = "ffprobe.duration_failed",
                        message_params = %serde_json::json!({ "error": error.to_string() }),
                        error = %error,
                        "FFprobe duration detection failed"
                    );
                    None
                }
            } else {
                tracing::warn!(message_key = "ffprobe.run_failed", "Failed to run FFprobe");
                None
            }
        } else {
            duration_seconds
        };

        tracing::info!(
            "Using FFmpeg to read directly from URL: {}",
            ffmpeg_tools.ffmpeg
        );

        // Build FFmpeg command to transcode directly from URL
        // This allows seeking support
        let mut cmd = Command::new(&ffmpeg_tools.ffmpeg);
        cmd.arg("-y").arg("-loglevel").arg("warning");

        // Add seek parameter if present (must be before -i for input seeking)
        if let Some(seek_time) = &params.seek {
            cmd.arg("-ss").arg(seek_time);
            tracing::info!("Seeking to position: {}", seek_time);
        }

        // Use URL as input directly (FFmpeg will handle HTTP/HTTPS)
        cmd.arg("-i").arg(&url);

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

    // Check if URL contains authentication (username:password@)
    // If it does, we need to proxy the request to avoid CORS issues
    let has_auth = url.contains("://")
        && url
            .split("://")
            .nth(1)
            .map(|s| s.contains('@'))
            .unwrap_or(false);

    // Safari/iOS browsers may need proxy if the upstream source doesn't support
    // Range requests. Do a quick HEAD probe first — most CDNs support Range,
    // so we only proxy when actually necessary.
    let user_agent = headers
        .get(header::USER_AGENT)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    let is_safari = user_agent.contains("Safari")
        && !user_agent.contains("Chrome")
        && !user_agent.contains("CriOS");
    let is_ios =
        user_agent.contains("iPhone") || user_agent.contains("iPad") || user_agent.contains("iPod");

    let needs_proxy = if has_auth {
        true
    } else if is_safari || is_ios {
        // HEAD probe: check if upstream supports Range
        let probe = reqwest::Client::builder()
                .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
                .timeout(std::time::Duration::from_secs(10))
                .build()
                .unwrap_or_else(|_| reqwest::Client::new());
        let supports_range = probe
            .head(&url)
            .send()
            .await
            .map(|r| {
                r.headers()
                    .get("accept-ranges")
                    .and_then(|v| v.to_str().ok())
                    .map(|v| v.contains("bytes"))
                    .unwrap_or(false)
            })
            .unwrap_or(false);
        if !supports_range {
            tracing::info!("Upstream does not support Range; Safari/iOS will use proxy");
            true
        } else {
            tracing::info!("Upstream supports Range; Safari/iOS will use 302 redirect");
            false
        }
    } else {
        false
    };

    if needs_proxy {
        // Proxy the request through our server:
        // - Auth URLs: strip credentials from URL, avoid CORS issues
        // - Safari/iOS + no Range: forward Range headers, add Accept-Ranges for seeking
        tracing::info!("Proxying strm URL");

        let range_header = headers.get(header::RANGE).and_then(|v| v.to_str().ok());

        // Build request with authentication and browser-like headers
        let client = reqwest::Client::builder()
                .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .unwrap_or_else(|_| reqwest::Client::new());
        let mut req = client
            .get(&url)
            .header("Accept", "*/*")
            .header("Accept-Language", "zh-CN,zh;q=0.9,en;q=0.8")
            .header("Accept-Encoding", "gzip, deflate, br")
            .header("Connection", "keep-alive");

        // Forward range header if present (use string literal to avoid type conflicts)
        if let Some(range) = range_header {
            req = req.header("range", range);
        }

        // Make the request
        let response = req.send().await.map_err(|e| {
            TingError::IoError(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Failed to fetch strm URL: {}", e),
            ))
        })?;

        let status = response.status();

        // Use string literals to avoid type conflicts between axum::http and reqwest::http
        let content_type = response
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("audio/mpeg")
            .to_string();

        let content_length = response
            .headers()
            .get("content-length")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<u64>().ok());

        let content_range = response
            .headers()
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
                (
                    "Cross-Origin-Resource-Policy".parse().unwrap(),
                    "cross-origin".to_string(),
                ),
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
                )
                    .into_response());
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
                )
                    .into_response());
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
            )
                .into_response());
        } else {
            return Ok((response_builder.0, response_builder.1, body).into_response());
        }
    } else {
        // 302 redirect (zero bandwidth cost)
        tracing::info!("Redirecting to strm URL");
        return Ok((StatusCode::FOUND, [(header::LOCATION, url)], Body::empty()).into_response());
    }
}
