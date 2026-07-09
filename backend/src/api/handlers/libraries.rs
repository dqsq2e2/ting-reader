use super::AppState;
use crate::api::models::{
    CreateLibraryRequest, FolderInfo, LibraryResponse, LibraryScanRequest, LibraryScanResponse,
    StorageRootInfo, TestWebDavRequest, TestWebDavResponse, UpdateLibraryRequest,
};
use crate::api::require_admin;
use crate::core::error::{Result, TingError};
use crate::core::local_paths::{
    discover_authorized_roots, ensure_path_inside_root, path_to_display_string,
    resolve_existing_local_library_root, resolve_local_library_path, resolve_storage_folder_target,
};
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use uuid::Uuid;

fn is_http_url(value: &str) -> bool {
    value.starts_with("http://") || value.starts_with("https://")
}

fn scraper_config_value_requires_writes(config: Option<&serde_json::Value>) -> bool {
    config
        .and_then(|value| {
            serde_json::from_value::<crate::db::models::ScraperConfig>(value.clone()).ok()
        })
        .map(|config| config.nfo_writing_enabled || config.metadata_writing_enabled)
        .unwrap_or(false)
}

fn scraper_config_str_requires_writes(config: Option<&str>) -> bool {
    config
        .and_then(|value| serde_json::from_str::<crate::db::models::ScraperConfig>(value).ok())
        .map(|config| config.nfo_writing_enabled || config.metadata_writing_enabled)
        .unwrap_or(false)
}

fn ensure_metadata_write_allowed(
    library_path: &std::path::Path,
    writes_enabled: bool,
) -> Result<()> {
    if !writes_enabled {
        return Ok(());
    }

    let metadata = std::fs::metadata(library_path)?;
    if metadata.permissions().readonly() {
        return Err(TingError::ValidationError(format!(
            "Local library path '{}' is read-only, but metadata/NFO writing is enabled",
            library_path.display()
        )));
    }

    Ok(())
}

/// Handler for GET /api/libraries - Get all libraries
pub async fn list_libraries(
    State(state): State<AppState>,
    user: crate::auth::middleware::AuthUser,
) -> Result<impl IntoResponse> {
    let libraries = if user.role == "admin" {
        state.library_repo.find_all().await?
    } else {
        state.library_repo.find_by_user_access(&user.id).await?
    };

    let library_responses: Vec<LibraryResponse> = libraries.into_iter().map(Into::into).collect();

    Ok(Json(library_responses))
}

