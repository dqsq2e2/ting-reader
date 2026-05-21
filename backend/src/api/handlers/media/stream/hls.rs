use crate::core::error::{Result, TingError};
use crate::api::handlers::AppState;
use crate::db::models::{Chapter, Book, Library};
use axum::{http::StatusCode, response::{IntoResponse, Response}};
use tokio::process::Command;
use std::process::Stdio;
use tokio::io::AsyncReadExt;

/// 处理 HLS 转码请求
pub async fn handle_hls_request(
    state: AppState,
    chapter: Chapter,
    _book: Book,
    library: Library,
    is_strm: bool,
    seek: Option<String>,
) -> Result<Response> {
    // 1. 获取输入源 URL
    let input_url = get_input_url(&state, &chapter, &library, is_strm).await?;
    
    // 2. 创建 HLS 会话
    let session_id = state.hls_session_manager.create_session(
        chapter.id.clone(),
        library.id.clone(),
        _book.id.clone(),
        is_strm,
        Some(input_url.clone()),
    ).await.map_err(|e| {
        if e.contains("Too many") {
            TingError::ResourceLimitExceeded(e)
        } else {
            TingError::ExternalError(e)
        }
    })?;
    
    let temp_dir = state.hls_session_manager.get_session(&session_id).await
        .ok_or_else(|| TingError::ExternalError("Failed to create session".to_string()))?;

    // 3. 启动 FFmpeg 转码
    start_hls_transcoding(
        &state,
        &session_id,
        &temp_dir,
        &input_url,
        is_strm,
        seek.as_deref(),
    ).await?;
    
    // 4. 等待首分片生成
    wait_for_first_segment(&temp_dir, is_strm).await;
    
    // 5. 返回播放列表 URL（JSON 格式，前端解析后传给 ExoPlayer）
    let playlist_url = format!("/api/stream/hls/{}/playlist.m3u8", session_id);
    
    Ok((
        StatusCode::OK,
        axum::Json(serde_json::json!({
            "type": "hls",
            "session_id": session_id,
            "playlist_url": playlist_url,
            "is_strm": is_strm,
            "ready": temp_dir.join("segment_000.ts").exists()
        }))
    ).into_response())
}

/// 获取输入源 URL（复用现有逻辑）
pub async fn get_input_url_for_seek(
    state: &AppState,
    chapter: &Chapter,
    library: &Library,
    is_strm: bool,
) -> Result<String> {
    get_input_url(state, chapter, library, is_strm).await
}

/// 启动 HLS 转码（公开接口用于 Seek）
pub async fn start_hls_transcoding_internal(
    state: &AppState,
    session_id: &str,
    temp_dir: &std::path::Path,
    input_url: &str,
    is_strm: bool,
    seek: Option<&str>,
) -> Result<()> {
    start_hls_transcoding(state, session_id, temp_dir, input_url, is_strm, seek).await
}

/// 获取输入源 URL（内部使用）
async fn get_input_url(
    state: &AppState,
    chapter: &Chapter,
    library: &Library,
    is_strm: bool,
) -> Result<String> {
    if is_strm {
        // 读取 strm 文件内容
        let url = if library.library_type == "local" {
            std::fs::read_to_string(&chapter.path)
                .map_err(|e| TingError::IoError(e))?
                .trim()
                .to_string()
        } else {
            let (mut reader, _) = state.storage_service.get_webdav_reader(
                library,
                &chapter.path,
                None,
                state.encryption_key.as_ref()
            ).await?;
            
            let mut content = String::new();
            reader.read_to_string(&mut content).await?;
            content.trim().to_string()
        };
        
        if url.is_empty() || !url.starts_with("http") {
            return Err(TingError::InvalidRequest(format!("Invalid strm URL")));
        }
        
        tracing::info!("HLS 转码 strm: {}", sanitize_url(&url));
        Ok(url)
    } else if library.library_type == "local" {
        Ok(chapter.path.clone())
    } else {
        // 构建 WebDAV URL
        build_webdav_url(library, &chapter.path, Some(state.encryption_key.as_ref()))
    }
}

