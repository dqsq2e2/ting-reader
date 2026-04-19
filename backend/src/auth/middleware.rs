//! Authentication middleware

use crate::auth::jwt::validate_token;
use crate::core::error::{Result, TingError};
use axum::{
    extract::{Request, State},
    middleware::Next,
    response::{IntoResponse, Response},
};

/// Extension to store authenticated user info in request
#[derive(Clone, Debug)]
pub struct AuthUser {
    pub user_id: String,
    pub id: String,  // Alias for user_id for convenience
    pub username: String,
    pub role: String,
}

/// Authentication middleware
pub async fn authenticate(
    State(state): State<crate::api::handlers::AppState>,
    mut request: Request,
    next: Next,
) -> Response {
    use axum::http::header;

    // 1. Try to get token from Authorization header
    let token_from_header = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|h| h.to_str().ok())
        .and_then(|h| h.strip_prefix("Bearer ").map(|t| t.to_string()));

    // 2. If header is missing, try to get token from query parameter "token"
    let token = token_from_header.or_else(|| {
        request.uri().query().and_then(|q| {
            url::form_urlencoded::parse(q.as_bytes())
                .find(|(k, _)| k == "token")
                .map(|(_, v)| v.to_string())
        })
    });

    // 3. If no token found, return error
    let token = match token {
        Some(t) => t,
        None => {
            // Check if it's a public endpoint (optional, but good practice)
            // For now, return 401
            let error = TingError::AuthenticationError("缺少认证令牌".to_string());
            return error.into_response();
        }
    };

    // Validate token
    let claims = if let Some(key_manager) = &state.jwt_key_manager {
        // 使用密钥管理器（新方式）- 支持多密钥验证
        let validation_secrets = key_manager.get_validation_secrets().await;
        match crate::auth::jwt::validate_token_with_secrets(&token, &validation_secrets) {
            Ok(c) => c,
            Err(e) => return e.into_response(),
        }
    } else {
        // 向后兼容：使用配置文件中的密钥
        match validate_token(&token, &state.jwt_secret) {
            Ok(c) => c,
            Err(e) => return e.into_response(),
        }
    };

    // Fetch user from database
    use crate::db::repository::Repository;
    let user_id = claims.user_id;
    
    // Check if user exists
    let user_result = state.user_repo.find_by_id(&user_id).await;
    
    let user = match user_result {
        Ok(Some(u)) => u,
        Ok(None) => {
            let error = TingError::AuthenticationError("用户不存在".to_string());
            return error.into_response();
        }
        Err(e) => return e.into_response(), // Database error
    };

    // Store authenticated user in request extensions
    request.extensions_mut().insert(AuthUser {
        user_id: user.id.clone(),
        id: user.id,
        username: user.username,
        role: user.role,
    });

    next.run(request).await
}

/// Extract authenticated user from request extensions
pub fn get_auth_user(request: &Request) -> Result<AuthUser> {
    request
        .extensions()
        .get::<AuthUser>()
        .cloned()
        .ok_or_else(|| TingError::AuthenticationError("用户未认证".to_string()))
}


// Implement FromRequestParts for AuthUser to enable extraction in handlers
use axum::{
    async_trait,
    extract::FromRequestParts,
    http::request::Parts,
};

#[async_trait]
impl<S> FromRequestParts<S> for AuthUser
where
    S: Send + Sync,
{
    type Rejection = TingError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self> {
        parts
            .extensions
            .get::<AuthUser>()
            .cloned()
            .ok_or_else(|| TingError::AuthenticationError("用户未认证".to_string()))
    }
}