/// Handler for POST /api/libraries - Create new library
pub async fn create_library(
    State(state): State<AppState>,
    user: crate::auth::middleware::AuthUser,
    Json(req): Json<CreateLibraryRequest>,
) -> Result<impl IntoResponse> {
    require_admin(&user)?;

    let library_type = req.library_type.trim().to_ascii_lowercase();
    let mut url = match library_type.as_str() {
        "local" => req.path.unwrap_or_default(),
        "webdav" => req.webdav_url.unwrap_or_default(),
        "rss" => req.rss_feed_url.unwrap_or_default(),
        _ => String::new(),
    }
    .trim()
    .to_string();

    let name_trimmed = req.name.trim();
    if name_trimmed.is_empty() {
        return Err(TingError::ValidationError(
            "Library name cannot be empty".to_string(),
        ));
    }

    if library_type != "local" && library_type != "webdav" && library_type != "rss" {
        return Err(TingError::ValidationError(format!(
            "Invalid library type '{}'. Must be 'local', 'webdav' or 'rss'",
            req.library_type
        )));
    }

    if library_type == "local" {
        let config = state.config.read().await;
        let canonical_path = resolve_local_library_path(&url, &config)?;
        ensure_metadata_write_allowed(
            &canonical_path,
            scraper_config_value_requires_writes(req.scraper_config.as_ref()),
        )?;
        url = path_to_display_string(&canonical_path);
    }

    if library_type == "webdav" {
        if !is_http_url(&url) {
            return Err(TingError::ValidationError(
                "WebDAV URL must start with http:// or https://".to_string(),
            ));
        }
    }

    if library_type == "rss" {
        if !is_http_url(&url) {
            return Err(TingError::ValidationError(
                "RSS feed URL must start with http:// or https://".to_string(),
            ));
        }
    }

    let encrypted_password = if library_type == "webdav" {
        if let Some(ref password) = req.webdav_password {
            if !password.is_empty() {
                Some(crate::core::crypto::encrypt(
                    password,
                    &state.encryption_key,
                )?)
            } else {
                None
            }
        } else {
            None
        }
    } else {
        None
    };

    let root_path = if library_type == "rss" {
        "/".to_string()
    } else {
        req.root_path.unwrap_or_else(|| "/".to_string())
    };

    let scraper_config = if library_type == "rss" {
        None
    } else {
        req.scraper_config.map(|v| v.to_string())
    };

    let library = crate::db::models::Library {
        id: Uuid::new_v4().to_string(),
        name: name_trimmed.to_string(),
        library_type: library_type.clone(),
        url,
        username: if library_type == "webdav" {
            req.webdav_username
        } else {
            None
        },
        password: encrypted_password,
        root_path,
        last_scanned_at: None,
        created_at: chrono::Utc::now().to_rfc3339(),
        scraper_config,
    };

    state.library_repo.create(&library).await?;

    tracing::info!(
        target: "audit::library",
        message_key = "library.created",
        message_params = %serde_json::json!({
            "actor": user.username.as_str(),
            "library_id": library.id.as_str(),
            "library_name": library.name.as_str(),
            "library_type": library.library_type.as_str(),
            "url": library.url.as_str(),
            "root_path": library.root_path.as_str(),
        }),
        actor_id = %user.id,
        actor = %user.username,
        library_id = %library.id,
        library_name = %library.name,
        library_type = %library.library_type,
        url = %library.url,
        root_path = %library.root_path,
        "Library created"
    );

    crate::core::notifications::dispatch_application_event(
        state.notification_repo.clone(),
        state.plugin_manager.clone(),
        crate::core::notifications::NotificationEventPayload::new(
            "library.created",
            "新增媒体库",
            format!("管理员 {} 创建了媒体库 {}", user.username, library.name),
            serde_json::json!({
                "actor_id": user.id,
                "actor": user.username,
                "library_id": library.id,
                "library_name": library.name,
                "library_type": library.library_type,
                "url": library.url,
                "root_path": library.root_path,
            }),
        ),
    );

    let library_path = if library.library_type == "local" {
        let config = state.config.read().await;
        let library_root = resolve_existing_local_library_root(&library, &config)?;
        path_to_display_string(&library_root)
    } else {
        library.url.clone()
    };

    let task_payload = crate::core::task_queue::TaskPayload::Custom {
        task_type: "library_scan".to_string(),
        data: serde_json::json!({
            "library_id": library.id,
            "library_path": library_path,
            "mode": crate::core::library_scanner::ScanMode::Incremental.as_str(),
        }),
    };

    let task = crate::core::task_queue::Task::new(
        format!("library_scan_{}", library.id),
        crate::core::task_queue::Priority::Normal,
        task_payload,
    );

    if let Err(e) = state.task_queue.submit(task).await {
        tracing::error!(
            library_id = %library.id,
            error = %e,
            message_key = "library.initial_scan.enqueue_failed",
            message_params = %serde_json::json!({
                "library_id": library.id,
                "error": e.to_string(),
            }),
            "Failed to enqueue initial library scan task"
        );
    }

    // Start watching the library if it's local
    if library.library_type == "local" {
        let scraper_config: crate::db::models::ScraperConfig = library
            .scraper_config
            .as_ref()
            .and_then(|json| serde_json::from_str(json).ok())
            .unwrap_or_default();

        if !scraper_config.disable_watcher {
            if let Err(e) = state
                .library_watcher
                .watch_library(&library.id, &library_path)
                .await
            {
                tracing::warn!(
                    library_id = %library.id,
                    error = %e,
                    message_key = "library.watcher.watch_failed",
                    message_params = %serde_json::json!({
                        "library_id": library.id,
                        "error": e.to_string(),
                    }),
                    "Failed to watch new library"
                );
            }
        }
    }

    Ok((StatusCode::CREATED, Json(LibraryResponse::from(library))))
}

