use super::{
    Result, StreamContext, TingError, create_decrypted_stream, get_remote_media_reader,
    is_download_query, preload, stream_mime_type_from_path,
};
use crate::core::app::error::{InputErrorKind, LookupErrorKind, PluginErrorKind};
use axum::{
    body::Body,
    http::{StatusCode, header},
    response::IntoResponse,
};
use ting_plugin_contract::format::FormatOperation;
use tokio::io::{AsyncReadExt, AsyncSeekExt};
use tokio_util::io::ReaderStream;

async fn remote_reader(
    state: &crate::api::state::AppState,
    library: &crate::db::models::Library,
    path: &str,
    range: Option<(u64, u64)>,
    source_audio: Option<&crate::core::books::sources::SourceAudio>,
) -> Result<(Box<dyn tokio::io::AsyncRead + Send + Unpin>, u64)> {
    if let Some(source) = source_audio {
        source.reader(&state.storage_service, range).await
    } else {
        get_remote_media_reader(state, library, path, range).await
    }
}

pub(super) async fn stream_source(context: StreamContext) -> Result<axum::response::Response> {
    let StreamContext {
        state,
        chapter,
        book,
        library,
        params,
        method,
        headers,
        user,
        source_audio,
    } = context;
    let mime_type = source_audio
        .as_ref()
        .map(|source| {
            source
                .descriptor
                .mime
                .clone()
                .unwrap_or_else(|| stream_mime_type_from_path(&source.descriptor.url))
        })
        .unwrap_or_else(|| stream_mime_type_from_path(&chapter.path));
    let chapter_id = chapter.id.clone();
    let is_head_request = method == axum::http::Method::HEAD;
    let is_download_request = is_download_query(&params);
    if !is_download_request && !params.preload.unwrap_or(false) {
        preload::maybe_spawn_auto_preload(&state, user.as_ref(), &book, &chapter_id, &library)
            .await;
    }
    let cached = preload::cached_chapter(&state, &chapter_id).await;
    let cached_size_unknown = cached
        .as_ref()
        .is_some_and(|entry| entry.total_size.is_none());
    let plugin_handles_format = if cached.is_some() && source_audio.is_none() {
        state
            .plugin_manager
            .has_format_operation(
                std::path::Path::new(&chapter.path),
                FormatOperation::OpenDecrypt,
            )
            .await?
    } else {
        false
    };
    // 1. Serve a bounded memory prefix, then stream the source remainder.
    if let Some(entry) = cached
        && !plugin_handles_format
    {
        let tail_state = state.clone();
        let tail_library = library.clone();
        let tail_path = chapter.path.clone();
        let tail_chapter_id = chapter_id.clone();
        let response = preload::prefix_response(
            entry,
            &state.audio_streamer,
            headers
                .get(header::RANGE)
                .and_then(|value| value.to_str().ok()),
            mime_type.clone(),
            is_head_request,
            move |start, end| async move {
                let cache_path = tail_state.cache_manager.get_cache_path(&tail_chapter_id);
                if cache_path.exists() || tail_library.library_type == "local" {
                    let path = if cache_path.exists() {
                        cache_path
                    } else {
                        std::path::PathBuf::from(tail_path)
                    };
                    let (file, size) = tail_state
                        .storage_service
                        .get_local_reader(&path, Some((start, end)))
                        .await?;
                    Ok((
                        Box::new(file) as Box<dyn tokio::io::AsyncRead + Send + Unpin>,
                        size,
                    ))
                } else {
                    get_remote_media_reader(
                        &tail_state,
                        &tail_library,
                        &tail_path,
                        Some((start, end)),
                    )
                    .await
                }
            },
        )?;
        if let Some(response) = response {
            tracing::debug!(target: "media", chapter_id = %chapter_id, "Serving preloaded prefix with streaming remainder");
            return Ok(response);
        }
    }

    // 2. Check Disk Cache
    let cache_path = state.cache_manager.get_cache_path(&chapter_id);
    if cache_path.exists() {
        tracing::debug!(target: "media", chapter_id = %chapter_id, "Serving from disk cache");

        // Check if we need to use a format plugin even for cached files (source file is cached)
        let plugin_handles_format = source_audio.is_none()
            && state
                .plugin_manager
                .has_format_operation(
                    std::path::Path::new(&chapter.path),
                    FormatOperation::OpenDecrypt,
                )
                .await?;

        if plugin_handles_format {
            // If a plugin handles this format, we use the cached file as the source for the plugin logic
            // instead of serving it directly.
            tracing::info!(
                chapter_id = %chapter_id,
                "Cached file requires format plugin processing"
            );

            // Fall through to the plugin handling logic below
            // We need to make sure the logic below knows to use the cache_path as source
            // This is handled by the `if cache_path.exists()` checks in the plugin block
        } else {
            let file_size = tokio::fs::metadata(&cache_path).await?.len();
            let mime_type = mime_type.clone();

            let range_header = headers.get(header::RANGE).and_then(|v| v.to_str().ok());
            if let Some(range_str) = range_header
                && let Ok(range) = state
                    .audio_streamer
                    .parse_range_header(range_str, file_size)
            {
                let content_length = range.end - range.start;
                let mut file = tokio::fs::File::open(&cache_path).await?;
                file.seek(std::io::SeekFrom::Start(range.start)).await?;
                let body = if is_head_request {
                    Body::empty()
                } else {
                    Body::from_stream(ReaderStream::new(file.take(content_length)))
                };

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
                        (
                            "Cross-Origin-Resource-Policy".parse().unwrap(),
                            "cross-origin".to_string(),
                        ),
                    ],
                    body,
                )
                    .into_response());
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

    // 3. Not cached. Fetch from source.
    tracing::debug!(target: "media", chapter_id = %chapter_id, "Serving from source stream");

    // Determine if we need to use a format plugin
    // Instead of hardcoding extensions, we ask the plugin manager if any loaded plugin supports this extension
    let plugin_handles_format = source_audio.is_none()
        && state
            .plugin_manager
            .has_format_operation(
                std::path::Path::new(&chapter.path),
                FormatOperation::OpenDecrypt,
            )
            .await?;

    if plugin_handles_format {
        tracing::info!(chapter_id = %chapter_id, "Processing file with format plugin");

        let range_header = headers.get(header::RANGE).and_then(|v| v.to_str().ok());
        let (stream, mime_type, output_extension, content_length, start, end, logic_size) =
            create_decrypted_stream(
                &state,
                &chapter,
                &library,
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
            let content_length = content_length.ok_or_else(|| {
                TingError::plugin_execution(
                    PluginErrorKind::Runtime,
                    "stream_source::plugin_execution",
                    "Format stream returned a range without a known length".into(),
                )
            })?;
            let logic_size = logic_size.ok_or_else(|| {
                TingError::plugin_execution(
                    PluginErrorKind::Runtime,
                    "stream_source::plugin_execution",
                    "Format stream returned a range without a known length".into(),
                )
            })?;
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
                    (
                        "Cross-Origin-Resource-Policy".parse().unwrap(),
                        "cross-origin".to_string(),
                    ),
                ],
                if is_head_request { Body::empty() } else { body },
            )
                .into_response());
        } else {
            if let Some(content_length) = content_length {
                return Ok((
                    StatusCode::OK,
                    [
                        (header::CONTENT_TYPE, mime_type.to_string()),
                        (header::CONTENT_LENGTH, content_length.to_string()),
                        (header::ACCEPT_RANGES, "bytes".to_string()),
                        ("X-Download-Extension".parse().unwrap(), download_extension),
                        (
                            "Cross-Origin-Resource-Policy".parse().unwrap(),
                            "cross-origin".to_string(),
                        ),
                    ],
                    if is_head_request { Body::empty() } else { body },
                )
                    .into_response());
            }
            return Ok((
                StatusCode::OK,
                [
                    (header::CONTENT_TYPE, mime_type.to_string()),
                    ("X-Download-Extension".parse().unwrap(), download_extension),
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

    // A suffix range needs the total length before its starting offset is known.
    // Keep the full reader if the source cannot advertise that length.
    let mut remote_probe = None;
    let mut range = if let Some(r) = range_header {
        if library.library_type == "local" {
            let file_size = tokio::fs::metadata(std::path::Path::new(&chapter.path))
                .await?
                .len();
            let parsed = state.audio_streamer.parse_range_header(r, file_size)?;
            Some((parsed.start, parsed.end))
        } else if cached_size_unknown || r.starts_with("bytes=-") {
            let (reader, total) =
                remote_reader(&state, &library, &chapter.path, None, source_audio.as_ref()).await?;
            if total > 0 {
                let parsed = state.audio_streamer.parse_range_header(r, total)?;
                Some((parsed.start, parsed.end))
            } else {
                remote_probe = Some((reader, total));
                None
            }
        } else {
            let (start, end) = r
                .strip_prefix("bytes=")
                .and_then(|value| value.split_once('-'))
                .ok_or_else(|| {
                    TingError::invalid_request(
                        InputErrorKind::OutOfRange,
                        "stream_source::invalid_request",
                        "Invalid Range header".into(),
                    )
                })?;
            let start = start.parse::<u64>().map_err(|_| {
                TingError::invalid_request(
                    InputErrorKind::OutOfRange,
                    "stream_source::invalid_request",
                    "Invalid Range start".into(),
                )
            })?;
            let end = if end.is_empty() {
                0
            } else {
                let inclusive_end = end.parse::<u64>().map_err(|_| {
                    TingError::invalid_request(
                        InputErrorKind::OutOfRange,
                        "stream_source::invalid_request",
                        "Invalid Range end".into(),
                    )
                })?;
                if inclusive_end < start {
                    return Err(TingError::invalid_request(
                        InputErrorKind::OutOfRange,
                        "stream_source::invalid_request",
                        "Invalid Range bounds".into(),
                    ));
                }
                inclusive_end.saturating_add(1)
            };
            Some((start, end))
        }
    } else {
        None
    };

    let (mut reader, mut total_size) = if library.library_type == "local" {
        let (f, size) = state
            .storage_service
            .get_local_reader(std::path::Path::new(&chapter.path), range)
            .await
            .map_err(|e| {
                let message = format!("Local file not found: {}", e);
                TingError::not_found(LookupErrorKind::File, "stream_source::not_found", message)
                    .with_source(anyhow::anyhow!(e))
            })?;
        (
            Box::new(f) as Box<dyn tokio::io::AsyncRead + Send + Unpin>,
            size,
        )
    } else if let Some(probe) = remote_probe {
        probe
    } else {
        remote_reader(
            &state,
            &library,
            &chapter.path,
            range,
            source_audio.as_ref(),
        )
        .await?
    };

    // A source without a total length cannot resolve suffix/open-ended ranges.
    // Ignore Range and stream the full representation without inventing a length.
    if total_size == 0 && range.is_some() && library.library_type != "local" {
        (reader, total_size) =
            remote_reader(&state, &library, &chapter.path, None, source_audio.as_ref()).await?;
        range = None;
    }

    if range.is_some() && total_size > 0 {
        let parsed = state
            .audio_streamer
            .parse_range_header(range_header.unwrap(), total_size)?;
        range = Some((parsed.start, parsed.end));
    }

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
    if total_size > 0 && content_length < total_size.saturating_sub(start) {
        reader = Box::new(reader.take(content_length));
    }

    // Convert AsyncRead to Stream
    let stream = ReaderStream::new(reader);
    let body = Body::from_stream(stream);

    if range.is_some() {
        let content_range = format!("bytes {}-{}/{}", start, end.saturating_sub(1), total_size);

        Ok((
            StatusCode::PARTIAL_CONTENT,
            [
                (header::CONTENT_TYPE, mime_type),
                (header::CONTENT_LENGTH, content_length.to_string()),
                (header::CONTENT_RANGE, content_range),
                (header::ACCEPT_RANGES, "bytes".to_string()),
                (
                    "Cross-Origin-Resource-Policy".parse().unwrap(),
                    "cross-origin".to_string(),
                ),
            ],
            if is_head_request { Body::empty() } else { body },
        )
            .into_response())
    } else {
        let mut response = (
            StatusCode::OK,
            [
                (header::CONTENT_TYPE, mime_type),
                (header::ACCEPT_RANGES, "bytes".to_string()),
                (
                    "Cross-Origin-Resource-Policy".parse().unwrap(),
                    "cross-origin".to_string(),
                ),
            ],
            if is_head_request { Body::empty() } else { body },
        )
            .into_response();
        if total_size > 0 || library.library_type == "local" {
            response
                .headers_mut()
                .insert(header::CONTENT_LENGTH, total_size.into());
        }
        Ok(response)
    }
}
