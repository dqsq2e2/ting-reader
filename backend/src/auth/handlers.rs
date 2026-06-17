//! Authentication API handlers

use crate::api::handlers::AppState;
use crate::api::utils::{request_info_from_headers, RequestInfo};
use crate::auth::jwt::{generate_token, validate_token, validate_token_with_secrets};
use crate::auth::models::{
    LoginRequest, LoginResponse, RegisterRequest, SessionRestoreRequest, SuccessResponse,
    TokenLoginRequest, UpdateUserRequest, UserInfo,
};
use crate::auth::password::{hash_password, verify_password};
use crate::core::error::{Result, TingError};
use crate::db::models::User;
use crate::db::repository::Repository;
use axum::{
    extract::{ConnectInfo, State},
    http::{header, HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use std::{
    collections::HashMap,
    net::SocketAddr,
    sync::{Mutex, OnceLock},
    time::{Duration, Instant},
};
use uuid::Uuid;

const SESSION_RESTORE_LOG_TTL: Duration = Duration::from_secs(7 * 24 * 60 * 60);
static SESSION_RESTORE_LOGS: OnceLock<Mutex<HashMap<String, Instant>>> = OnceLock::new();

fn user_info_from_user(user: &User) -> UserInfo {
    UserInfo {
        id: user.id.clone(),
        username: user.username.clone(),
        role: user.role.clone(),
    }
}

fn record_login_success(
    state: &AppState,
    user: &User,
    request_info: &RequestInfo,
    login_method: &str,
) {
    tracing::info!(
        target: "audit::login",
        user_id = %user.id,
        username = %user.username,
        real_ip = %request_info.real_ip,
        user_agent = %request_info.user_agent,
        device = %request_info.device,
        login_method = %login_method,
        "用户 '{}' 登录成功",
        user.username
    );

    crate::core::notifications::dispatch_notification_event(
        state.notification_repo.clone(),
        crate::core::notifications::NotificationEventPayload::new(
            "user.login",
            "用户登录",
            format!("用户 {} 登录成功", user.username),
            serde_json::json!({
                "userId": user.id,
                "username": user.username,
                "role": user.role,
                "realIp": request_info.real_ip,
                "userAgent": request_info.user_agent,
                "device": request_info.device,
                "loginMethod": login_method,
            }),
        ),
    );
}

fn bearer_token_from_headers(headers: &HeaderMap) -> Option<String> {
    headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .map(str::trim)
        .filter(|token| !token.is_empty())
        .map(ToOwned::to_owned)
}

fn cookie_token_from_headers(headers: &HeaderMap) -> Option<String> {
    headers
        .get(header::COOKIE)
        .and_then(|value| value.to_str().ok())
        .and_then(|cookies| {
            cookies.split(';').find_map(|cookie| {
                let (name, value) = cookie.trim().split_once('=')?;
                if name == "ting_reader_token" || name == "auth_token" {
                    Some(value.trim().to_string())
                } else {
                    None
                }
            })
        })
        .filter(|token| !token.is_empty())
}

async fn user_from_token(
    state: &AppState,
    token: &str,
    request_info: &RequestInfo,
    login_method: &str,
) -> Result<User> {
    let claims = match if let Some(key_manager) = &state.jwt_key_manager {
        let validation_secrets = key_manager.get_validation_secrets().await;
        validate_token_with_secrets(token, &validation_secrets)
    } else {
        validate_token(token, &state.jwt_secret)
    } {
        Ok(claims) => claims,
        Err(error) => {
            tracing::warn!(
                target: "audit::login",
                real_ip = %request_info.real_ip,
                user_agent = %request_info.user_agent,
                device = %request_info.device,
                login_method = %login_method,
                error = %error,
                "JWT Token 登录失败：令牌验证失败"
            );
            return Err(error);
        }
    };

    match state.user_repo.find_by_id(&claims.user_id).await? {
        Some(user) => Ok(user),
        None => {
            tracing::warn!(
                target: "audit::login",
                user_id = %claims.user_id,
                real_ip = %request_info.real_ip,
                user_agent = %request_info.user_agent,
                device = %request_info.device,
                login_method = %login_method,
                "JWT Token 登录失败：用户不存在"
            );
            Err(TingError::AuthenticationError("用户不存在".to_string()))
        }
    }
}

fn should_record_session_restore(
    user: &User,
    request_info: &RequestInfo,
    session_id: Option<&str>,
) -> bool {
    let session_key = session_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| {
            format!(
                "{}|{}|{}",
                request_info.real_ip, request_info.user_agent, request_info.device
            )
        });
    let key = format!("{}|{}", user.id, session_key);
    let now = Instant::now();
    let mut logs = SESSION_RESTORE_LOGS
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .expect("session restore log cache poisoned");

    logs.retain(|_, recorded_at| now.duration_since(*recorded_at) < SESSION_RESTORE_LOG_TTL);
    if logs.contains_key(&key) {
        return false;
    }

    logs.insert(key, now);
    true
}

/// Handler for POST /api/auth/register - User registration
pub async fn register(
    State(state): State<AppState>,
    Json(req): Json<RegisterRequest>,
) -> Result<impl IntoResponse> {
    tracing::debug!(username = %req.username, "尝试注册新用户");

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
                target: "audit::login",
                "用户 '{}' 注册成功, 角色: {}", req.username, role
            );
            Ok((StatusCode::CREATED, Json(SuccessResponse { success: true })))
        }
        Err(e) => {
            tracing::warn!(target: "audit::login", "用户 '{}' 注册失败: {}", req.username, e);
            Err(TingError::InvalidRequest("用户名已存在".to_string()))
        }
    }
}