/// Handler for PATCH /api/libraries/:id - Update library
pub async fn update_library(
    State(state): State<AppState>,
    Path(library_id): Path<String>,
    user: crate::auth::middleware::AuthUser,
    Json(req): Json<UpdateLibraryRequest>,
) -> Result<impl IntoResponse> {
    require_admin(&user)?;

    let mut library = state
        .library_repo
        .find_by_id(&library_id)
        .await?
        .ok_or_else(|| TingError::NotFound(format!("Library {} not found", library_id)))?;

    if let Some(name) = req.name {
        library.name = name;
    }

    if let Some(library_type) = req.library_type {
        let library_type = library_type.trim().to_ascii_lowercase();
        if library_type != "local" && library_type != "webdav" && library_type != "rss" {
            return Err(TingError::ValidationError(format!(
                "Invalid library type '{}'. Must be 'local', 'webdav' or 'rss'",
                library_type
            )));
        }
        library.library_type = library_type;
    }

    if library.library_type == "local" {
        if let Some(path) = req.path {
            let config = state.config.read().await;
            let canonical_path = resolve_local_library_path(&path, &config)?;
            library.url = path_to_display_string(&canonical_path);
        }
    } else if library.library_type == "webdav" {
        if let Some(webdav_url) = req.webdav_url {
            let webdav_url = webdav_url.trim().to_string();
            if !is_http_url(&webdav_url) {
                return Err(TingError::ValidationError(
                    "WebDAV URL must start with http:// or https://".to_string(),
                ));
            }
            library.url = webdav_url;
        }
    } else if library.library_type == "rss" {
        if let Some(rss_feed_url) = req.rss_feed_url {
            let rss_feed_url = rss_feed_url.trim().to_string();
            if !is_http_url(&rss_feed_url) {
                return Err(TingError::ValidationError(
                    "RSS feed URL must start with http:// or https://".to_string(),
                ));
            }
            library.url = rss_feed_url;
        }
        if !is_http_url(&library.url) {
            return Err(TingError::ValidationError(
                "RSS feed URL must start with http:// or https://".to_string(),
            ));
        }
        library.username = None;
        library.password = None;
        library.root_path = "/".to_string();
    }

    if library.library_type == "webdav" {
        if let Some(username) = req.webdav_username {
            library.username = Some(username);
        }

        if let Some(password) = req.webdav_password {
            let encrypted = crate::core::crypto::encrypt(&password, &state.encryption_key)?;
            library.password = Some(encrypted);
        }
    }

    if library.library_type == "webdav" || library.library_type == "local" {
        if let Some(root_path) = req.root_path {
            library.root_path = root_path;
        }
    }

    if library.library_type == "rss" {
        library.scraper_config = None;
    } else if let Some(config) = req.scraper_config {
        library.scraper_config = Some(config.to_string());
    }

    if library.library_type == "local" {
        let config = state.config.read().await;
        let library_path = resolve_existing_local_library_root(&library, &config)?;
        ensure_metadata_write_allowed(
            &library_path,
            scraper_config_str_requires_writes(library.scraper_config.as_deref()),
        )?;
    }

    state.library_repo.update(&library).await?;

    // Update watcher
    state.library_watcher.stop_watching(&library_id).await;
    if library.library_type == "local" {
        let scraper_config: crate::db::models::ScraperConfig = library
            .scraper_config
            .as_ref()
            .and_then(|json| serde_json::from_str(json).ok())
            .unwrap_or_default();

        if !scraper_config.disable_watcher {
            let config = state.config.read().await;
            let library_path =
                path_to_display_string(&resolve_existing_local_library_root(&library, &config)?);

            if let Err(e) = state
                .library_watcher
                .watch_library(&library.id, &library_path)
                .await
            {
                tracing::warn!(
                    library_id = %library.id,
                    error = %e,
                    message_key = "library.watcher.update_failed",
                    message_params = %serde_json::json!({
                        "library_id": library.id,
                        "error": e.to_string(),
                    }),
                    "Failed to update library watcher"
                );
            }
        }
    }

    Ok(Json(LibraryResponse::from(library)))
}

