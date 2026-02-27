use axum::{
    extract::Request,
    http::HeaderValue,
    middleware::Next,
    response::Response,
};

/// Security headers middleware
///
/// This middleware adds security-related HTTP headers to all responses:
/// - X-Content-Type-Options: nosniff (prevents MIME type sniffing)
/// - X-Frame-Options: DENY (prevents clickjacking)
/// - X-XSS-Protection: 1; mode=block (enables XSS filter in older browsers)
/// - Strict-Transport-Security: enforces HTTPS (only in production)
/// - Content-Security-Policy: restricts resource loading
///
/// These headers help protect against common web vulnerabilities.
pub async fn security_headers_middleware(
    request: Request,
    next: Next,
) -> Response {
    // Get security configuration from request extensions
    let security_config = request
        .extensions()
        .get::<SecurityHeadersConfig>()
        .cloned();
    
    // Process the request
    let response = next.run(request).await;
    
    // Add security headers to response
    let (mut parts, body) = response.into_parts();
    
    // Always add these security headers
    parts.headers.insert(
        "X-Content-Type-Options",
        HeaderValue::from_static("nosniff"),
    );
    
    parts.headers.insert(
        "X-Frame-Options",
        HeaderValue::from_static("DENY"),
    );
    
    parts.headers.insert(
        "X-XSS-Protection",
        HeaderValue::from_static("1; mode=block"),
    );
    
    // Add Content-Security-Policy
    parts.headers.insert(
        "Content-Security-Policy",
        HeaderValue::from_static("default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data: https:; font-src 'self'; connect-src 'self'; media-src 'self'; object-src 'none'; frame-ancestors 'none';"),
    );
    
    // Add HSTS header if HTTPS is enabled
    if let Some(config) = security_config {
        if config.enable_hsts {
            let hsts_value = format!(
                "max-age={}; includeSubDomains",
                config.hsts_max_age
            );
            parts.headers.insert(
                "Strict-Transport-Security",
                HeaderValue::from_str(&hsts_value)
                    .unwrap_or_else(|_| HeaderValue::from_static("max-age=31536000; includeSubDomains")),
            );
        }
    }
    
    Response::from_parts(parts, body)
}

/// Configuration for security headers
#[derive(Clone, Debug)]
pub struct SecurityHeadersConfig {
    /// Enable HSTS (HTTP Strict Transport Security) header
    pub enable_hsts: bool,
    /// HSTS max-age in seconds (default: 1 year = 31536000)
    pub hsts_max_age: u64,
}

impl SecurityHeadersConfig {
    /// Create a new security headers configuration
    pub fn new(enable_hsts: bool, hsts_max_age: u64) -> Self {
        Self {
            enable_hsts,
            hsts_max_age,
        }
    }
    
    /// Create default configuration for development (HSTS disabled)
    pub fn development() -> Self {
        Self {
            enable_hsts: false,
            hsts_max_age: 0,
        }
    }
    
