//! Authentication request/response models

use serde::{Deserialize, Serialize};

/// Register request
#[derive(Debug, Deserialize)]
pub struct RegisterRequest {
    pub username: String,
    pub password: String,
}

/// Login request
#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

/// Token login request
#[derive(Debug, Deserialize)]
pub struct TokenLoginRequest {
    pub token: String,
}

/// Restored browser session request
#[derive(Debug, Default, Deserialize)]
pub struct SessionRestoreRequest {
    pub token: Option<String>,
    pub session_id: Option<String>,
}

/// Login response
#[derive(Debug, Serialize)]
pub struct LoginResponse {
    pub user: UserInfo,
    pub token: String,
}

/// User info (without password)
#[derive(Debug, Serialize)]
pub struct UserInfo {
    pub id: String,
    pub username: String,
    pub role: String,
}

/// Update user request
#[derive(Debug, Deserialize)]
pub struct UpdateUserRequest {
    pub username: Option<String>,
    pub password: Option<String>,
}

/// Generic success response
#[derive(Debug, Serialize)]
pub struct SuccessResponse {
    pub success: bool,
}
