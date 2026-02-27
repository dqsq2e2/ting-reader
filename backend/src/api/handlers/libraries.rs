use crate::api::models::{
    LibraryResponse, CreateLibraryRequest, UpdateLibraryRequest, LibraryScanResponse, FolderInfo,
};
use crate::core::error::{Result, TingError};
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use uuid::Uuid;
use super::AppState;

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
    if user.role != "admin" {
        return Err(TingError::PermissionDenied("Admin access required".to_string()));
    }

    let url = if req.library_type == "local" {
        req.path.unwrap_or_default()
    } else {
        req.webdav_url.unwrap_or_default()
    };

    let name_trimmed = req.name.trim();
    if name_trimmed.is_empty() {
        return Err(TingError::ValidationError(
            "Library name cannot be empty".to_string()
        ));
    }

    if req.library_type != "local" && req.library_type != "webdav" {
        return Err(TingError::ValidationError(
            format!("Invalid library type '{}'. Must be 'local' or 'webdav'", req.library_type)
        ));
    }

    if req.library_type == "local" {
        let config = state.config.read().await;
        let storage_root = config.storage.local_storage_root.clone();
        drop(config);
        
        let full_path = storage_root.join(&url);
        
        let canonical_path = full_path.canonicalize()
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::NotFound {
                    TingError::ValidationError(format!("Path '{}' does not exist", url))
                } else {
                    TingError::IoError(e)
                }
            })?;
        
        let canonical_root = storage_root.canonicalize()
            .map_err(|e| TingError::IoError(e))?;
        
        if !canonical_path.starts_with(&canonical_root) {
            return Err(TingError::ValidationError(
                "Invalid local path".to_string()
            ));
        }
    }

    if req.library_type == "webdav" {
        if !url.starts_with("http://") && !url.starts_with("https://") {
            return Err(TingError::ValidationError(
                "WebDAV URL must start with http:// or https://".to_string()
            ));
        }
    }

    let encrypted_password = if let Some(ref password) = req.webdav_password {
        if !password.is_empty() {
            Some(crate::core::crypto::encrypt(password, &state.encryption_key)?)
        } else {
            None
        }
    } else {
        None
    };

    let root_path = req.root_path.unwrap_or_else(|| "/".to_string());

    let scraper_config = req.scraper_config.map(|v| v.to_string());

    let library = crate::db::models::Library {
        id: Uuid::new_v4().to_string(),
        name: name_trimmed.to_string(),
        library_type: req.library_type.clone(),
        url,
        username: req.webdav_username,
        password: encrypted_password,
        root_path,
        last_scanned_at: None,
        created_at: chrono::Utc::now().to_rfc3339(),
        scraper_config,
    };

    state.library_repo.create(&library).await?;

    let library_path = if library.library_type == "local" {
        let config = state.config.read().await;
        let storage_root = config.storage.local_storage_root.clone();
        drop(config);
        
        let full_path = storage_root.join(&library.url);
        full_path.to_string_lossy().to_string()
    } else {
        library.url.clone()
    };

    let task_payload = crate::core::task_queue::TaskPayload::Custom {
        task_type: "library_scan".to_string(),
        data: serde_json::json!({
            "library_id": library.id,
            "library_path": library_path,
        }),
    };
    
    let task = crate::core::task_queue::Task::new(
        format!("library_scan_{}", library.id),
        crate::core::task_queue::Priority::Normal,
        task_payload,
    );

    if let Err(e) = state.task_queue.submit(task).await {
        tracing::error!(library_id = %library.id, error = %e, "Failed to queue initial scan task");
    }

    Ok((
        StatusCode::CREATED,
        Json(LibraryResponse::from(library)),
    ))
}

/// Handler for PATCH /api/libraries/:id - Update library
pub async fn update_library(
    State(state): State<AppState>,
    Path(library_id): Path<String>,
    user: crate::auth::middleware::AuthUser,
    Json(req): Json<UpdateLibraryRequest>,
) -> Result<impl IntoResponse> {
    if user.role != "admin" {
        return Err(TingError::PermissionDenied("Admin access required".to_string()));
    }

    let mut library = state.library_repo.find_by_id(&library_id).await?
        .ok_or_else(|| TingError::NotFound(format!("Library {} not found", library_id)))?;

    if let Some(name) = req.name {
        library.name = name;
    }

    if let Some(library_type) = req.library_type {
        if library_type != "local" && library_type != "webdav" {
            return Err(TingError::ValidationError(
                format!("Invalid library type '{}'. Must be 'local' or 'webdav'", library_type)
            ));
        }
        library.library_type = library_type;
    }

    if library.library_type == "local" {
        if let Some(path) = req.path {
            let config = state.config.read().await;
            let storage_root = config.storage.local_storage_root.clone();
            drop(config);
            
            let full_path = storage_root.join(&path);
            if !full_path.exists() {
                return Err(TingError::ValidationError(
                    format!("Path '{}' does not exist", path)
                ));
            }
            library.url = path;
        }
    } else {
        if let Some(webdav_url) = req.webdav_url {
            library.url = webdav_url;
        }
    }

    if let Some(username) = req.webdav_username {
        library.username = Some(username);
    }

    if let Some(password) = req.webdav_password {
        let encrypted = crate::core::crypto::encrypt(&password, &state.encryption_key)?;
        library.password = Some(encrypted);
    }

    if let Some(root_path) = req.root_path {
        library.root_path = root_path;
    }

    if let Some(config) = req.scraper_config {
        library.scraper_config = Some(config.to_string());
    }

    state.library_repo.update(&library).await?;

    Ok(Json(LibraryResponse::from(library)))
}

