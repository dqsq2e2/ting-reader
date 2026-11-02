//! Scoped format playback: source bytes and plaintext chunks stay in Host
//! resources, while plugins own format detection and decoding algorithms.

use crate::api::state::AppState;
use crate::core::app::error::{DeadlineErrorKind, InputErrorKind, PluginErrorKind};
use crate::core::app::error::{Result, TingError};
use crate::db::models::{Chapter, Library};
use crate::plugin::host_api::resources::{
    ResourceError, ResourceResult, ResourceScope, ResourceSource,
};
use crate::plugin::manager::formats::SelectedFormat;
use crate::plugin::types::PluginInvocationContext;
use futures::StreamExt;
use std::sync::Arc;
use ting_plugin_contract::format::FormatOperation;
use ting_plugin_contract::format_calls::{
    MAX_MEDIA_CHUNK_BYTES, MAX_SAFE_INTEGER, OpenDecryptResult, ReadChunkResult, SeekResult,
    SessionId,
};
use ting_plugin_contract::protocol::PluginErrorCode;
use ting_plugin_contract::resources::ResourceStat;
use tokio::io::AsyncReadExt;
use tokio_util::sync::CancellationToken;

async fn remote_reader(
    state: &AppState,
    library: &Library,
    path: &str,
    range: (u64, u64),
) -> Result<(Box<dyn tokio::io::AsyncRead + Send + Unpin>, u64)> {
    if library.library_type == "webdav" {
        state
            .storage_service
            .get_webdav_reader(library, path, Some(range), state.encryption_key.as_ref())
            .await
    } else if library.library_type == "rss"
        || path.starts_with("http://")
        || path.starts_with("https://")
    {
        state
            .storage_service
            .get_http_reader(path, Some(range), None, None)
            .await
    } else {
        Err(TingError::validation(
            InputErrorKind::UnsupportedValue,
            "remote_reader::validation",
            "Unsupported remote format source".into(),
        ))
    }
}

fn remote_error(code: PluginErrorCode, message: &'static str) -> ResourceError {
    ResourceError { code, message }
}

struct RemoteMediaSource {
    state: AppState,
    library: Library,
    path: String,
    length: u64,
    runtime: tokio::runtime::Handle,
}

impl ResourceSource for RemoteMediaSource {
    fn stat(&self) -> ResourceStat {
        ResourceStat {
            length: Some(self.length),
            mime_type: None,
            readable: true,
            writable: false,
            seekable: true,
            revision: None,
            finished: true,
        }
    }

    fn read_at(
        &self,
        offset: u64,
        max_bytes: usize,
        cancel: &CancellationToken,
    ) -> ResourceResult<Vec<u8>> {
        let state = self.state.clone();
        let library = self.library.clone();
        let path = self.path.clone();
        let token = cancel.clone();
        let runtime = self.runtime.clone();
        let result = std::thread::spawn(move || runtime.block_on(async move {
            let (mut reader, _) = remote_reader(&state, &library, &path,
                (offset, offset.saturating_add(max_bytes as u64))).await?;
            let mut data = vec![0; max_bytes];
            let mut count = 0;
            while count < data.len() {
                let read = tokio::select! {
                    _ = token.cancelled() => return Err(TingError::timeout(DeadlineErrorKind::Cancelled, "read_at::timeout", "Remote resource cancelled".into())),
                    read = reader.read(&mut data[count..]) => read.map_err(TingError::IoError)?,
                    _ = tokio::time::sleep(std::time::Duration::from_secs(15)) =>
                        return Err(TingError::timeout(DeadlineErrorKind::Elapsed, "read_at::timeout", "Remote resource read timed out".into())),
                };
                if read == 0 { break; }
                count += read;
            }
            data.truncate(count);
            Ok::<_, TingError>(data)
        })).join();
        result
            .map_err(|_| {
                remote_error(
                    PluginErrorCode::InternalError,
                    "Remote resource worker failed",
                )
            })?
            .map_err(|_| remote_error(PluginErrorCode::NetworkError, "Remote resource read failed"))
    }
}

struct DecryptPlayback {
    state: AppState,
    plugin_id: String,
    capability_id: String,
    session_id: SessionId,
    scope: Arc<ResourceScope>,
    remaining: Option<u64>,
    finished: bool,
}

impl Drop for DecryptPlayback {
    fn drop(&mut self) {
        self.scope.cancel();
        let state = self.state.clone();
        let plugin_id = self.plugin_id.clone();
        let capability_id = self.capability_id.clone();
        let session_id = self.session_id.clone();
        let scope = Arc::clone(&self.scope);
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            handle.spawn(async move {
                let _ = state
                    .plugin_manager
                    .invoke_capability(
                        &plugin_id,
                        &capability_id,
                        "close",
                        serde_json::json!({ "session_id": session_id }),
                        &PluginInvocationContext {
                            user: None,
                            resources: Some(scope),
                        },
                    )
                    .await;
            });
        }
    }
}

