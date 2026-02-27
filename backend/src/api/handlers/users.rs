use crate::api::models::{
    UserInfoResponse, CreateUserRequest, UpdateUserRequest, UserActionResponse,
    UserSettingsResponse, UpdateUserSettingsRequest,
    FavoriteActionResponse,
    ProgressResponse, UpdateProgressRequest,
};
use crate::core::error::{Result, TingError};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use uuid::Uuid;
use super::AppState;
use crate::db::repository::Repository;

/// Handler for GET /api/users - Get all users (admin only)
pub async fn list_users(
    State(state): State<AppState>,
    user: crate::auth::middleware::AuthUser,
) -> Result<impl IntoResponse> {
    if user.role != "admin" {
        return Err(TingError::PermissionDenied("Admin access required".to_string()));
    }

    let users_list = state.user_repo.find_all().await?;
    let mut users: Vec<UserInfoResponse> = Vec::new();

    for user_model in users_list {
        let mut user_resp = UserInfoResponse::from(user_model.clone());
        user_resp.libraries_accessible = state.user_repo.get_accessible_libraries(&user_model.id).await?;
        user_resp.books_accessible = state.user_repo.get_accessible_books(&user_model.id).await?;
        users.push(user_resp);
    }

    Ok(Json(users))
}

/// Handler for POST /api/users - Create new user (admin only)
pub async fn create_user(
    State(state): State<AppState>,
    admin: crate::auth::middleware::AuthUser,
    Json(req): Json<CreateUserRequest>,
) -> Result<impl IntoResponse> {
    if admin.role != "admin" {
        return Err(TingError::PermissionDenied("Admin access required".to_string()));
    }

    if state.user_repo.find_by_username(&req.username).await?.is_some() {
        return Err(TingError::ValidationError(
            format!("Username '{}' already exists", req.username)
        ));
    }

    let password_hash = bcrypt::hash(&req.password, bcrypt::DEFAULT_COST)
        .map_err(|e| TingError::PluginExecutionError(format!("Failed to hash password: {}", e)))?;

    let new_user = crate::db::models::User {
        id: Uuid::new_v4().to_string(),
        username: req.username.clone(),
        password_hash,
        role: req.role.unwrap_or_else(|| "user".to_string()),
        created_at: chrono::Utc::now().to_rfc3339(),
    };

    state.user_repo.create(&new_user).await?;

    state.user_repo.update_permissions(
        &new_user.id,
        req.libraries_accessible.clone(),
        req.books_accessible.clone()
    ).await?;

    let mut user_resp = UserInfoResponse::from(new_user);
    user_resp.libraries_accessible = req.libraries_accessible.unwrap_or_default();
    user_resp.books_accessible = req.books_accessible.unwrap_or_default();

    Ok((
        StatusCode::CREATED,
        Json(UserActionResponse {
            message: "User created successfully".to_string(),
            user: user_resp,
        }),
    ))
}

/// Handler for PATCH /api/users/:id - Update user (admin only)
pub async fn update_user(
    State(state): State<AppState>,
    Path(user_id): Path<String>,
    admin: crate::auth::middleware::AuthUser,
    Json(req): Json<UpdateUserRequest>,
) -> Result<impl IntoResponse> {
    if admin.role != "admin" {
        return Err(TingError::PermissionDenied("Admin access required".to_string()));
    }

    let mut user = state.user_repo.find_by_id(&user_id).await?
        .ok_or_else(|| TingError::NotFound(format!("User {} not found", user_id)))?;

    if let Some(username) = req.username {
        if let Some(existing) = state.user_repo.find_by_username(&username).await? {
            if existing.id != user_id {
                return Err(TingError::ValidationError(
                    format!("Username '{}' already exists", username)
                ));
            }
        }
        user.username = username;
    }

    if let Some(password) = req.password {
        user.password_hash = bcrypt::hash(&password, bcrypt::DEFAULT_COST)
            .map_err(|e| TingError::PluginExecutionError(format!("Failed to hash password: {}", e)))?;
    }

    if let Some(role) = req.role {
        user.role = role;
    }

    state.user_repo.update(&user).await?;

    if req.libraries_accessible.is_some() || req.books_accessible.is_some() {
        state.user_repo.update_permissions(
            &user.id,
            req.libraries_accessible.clone(),
            req.books_accessible.clone()
        ).await?;
    }

    let mut user_resp = UserInfoResponse::from(user);
    user_resp.libraries_accessible = state.user_repo.get_accessible_libraries(&user_id).await?;
    user_resp.books_accessible = state.user_repo.get_accessible_books(&user_id).await?;

    Ok(Json(UserActionResponse {
        message: "User updated successfully".to_string(),
        user: user_resp,
    }))
}

