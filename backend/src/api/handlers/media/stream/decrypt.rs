use base64::Engine;
use crate::core::error::{Result, TingError};
use crate::db::models::{Chapter, Library};
use crate::plugin::types::{DecryptionPlan, DecryptionSegment};
use crate::plugin::manager::{FormatMethod, PluginInfo};
use crate::api::handlers::AppState;
use tokio::io::{AsyncReadExt, AsyncRead};
use tokio_util::io::ReaderStream;
use futures::StreamExt;

/// Create a decrypted stream for a file using the specified plugin
pub(crate) async fn create_decrypted_stream(
    state: &AppState,
    chapter: &Chapter,
    library: &Library,
    plugin: &PluginInfo,
    range_header: Option<String>,
) -> Result<(futures::stream::BoxStream<'static, std::io::Result<bytes::Bytes>>, String, u64, u64, u64, u64)> {
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