/// Handler for DELETE /api/libraries/:id - Delete library
pub async fn delete_library(
    State(state): State<AppState>,
    Path(library_id): Path<String>,
    user: crate::auth::middleware::AuthUser,
) -> Result<impl IntoResponse> {
    require_admin(&user)?;

    let library = state
        .library_repo
        .find_by_id(&library_id)
        .await?
        .ok_or_else(|| TingError::NotFound(format!("Library {} not found", library_id)))?;

    // Cancel any running tasks for this library first
    if let Err(e) = state.task_queue.cancel_library_tasks(&library_id).await {
        tracing::error!(
            library_id = %library_id,
            error = %e,
            message_key = "library.tasks.cancel_failed",
            message_params = %serde_json::json!({
                "library_id": library_id,
                "error": e.to_string(),
            }),
            "Failed to cancel library tasks"
        );
        // Continue with deletion even if cancellation fails
    }

    // Get books to clean up covers before deleting library
    let books = state
        .book_repo
        .find_by_library(&library_id)
        .await
        .unwrap_or_default();
    let covers_to_delete: Vec<String> = books
        .into_iter()
        .filter_map(|b| b.cover_url)
        .filter(|url| url.contains("temp/covers") || url.contains("storage/cache/covers"))
        .collect();

    state.library_repo.delete(&library_id).await?;

    // Cleanup any orphan books that might have been created during the deletion process
    // (e.g. by a race condition with a running scanner task)
    if let Err(e) = state.book_repo.cleanup_orphans().await {
        tracing::error!(
            library_id = %library_id,
            error = %e,
            message_key = "library.orphan_books.cleanup_failed",
            message_params = %serde_json::json!({
                "library_id": library_id,
                "error": e.to_string(),
            }),
            "Failed to clean up orphan books"
        );
    }

    // Cleanup cached covers for WebDAV libraries
    for cover_path in covers_to_delete {
        // Normalize path just in case
        let path_str = cover_path.replace('\\', "/");
        let path = std::path::Path::new(&path_str);

        // Security check: ensure we are deleting from allowed directories
        if path_str.contains("/temp/covers/") || path_str.contains("/storage/cache/covers/") {
            if path.exists() {
                if let Err(e) = std::fs::remove_file(path) {
                    tracing::warn!(
                        path = %cover_path,
                        error = %e,
                        message_key = "library.cover_cache.delete_failed",
                        message_params = %serde_json::json!({
                            "path": cover_path,
                            "error": e.to_string(),
                        }),
                        "Failed to delete cover cache"
                    );
                } else {
                    tracing::info!("Deleted orphan cover cache: {}", cover_path);
                }
            }
        }
    }

    state.library_watcher.stop_watching(&library_id).await;

    tracing::info!(
        target: "audit::library",
        message_key = "library.deleted",
        message_params = %serde_json::json!({
            "actor": user.username.as_str(),
            "library_id": library.id.as_str(),
            "library_name": library.name.as_str(),
            "library_type": library.library_type.as_str(),
            "url": library.url.as_str(),
            "root_path": library.root_path.as_str(),
        }),
        actor_id = %user.id,
        actor = %user.username,
        library_id = %library.id,
        library_name = %library.name,
        library_type = %library.library_type,
        url = %library.url,
        root_path = %library.root_path,
        "Library deleted"
    );

    crate::core::notifications::dispatch_application_event(
        state.notification_repo.clone(),
        state.plugin_manager.clone(),
        crate::core::notifications::NotificationEventPayload::new(
            "library.deleted",
            "删除媒体库",
            format!("管理员 {} 删除了媒体库 {}", user.username, library.name),
            serde_json::json!({
                "actor_id": user.id,
                "actor": user.username,
                "library_id": library.id,
                "library_name": library.name,
                "library_type": library.library_type,
                "url": library.url,
                "root_path": library.root_path,
            }),
        ),
    );

    Ok(Json(serde_json::json!({
        "success": true,
        "message": "Library deleted successfully"
    })))
}

