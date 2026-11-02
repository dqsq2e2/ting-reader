use super::{
    AppState, Chapter, Library, Result, StreamQuery, TingError, create_decrypted_stream,
    get_remote_media_reader,
};
use crate::core::app::error::InputErrorKind;
use crate::db::repository::Repository;
use axum::{
    body::Body,
    http::{StatusCode, header},
    response::IntoResponse,
};
use bytes::Bytes;
use futures::Stream;
use std::pin::Pin;
use std::process::Stdio;
use std::task::{Context, Poll};
use ting_plugin_contract::format::FormatOperation;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// Response-owned input and stderr tasks. Cancellation releases remote readers
/// and plugin streams even when the producer is blocked waiting for more data.
#[derive(Default)]
struct TranscodeTasks(Vec<tokio::task::JoinHandle<()>>);

impl Drop for TranscodeTasks {
    fn drop(&mut self) {
        for task in &self.0 {
            task.abort();
        }
    }
}

struct TranscodeOutput<S> {
    output: Pin<Box<S>>,
    tasks: TranscodeTasks,
}

impl<S: Stream<Item = std::io::Result<Bytes>>> Stream for TranscodeOutput<S> {
    type Item = std::io::Result<Bytes>;

    fn poll_next(mut self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let result = self.output.as_mut().poll_next(context);
        if matches!(result, Poll::Ready(None) | Poll::Ready(Some(Err(_)))) {
            self.tasks.0.iter().for_each(|task| task.abort());
            self.tasks.0.clear();
        }
        result
    }
}

fn transcode_body(child: tokio::process::Child, tasks: TranscodeTasks) -> Result<Body> {
    let output = crate::core::audio::AudioService::output_stream(child)?;
    Ok(Body::from_stream(TranscodeOutput {
        output: Box::pin(output),
        tasks,
    }))
}

fn configure_output(command: &mut tokio::process::Command, format: &str) {
    if format == "mp3" {
        command.args([
            "-fflags",
            "+genpts+igndts",
            "-avoid_negative_ts",
            "make_zero",
            "-acodec",
            "libmp3lame",
            "-b:a",
            "128k",
            "-ac",
            "2",
            "-ar",
            "44100",
        ]);
    }
    command.args(["-vn", "-map", "0:a:0", "-f", format]);
}

fn capture_stderr(child: &mut tokio::process::Child, tasks: &mut TranscodeTasks) {
    if let Some(mut stderr) = child.stderr.take() {
        tasks.0.push(tokio::spawn(async move {
            // Retain only a bounded diagnostic prefix, then drain without buffering.
            let mut prefix = Vec::new();
            let read = (&mut stderr).take(16 * 1024).read_to_end(&mut prefix).await;
            let _ = tokio::io::copy(&mut stderr, &mut tokio::io::sink()).await;
            if read.is_ok() && !prefix.is_empty() {
                tracing::warn!("FFmpeg stderr: {}", String::from_utf8_lossy(&prefix));
            }
        }));
    }
}

struct PluginTranscodeOptions<'a> {
    format: &'a str,
    content_type: &'a str,
    seek: Option<&'a str>,
}

async fn transcode_plugin_stream(
    state: &AppState,
    chapter: &Chapter,
    library: &Library,
    options: PluginTranscodeOptions<'_>,
) -> Result<axum::response::Response> {
    let PluginTranscodeOptions {
        format,
        content_type,
        seek,
    } = options;
    let (plugin_stream, _, _, _, _, _, _) =
        create_decrypted_stream(state, chapter, library, None).await?;

    let mut cmd = crate::core::audio::AudioService::ffmpeg_command()?;
    cmd.arg("-y").arg("-loglevel").arg("error");
    if let Some(seek_time) = seek {
        cmd.arg("-ss").arg(seek_time);
    }
    cmd.arg("-i").arg("pipe:0");

    configure_output(&mut cmd, format);

    cmd.arg("pipe:1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    tracing::info!(
        chapter_id = %chapter.id,
        format = %format,
        "Using format plugin decoded stream for transcoded output"
    );

    let mut child = cmd.spawn().map_err(TingError::IoError)?;

    let mut stdin = child.stdin.take().ok_or_else(|| {
        TingError::IoError(std::io::Error::other("Failed to capture ffmpeg stdin"))
    })?;

    let mut tasks = TranscodeTasks::default();
    tasks.0.push(tokio::spawn(async move {
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
    }));
    capture_stderr(&mut child, &mut tasks);
    let body = transcode_body(child, tasks)?;
    Ok((
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, content_type.to_string()),
            (
                "Cross-Origin-Resource-Policy".parse().unwrap(),
                "cross-origin".to_string(),
            ),
        ],
        body,
    )
        .into_response())
}

