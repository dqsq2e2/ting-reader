use super::hls_session::{HlsGeneration, HlsProcess, HlsSession};
use crate::api::state::AppState;
use crate::core::app::error::{
    DeadlineErrorKind, ExternalErrorKind, InputErrorKind, LookupErrorKind,
};
use crate::core::app::error::{Result, TingError};
use crate::db::models::{Book, Chapter, Library};
use axum::response::{IntoResponse, Response};
use bytes::Bytes;
use futures::{StreamExt, stream::BoxStream};
use std::path::Path;
use std::process::Stdio;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::{Duration, Instant};
use ting_plugin_contract::format::FormatOperation;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

pub async fn handle_hls_request(
    state: AppState,
    chapter: Chapter,
    _book: Book,
    library: Library,
    is_strm: bool,
    seek: Option<String>,
) -> Result<Response> {
    let start_offset = parse_seek(seek.as_deref())?;
    let pending = state
        .hls_session_manager
        .create_session(chapter.id.clone(), library.id.clone(), is_strm)
        .await?;
    let mut session = pending.session.lock().await;
    let generation =
        start_generation(&state, &mut session, &chapter, &library, start_offset).await?;
    drop(session);
    let session_id = pending.commit();
    Ok(axum::Json(serde_json::json!({
        "type": "hls",
        "session_id": session_id,
        "playlist_url": playlist_url(&session_id, generation.seq),
        "start_offset": generation.start_offset,
        "is_strm": is_strm,
        "ready": true
    }))
    .into_response())
}

pub(super) fn parse_seek(seek: Option<&str>) -> Result<f64> {
    let value = seek.unwrap_or("0").parse::<f64>().map_err(|_| {
        TingError::invalid_request(
            InputErrorKind::InvalidValue,
            "parse_seek::invalid_request",
            "Invalid HLS seek time".into(),
        )
    })?;
    validate_seek(value)?;
    Ok(value)
}

pub(super) fn validate_seek(value: f64) -> Result<()> {
    if !value.is_finite() || value < 0.0 {
        return Err(TingError::invalid_request(
            InputErrorKind::OutOfRange,
            "validate_seek::invalid_request",
            "HLS seek time must be finite and nonnegative".into(),
        ));
    }
    Ok(())
}

pub(super) fn playlist_url(session_id: &str, seq: u32) -> String {
    format!("/api/stream/hls/{session_id}/{seq}/playlist.m3u8")
}