/// 启动 HLS 转码
async fn start_hls_transcoding(
    state: &AppState,
    session_id: &str,
    temp_dir: &std::path::Path,
    input_url: &str,
    is_strm: bool,
    seek: Option<&str>,
) -> Result<()> {
    let ffmpeg_path = state.plugin_manager.get_ffmpeg_path().await
        .ok_or_else(|| TingError::IoError(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "FFmpeg not found"
        )))?;

    let playlist_path = temp_dir.join("playlist.m3u8");
    let segment_pattern = temp_dir.join("segment_%03d.ts");
    
    let mut cmd = Command::new(&ffmpeg_path);
    cmd.arg("-y")
       .arg("-loglevel").arg("warning")
       .arg("-fflags").arg("+genpts+igndts")  // 快速启动标志
       .arg("-analyzeduration").arg("1000000")  // 减少分析时间到 1 秒
       .arg("-probesize").arg("5000000");  // 减少探测大小到 5MB
    
    // STRM 特殊参数：网络重连
    if is_strm {
        cmd.arg("-reconnect").arg("1")
           .arg("-reconnect_streamed").arg("1")
           .arg("-reconnect_delay_max").arg("5")
           .arg("-timeout").arg("10000000");
    }
    
    // Seek 支持
    if let Some(seek_time) = seek {
        cmd.arg("-ss").arg(seek_time);
    }
    
    cmd.arg("-i").arg(input_url)
       .arg("-c:a").arg("aac")
       .arg("-b:a").arg("128k")
       .arg("-ar").arg("44100")
       .arg("-ac").arg("2")
       .arg("-channel_layout").arg("stereo")  // 明确指定通道布局
       .arg("-aac_coder").arg("fast")  // 使用快速 AAC 编码器
       .arg("-vn")
       .arg("-f").arg("hls")
       .arg("-hls_time").arg("4")  // 减少到 4 秒，加快起播
       .arg("-hls_list_size").arg("0")
       .arg("-hls_playlist_type").arg("vod")  // 明确指定为 VOD 类型
       .arg("-hls_segment_type").arg("mpegts")  // 明确指定分片类型
       .arg("-hls_flags").arg("append_list+split_by_time")  // 追加模式 + 精确时间分割
       .arg("-hls_segment_filename").arg(&segment_pattern)
       .arg(&playlist_path);
    
    cmd.stdout(Stdio::null());
    cmd.stderr(Stdio::piped());
    
    let mut child = cmd.spawn().map_err(|e| TingError::IoError(e))?;
    
    // 监控 stderr
    if let Some(mut stderr) = child.stderr.take() {
        let session_id = session_id.to_string();
        tokio::spawn(async move {
            let mut buffer = String::new();
            if let Ok(_) = stderr.read_to_string(&mut buffer).await {
                if !buffer.is_empty() {
                    tracing::warn!("HLS [{}] FFmpeg: {}", session_id, buffer);
                }
            }
        });
    }
    
    // 保存进程到会话
    state.hls_session_manager.set_process(session_id, child).await;
    
    Ok(())
}

/// 等待首分片生成
async fn wait_for_first_segment(temp_dir: &std::path::Path, is_strm: bool) {
    let first_segment = temp_dir.join("segment_000.ts");
    let playlist = temp_dir.join("playlist.m3u8");
    let max_wait = if is_strm { 30 } else { 8 };  // 减少本地文件等待时间到 8 秒
    
    for _ in 0..(max_wait * 20) {  // 增加检查频率到每 50ms
        // 检查播放列表是否存在（FFmpeg 会先创建播放列表）
        if playlist.exists() && first_segment.exists() {
            if let Ok(metadata) = tokio::fs::metadata(&first_segment).await {
                // 降低文件大小要求到 512 字节（约 0.03 秒的音频）
                if metadata.len() > 512 {
                    tracing::info!("首分片已生成: {} bytes", metadata.len());
                    return;
                }
            }
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    
    tracing::warn!("等待首分片超时，但继续返回（可能仍在转码中）");
}

/// 清理 URL 中的敏感信息（用于日志）
fn sanitize_url(url: &str) -> String {
    if let Ok(parsed) = url::Url::parse(url) {
        let mut sanitized = parsed.clone();
        if parsed.username() != "" || parsed.password().is_some() {
            let _ = sanitized.set_username("***");
            let _ = sanitized.set_password(None);
            return sanitized.to_string();
        }
    }
    url.to_string()
}

/// 构建 WebDAV URL
fn build_webdav_url(
    library: &Library,
    path: &str,
    encryption_key: Option<&[u8; 32]>,
) -> Result<String> {
    // 构建 WebDAV URL
    let mut webdav_url = if path.starts_with("http://") || path.starts_with("https://") {
        url::Url::parse(path)
            .map_err(|e| TingError::ValidationError(e.to_string()))?
    } else {
        let base_url = url::Url::parse(&library.url)
            .map_err(|e| TingError::ValidationError(e.to_string()))?;
        let mut url = base_url.clone();
        
        let root = library.root_path.as_str();
        let root = if root.is_empty() { "/" } else { root };
        let root_trimmed = root.trim_matches('/');
        let rel_trimmed = path.trim_matches('/');
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
    
    // 添加认证信息
    if let (Some(username), Some(password)) = (&library.username, &library.password) {
        let decrypted_password = if let Some(key) = encryption_key {
            crate::core::crypto::decrypt(password, key)
                .unwrap_or_else(|_| password.clone())
        } else {
            password.clone()
        };
        webdav_url.set_username(username).ok();
        webdav_url.set_password(Some(&decrypted_password)).ok();
    }
    
    Ok(webdav_url.to_string())
}
