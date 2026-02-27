//! Authentication API handlers

use crate::api::handlers::AppState;
use crate::auth::jwt::generate_token;
use crate::auth::models::{
    LoginRequest, LoginResponse, RegisterRequest, SuccessResponse, UpdateUserRequest, UserInfo,
};
use crate::auth::password::{hash_password, verify_password};
use crate::core::error::{Result, TingError};
use crate::db::models::User;
use crate::db::repository::Repository;
use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use uuid::Uuid;

/// Handler for POST /api/auth/register - User registration
pub async fn register(
    State(state): State<AppState>,
    Json(req): Json<RegisterRequest>,
) -> Result<impl IntoResponse> {
    tracing::info!(username = %req.username, "User registration attempt");

    // Check if this is the first user (will be admin)
    let user_count = state.user_repo.count().await?;
    let role = if user_count == 0 { "admin" } else { "user" };

    // Hash password
    let password_hash = hash_password(&req.password)?;

    // Create user
    let user_id = Uuid::new_v4().to_string();
    let created_at = chrono::Utc::now().to_rfc3339();

    let user = User {
        id: user_id.clone(),
        username: req.username.clone(),
        password_hash,
        role: role.to_string(),
        created_at,
    };

    // Try to create user (will fail if username exists due to UNIQUE constraint)
    match state.user_repo.create(&user).await {
        Ok(_) => {
            tracing::info!(
                user_id = %user_id,
                username = %req.username,
                role = %role,
                "User registered successfully"
            );
            Ok((
                StatusCode::CREATED,
                Json(SuccessResponse { success: true }),
            ))
        }
        Err(e) => {
            tracing::warn!(username = %req.username, error = %e, "Registration failed");
            Err(TingError::InvalidRequest(
                "Username already exists".to_string(),
            ))
        }
    }
}

/// Handler for POST /api/auth/login - User login
pub async fn login(
    State(state): State<AppState>,
    Json(req): Json<LoginRequest>,
) -> Result<impl IntoResponse> {
    tracing::info!(username = %req.username, "Login attempt");

    // Find user by username
    let user = state
        .user_repo
        .find_by_username(&req.username)
        .await?
        .ok_or_else(|| TingError::AuthenticationError("Invalid credentials".to_string()))?;

    // Verify password
    let is_valid = verify_password(&req.password, &user.password_hash)?;
    if !is_valid {
        tracing::warn!(username = %req.username, "Invalid password");
        return Err(TingError::AuthenticationError("Invalid credentials".to_string()));
    }

    // Generate JWT token
    let token = generate_token(&user.id, &state.jwt_secret)?;

    tracing::info!(user_id = %user.id, username = %user.username, "Login successful");

    Ok(Json(LoginResponse {
        user: UserInfo {
            id: user.id,
            username: user.username,
            role: user.role,
        },
        token,
    }))
}

/// Handler for GET /api/me - Get current user info
pub async fn get_me(
    State(state): State<AppState>,
    user: crate::auth::middleware::AuthUser,
) -> Result<Json<UserInfo>> {
    tracing::info!(user_id = %user.id, "Getting current user info");

    // Fetch full user info from database
    let db_user = state.user_repo.find_by_id(&user.id).await?
        .ok_or_else(|| TingError::AuthenticationError("User not found".to_string()))?;

    Ok(Json(UserInfo {
        id: db_user.id,
        username: db_user.username,
        role: db_user.role,
    }))
}

/// Handler for PATCH /api/me - Update current user info
pub async fn update_me(
    State(state): State<AppState>,
    user: crate::auth::middleware::AuthUser,
    Json(req): Json<UpdateUserRequest>,
) -> Result<Json<UserInfo>> {
    tracing::info!(user_id = %user.id, "Updating current user info");

    // Fetch current user
    let mut db_user = state.user_repo.find_by_id(&user.id).await?
        .ok_or_else(|| TingError::AuthenticationError("User not found".to_string()))?;

    // Update username if provided
    if let Some(new_username) = req.username {
        if !new_username.is_empty() {
            db_user.username = new_username;
        }
    }

    // Update password if provided
    if let Some(new_password) = req.password {
        if !new_password.is_empty() {
            db_user.password_hash = hash_password(&new_password)?;
        }
    }

    // Save updated user
    state.user_repo.update(&db_user).await?;

    tracing::info!(user_id = %user.id, "User info updated successfully");

    Ok(Json(UserInfo {
        id: db_user.id,
        username: db_user.username,
        role: db_user.role,
    }))
}