/// Handler for DELETE /api/libraries/:id - Delete library
pub async fn delete_library(
    State(state): State<AppState>,
    Path(library_id): Path<String>,
    user: crate::auth::middleware::AuthUser,
) -> Result<impl IntoResponse> {
    if user.role != "admin" {
        return Err(TingError::PermissionDenied("Admin access required".to_string()));
    }

    state.library_repo.find_by_id(&library_id).await?
        .ok_or_else(|| TingError::NotFound(format!("Library {} not found", library_id)))?;

    // Cancel any running tasks for this library first
    if let Err(e) = state.task_queue.cancel_library_tasks(&library_id).await {
        tracing::error!(library_id = %library_id, error = %e, "Failed to cancel library tasks");
        // Continue with deletion even if cancellation fails
    }

    state.library_repo.delete(&library_id).await?;

    // Cleanup any orphan books that might have been created during the deletion process
    // (e.g. by a race condition with a running scanner task)
    if let Err(e) = state.book_repo.cleanup_orphans().await {
        tracing::error!(library_id = %library_id, error = %e, "Failed to cleanup orphan books");
    }

    Ok(Json(serde_json::json!({
        "message": "Library deleted successfully"
    })))
}

/// Handler for POST /api/libraries/:id/scan - Scan library (create async task)
pub async fn scan_library(
    State(state): State<AppState>,
    Path(library_id): Path<String>,
    user: crate::auth::middleware::AuthUser,
) -> Result<impl IntoResponse> {
    if user.role != "admin" {
        return Err(TingError::PermissionDenied("Admin access required".to_string()));
    }

    let library = state.library_repo.find_by_id(&library_id).await?
        .ok_or_else(|| TingError::NotFound(format!("Library {} not found", library_id)))?;

    let library_path = if library.library_type == "local" {
        let config = state.config.read().await;
        let storage_root = config.storage.local_storage_root.clone();
        drop(config);
        
        let full_path = storage_root.join(&library.url);
        full_path.to_string_lossy().to_string()
    } else {
        library.url.clone()
    };

    let task_payload = crate::core::task_queue::TaskPayload::Custom {
        task_type: "library_scan".to_string(),
        data: serde_json::json!({
            "library_id": library.id,
            "library_path": library_path,
        }),
    };
    
    let task = crate::core::task_queue::Task::new(
        format!("library_scan_{}", library.id),
        crate::core::task_queue::Priority::Normal,
        task_payload,
    );

    let submitted_task_id = state.task_queue.submit(task).await
        .map_err(|e| TingError::TaskError(format!("Failed to queue scan task: {}", e)))?;

    Ok((
        StatusCode::ACCEPTED,
        Json(LibraryScanResponse {
            task_id: submitted_task_id,
            status: "queued".to_string(),
            message: format!("Library scan started for '{}'", library.name),
        }),
    ))
}

/// Handler for GET /api/storage/folders - Get storage folders
pub async fn get_storage_folders(
    State(state): State<AppState>,
    user: crate::auth::middleware::AuthUser,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> Result<impl IntoResponse> {
    if user.role != "admin" {
        return Err(TingError::PermissionDenied("Admin access required".to_string()));
    }

    let config = state.config.read().await;
    let storage_root = config.storage.local_storage_root.clone();
    drop(config);
    
    let sub_path = params.get("subPath")
        .or_else(|| params.get("path"))
        .map(|s| s.as_str())
        .unwrap_or("");
    
    if sub_path.contains("..") {
        return Err(TingError::ValidationError("Invalid path: contains '..'".to_string()));
    }

    let target_path = if sub_path.is_empty() {
        storage_root.clone()
    } else {
        storage_root.join(sub_path)
    };

    if !storage_root.exists() {
        if let Err(e) = std::fs::create_dir_all(&storage_root) {
            return Err(TingError::IoError(e));
        }
    }
    
    if !target_path.exists() {
         return Err(TingError::ValidationError(format!("Path '{}' does not exist", sub_path)));
    }
    
    if !target_path.is_dir() {
        return Err(TingError::ValidationError(format!("Path '{}' is not a directory", sub_path)));
    }

    let mut folders = Vec::new();

    let entries = std::fs::read_dir(&target_path)
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::PermissionDenied {
                TingError::ValidationError(format!("Permission denied: Cannot access directory '{}'", sub_path))
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
            if let Some(name) = entry_path.file_name().and_then(|n| n.to_str()) {
                if !name.starts_with('.') {
                    let relative_path = if sub_path.is_empty() {
                        name.to_string()
                    } else {
                        format!("{}/{}", sub_path.replace("\\", "/"), name)
                    };
                    
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