/// Handler for DELETE /api/users/:id - Delete user (admin only)
pub async fn delete_user(
    State(state): State<AppState>,
    Path(user_id): Path<String>,
    admin: crate::auth::middleware::AuthUser,
) -> Result<impl IntoResponse> {
    if admin.role != "admin" {
        return Err(TingError::PermissionDenied("Admin access required".to_string()));
    }

    if admin.id == user_id {
        return Err(TingError::ValidationError(
            "Cannot delete your own account".to_string()
        ));
    }

    state.user_repo.find_by_id(&user_id).await?
        .ok_or_else(|| TingError::NotFound(format!("User {} not found", user_id)))?;

    state.user_repo.delete(&user_id).await?;

    Ok(Json(serde_json::json!({
        "message": "User deleted successfully"
    })))
}

/// Handler for GET /api/settings - Get user settings
pub async fn get_user_settings(
    State(state): State<AppState>,
    user: crate::auth::middleware::AuthUser,
) -> Result<impl IntoResponse> {
    match state.settings_repo.get_by_user(&user.id).await? {
        Some(settings) => {
            let mut response = UserSettingsResponse::from(settings);
            
            if let Some(ref json_val) = response.settings_json {
                if let Some(val) = json_val.get("sleepTimerDefault") {
                    if let Some(i) = val.as_i64() {
                        response.sleep_timer_default = i as i32;
                    }
                }
                if let Some(val) = json_val.get("autoPreload") {
                    if let Some(b) = val.as_bool() {
                        response.auto_preload = b;
                    }
                }
                if let Some(val) = json_val.get("autoCache") {
                    if let Some(b) = val.as_bool() {
                        response.auto_cache = b;
                    }
                }
                if let Some(val) = json_val.get("widgetCss") {
                    if let Some(s) = val.as_str() {
                        response.widget_css = Some(s.to_string());
                    }
                }
            }
            
            Ok(Json(response))
        }
        None => {
            let default_settings = crate::db::models::UserSettings {
                user_id: user.id.clone(),
                playback_speed: 1.0,
                theme: "auto".to_string(),
                auto_play: 1,
                skip_intro: 0,
                skip_outro: 0,
                settings_json: None,
                updated_at: chrono::Utc::now().to_rfc3339(),
            };
            Ok(Json(UserSettingsResponse::from(default_settings)))
        }
    }
}

