use axum::{
    extract::Request,
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;

/// HTTP header name for authorization
pub const AUTHORIZATION_HEADER: &str = "Authorization";

/// Authentication middleware that validates Bearer tokens
///
/// This middleware:
/// - Checks for the Authorization header with Bearer token format
/// - Validates the token against the configured API key
/// - Returns 401 Unauthorized for invalid/missing tokens
/// - Allows requests to proceed if authentication is disabled in config
///
/// The middleware should be applied selectively to routes that require authentication.
/// Public endpoints (like /health) should not have this middleware applied.
pub async fn auth_middleware(
    request: Request,
    next: Next,
) -> Result<Response, AuthError> {
    // Extract the Authorization header
    let auth_header = request
        .headers()
        .get(AUTHORIZATION_HEADER)
        .and_then(|h| h.to_str().ok());
    
    // Get the API key from request extensions (injected by the router)
    let api_key = request
        .extensions()
        .get::<ApiKey>()
        .ok_or(AuthError::ConfigurationError)?;
    
    // If authentication is disabled, allow the request
    if !api_key.enabled {
        return Ok(next.run(request).await);
    }
    
    // Parse the Bearer token
    let token = auth_header
        .and_then(|h| h.strip_prefix("Bearer "))
        .ok_or(AuthError::MissingToken)?;
    
    // Validate the token
    if token != api_key.key {
        return Err(AuthError::InvalidToken);
    }
    
    // Token is valid, proceed with the request
    Ok(next.run(request).await)
}

/// Extension type for storing API key configuration in request extensions
#[derive(Clone, Debug)]
pub struct ApiKey {
    pub enabled: bool,
    pub key: String,
}

impl ApiKey {
    /// Create a new ApiKey configuration
    pub fn new(enabled: bool, key: String) -> Self {
        Self { enabled, key }
    }
}

/// Authentication errors
#[derive(Debug)]
pub enum AuthError {
    /// No Authorization header or invalid format
    MissingToken,
    /// Token does not match configured API key
    InvalidToken,
    /// Authentication configuration not found
    ConfigurationError,
}

impl IntoResponse for AuthError {
    fn into_response(self) -> Response {
        let (status, error_message) = match self {
            AuthError::MissingToken => (
                StatusCode::UNAUTHORIZED,
                "Missing or invalid Authorization header. Expected format: 'Authorization: Bearer <token>'",
            ),
            AuthError::InvalidToken => (
                StatusCode::UNAUTHORIZED,
                "Invalid authentication token",
            ),
            AuthError::ConfigurationError => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Authentication configuration error",
            ),
        };
        
        let body = Json(json!({
            "error": "AuthenticationError",
            "message": error_message,
        }));
        
        (status, body).into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        http::StatusCode,
        middleware,
        response::IntoResponse,
        routing::get,
        Router,
    };
    use tower::util::ServiceExt; // For oneshot method
    
    async fn protected_handler() -> impl IntoResponse {
        (StatusCode::OK, "Protected resource")
    }
    
    #[tokio::test]
    async fn test_auth_middleware_with_valid_token() {
        let api_key = ApiKey::new(true, "test-secret-key".to_string());
        
        let app = Router::new()
            .route("/protected", get(protected_handler))
            .layer(middleware::from_fn(move |mut req: Request<Body>, next: Next| {
                let api_key = api_key.clone();
                async move {
                    req.extensions_mut().insert(api_key);
                    auth_middleware(req, next).await
                }
            }));
        
        let request = Request::builder()
            .uri("/protected")
            .header(AUTHORIZATION_HEADER, "Bearer test-secret-key")
            .body(Body::empty())
            .unwrap();
        
        let response = app.oneshot(request).await.unwrap();
        
        assert_eq!(response.status(), StatusCode::OK);
    }
    
    #[tokio::test]
    async fn test_auth_middleware_with_invalid_token() {
        let api_key = ApiKey::new(true, "test-secret-key".to_string());
        
        let app = Router::new()
            .route("/protected", get(protected_handler))
            .layer(middleware::from_fn(move |mut req: Request<Body>, next: Next| {
                let api_key = api_key.clone();
                async move {
                    req.extensions_mut().insert(api_key);
                    auth_middleware(req, next).await
                }
            }));
        
        let request = Request::builder()
            .uri("/protected")
            .header(AUTHORIZATION_HEADER, "Bearer wrong-token")
            .body(Body::empty())
            .unwrap();
        
        let response = app.oneshot(request).await.unwrap();
        
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }
    
    #[tokio::test]
    async fn test_auth_middleware_with_missing_token() {
        let api_key = ApiKey::new(true, "test-secret-key".to_string());
        
        let app = Router::new()
            .route("/protected", get(protected_handler))
            .layer(middleware::from_fn(move |mut req: Request<Body>, next: Next| {
                let api_key = api_key.clone();
                async move {
                    req.extensions_mut().insert(api_key);
                    auth_middleware(req, next).await
                }
            }));
        
        let request = Request::builder()
            .uri("/protected")
            .body(Body::empty())
            .unwrap();
        
        let response = app.oneshot(request).await.unwrap();
        
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }
    
    #[tokio::test]
    async fn test_auth_middleware_with_invalid_format() {
        let api_key = ApiKey::new(true, "test-secret-key".to_string());
        
        let app = Router::new()
            .route("/protected", get(protected_handler))
            .layer(middleware::from_fn(move |mut req: Request<Body>, next: Next| {
                let api_key = api_key.clone();
                async move {
                    req.extensions_mut().insert(api_key);
                    auth_middleware(req, next).await
                }
            }));
        
        let request = Request::builder()
            .uri("/protected")
            .header(AUTHORIZATION_HEADER, "Basic dGVzdDp0ZXN0")
            .body(Body::empty())
            .unwrap();
        
        let response = app.oneshot(request).await.unwrap();
        
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }
    
    #[tokio::test]
    async fn test_auth_middleware_disabled() {
        let api_key = ApiKey::new(false, "test-secret-key".to_string());
        
        let app = Router::new()
            .route("/protected", get(protected_handler))
            .layer(middleware::from_fn(move |mut req: Request<Body>, next: Next| {
                let api_key = api_key.clone();
                async move {
                    req.extensions_mut().insert(api_key);
                    auth_middleware(req, next).await
                }
            }));
        
        // Request without token should succeed when auth is disabled
        let request = Request::builder()
            .uri("/protected")
            .body(Body::empty())
            .unwrap();
        
        let response = app.oneshot(request).await.unwrap();
        
        assert_eq!(response.status(), StatusCode::OK);
    }
}
