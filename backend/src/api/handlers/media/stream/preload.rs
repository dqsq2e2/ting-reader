use crate::api::handlers::AppState;
use crate::auth::middleware::AuthUser;
use crate::db::models::{Book, Library};
use tokio::io::AsyncReadExt;

pub(super) async fn maybe_spawn_auto_preload(
    state: &AppState,
    user: Option<&AuthUser>,
    book: &Book,
    chapter_id: &str,
    library: &Library,
) {
    // Auto Preload / Cache Logic
    if let Some(user) = user {
        // Auto-preload is available for all users; server-side auto-cache is admin only.
        if let Ok(Some(settings)) = state.settings_repo.get_by_user(&user.id).await {
            let settings_val = settings
                .settings_json
                .as_ref()
                .and_then(|s| serde_json::from_str::<serde_json::Value>(s).ok());

            let auto_preload = settings_val
                .as_ref()
                .and_then(|v| v.get("autoPreload"))
                .and_then(|v| v.as_bool())
                .unwrap_or(true);

            let auto_cache = user.role == "admin"
                && settings_val
                    .as_ref()
                    .and_then(|v| v.get("autoCache"))
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);

            if auto_preload || auto_cache {
                if let Ok(chapters) = state.chapter_repo.find_by_book(&book.id).await {
                    if let Some(pos) = chapters.iter().position(|c| c.id == chapter_id) {
                        if let Some(next_chapter) = chapters.get(pos + 1).cloned() {
                            // Spawn preload task
                            let state_clone = state.clone();
                            let next_chapter_id = next_chapter.id.clone();
                            let next_chapter_path = next_chapter.path.clone();
                            let lib_clone = library.clone();
                            let user_id = user.id.clone();

                            // Cancel any previous preload task for this user
                            {
                                let mut tasks = state.active_preload_tasks.lock().await;
                                if let Some(handle) = tasks.remove(&user_id) {
                                    handle.abort();
                                    tracing::debug!(
                                        "Cancelled previous preload task for user {}",
                                        user_id
                                    );
                                }
                            }

                            let handle = tokio::spawn(async move {
                                // Check if already in cache BEFORE starting any heavy work
                                if auto_preload {
                                    let cache = state_clone.preload_cache.read().await;
                                    if cache.contains_key(&next_chapter_id) {
                                        tracing::debug!(
                                            "Skipping automatic preload for {} - already cached",
                                            next_chapter_id
                                        );
                                        return;
                                    }
                                }

                                // Add a small delay to debounce rapid switching
                                tokio::time::sleep(std::time::Duration::from_millis(500)).await;

                                let reader_res = if lib_clone.library_type.to_lowercase() == "local"
                                {
                                    state_clone
                                        .storage_service
                                        .get_local_reader(
                                            std::path::Path::new(&next_chapter_path),
                                            None,
                                        )
                                        .await
                                        .map(|(f, s)| {
                                            (
                                                Box::new(f)
                                                    as Box<dyn tokio::io::AsyncRead + Send + Unpin>,
                                                s,
                                            )
                                        })
                                } else if lib_clone.library_type.to_lowercase() == "webdav" {
                                    state_clone
                                        .storage_service
                                        .get_webdav_reader(
                                            &lib_clone,
                                            &next_chapter_path,
                                            None,
                                            state_clone.encryption_key.as_ref(),
                                        )
                                        .await
                                } else {
                                    state_clone
                                        .storage_service
                                        .get_http_reader(&next_chapter_path, None)
                                        .await
                                };

                                match reader_res {
                                    Ok((mut r, _)) => {
                                        // For auto_preload (memory), we need to read to buffer
                                        if auto_preload {
                                            // Double check cache before reading heavy data
                                            {
                                                let cache = state_clone.preload_cache.read().await;
                                                if cache.contains_key(&next_chapter_id) {
                                                    tracing::debug!("Skipping automatic preload for {} - already cached (second check)", next_chapter_id);
                                                    return;
                                                }
                                            }

                                            let mut buf = Vec::new();
                                            if let Ok(_) = r.read_to_end(&mut buf).await {
                                                let bytes_data = bytes::Bytes::from(buf);

                                                // Limit preload cache size to prevent memory leaks
                                                {
                                                    let mut cache =
                                                        state_clone.preload_cache.write().await;
                                                    const MAX_PRELOAD_SIZE: usize = 3;

                                                    if cache.len() >= MAX_PRELOAD_SIZE {
                                                        // Find oldest entry to remove
                                                        let oldest_key = cache
                                                            .iter()
                                                            .min_by_key(|(_, (_, time))| *time)
                                                            .map(|(k, _)| k.clone());

                                                        if let Some(key) = oldest_key {
                                                            cache.remove(&key);
                                                            tracing::debug!("Evicted oldest preloaded chapter from memory: {}", key);
                                                        }
                                                    }

                                                    cache.insert(
                                                        next_chapter_id.clone(),
                                                        (
                                                            bytes_data.clone(),
                                                            std::time::Instant::now(),
                                                        ),
                                                    );
                                                }
                                                tracing::info!(
                                                    "Automatically preloaded next chapter: {}",
                                                    next_chapter_id
                                                );

                                                // If auto_cache is also enabled, use the buffer to write to disk
                                                if auto_cache
                                                    && lib_clone.library_type.to_lowercase()
                                                        != "local"
                                                {
                                                    let cache_path = state_clone
                                                        .cache_manager
                                                        .get_cache_path(&next_chapter_id);
                                                    if !cache_path.exists() {
                                                        // Use temp file to ensure atomicity and prevent race conditions
                                                        let temp_path =
                                                            cache_path.with_extension("tmp");
                                                        if let Ok(_) = tokio::fs::write(
                                                            &temp_path,
                                                            &bytes_data,
                                                        )
                                                        .await
                                                        {
                                                            if let Ok(_) = tokio::fs::rename(
                                                                &temp_path,
                                                                &cache_path,
                                                            )
                                                            .await
                                                            {
                                                                tracing::info!("Automatically cached next chapter (from buffer): {}", next_chapter_id);

                                                                // Enforce limits
                                                                let config =
                                                                    state_clone.config.read().await;
                                                                let _ = state_clone
                                                                    .cache_manager
                                                                    .enforce_limits(
                                                                        50,
                                                                        config
                                                                            .storage
                                                                            .max_disk_usage,
                                                                    )
                                                                    .await;
                                                            } else {
                                                                tracing::error!(
                                                                    chapter_id = %next_chapter_id,
                                                                    message_key = "media.cache.rename_failed",
                                                                    message_params = %serde_json::json!({
                                                                        "chapter_id": next_chapter_id,
                                                                    }),
                                                                    "Failed to rename temporary cache file"
                                                                );
                                                                let _ = tokio::fs::remove_file(
                                                                    &temp_path,
                                                                )
                                                                .await;
                                                            }
                                                        } else {
                                                            tracing::error!(
                                                                chapter_id = %next_chapter_id,
                                                                message_key = "media.cache.write_failed",
                                                                message_params = %serde_json::json!({
                                                                    "chapter_id": next_chapter_id,
                                                                }),
                                                                "Failed to write temporary cache file from buffer"
                                                            );
                                                        }
                                                    }
                                                }
                                            } else {
                                                tracing::error!(
                                                    chapter_id = %next_chapter_id,
                                                    message_key = "media.preload.read_failed",
                                                    message_params = %serde_json::json!({
                                                        "chapter_id": next_chapter_id,
                                                    }),
                                                    "Failed to read next chapter preload"
                                                );
                                            }
                                        } else if auto_cache
                                            && lib_clone.library_type.to_lowercase() != "local"
                                        {
                                            // For auto_cache ONLY (disk), stream directly to file to save memory
                                            let cache_path = state_clone
                                                .cache_manager
                                                .get_cache_path(&next_chapter_id);
                                            if !cache_path.exists() {
                                                // Create temp file first
                                                let temp_path = cache_path.with_extension("tmp");
                                                match tokio::fs::File::create(&temp_path).await {
                                                    Ok(file) => {
                                                        let mut writer =
                                                            tokio::io::BufWriter::new(file);
                                                        match tokio::io::copy(&mut r, &mut writer)
                                                            .await
                                                        {
                                                            Ok(_) => {
                                                                // Rename to final path
                                                                if let Ok(_) = tokio::fs::rename(
                                                                    &temp_path,
                                                                    &cache_path,
                                                                )
                                                                .await
                                                                {
                                                                    tracing::info!("Automatically cached next chapter (streaming): {}", next_chapter_id);

                                                                    // Enforce limits
                                                                    let config = state_clone
                                                                        .config
                                                                        .read()
                                                                        .await;
                                                                    let _ = state_clone
                                                                        .cache_manager
                                                                        .enforce_limits(
                                                                            50,
                                                                            config
                                                                                .storage
                                                                                .max_disk_usage,
                                                                        )
                                                                        .await;
                                                                } else {
                                                                    tracing::error!(
                                                                        chapter_id = %next_chapter_id,
                                                                        message_key = "media.cache.rename_failed",
                                                                        message_params = %serde_json::json!({
                                                                            "chapter_id": next_chapter_id,
                                                                        }),
                                                                        "Failed to rename temporary cache file"
                                                                    );
                                                                }
                                                            }
                                                            Err(e) => {
                                                                tracing::error!(
                                                                    chapter_id = %next_chapter_id,
                                                                    error = %e,
                                                                    message_key = "media.cache.stream_copy_failed",
                                                                    message_params = %serde_json::json!({
                                                                        "chapter_id": next_chapter_id,
                                                                        "error": e.to_string(),
                                                                    }),
                                                                    "Auto-cache stream copy failed"
                                                                );
                                                                let _ = tokio::fs::remove_file(
                                                                    &temp_path,
                                                                )
                                                                .await;
                                                            }
                                                        }
                                                    }
                                                    Err(e) => {
                                                        tracing::error!(
                                                            chapter_id = %next_chapter_id,
                                                            error = %e,
                                                            message_key = "media.cache.temp_create_failed",
                                                            message_params = %serde_json::json!({
                                                                "chapter_id": next_chapter_id,
                                                                "error": e.to_string(),
                                                            }),
                                                            "Failed to create temporary cache file"
                                                        );
                                                    }
                                                }
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        tracing::error!(
                                            message_key = "media.preload.reader_failed",
                                            message_params = %serde_json::json!({
                                                "chapter_id": next_chapter_id.as_str(),
                                                "error": e.to_string(),
                                            }),
                                            chapter_id = %next_chapter_id,
                                            error = %e,
                                            "Failed to get next chapter reader"
                                        );
                                    }
                                }
                            });

                            // Store the handle for cancellation
                            let mut tasks = state.active_preload_tasks.lock().await;
                            tasks.insert(user.id.clone(), handle);
                        }
                    }
                }
            }
        }
    }
}