pub(crate) async fn create_decrypted_stream(
    state: &AppState,
    chapter: &Chapter,
    library: &Library,
    range_header: Option<String>,
) -> Result<(
    futures::stream::BoxStream<'static, std::io::Result<bytes::Bytes>>,
    String,
    Option<String>,
    Option<u64>,
    u64,
    u64,
    Option<u64>,
)> {
    let format_path = std::path::Path::new(&chapter.path);
    let cache_path = state.cache_manager.get_cache_path(&chapter.id);
    let selected: Option<SelectedFormat>;
    let length: u64;
    if cache_path.exists() || library.library_type == "local" {
        let source_path = if cache_path.exists() {
            cache_path.as_path()
        } else {
            format_path
        };
        selected = state
            .plugin_manager
            .select_local_format_with_operation(
                format_path,
                source_path,
                None,
                FormatOperation::OpenDecrypt,
            )
            .await?;
        length = selected
            .as_ref()
            .map(|format| {
                format
                    .scope
                    .stat(&format.input)
                    .map(|stat| stat.length.unwrap_or(0))
            })
            .transpose()
            .map_err(|error| {
                let message = error.to_string();
                TingError::plugin_execution(
                    PluginErrorKind::Runtime,
                    "create_decrypted_stream::plugin_execution",
                    message,
                )
                .with_source(anyhow::anyhow!(error))
            })?
            .unwrap_or(0);
    } else {
        let (_, total_size) = remote_reader(state, library, &chapter.path, (0, 1)).await?;
        if total_size == 0 || total_size > MAX_SAFE_INTEGER {
            return Err(TingError::plugin_execution(
                PluginErrorKind::Unavailable,
                "create_decrypted_stream::plugin_execution",
                "Remote format length unavailable".into(),
            ));
        }
        let source = Arc::new(RemoteMediaSource {
            state: state.clone(),
            library: library.clone(),
            path: chapter.path.clone(),
            length: total_size,
            runtime: tokio::runtime::Handle::current(),
        });
        selected = state
            .plugin_manager
            .select_source_format(
                format_path,
                source,
                total_size,
                None,
                FormatOperation::OpenDecrypt,
            )
            .await?;
        length = total_size;
    }
    let selected = selected.ok_or_else(|| {
        TingError::plugin_execution(
            PluginErrorKind::Runtime,
            "create_decrypted_stream::plugin_execution",
            "No format plugin matched the media source".into(),
        )
    })?;
    let plugin_id = selected.plugin_id.clone();
    let capability_id = selected.capability_id.clone();
    let scope = Arc::clone(&selected.scope);
    let input = selected.input.clone();
    let scoped = PluginInvocationContext {
        user: None,
        resources: Some(Arc::clone(&scope)),
    };
    scope.extend_readable_end(&input, length).map_err(|error| {
        let message = error.to_string();
        TingError::plugin_execution(
            PluginErrorKind::Runtime,
            "create_decrypted_stream::plugin_execution",
            message,
        )
        .with_source(anyhow::anyhow!(error))
    })?;
    let output = state
        .plugin_manager
        .invoke_capability(
            &plugin_id,
            &capability_id,
            "open_decrypt",
            serde_json::json!({ "input": input }),
            &scoped,
        )
        .await?;
    let opened: OpenDecryptResult = serde_json::from_value(output).map_err(|error| {
        let message = format!("Invalid format session: {error}");
        TingError::plugin_execution(
            PluginErrorKind::InvalidOutput,
            "create_decrypted_stream::plugin_execution",
            message,
        )
        .with_source(anyhow::anyhow!(error))
    })?;
    scope.check_session(&opened.session_id).map_err(|error| {
        let message = error.to_string();
        TingError::plugin_execution(
            PluginErrorKind::Runtime,
            "create_decrypted_stream::plugin_execution",
            message,
        )
        .with_source(anyhow::anyhow!(error))
    })?;
    let mut playback = DecryptPlayback {
        state: state.clone(),
        plugin_id,
        capability_id,
        session_id: opened.session_id,
        scope,
        remaining: None,
        finished: false,
    };
    let (start, end) = if let Some(header) = range_header {
        let Some(total) = opened.output.length else {
            return Err(TingError::invalid_request(
                InputErrorKind::UnsupportedValue,
                "create_decrypted_stream::invalid_request",
                "Format stream does not support byte ranges".into(),
            ));
        };
        let range = state.audio_streamer.parse_range_header(&header, total)?;
        (range.start, range.end)
    } else {
        (0, opened.output.length.unwrap_or(0))
    };
    if let Some(total) = opened.output.length
        && (start > end || end > total)
    {
        return Err(TingError::invalid_request(
            InputErrorKind::OutOfRange,
            "create_decrypted_stream::invalid_request",
            "Invalid format playback range".into(),
        ));
    }
    if start != 0 {
        let position = state
            .plugin_manager
            .invoke_capability(
                &playback.plugin_id,
                &playback.capability_id,
                "seek",
                serde_json::json!({ "session_id": playback.session_id, "offset": start }),
                &scoped,
            )
            .await?;
        let position: SeekResult = serde_json::from_value(position).map_err(|error| {
            let message = format!("Invalid format seek: {error}");
            TingError::plugin_execution(
                PluginErrorKind::InvalidOutput,
                "create_decrypted_stream::plugin_execution",
                message,
            )
            .with_source(anyhow::anyhow!(error))
        })?;
        if position.position != start {
            return Err(TingError::plugin_execution(
                PluginErrorKind::Runtime,
                "create_decrypted_stream::plugin_execution",
                "Format seek returned a different position".into(),
            ));
        }
    }
    playback.remaining = opened.output.length.map(|_| end.saturating_sub(start));
    let stream = futures::stream::unfold(playback, |mut playback| async move {
        if playback.finished || playback.remaining == Some(0) {
            return None;
        }
        let requested = playback
            .remaining
            .unwrap_or(MAX_MEDIA_CHUNK_BYTES)
            .min(MAX_MEDIA_CHUNK_BYTES);
        let context = PluginInvocationContext {
            user: None, resources: Some(Arc::clone(&playback.scope)),
        };
        let result: Result<bytes::Bytes> = async {
            let output = playback.state.plugin_manager.invoke_capability(
                &playback.plugin_id, &playback.capability_id, "read_chunk",
                serde_json::json!({ "session_id": playback.session_id, "max_bytes": requested }), &context,
            ).await?;
            let chunk: ReadChunkResult = serde_json::from_value(output)
                .map_err(|error| { let message = format!("Invalid format chunk: {error}"); TingError::plugin_execution(PluginErrorKind::InvalidOutput, "create_decrypted_stream::plugin_execution", message).with_source(anyhow::anyhow!(error)) })?;
            chunk.validate(requested).map_err(|error| { let message = error.to_string(); TingError::plugin_execution(PluginErrorKind::Runtime, "create_decrypted_stream::plugin_execution", message).with_source(anyhow::anyhow!(error)) })?;
            if chunk.length == 0 {
                if chunk.eof {
                    playback.finished = true;
                    return Ok(bytes::Bytes::new());
                }
                return Err(TingError::plugin_execution(PluginErrorKind::Runtime, "create_decrypted_stream::plugin_execution", "Format stream returned an empty non-EOF chunk".into()));
            }
            let chunk_ref = chunk.chunk.ok_or_else(||
                TingError::plugin_execution(PluginErrorKind::Runtime, "create_decrypted_stream::plugin_execution", "Format chunk lease missing".into()))?;
            let data = playback.scope.chunk(&chunk_ref).map_err(|error|
                { let message = error.to_string(); TingError::plugin_execution(PluginErrorKind::Runtime, "create_decrypted_stream::plugin_execution", message).with_source(anyhow::anyhow!(error)) })?;
            playback.scope.release_chunk(&chunk_ref).map_err(|error|
                { let message = error.to_string(); TingError::plugin_execution(PluginErrorKind::Runtime, "create_decrypted_stream::plugin_execution", message).with_source(anyhow::anyhow!(error)) })?;
            if data.len() as u64 != chunk.length
                || playback
                    .remaining
                    .is_some_and(|remaining| chunk.length > remaining)
            {
                return Err(TingError::plugin_execution(PluginErrorKind::InvalidOutput, "create_decrypted_stream::plugin_execution", "Invalid format chunk length".into()));
            }
            if let Some(remaining) = &mut playback.remaining {
                *remaining -= chunk.length;
            }
            playback.finished = chunk.eof;
            Ok(bytes::Bytes::copy_from_slice(&data))
        }.await;
        let ended = result.is_err();
        Some((result.map_err(std::io::Error::other), if ended {
            playback.finished = true;
            playback
        } else { playback }))
    }).boxed();
    Ok((
        stream,
        opened.output.mime_type,
        Some(opened.output.format),
        opened.output.length.map(|_| end - start),
        start,
        end,
        opened.output.length,
    ))
}
