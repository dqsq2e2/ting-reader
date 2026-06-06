use crate::api::handlers::AppState;
use crate::core::error::{Result, TingError};
use crate::db::repository::Repository;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
};

/// 获取 HLS 播放列表
pub async fn get_hls_playlist(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
) -> Result<impl IntoResponse> {
    let temp_dir = state
        .hls_session_manager
        .get_session(&session_id)
        .await
        .ok_or_else(|| TingError::NotFound("Session not found".to_string()))?;

    let playlist_path = temp_dir.join("playlist.m3u8");

    // 等待播放列表生成（最多 10 秒）
    for _ in 0..100 {
        if playlist_path.exists() {
            let content = tokio::fs::read_to_string(&playlist_path)
                .await
                .map_err(|e| TingError::IoError(e))?;

            return Ok((
                StatusCode::OK,
                [
                    ("Content-Type", "application/vnd.apple.mpegurl"),
                    ("Access-Control-Allow-Origin", "*"),
                    ("Cache-Control", "no-cache"),
                ],
                content,
            ));
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }

    Err(TingError::ExternalError(
        "Playlist generation timeout".to_string(),
    ))
}

/// 获取 HLS 分片文件
pub async fn get_hls_segment(
    State(state): State<AppState>,
    Path((session_id, filename)): Path<(String, String)>,
) -> Result<impl IntoResponse> {
    use axum::http::header;

    // 路径穿越防护
    let filename_path = std::path::PathBuf::from(&filename);
    for component in filename_path.components() {
        match component {
            std::path::Component::Normal(_) => continue,
            _ => {
                return Err(TingError::InvalidRequest(
                    "Path traversal detected".to_string(),
                ))
            }
        }
    }

    // 只允许 .ts 和 .m3u8 文件
    let ext = filename_path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");

    if ext != "ts" && ext != "m3u8" {
        return Err(TingError::InvalidRequest("Invalid file type".to_string()));
    }

    let temp_dir = state
        .hls_session_manager
        .get_session(&session_id)
        .await
        .ok_or_else(|| TingError::NotFound("Session not found".to_string()))?;

    let segment_path = temp_dir.join(&filename);

    // 确保文件在会话目录内
    if !segment_path.starts_with(&temp_dir) {
        return Err(TingError::InvalidRequest(
            "Path traversal detected".to_string(),
        ));
    }

    if !segment_path.exists() {
        return Err(TingError::NotFound(format!(
            "Segment {} not found",
            filename
        )));
    }

    let content = tokio::fs::read(&segment_path)
        .await
        .map_err(|e| TingError::IoError(e))?;

    Ok((
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, "video/mp2t"),
            (header::ACCEPT_RANGES, "bytes"),
            (header::ACCESS_CONTROL_ALLOW_ORIGIN, "*"),
            (header::CACHE_CONTROL, "no-cache, no-store, must-revalidate"),
        ],
        content,
    ))
}

/// Seek 操作
#[derive(Debug, serde::Deserialize)]
pub struct SeekQuery {
    pub seek: Option<f64>,
}

pub async fn seek_hls_stream(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
    Query(params): Query<SeekQuery>,
) -> Result<impl IntoResponse> {
    // 获取会话数据
    let session_data = state
        .hls_session_manager
        .get_session_data(&session_id)
        .await
        .ok_or_else(|| TingError::NotFound("Session not found".to_string()))?;

    let (chapter_id, library_id, _book_id, is_strm, original_url) = session_data;

    // 终止当前 FFmpeg 进程
    state.hls_session_manager.kill_session(&session_id).await;

    // 增加序列号
    let seq = state.hls_session_manager.increment_seq(&session_id).await;

    // 清理旧的分片文件
    if let Some(temp_dir) = state.hls_session_manager.get_session(&session_id).await {
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).ok();
    }

    // 获取章节、书籍和库信息
    let chapter = state
        .chapter_repo
        .find_by_id(&chapter_id)
        .await?
        .ok_or_else(|| TingError::NotFound(format!("Chapter {} not found", chapter_id)))?;

    let library = state
        .library_repo
        .find_by_id(&library_id)
        .await?
        .ok_or_else(|| TingError::NotFound(format!("Library {} not found", library_id)))?;

    // 使用保存的 URL 或重新获取
    let input_url = if let Some(url) = original_url {
        url
    } else {
        // 重新获取输入 URL
        crate::api::handlers::media::stream::hls::get_input_url_for_seek(
            &state, &chapter, &library, is_strm,
        )
        .await?
    };

    // 获取临时目录
    let temp_dir = state
        .hls_session_manager
        .get_session(&session_id)
        .await
        .ok_or_else(|| TingError::NotFound("Session not found".to_string()))?;

    // 重新启动转码，带上 seek 参数
    let seek_time = params.seek.map(|s| s.to_string());
    crate::api::handlers::media::stream::hls::start_hls_transcoding_internal(
        &state,
        &session_id,
        &temp_dir,
        &input_url,
        is_strm,
        seek_time.as_deref(),
    )
    .await?;

    Ok((
        StatusCode::OK,
        axum::Json(serde_json::json!({
            "status": "seeked",
            "seek_time": params.seek.unwrap_or(0.0),
            "seq": seq,
            "playlist_url": format!("/api/stream/hls/{}/playlist.m3u8?seq={}", session_id, seq)
        })),
    ))
}