/// Handler for POST /api/settings - Update user settings (UPSERT)
pub async fn update_user_settings(
    State(state): State<AppState>,
    user: crate::auth::middleware::AuthUser,
    Json(req): Json<UpdateUserSettingsRequest>,
) -> Result<impl IntoResponse> {
    let existing = state.settings_repo.get_by_user(&user.id).await?;
    
    let mut settings_obj = if let Some(ref s) = existing {
        if let Some(ref json_str) = s.settings_json {
            serde_json::from_str(json_str).unwrap_or(serde_json::json!({}))
        } else {
            serde_json::json!({})
        }
    } else {
        serde_json::json!({})
    };

    if let Some(sleep_timer) = req.sleep_timer_default {
        settings_obj["sleepTimerDefault"] = serde_json::json!(sleep_timer);
    }
    if let Some(auto_preload) = req.auto_preload {
        settings_obj["autoPreload"] = serde_json::json!(auto_preload);
    }
    if let Some(auto_cache) = req.auto_cache {
        settings_obj["autoCache"] = serde_json::json!(auto_cache);
    }
    if let Some(widget_css) = &req.widget_css {
        settings_obj["widgetCss"] = serde_json::json!(widget_css);
    }

    for (k, v) in req.extra {
        // Prevent recursion: Do not merge settings_json back into itself
        // Also skip system fields that shouldn't be in the settings JSON
        if k != "settings_json" && k != "user_id" && k != "updated_at" && k != "settingsJson" {
            settings_obj[k] = v;
        }
    }

    // Clean up any potential recursive nesting from previous bugs in the existing object
    if let Some(obj) = settings_obj.as_object_mut() {
        obj.remove("settings_json");
        obj.remove("settingsJson");
        obj.remove("user_id");
        obj.remove("updated_at");
    }

    let settings = crate::db::models::UserSettings {
        user_id: user.id.clone(),
        playback_speed: req.playback_speed.unwrap_or_else(|| {
            existing.as_ref().map(|s| s.playback_speed).unwrap_or(1.0)
        }),
        theme: req.theme.unwrap_or_else(|| {
            existing.as_ref().map(|s| s.theme.clone()).unwrap_or_else(|| "auto".to_string())
        }),
        auto_play: req.auto_play.map(|v| if v { 1 } else { 0 }).unwrap_or_else(|| {
            existing.as_ref().map(|s| s.auto_play).unwrap_or(1)
        }),
        skip_intro: req.skip_intro.unwrap_or_else(|| {
            existing.as_ref().map(|s| s.skip_intro).unwrap_or(0)
        }),
        skip_outro: req.skip_outro.unwrap_or_else(|| {
            existing.as_ref().map(|s| s.skip_outro).unwrap_or(0)
        }),
        settings_json: Some(settings_obj.to_string()),
        updated_at: chrono::Utc::now().to_rfc3339(),
    };

    state.settings_repo.upsert(&settings).await?;

    let mut response = UserSettingsResponse::from(settings);
    if let Some(ref json_val) = response.settings_json {
        if let Some(val) = json_val.get("sleepTimerDefault") {
            if let Some(i) = val.as_i64() {
                response.sleep_timer_default = i as i32;
            }
        }
        if let Some(val) = json_val.get("autoPreload") {
            if let Some(b) = val.as_bool() {
                response.auto_preload = b;
            }
        }
        if let Some(val) = json_val.get("autoCache") {
            if let Some(b) = val.as_bool() {
                response.auto_cache = b;
            }
        }
        if let Some(val) = json_val.get("widgetCss") {
            if let Some(s) = val.as_str() {
                response.widget_css = Some(s.to_string());
            }
        }
    }

    Ok(Json(response))
}

/// Handler for GET /api/favorites - Get user's favorites
pub async fn get_favorites(
    State(state): State<AppState>,
    user: crate::auth::middleware::AuthUser,
) -> Result<impl IntoResponse> {
    let favorites_list = state.favorite_repo.get_by_user(&user.id).await?;
    
    let mut books = Vec::new();
    
    for fav in favorites_list {
        if let Some(book) = state.book_repo.find_by_id(&fav.book_id).await? {
            let mut book_resp = crate::api::models::BookResponse::from(book);
            book_resp.is_favorite = true;
            books.push(book_resp);
        }
    }

    Ok(Json(books))
}