pub(super) async fn transcode_stream(
    state: &AppState,
    chapter: &Chapter,
    library: &Library,
    params: &StreamQuery,
) -> Result<axum::response::Response> {
    let chapter_id = &chapter.id;
    let format = params.transcode.as_deref().ok_or_else(|| {
        TingError::invalid_request(
            InputErrorKind::MissingField,
            "transcode_stream::invalid_request",
            "Missing transcode format".into(),
        )
    })?;
    tracing::info!("Requested transcoding: {} -> {}", chapter.path, format);

    let content_type = match format {
        "mp3" => "audio/mpeg",
        "wav" => "audio/wav",
        _ => {
            return Err(TingError::invalid_request(
                InputErrorKind::UnsupportedValue,
                "transcode_stream::invalid_request",
                "Unsupported transcode format".to_string(),
            ));
        }
    };

    let cache_path = state.cache_manager.get_cache_path(chapter_id);
    let plugin_handles_format = library.library_type != "source"
        && state
            .plugin_manager
            .has_format_operation(
                std::path::Path::new(&chapter.path),
                FormatOperation::OpenDecrypt,
            )
            .await?;

    // Check if we can use direct URL transcoding (for WebDAV or cached files).
    // Plugin-backed formats must be decoded/decrypted before FFmpeg sees them.
    let can_use_direct_url =
        library.library_type != "local" && !cache_path.exists() && !plugin_handles_format;

    if can_use_direct_url {
        let webdav_url_str = crate::core::StorageService::authenticated_media_url(
            library,
            &chapter.path,
            Some(state.encryption_key.as_ref()),
        )?;

        tracing::info!(chapter_id = %chapter.id, "Transcoding from remote source");

        // Get duration using FFprobe

        // Add delay to avoid overwhelming the server
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;

        let duration_output = crate::core::audio::AudioService::ffprobe_command()?
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
            if let Ok(Some(mut chapter_record)) = state.chapter_repo.find_by_id(chapter_id).await {
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
        let mut cmd = crate::core::audio::AudioService::ffmpeg_command()?;
        cmd.arg("-y").arg("-loglevel").arg("warning");

        // Add seek parameter if present (must be before -i for input seeking)
        if let Some(seek_time) = &params.seek {
            cmd.arg("-ss").arg(seek_time);
            tracing::info!("Seeking to position: {}", seek_time);
        }

        // Use URL as input directly (FFmpeg will handle HTTP/HTTPS)
        cmd.arg("-i").arg(&webdav_url_str);

        // Add transcoding parameters
        configure_output(&mut cmd, format);

        cmd.arg("pipe:1"); // Output to stdout

        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        tracing::info!("Starting FFmpeg process reading directly from URL...");

        // Spawn FFmpeg process
        let mut child = cmd.spawn().map_err(TingError::IoError)?;

        let mut tasks = TranscodeTasks::default();
        capture_stderr(&mut child, &mut tasks);
        let body = transcode_body(child, tasks)?;

        // Build response with duration header if available
        if let Some(dur) = duration_seconds {
            return Ok((
                StatusCode::OK,
                [
                    (header::CONTENT_TYPE, content_type.to_string()),
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

    if plugin_handles_format {
        return transcode_plugin_stream(
            state,
            chapter,
            library,
            PluginTranscodeOptions {
                format,
                content_type,
                seek: params.seek.as_deref(),
            },
        )
        .await;
    }

    // Streaming MP3 output may not expose a finite duration to clients.
    // Recover missing metadata from the full local/cache source before seeking.
    let mut duration_seconds = chapter
        .duration
        .filter(|duration| *duration > 0)
        .map(f64::from);
    if duration_seconds.is_none() {
        let source_path = if cache_path.exists() {
            Some(cache_path.as_path())
        } else if library.library_type == "local" {
            Some(std::path::Path::new(&chapter.path))
        } else {
            None
        };
        if let Some(source_path) = source_path {
            match crate::core::audio::AudioService::probe_duration(source_path).await {
                Ok(Some(duration)) => {
                    duration_seconds = Some(duration);
                    if let Ok(Some(mut chapter_record)) =
                        state.chapter_repo.find_by_id(chapter_id).await
                        && chapter_record.duration.unwrap_or(0) <= 0
                    {
                        chapter_record.duration = Some(duration.round() as i32);
                        if let Err(error) = state.chapter_repo.update(&chapter_record).await {
                            tracing::warn!(
                                chapter_id = %chapter_id,
                                %error,
                                "Failed to save transcoded source duration"
                            );
                        }
                    }
                }
                Ok(None) => {}
                Err(error) => tracing::warn!(
                    chapter_id = %chapter_id,
                    %error,
                    "Failed to detect transcoded source duration"
                ),
            }
        }
    }

    let mut cmd = crate::core::audio::AudioService::ffmpeg_command()?;
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

    configure_output(&mut cmd, format);
    cmd.arg("-");

    cmd.stdout(Stdio::piped());

    let mut child = cmd.spawn().map_err(TingError::IoError)?;
    let mut tasks = TranscodeTasks::default();

    // Handle input pipe if needed (Only if we are using the fallback pipe logic)
    let use_pipe = !cache_path.exists() && library.library_type != "local";
    if use_pipe
        && child.stdin.is_some()
        && let Some(mut stdin) = child.stdin.take()
    {
        // Get reader
        let (mut reader, _) = get_remote_media_reader(state, library, &chapter.path, None).await?;

        tasks.0.push(tokio::spawn(async move {
            if let Err(e) = tokio::io::copy(&mut reader, &mut stdin).await {
                tracing::error!(
                    error = %e,
                    message_key = "media.ffmpeg.pipe_failed",
                    message_params = %serde_json::json!({ "error": e.to_string() }),
                    "Failed to pipe input to FFmpeg"
                );
            }
        }));
    }

    let body = transcode_body(child, tasks)?;

    let mut response = (
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, content_type.to_string()),
            (
                "Cross-Origin-Resource-Policy".parse().unwrap(),
                "cross-origin".to_string(),
            ),
        ],
        body,
    )
        .into_response();
    if let Some(duration) = duration_seconds {
        response
            .headers_mut()
            .insert("X-Audio-Duration", duration.to_string().parse().unwrap());
    }
    Ok(response)
}

#[cfg(test)]
#[path = "../../../../../tests/unit/api/handlers/media/stream/transcode.rs"]
mod tests;