    /// Create default configuration for production (HSTS enabled)
    pub fn production() -> Self {
        Self {
            enable_hsts: true,
            hsts_max_age: 31536000, // 1 year
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        middleware,
        routing::get,
        Router,
    };
    use tower::util::ServiceExt; // For oneshot method
    
    #[tokio::test]
    async fn test_security_headers_middleware_basic() {
        let app = Router::new()
            .route("/test", get(|| async { "OK" }))
            .layer(middleware::from_fn(security_headers_middleware));
        
        let request = Request::builder()
            .uri("/test")
            .body(Body::empty())
            .unwrap();
        
        let response = app.oneshot(request).await.unwrap();
        
        // Check that security headers are present
        assert_eq!(
            response.headers().get("X-Content-Type-Options").unwrap(),
            "nosniff"
        );
        assert_eq!(
            response.headers().get("X-Frame-Options").unwrap(),
            "DENY"
        );
        assert_eq!(
            response.headers().get("X-XSS-Protection").unwrap(),
            "1; mode=block"
        );
        assert!(response.headers().contains_key("Content-Security-Policy"));
    }
    
    #[tokio::test]
    async fn test_security_headers_middleware_with_hsts_disabled() {
        let config = SecurityHeadersConfig::development();
        
        let app = Router::new()
            .route("/test", get(|| async { "OK" }))
            .layer(middleware::from_fn(move |mut req: Request<Body>, next: Next| {
                let config = config.clone();
                async move {
                    req.extensions_mut().insert(config);
                    security_headers_middleware(req, next).await
                }
            }));
        
        let request = Request::builder()
            .uri("/test")
            .body(Body::empty())
            .unwrap();
        
        let response = app.oneshot(request).await.unwrap();
        
        // HSTS header should not be present when disabled
        assert!(!response.headers().contains_key("Strict-Transport-Security"));
        
        // Other security headers should still be present
        assert!(response.headers().contains_key("X-Content-Type-Options"));
        assert!(response.headers().contains_key("X-Frame-Options"));
    }
    
    #[tokio::test]
    async fn test_security_headers_middleware_with_hsts_enabled() {
        let config = SecurityHeadersConfig::production();
        
        let app = Router::new()
            .route("/test", get(|| async { "OK" }))
            .layer(middleware::from_fn(move |mut req: Request<Body>, next: Next| {
                let config = config.clone();
                async move {
                    req.extensions_mut().insert(config);
                    security_headers_middleware(req, next).await
                }
            }));
        
        let request = Request::builder()
            .uri("/test")
            .body(Body::empty())
            .unwrap();
        
        let response = app.oneshot(request).await.unwrap();
        
        // HSTS header should be present when enabled
        let hsts_header = response.headers().get("Strict-Transport-Security").unwrap();
        let hsts_value = hsts_header.to_str().unwrap();
        
        assert!(hsts_value.contains("max-age=31536000"));
        assert!(hsts_value.contains("includeSubDomains"));
    }
    
    #[tokio::test]
    async fn test_security_headers_middleware_custom_hsts_max_age() {
        let config = SecurityHeadersConfig::new(true, 86400); // 1 day
        
        let app = Router::new()
            .route("/test", get(|| async { "OK" }))
            .layer(middleware::from_fn(move |mut req: Request<Body>, next: Next| {
                let config = config.clone();
                async move {
                    req.extensions_mut().insert(config);
                    security_headers_middleware(req, next).await
                }
            }));
        
        let request = Request::builder()
            .uri("/test")
            .body(Body::empty())
            .unwrap();
        
        let response = app.oneshot(request).await.unwrap();
        
        let hsts_header = response.headers().get("Strict-Transport-Security").unwrap();
        let hsts_value = hsts_header.to_str().unwrap();
        
        assert!(hsts_value.contains("max-age=86400"));
    }
    
    #[tokio::test]
    async fn test_security_headers_middleware_csp_header() {
        let app = Router::new()
            .route("/test", get(|| async { "OK" }))
            .layer(middleware::from_fn(security_headers_middleware));
        
        let request = Request::builder()
            .uri("/test")
            .body(Body::empty())
            .unwrap();
        
        let response = app.oneshot(request).await.unwrap();
        
        let csp_header = response.headers().get("Content-Security-Policy").unwrap();
        let csp_value = csp_header.to_str().unwrap();
        
        // Verify CSP contains expected directives
        assert!(csp_value.contains("default-src 'self'"));
        assert!(csp_value.contains("script-src 'self'"));
        assert!(csp_value.contains("object-src 'none'"));
        assert!(csp_value.contains("frame-ancestors 'none'"));
    }
    
    #[tokio::test]
    async fn test_security_headers_middleware_all_headers_present() {
        let config = SecurityHeadersConfig::production();
        
        let app = Router::new()
            .route("/test", get(|| async { "OK" }))
            .layer(middleware::from_fn(move |mut req: Request<Body>, next: Next| {
                let config = config.clone();
                async move {
                    req.extensions_mut().insert(config);
                    security_headers_middleware(req, next).await
                }
            }));
        
        let request = Request::builder()
            .uri("/test")
            .body(Body::empty())
            .unwrap();
        
        let response = app.oneshot(request).await.unwrap();
        
        // Verify all security headers are present
        let expected_headers = vec![
            "X-Content-Type-Options",
            "X-Frame-Options",
            "X-XSS-Protection",
            "Content-Security-Policy",
            "Strict-Transport-Security",
        ];
        
        for header in expected_headers {
            assert!(
                response.headers().contains_key(header),
                "Missing security header: {}",
                header
            );
        }
    }
}