/// Handler for POST /api/libraries/:id/scan - Scan library (create async task)
pub async fn scan_library(
    State(state): State<AppState>,
    Path(library_id): Path<String>,
    user: crate::auth::middleware::AuthUser,
    req: Option<Json<LibraryScanRequest>>,
) -> Result<impl IntoResponse> {
    require_admin(&user)?;

    let library = state
        .library_repo
        .find_by_id(&library_id)
        .await?
        .ok_or_else(|| TingError::NotFound(format!("Library {} not found", library_id)))?;

    let library_path = if library.library_type == "local" {
        let config = state.config.read().await;
        let library_root = resolve_existing_local_library_root(&library, &config)?;
        path_to_display_string(&library_root)
    } else {
        library.url.clone()
    };

    let scan_mode = req
        .and_then(|Json(body)| body.mode)
        .map(|mode| crate::core::library_scanner::ScanMode::from_str(&mode))
        .unwrap_or(crate::core::library_scanner::ScanMode::Incremental);

    let task_payload = crate::core::task_queue::TaskPayload::Custom {
        task_type: "library_scan".to_string(),
        data: serde_json::json!({
            "library_id": library.id,
            "library_path": library_path,
            "mode": scan_mode.as_str(),
        }),
    };

    let task = crate::core::task_queue::Task::new(
        format!("library_scan_{}", library.id),
        crate::core::task_queue::Priority::Normal,
        task_payload,
    );

    let submitted_task_id = state
        .task_queue
        .submit(task)
        .await
        .map_err(|e| TingError::TaskError(format!("Failed to queue scan task: {}", e)))?;

    Ok((
        StatusCode::ACCEPTED,
        Json(LibraryScanResponse {
            task_id: submitted_task_id,
            status: "queued".to_string(),
            message: format!(
                "{} scan started for '{}'",
                if scan_mode.is_full() {
                    "Full"
                } else {
                    "Incremental"
                },
                library.name
            ),
        }),
    ))
}

/// Handler for GET /api/storage/roots - Get authorized local storage roots
pub async fn get_storage_roots(
    State(state): State<AppState>,
    user: crate::auth::middleware::AuthUser,
) -> Result<impl IntoResponse> {
    require_admin(&user)?;

    let config = state.config.read().await;
    let roots: Vec<StorageRootInfo> = discover_authorized_roots(&config)
        .into_iter()
        .map(Into::into)
        .collect();

    Ok(Json(roots))
}

/// Handler for GET /api/storage/folders - Get storage folders
pub async fn get_storage_folders(
    State(state): State<AppState>,
    user: crate::auth::middleware::AuthUser,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> Result<impl IntoResponse> {
    require_admin(&user)?;

    let config = state.config.read().await;

    let sub_path = params
        .get("sub_path")
        .or_else(|| params.get("path"))
        .map(|s| s.as_str())
        .unwrap_or("");

    let root_param = params.get("root").map(|s| s.as_str());
    let (storage_root, target_path) = resolve_storage_folder_target(root_param, sub_path, &config)?;

    let mut folders = Vec::new();

    let entries = std::fs::read_dir(&target_path).map_err(|e| {
        if e.kind() == std::io::ErrorKind::PermissionDenied {
            TingError::ValidationError(format!(
                "Permission denied: Cannot access directory '{}'",
                sub_path
            ))
        } else {
            TingError::IoError(e)
        }
    })?;

    for entry in entries {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };
        let entry_path = entry.path();

        if entry_path.is_dir() {
            let canonical_entry = match std::fs::canonicalize(&entry_path) {
                Ok(path) => path,
                Err(_) => continue,
            };
            if ensure_path_inside_root(&storage_root, &canonical_entry).is_err() {
                continue;
            }

            if let Some(name) = entry_path.file_name().and_then(|n| n.to_str()) {
                if !name.starts_with('.') {
                    let relative_path = canonical_entry
                        .strip_prefix(&storage_root)
                        .unwrap_or(&canonical_entry)
                        .to_string_lossy()
                        .replace('\\', "/");

                    folders.push(FolderInfo {
                        name: name.to_string(),
                        path: relative_path,
                        is_directory: true,
                    });
                }
            }
        }
    }

    folders.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));

    Ok(Json(folders))
}