enum HlsInput {
    Direct(String),
    Decrypted(BoxStream<'static, std::io::Result<Bytes>>),
}

async fn get_input(
    state: &AppState,
    chapter: &Chapter,
    library: &Library,
    is_strm: bool,
) -> Result<HlsInput> {
    if library.library_type == "source" {
        let (reader, _) =
            super::get_remote_media_reader(state, library, &chapter.path, None).await?;
        return Ok(HlsInput::Decrypted(
            tokio_util::io::ReaderStream::new(reader).boxed(),
        ));
    }
    if is_strm {
        let reader: Box<dyn tokio::io::AsyncRead + Send + Unpin> =
            if library.library_type == "local" {
                Box::new(tokio::fs::File::open(&chapter.path).await?)
            } else {
                super::get_remote_media_reader(state, library, &chapter.path, None)
                    .await?
                    .0
            };
        let url = super::strm::read_strm_url(reader).await?;
        super::strm::validate_strm_url(&url)?;
        return Ok(HlsInput::Direct(url));
    }
    if state
        .plugin_manager
        .has_format_operation(Path::new(&chapter.path), FormatOperation::OpenDecrypt)
        .await?
    {
        let (stream, _, _, _, _, _, _) =
            super::create_decrypted_stream(state, chapter, library, None).await?;
        return Ok(HlsInput::Decrypted(stream));
    }
    let cache_path = state.cache_manager.get_cache_path(&chapter.id);
    if tokio::fs::try_exists(&cache_path).await? {
        return Ok(HlsInput::Direct(cache_path.to_string_lossy().into_owned()));
    }
    if library.library_type == "webdav" {
        Ok(HlsInput::Direct(
            crate::core::StorageService::authenticated_media_url(
                library,
                &chapter.path,
                Some(state.encryption_key.as_ref()),
            )?,
        ))
    } else {
        Ok(HlsInput::Direct(chapter.path.clone()))
    }
}

pub(super) async fn start_generation(
    state: &AppState,
    session: &mut HlsSession,
    chapter: &Chapter,
    library: &Library,
    start_offset: f64,
) -> Result<HlsGeneration> {
    let result = tokio::time::timeout(
        Duration::from_secs(40),
        start_generation_inner(state, session, chapter, library, start_offset),
    )
    .await
    .unwrap_or_else(|_| {
        Err(TingError::timeout(
            DeadlineErrorKind::Elapsed,
            "start_generation::timeout",
            "HLS startup timed out".into(),
        ))
    });
    if result.is_err()
        && let Some(process) = session.process.take()
    {
        process.stop().await;
    }
    result
}

async fn start_generation_inner(
    state: &AppState,
    session: &mut HlsSession,
    chapter: &Chapter,
    library: &Library,
    start_offset: f64,
) -> Result<HlsGeneration> {
    if session.closed {
        return Err(TingError::not_found(
            LookupErrorKind::Session,
            "start_generation_inner::not_found",
            "HLS session closed".into(),
        ));
    }
    // Decode before FFmpeg for plugin formats; the stream is owned by this session.
    let input = get_input(state, chapter, library, session.is_strm).await?;
    let generation = session.next_generation(start_offset).await?;
    let mut cmd = crate::core::audio::AudioService::ffmpeg_command()?;
    cmd.arg("-y")
        .arg("-nostdin")
        .arg("-loglevel")
        .arg("warning")
        .arg("-fflags")
        .arg("+genpts+igndts")
        .arg("-analyzeduration")
        .arg("1000000")
        .arg("-probesize")
        .arg("5000000");
    let is_remote = matches!(&input, HlsInput::Direct(url) if url.starts_with("http://") || url.starts_with("https://"));
    let is_pipe = matches!(&input, HlsInput::Decrypted(_));
    if is_remote {
        cmd.arg("-rw_timeout").arg("10000000");
    }
    if !is_pipe && start_offset > 0.0 {
        cmd.arg("-ss").arg(start_offset.to_string());
    }
    let input_name = match &input {
        HlsInput::Direct(path) => path.as_str(),
        HlsInput::Decrypted(_) => "pipe:0",
    };
    cmd.arg("-i").arg(input_name);
    // Pipe inputs cannot perform input seeking; decode/discard until the target.
    if is_pipe && start_offset > 0.0 {
        cmd.arg("-ss").arg(start_offset.to_string());
    }
    configure_hls_output(&mut cmd, &generation.temp_dir);
    cmd.stdin(if is_pipe {
        Stdio::piped()
    } else {
        Stdio::null()
    })
    .stdout(Stdio::null())
    .stderr(Stdio::piped());
    let mut process = HlsProcess {
        child: cmd.spawn()?,
        input_task: None,
        stderr_task: None,
        input_error: Arc::new(AtomicBool::new(false)),
    };
    process.stderr_task = process.child.stderr.take().map(|mut stderr| {
        tokio::spawn(async move {
            // Drain continuously with a fixed buffer, even for long chapters.
            // FFmpeg diagnostics can include signed URLs, so don't log raw data.
            let mut buffer = [0; 4096];
            let mut count = 0_u64;
            while let Ok(read) = stderr.read(&mut buffer).await {
                if read == 0 {
                    break;
                }
                count = count.saturating_add(read as u64);
            }
            if count > 0 {
                tracing::debug!(diagnostic_bytes = count, "HLS FFmpeg emitted diagnostics");
            }
        })
    });
    process.input_task = if let HlsInput::Decrypted(mut stream) = input {
        let input_error = process.input_error.clone();
        let mut stdin = process.child.stdin.take().ok_or_else(|| {
            TingError::external_tool(
                ExternalErrorKind::Process,
                "start_generation_inner::external_tool",
                "FFmpeg stdin unavailable".into(),
            )
        })?;
        Some(tokio::spawn(async move {
            while let Some(chunk) = stream.next().await {
                match chunk {
                    Ok(bytes) => {
                        if stdin.write_all(&bytes).await.is_err() {
                            break;
                        }
                    }
                    Err(error) => {
                        input_error.store(true, Ordering::Relaxed);
                        tracing::warn!(%error, "HLS decoded input failed");
                        break;
                    }
                }
            }
            let _ = stdin.shutdown().await;
        }))
    } else {
        None
    };
    session.process = Some(process);
    let deadline = Instant::now() + Duration::from_secs(if is_remote || is_pipe { 30 } else { 10 });
    let result = wait_for_first_segment(session, &generation.temp_dir, deadline).await;
    session.last_accessed = Instant::now();
    result?;
    Ok(generation)
}

pub(super) fn configure_hls_output(cmd: &mut tokio::process::Command, temp_dir: &Path) {
    cmd.arg("-map")
        .arg("0:a:0")
        .arg("-vn")
        .arg("-c:a")
        .arg("aac")
        .arg("-b:a")
        .arg("128k")
        .arg("-ar")
        .arg("44100")
        .arg("-ac")
        .arg("2")
        .arg("-aac_coder")
        .arg("fast")
        .arg("-f")
        .arg("hls")
        .arg("-hls_time")
        .arg("4")
        .arg("-hls_list_size")
        .arg("0")
        .arg("-hls_playlist_type")
        .arg("event")
        .arg("-hls_segment_type")
        .arg("mpegts")
        .arg("-hls_flags")
        .arg("temp_file")
        .arg("-hls_segment_filename")
        .arg(temp_dir.join("segment_%03d.ts"))
        .arg(temp_dir.join("playlist.m3u8"));
}

pub(super) fn playlist_duration(content: &str) -> f64 {
    content
        .lines()
        .filter_map(|line| line.strip_prefix("#EXTINF:"))
        .filter_map(|value| value.split(',').next()?.parse::<f64>().ok())
        .filter(|value| value.is_finite() && *value > 0.0)
        .sum()
}

pub(super) async fn cached_generation(session: &HlsSession, target: f64) -> Option<HlsGeneration> {
    if session
        .process
        .as_ref()
        .is_some_and(|process| process.check_input().is_err())
    {
        return None;
    }
    let current = session.generations.back()?;
    let content = tokio::fs::read_to_string(current.temp_dir.join("playlist.m3u8"))
        .await
        .ok()?;
    (target >= current.start_offset && target < current.start_offset + playlist_duration(&content))
        .then(|| current.clone())
}

pub(super) fn valid_segment_filename(filename: &str) -> bool {
    filename
        .strip_prefix("segment_")
        .and_then(|value| value.strip_suffix(".ts"))
        .is_some_and(|number| !number.is_empty() && number.bytes().all(|b| b.is_ascii_digit()))
}

async fn wait_for_first_segment(
    session: &mut HlsSession,
    temp_dir: &Path,
    deadline: Instant,
) -> Result<()> {
    loop {
        let process = session.process.as_mut().ok_or_else(|| {
            TingError::external_tool(
                ExternalErrorKind::Process,
                "wait_for_first_segment::external_tool",
                "HLS process unavailable".into(),
            )
        })?;
        process.check_input()?;
        let status = process.child.try_wait()?;
        if let Some(status) = status
            && !status.success()
        {
            return Err(TingError::external_tool(
                ExternalErrorKind::Process,
                "wait_for_first_segment::external_tool",
                format!("HLS FFmpeg exited with {status}"),
            ));
        }
        if let Ok(content) = tokio::fs::read_to_string(temp_dir.join("playlist.m3u8")).await
            && playlist_duration(&content) > 0.0
            && let Some(filename) = content.lines().find(|line| valid_segment_filename(line))
            && tokio::fs::metadata(temp_dir.join(filename))
                .await
                .is_ok_and(|meta| meta.len() > 0)
        {
            return Ok(());
        }
        if status.is_some() {
            return Err(TingError::external_tool(
                ExternalErrorKind::Process,
                "wait_for_first_segment::external_tool",
                "HLS FFmpeg exited before producing playable audio".into(),
            ));
        }
        if Instant::now() >= deadline {
            return Err(TingError::timeout(
                DeadlineErrorKind::Elapsed,
                "wait_for_first_segment::timeout",
                "Timed out waiting for playable HLS audio".into(),
            ));
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

#[cfg(test)]
#[path = "../../../../../tests/unit/api/handlers/media/stream/hls.rs"]
mod tests;