/// Handler for POST /api/favorites/:bookId - Add book to favorites
pub async fn add_favorite(
    State(state): State<AppState>,
    Path(book_id): Path<String>,
    user: crate::auth::middleware::AuthUser,
) -> Result<impl IntoResponse> {
    state.book_repo.find_by_id(&book_id).await?
        .ok_or_else(|| TingError::NotFound(format!("Book {} not found", book_id)))?;

    if state.favorite_repo.is_favorited(&user.id, &book_id).await? {
        return Ok((
            StatusCode::OK,
            Json(FavoriteActionResponse {
                message: "Book is already in favorites".to_string(),
            }),
        ));
    }

    let favorite = crate::db::models::Favorite {
        id: Uuid::new_v4().to_string(),
        user_id: user.id.clone(),
        book_id: book_id.clone(),
        created_at: chrono::Utc::now().to_rfc3339(),
    };

    state.favorite_repo.add(&favorite).await?;

    Ok((
        StatusCode::CREATED,
        Json(FavoriteActionResponse {
            message: "Book added to favorites".to_string(),
        }),
    ))
}

/// Handler for DELETE /api/favorites/:bookId - Remove book from favorites
pub async fn remove_favorite(
    State(state): State<AppState>,
    Path(book_id): Path<String>,
    user: crate::auth::middleware::AuthUser,
) -> Result<impl IntoResponse> {
    state.favorite_repo.remove(&user.id, &book_id).await?;

    Ok(Json(FavoriteActionResponse {
        message: "Book removed from favorites".to_string(),
    }))
}

/// Handler for GET /api/progress/recent - Get recent playback progress
pub async fn get_recent_progress(
    State(state): State<AppState>,
    user: crate::auth::middleware::AuthUser,
) -> Result<impl IntoResponse> {
    let progress_list = state.progress_repo.get_recent_enriched(&user.id, 4).await?;
    let progress: Vec<ProgressResponse> = progress_list.into_iter().map(|(p, b_title, c_url, l_id, c_title, c_dur)| {
        let mut response = ProgressResponse::from(p);
        response.book_title = b_title;
        response.cover_url = c_url;
        response.library_id = l_id;
        response.chapter_title = c_title;
        response.chapter_duration = c_dur;
        response
    }).collect();

    Ok(Json(progress))
}

/// Handler for GET /api/progress/:bookId - Get book playback progress
pub async fn get_book_progress(
    State(state): State<AppState>,
    Path(book_id): Path<String>,
    user: crate::auth::middleware::AuthUser,
) -> Result<impl IntoResponse> {
    match state.progress_repo.get_by_book(&user.id, &book_id).await? {
        Some(progress) => {
            Ok(Json(ProgressResponse::from(progress)))
        }
        None => {
            Err(TingError::NotFound(format!("Progress not found for book {}", book_id)))
        }
    }
}

/// Handler for POST /api/progress - Update playback progress (UPSERT)
pub async fn update_progress(
    State(state): State<AppState>,
    user: crate::auth::middleware::AuthUser,
    Json(req): Json<UpdateProgressRequest>,
) -> Result<impl IntoResponse> {
    let book = state.book_repo.find_by_id(&req.book_id).await?
        .ok_or_else(|| TingError::NotFound(format!("Book {} not found", req.book_id)))?;

    if let Some(ref chapter_id) = req.chapter_id {
        let chapter = state.chapter_repo.find_by_id(chapter_id).await?
            .ok_or_else(|| TingError::NotFound(format!("Chapter {} not found", chapter_id)))?;
        
        if chapter.book_id != book.id {
            return Err(TingError::ValidationError(
                "Chapter does not belong to the specified book".to_string()
            ));
        }
    }

    let progress = crate::db::models::Progress {
        id: Uuid::new_v4().to_string(),
        user_id: user.id.clone(),
        book_id: req.book_id.clone(),
        chapter_id: req.chapter_id.clone(),
        position: req.position,
        duration: req.duration,
        updated_at: chrono::Utc::now().to_rfc3339(),
    };

    state.progress_repo.upsert(&progress).await?;

    Ok((StatusCode::OK, Json(ProgressResponse::from(progress))))
}
