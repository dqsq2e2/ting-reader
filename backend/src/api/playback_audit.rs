use crate::api::handlers::AppState;
use crate::db::repository::Repository;
use std::{
    collections::HashMap,
    sync::{Mutex, OnceLock},
    time::{Duration, Instant},
};

static PLAYBACK_START_LOG_CACHE: OnceLock<Mutex<HashMap<String, Instant>>> = OnceLock::new();

fn should_record_playback_start(user_id: &str, chapter_id: Option<&str>) -> bool {
    let window = Duration::from_secs(10);
    let key = format!("{}:{}", user_id, chapter_id.unwrap_or("-"));
    let now = Instant::now();
    let cache = PLAYBACK_START_LOG_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    let mut cache = match cache.lock() {
        Ok(cache) => cache,
        Err(_) => return true,
    };

    cache.retain(|_, last_seen| now.duration_since(*last_seen) < window);
    if cache.contains_key(&key) {
        return false;
    }
    cache.insert(key, now);
    true
}

fn format_playback_time(seconds: f64) -> String {
    if !seconds.is_finite() || seconds < 0.0 {
        return "0:00".to_string();
    }
    let total_seconds = seconds.floor() as u64;
    let hours = total_seconds / 3600;
    let minutes = (total_seconds % 3600) / 60;
    let seconds = total_seconds % 60;
    if hours > 0 {
        format!("{}:{:02}:{:02}", hours, minutes, seconds)
    } else {
        format!("{}:{:02}", minutes, seconds)
    }
}

pub async fn record_playback_start(
    state: &AppState,
    user_id: &str,
    book_id: &str,
    chapter_id: Option<&str>,
    position: f64,
) {
    let position = if position.is_finite() {
        position.max(0.0)
    } else {
        0.0
    };
    let Ok(Some(user)) = state.user_repo.find_by_id(user_id).await else {
        return;
    };
    let Ok(Some(book)) = state.book_repo.find_by_id(book_id).await else {
        return;
    };
    let chapter = if let Some(chapter_id) = chapter_id {
        match state.chapter_repo.find_by_id(chapter_id).await {
            Ok(Some(chapter)) if chapter.book_id == book.id => Some(chapter),
            _ => None,
        }
    } else {
        None
    };
    let library = state
        .library_repo
        .find_by_id(&book.library_id)
        .await
        .ok()
        .flatten();
    if !should_record_playback_start(user_id, chapter_id) {
        return;
    }

    let book_title = book.title.clone().unwrap_or_default();
    let book_author = book.author.clone().unwrap_or_default();
    let book_narrator = book.narrator.clone().unwrap_or_default();
    let chapter_title = chapter
        .as_ref()
        .and_then(|chapter| chapter.title.clone())
        .unwrap_or_default();
    let chapter_index = chapter.as_ref().and_then(|chapter| chapter.chapter_index);
    let duration_seconds = chapter
        .as_ref()
        .and_then(|chapter| chapter.duration)
        .map(f64::from);
    let position_text = format_playback_time(position);
    let duration_text = duration_seconds
        .map(format_playback_time)
        .unwrap_or_else(|| "--:--".to_string());
    let library_id = library
        .as_ref()
        .map(|library| library.id.as_str())
        .unwrap_or(book.library_id.as_str());
    let library_name = library
        .as_ref()
        .map(|library| library.name.as_str())
        .unwrap_or("");
    let library_type = library
        .as_ref()
        .map(|library| library.library_type.as_str())
        .unwrap_or("");

    tracing::info!(
        target: "audit::playback",
        message_key = "playback.started",
        message_params = %serde_json::json!({
            "username": user.username,
            "book_title": book_title,
            "chapter_title": chapter_title,
            "position": position_text,
            "duration": duration_text,
        }),
        user_id = %user.id,
        username = %user.username,
        action = "playback_start",
        book_id = %book.id,
        book_title = %book_title,
        book_author = %book_author,
        book_narrator = %book_narrator,
        chapter_id = chapter_id.unwrap_or("-"),
        chapter_title = %chapter_title,
        chapter_index = chapter_index,
        chapter_duration_seconds = duration_seconds,
        chapter_duration_text = %duration_text,
        position_seconds = position,
        position_text = %position_text,
        library_id = %library_id,
        library_name = %library_name,
        library_type = %library_type,
        source = "progress_sync",
        "Playback started"
    );

    crate::core::notifications::dispatch_application_event(
        state.notification_repo.clone(),
        state.plugin_manager.clone(),
        crate::core::notifications::NotificationEventPayload::new(
            "playback.play",
            "播放开始",
            format!("用户 {} 开始播放 {}", user.username, book_title),
            serde_json::json!({
                "user_id": user.id,
                "username": user.username,
                "book_id": book.id,
                "book_title": book_title,
                "book_author": book_author,
                "book_narrator": book_narrator,
                "chapter_id": chapter.as_ref().map(|chapter| chapter.id.as_str()),
                "chapter_title": chapter_title,
                "chapter_index": chapter_index,
                "chapter_duration_seconds": duration_seconds,
                "chapter_duration_text": duration_text,
                "position": position,
                "position_text": position_text,
                "library_id": library_id,
                "library_name": library_name,
                "library_type": library_type,
            }),
        ),
    );
}