/// Handler for POST /api/auth/login - User login
pub async fn login(
    State(state): State<AppState>,
    headers: HeaderMap,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    Json(req): Json<LoginRequest>,
) -> Result<impl IntoResponse> {
    let peer_addr = connect_info.map(|ConnectInfo(addr)| addr);
    let request_info = request_info_from_headers(&headers, peer_addr);
    tracing::debug!(username = %req.username, "尝试登录");

    // Find user by username
    let user = match state.user_repo.find_by_username(&req.username).await? {
        Some(user) => user,
        None => {
            tracing::warn!(
                target: "audit::login",
                username = %req.username,
                real_ip = %request_info.real_ip,
                user_agent = %request_info.user_agent,
                device = %request_info.device,
                "用户 '{}' 登录失败：用户不存在",
                req.username
            );
            return Err(TingError::AuthenticationError(
                "用户名或密码错误".to_string(),
            ));
        }
    };

    // Verify password
    let is_valid = verify_password(&req.password, &user.password_hash)?;
    if !is_valid {
        tracing::warn!(
            target: "audit::login",
            user_id = %user.id,
            username = %req.username,
            real_ip = %request_info.real_ip,
            user_agent = %request_info.user_agent,
            device = %request_info.device,
            "用户 '{}' 登录失败：密码错误",
            req.username
        );
        return Err(TingError::AuthenticationError(
            "用户名或密码错误".to_string(),
        ));
    }

    // Generate JWT token
    let token = if let Some(key_manager) = &state.jwt_key_manager {
        // 使用密钥管理器（新方式）
        let signing_secret = key_manager.get_signing_secret().await;
        generate_token(&user.id, &signing_secret)?
    } else {
        // 向后兼容：使用配置文件中的密钥
        generate_token(&user.id, &state.jwt_secret)?
    };

    record_login_success(&state, &user, &request_info, "password");

    Ok(Json(LoginResponse {
        user: user_info_from_user(&user),
        token,
    }))
}

/// Handler for POST /api/auth/token-login - Login with an existing JWT token
pub async fn token_login(
    State(state): State<AppState>,
    headers: HeaderMap,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    Json(req): Json<TokenLoginRequest>,
) -> Result<impl IntoResponse> {
    let peer_addr = connect_info.map(|ConnectInfo(addr)| addr);
    let request_info = request_info_from_headers(&headers, peer_addr);
    let token = req.token.trim();

    if token.is_empty() {
        tracing::warn!(
            target: "audit::login",
            real_ip = %request_info.real_ip,
            user_agent = %request_info.user_agent,
            device = %request_info.device,
            login_method = "jwt_token",
            "JWT Token 登录失败：令牌为空"
        );
        return Err(TingError::AuthenticationError(
            "JWT Token 不能为空".to_string(),
        ));
    }

    let user = user_from_token(&state, token, &request_info, "jwt_token").await?;

    record_login_success(&state, &user, &request_info, "jwt_token");

    Ok(Json(LoginResponse {
        user: user_info_from_user(&user),
        token: token.to_string(),
    }))
}

/// Handler for POST /api/auth/session-restore - Validate a stored browser session.
pub async fn session_restore(
    State(state): State<AppState>,
    headers: HeaderMap,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    body: Option<Json<SessionRestoreRequest>>,
) -> Result<impl IntoResponse> {
    let peer_addr = connect_info.map(|ConnectInfo(addr)| addr);
    let request_info = request_info_from_headers(&headers, peer_addr);
    let req = body.map(|Json(req)| req).unwrap_or_default();
    let token = req
        .token
        .as_deref()
        .map(str::trim)
        .filter(|token| !token.is_empty())
        .map(ToOwned::to_owned)
        .or_else(|| bearer_token_from_headers(&headers))
        .or_else(|| cookie_token_from_headers(&headers))
        .ok_or_else(|| TingError::AuthenticationError("登录凭证不存在".to_string()))?;

    let user = user_from_token(&state, &token, &request_info, "session_restore").await?;
    if should_record_session_restore(&user, &request_info, req.session_id.as_deref()) {
        record_login_success(&state, &user, &request_info, "session_restore");
    } else {
        tracing::debug!(
            target: "audit::login",
            user_id = %user.id,
            username = %user.username,
            login_method = "session_restore",
            "跳过重复的浏览器会话恢复登录日志"
        );
    }

    Ok(Json(LoginResponse {
        user: user_info_from_user(&user),
        token,
    }))
}

/// Handler for GET /api/me - Get current user info
pub async fn get_me(
    State(state): State<AppState>,
    user: crate::auth::middleware::AuthUser,
) -> Result<Json<UserInfo>> {
    tracing::debug!(user_id = %user.id, username = %user.username, "获取当前用户信息");

    // Fetch full user info from database
    let db_user = state
        .user_repo
        .find_by_id(&user.id)
        .await?
        .ok_or_else(|| TingError::AuthenticationError("用户不存在".to_string()))?;

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
    tracing::info!(user_id = %user.id, username = %user.username, "更新当前用户信息");

    // Fetch current user
    let mut db_user = state
        .user_repo
        .find_by_id(&user.id)
        .await?
        .ok_or_else(|| TingError::AuthenticationError("用户不存在".to_string()))?;

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

    tracing::info!(user_id = %user.id, "用户信息更新成功");

    Ok(Json(UserInfo {
        id: db_user.id,
        username: db_user.username,
        role: db_user.role,
    }))
}