/// Handler for POST /api/libraries/test-connection - Test WebDAV connection
pub async fn test_webdav_connection(
    State(_state): State<AppState>,
    user: crate::auth::middleware::AuthUser,
    Json(req): Json<TestWebDavRequest>,
) -> Result<impl IntoResponse> {
    require_admin(&user)?;

    if !req.url.starts_with("http://") && !req.url.starts_with("https://") {
        return Ok(Json(TestWebDavResponse {
            success: false,
            message: "WebDAV URL must start with http:// or https://".to_string(),
        }));
    }

    // 拼接 URL 和 root_path
    let test_url = if let Some(root_path) = &req.root_path {
        let root_path = root_path.trim();
        if !root_path.is_empty() && root_path != "/" {
            // 确保 URL 末尾没有斜杠，root_path 开头有斜杠
            let base_url = req.url.trim_end_matches('/');
            let path = if root_path.starts_with('/') {
                root_path.to_string()
            } else {
                format!("/{}", root_path)
            };
            format!("{}{}", base_url, path)
        } else {
            req.url.clone()
        }
    } else {
        req.url.clone()
    };

    let client = match reqwest::Client::builder()
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
        .timeout(std::time::Duration::from_secs(10))
        .build() {
            Ok(c) => c,
            Err(e) => return Err(TingError::IoError(std::io::Error::new(std::io::ErrorKind::Other, e))),
        };

    // 尝试多种方法以兼容不同的 WebDAV 实现（如 Alist）
    let methods = vec![
        ("PROPFIND", Some("0")), // 标准 WebDAV 方法
        ("OPTIONS", None),       // 备用方法 1
        ("HEAD", None),          // 备用方法 2
    ];

    let mut last_error = String::new();

    for (method_name, depth_header) in methods {
        let mut request = client.request(
            reqwest::Method::from_bytes(method_name.as_bytes()).unwrap(),
            &test_url,
        );

        if let Some(depth) = depth_header {
            request = request.header("Depth", depth);
        }

        if let Some(ref username) = req.username {
            if !username.is_empty() {
                request = request.basic_auth(username, req.password.as_ref());
            }
        }

        match request.send().await {
            Ok(res) => {
                let status = res.status().as_u16();

                // 成功的状态码
                if res.status().is_success() || status == 207 {
                    return Ok(Json(TestWebDavResponse {
                        success: true,
                        message: format!("连接成功 (使用 {} 方法)", method_name),
                    }));
                }

                // 认证失败
                if status == 401 {
                    return Ok(Json(TestWebDavResponse {
                        success: false,
                        message: "连接失败: 认证失败 (401 Unauthorized)".to_string(),
                    }));
                }

                // 405 表示方法不支持，尝试下一个方法
                if status == 405 {
                    last_error = format!("{} 方法不支持 (HTTP 405)", method_name);
                    continue;
                }

                // 其他错误
                last_error = format!("HTTP {} (使用 {} 方法)", status, method_name);
            }
            Err(e) => {
                last_error = format!("{} (使用 {} 方法)", e, method_name);
            }
        }
    }

    // 所有方法都失败
    Ok(Json(TestWebDavResponse {
        success: false,
        message: format!("连接失败: {}", last_error),
    }))
}
