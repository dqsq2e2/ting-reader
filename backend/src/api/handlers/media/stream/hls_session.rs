use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::process::Child;
use tokio::sync::RwLock;
use uuid::Uuid;

/// HLS 会话信息
pub struct HlsSession {
    pub session_id: String,
    pub chapter_id: String,
    pub temp_dir: PathBuf,
    pub ffmpeg_process: Option<Child>,
    pub created_at: std::time::Instant,
    pub last_accessed: std::time::Instant,
    pub seq: u32,
    pub is_strm: bool,
    pub original_url: Option<String>,
    pub library_id: String,
    pub book_id: String,
}

/// HLS 会话管理器
pub struct HlsSessionManager {
    sessions: Arc<RwLock<HashMap<String, HlsSession>>>,
    base_temp_dir: PathBuf,
    max_concurrent: usize,
}

impl HlsSessionManager {
    /// 创建新的会话管理器
    pub fn new(base_temp_dir: PathBuf) -> Self {
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
            base_temp_dir,
            max_concurrent: 3,
        }
    }

    /// 创建新的 HLS 会话
    pub async fn create_session(
        &self,
        chapter_id: String,
        library_id: String,
        book_id: String,
        is_strm: bool,
        url: Option<String>,
    ) -> Result<String, String> {
        // 先清理已完成的会话（FFmpeg 进程已退出）
        self.cleanup_finished_sessions().await;

        let sessions = self.sessions.read().await;

        // 检查并发限制：只计算真正在运行的进程
        let active_count = sessions
            .values()
            .filter(|s| {
                if let Some(child) = &s.ffmpeg_process {
                    // 检查进程是否还在运行
                    child.id().is_some()
                } else {
                    false
                }
            })
            .count();

        if active_count >= self.max_concurrent {
            tracing::warn!(
                message_key = "media.hls.concurrent_limit",
                message_params = %serde_json::json!({
                    "active_count": active_count,
                    "max_concurrent": self.max_concurrent,
                }),
                active_count = active_count,
                max_concurrent = self.max_concurrent,
                "HLS concurrent session limit reached"
            );
            return Err("Too many concurrent transcoding sessions".to_string());
        }

        drop(sessions);

        // 生成会话 ID
        let session_id = Uuid::new_v4().to_string();
        let temp_dir = self.base_temp_dir.join(&session_id);

        // 创建临时目录
        std::fs::create_dir_all(&temp_dir)
            .map_err(|e| format!("Failed to create temp directory: {}", e))?;

        let session = HlsSession {
            session_id: session_id.clone(),
            chapter_id,
            library_id,
            book_id,
            temp_dir,
            ffmpeg_process: None,
            created_at: std::time::Instant::now(),
            last_accessed: std::time::Instant::now(),
            seq: 0,
            is_strm,
            original_url: url,
        };

        self.sessions
            .write()
            .await
            .insert(session_id.clone(), session);
        tracing::info!(
            "Created HLS session: {} (active: {}/{})",
            session_id,
            active_count,
            self.max_concurrent
        );
        Ok(session_id)
    }

    /// 获取会话的临时目录
    pub async fn get_session(&self, session_id: &str) -> Option<PathBuf> {
        let mut sessions = self.sessions.write().await;
        if let Some(session) = sessions.get_mut(session_id) {
            session.last_accessed = std::time::Instant::now();
            Some(session.temp_dir.clone())
        } else {
            None
        }
    }

    /// 设置会话的 FFmpeg 进程
    pub async fn set_process(&self, session_id: &str, child: Child) {
        let mut sessions = self.sessions.write().await;
        if let Some(session) = sessions.get_mut(session_id) {
            session.ffmpeg_process = Some(child);
        }
    }

    /// 终止会话的 FFmpeg 进程
    pub async fn kill_session(&self, session_id: &str) {
        let mut sessions = self.sessions.write().await;
        if let Some(session) = sessions.get_mut(session_id) {
            if let Some(mut child) = session.ffmpeg_process.take() {
                let _ = child.kill().await;
                tracing::info!("Terminated FFmpeg process for HLS session {}", session_id);
            }
        }
    }

    /// 增加会话的序列号（用于 Seek）
    pub async fn increment_seq(&self, session_id: &str) -> u32 {
        let mut sessions = self.sessions.write().await;
        if let Some(session) = sessions.get_mut(session_id) {
            session.seq += 1;
            session.seq
        } else {
            0
        }
    }

    /// 清理已完成的会话（FFmpeg 进程已退出）
    async fn cleanup_finished_sessions(&self) {
        let mut sessions = self.sessions.write().await;
        let mut finished = Vec::new();

        for (id, session) in sessions.iter_mut() {
            if let Some(child) = &mut session.ffmpeg_process {
                // 尝试检查进程状态（非阻塞）
                match child.try_wait() {
                    Ok(Some(_status)) => {
                        // 进程已退出
                        finished.push(id.clone());
                    }
                    Ok(None) => {
                        // 进程仍在运行
                    }
                    Err(_) => {
                        // 检查失败，假设进程已退出
                        finished.push(id.clone());
                    }
                }
            }
        }

        // 清理已完成的会话
        for id in finished {
            if let Some(mut session) = sessions.remove(&id) {
                session.ffmpeg_process = None;
                tracing::info!("Cleaning up completed HLS session: {}", id);

                // 重新插入会话（保留会话信息，但移除进程）
                sessions.insert(id, session);
            }
        }
    }

    /// 清理过期的会话（30 分钟未访问）
    pub async fn cleanup_expired(&self) {
        let mut sessions = self.sessions.write().await;
        let now = std::time::Instant::now();

        // 找出过期的会话
        let expired: Vec<String> = sessions
            .iter()
            .filter(|(_, s)| now.duration_since(s.last_accessed).as_secs() > 1800)
            .map(|(id, _)| id.clone())
            .collect();

        // 清理过期会话
        for id in expired {
            if let Some(mut session) = sessions.remove(&id) {
                // 终止 FFmpeg 进程
                if let Some(mut child) = session.ffmpeg_process.take() {
                    let _ = child.kill().await;
                }

                // 删除临时文件
                let _ = std::fs::remove_dir_all(&session.temp_dir);

                tracing::info!("Cleaning up expired HLS session: {}", id);
            }
        }
    }

    /// 获取会话信息（用于调试和监控）
    pub async fn get_session_info(
        &self,
        session_id: &str,
    ) -> Option<(
        String,
        String,
        String,
        String,
        bool,
        u32,
        std::time::Instant,
    )> {
        let sessions = self.sessions.read().await;
        sessions.get(session_id).map(|s| {
            (
                s.session_id.clone(),
                s.chapter_id.clone(),
                s.library_id.clone(),
                s.book_id.clone(),
                s.is_strm,
                s.seq,
                s.created_at,
            )
        })
    }

    /// 获取所有活跃会话的统计信息
    pub async fn get_stats(&self) -> (usize, usize) {
        let sessions = self.sessions.read().await;
        let total = sessions.len();
        let active = sessions
            .values()
            .filter(|s| {
                if let Some(child) = &s.ffmpeg_process {
                    child.id().is_some()
                } else {
                    false
                }
            })
            .count();
        (total, active)
    }

    /// 获取会话的原始 URL（用于重新启动转码）
    pub async fn get_original_url(&self, session_id: &str) -> Option<String> {
        let sessions = self.sessions.read().await;
        sessions
            .get(session_id)
            .and_then(|s| s.original_url.clone())
    }

    /// 获取会话的完整信息（用于 Seek 重启）
    pub async fn get_session_data(
        &self,
        session_id: &str,
    ) -> Option<(String, String, String, bool, Option<String>)> {
        let sessions = self.sessions.read().await;
        sessions.get(session_id).map(|s| {
            (
                s.chapter_id.clone(),
                s.library_id.clone(),
                s.book_id.clone(),
                s.is_strm,
                s.original_url.clone(),
            )
        })
    }
}
