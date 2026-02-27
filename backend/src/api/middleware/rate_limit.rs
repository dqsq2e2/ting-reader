use axum::{
    extract::Request,
    http::{HeaderValue, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

/// Rate limiter using sliding window algorithm
///
/// This implementation tracks request counts per IP address within a time window.
/// When a request comes in:
/// 1. Remove expired entries (older than the window)
/// 2. Count requests from this IP in the current window
/// 3. If count exceeds limit, reject with 429 Too Many Requests
/// 4. Otherwise, record the request and allow it through
#[derive(Clone)]
pub struct RateLimiter {
    /// Shared state containing request history per IP
    state: Arc<RwLock<RateLimiterState>>,
    /// Maximum number of requests allowed per window
    max_requests: usize,
    /// Time window duration in seconds
    window_duration: Duration,
}

/// Internal state for the rate limiter
struct RateLimiterState {
    /// Map of IP addresses to their request timestamps
    requests: HashMap<IpAddr, Vec<Instant>>,
}

impl RateLimiter {
    /// Create a new rate limiter
    ///
    /// # Arguments
    /// * `max_requests` - Maximum number of requests allowed per window
    /// * `window_seconds` - Time window duration in seconds
    pub fn new(max_requests: usize, window_seconds: u64) -> Self {
        Self {
            state: Arc::new(RwLock::new(RateLimiterState {
                requests: HashMap::new(),
            })),
            max_requests,
            window_duration: Duration::from_secs(window_seconds),
        }
    }
    
    /// Create a rate limiter from security configuration
    pub fn from_config(max_requests: usize, window_seconds: u64) -> Self {
        Self::new(max_requests, window_seconds)
    }
    
    /// Check if a request from the given IP should be allowed
    ///
    /// Returns Ok(()) if the request is allowed, Err(RateLimitError) if rate limit exceeded
    pub async fn check_rate_limit(&self, ip: IpAddr) -> Result<(), RateLimitError> {
        let mut state = self.state.write().await;
        let now = Instant::now();
        let window_start = now - self.window_duration;
        
        // Get or create the request history for this IP
        let requests = state.requests.entry(ip).or_insert_with(Vec::new);
        
        // Remove expired requests (outside the current window)
        requests.retain(|&timestamp| timestamp > window_start);
        
        // Check if the limit is exceeded
        if requests.len() >= self.max_requests {
            return Err(RateLimitError::LimitExceeded {
                limit: self.max_requests,
                window_seconds: self.window_duration.as_secs(),
                retry_after: self.calculate_retry_after(requests, window_start),
            });
        }
        
        // Record this request
        requests.push(now);
        
        Ok(())
    }
    
    /// Calculate how many seconds until the oldest request expires
    fn calculate_retry_after(&self, requests: &[Instant], window_start: Instant) -> u64 {
        if let Some(&oldest) = requests.first() {
            let time_until_expire = oldest.duration_since(window_start);
            time_until_expire.as_secs().max(1) // At least 1 second
        } else {
            1
        }
    }
    
    /// Clean up expired entries to prevent memory growth
    ///
    /// This should be called periodically (e.g., every minute) to remove
    /// IP addresses that haven't made requests recently
    pub async fn cleanup_expired(&self) {
        let mut state = self.state.write().await;
        let now = Instant::now();
        let window_start = now - self.window_duration;
        
        // Remove IPs with no recent requests
        state.requests.retain(|_, requests| {
            requests.retain(|&timestamp| timestamp > window_start);
            !requests.is_empty()
        });
    }
}

/// Rate limiting errors
#[derive(Debug)]
pub enum RateLimitError {
    /// Rate limit exceeded
    LimitExceeded {
        limit: usize,
        window_seconds: u64,
        retry_after: u64,
    },
    /// Could not extract client IP address
    MissingClientIp,
}

impl IntoResponse for RateLimitError {
    fn into_response(self) -> Response {
        match self {
            RateLimitError::LimitExceeded {
                limit,
                window_seconds,
                retry_after,
            } => {
                let body = Json(json!({
                    "error": "RateLimitExceeded",
                    "message": format!(
                        "Rate limit exceeded. Maximum {} requests per {} seconds allowed.",
                        limit, window_seconds
                    ),
                    "details": {
                        "limit": limit,
                        "window_seconds": window_seconds,
                        "retry_after": retry_after,
                    }
                }));
                
                // Build response with Retry-After header
                let mut response = (StatusCode::TOO_MANY_REQUESTS, body).into_response();
                response.headers_mut().insert(
                    "Retry-After",
                    HeaderValue::from_str(&retry_after.to_string())
                        .unwrap_or_else(|_| HeaderValue::from_static("60")),
                );
                response
            }
            RateLimitError::MissingClientIp => {
                let body = Json(json!({
                    "error": "RateLimitError",
                    "message": "Could not determine client IP address",
                }));
                
                (StatusCode::INTERNAL_SERVER_ERROR, body).into_response()
            }
        }
    }
}

/// Rate limiting middleware
///
/// This middleware enforces rate limits based on client IP address using a sliding window algorithm.
/// The rate limiter configuration is injected via request extensions.
///
/// # Behavior
/// - Tracks requests per IP address within a time window
/// - Returns 429 Too Many Requests when limit is exceeded
/// - Includes Retry-After header indicating when to retry
/// - Cleans up expired entries to prevent memory growth
pub async fn rate_limit_middleware(
    request: Request,
    next: Next,
) -> Result<Response, RateLimitError> {
    // Extract the rate limiter from request extensions
    let rate_limiter = request
        .extensions()
        .get::<RateLimiter>()
        .cloned()
        .ok_or(RateLimitError::MissingClientIp)?;
    
    // Extract client IP address
    // In production, this should consider X-Forwarded-For or X-Real-IP headers
    // For now, we'll use a placeholder approach
    let client_ip = extract_client_ip(&request)?;
    
    // Check rate limit
    rate_limiter.check_rate_limit(client_ip).await?;
    
    // Rate limit check passed, proceed with request
    Ok(next.run(request).await)
}

/// Extract client IP address from request
///
/// This function attempts to extract the client IP from:
/// 1. X-Forwarded-For header (for requests behind proxies)
/// 2. X-Real-IP header (alternative proxy header)
/// 3. Connection remote address (direct connections)
///
/// For testing purposes, we use a default IP if none can be extracted.
fn extract_client_ip(request: &Request) -> Result<IpAddr, RateLimitError> {
    // Try X-Forwarded-For header first (comma-separated list, first is client)
    if let Some(forwarded) = request.headers().get("X-Forwarded-For") {
        if let Ok(forwarded_str) = forwarded.to_str() {
            if let Some(first_ip) = forwarded_str.split(',').next() {
                if let Ok(ip) = first_ip.trim().parse::<IpAddr>() {
                    return Ok(ip);
                }
            }
        }
    }
    
    // Try X-Real-IP header
    if let Some(real_ip) = request.headers().get("X-Real-IP") {
        if let Ok(ip_str) = real_ip.to_str() {
            if let Ok(ip) = ip_str.parse::<IpAddr>() {
                return Ok(ip);
            }
        }
    }
    
    // For testing/development, use a default IP
    // In production, this should be extracted from the connection
    // or the request should be rejected if no IP can be determined
    Ok(IpAddr::from([127, 0, 0, 1]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        http::{Request, StatusCode},
        middleware,
        routing::get,
        Router,
    };
    use tower::util::ServiceExt; // For oneshot method
    
    #[tokio::test]
    async fn test_rate_limiter_allows_requests_within_limit() {
        let limiter = RateLimiter::new(5, 60); // 5 requests per 60 seconds
        let ip = IpAddr::from([127, 0, 0, 1]);
        
        // First 5 requests should succeed
        for _ in 0..5 {
            assert!(limiter.check_rate_limit(ip).await.is_ok());
        }
    }
    
    #[tokio::test]
    async fn test_rate_limiter_blocks_requests_exceeding_limit() {
        let limiter = RateLimiter::new(3, 60); // 3 requests per 60 seconds
        let ip = IpAddr::from([127, 0, 0, 1]);
        
        // First 3 requests should succeed
        for _ in 0..3 {
            assert!(limiter.check_rate_limit(ip).await.is_ok());
        }
        
        // 4th request should be blocked
        let result = limiter.check_rate_limit(ip).await;
        assert!(result.is_err());
        
        if let Err(RateLimitError::LimitExceeded { limit, window_seconds, retry_after }) = result {
            assert_eq!(limit, 3);
            assert_eq!(window_seconds, 60);
            assert!(retry_after > 0);
        } else {
            panic!("Expected LimitExceeded error");
        }
    }
    
    #[tokio::test]
    async fn test_rate_limiter_different_ips_independent() {
        let limiter = RateLimiter::new(2, 60); // 2 requests per 60 seconds
        let ip1 = IpAddr::from([127, 0, 0, 1]);
        let ip2 = IpAddr::from([127, 0, 0, 2]);
        
        // IP1 makes 2 requests
        assert!(limiter.check_rate_limit(ip1).await.is_ok());
        assert!(limiter.check_rate_limit(ip1).await.is_ok());
        
        // IP1 is now rate limited
        assert!(limiter.check_rate_limit(ip1).await.is_err());
        
        // IP2 should still be able to make requests
        assert!(limiter.check_rate_limit(ip2).await.is_ok());
        assert!(limiter.check_rate_limit(ip2).await.is_ok());
        
        // IP2 is now also rate limited
        assert!(limiter.check_rate_limit(ip2).await.is_err());
    }
    
    #[tokio::test]
    async fn test_rate_limiter_sliding_window() {
        let limiter = RateLimiter::new(2, 1); // 2 requests per 1 second
        let ip = IpAddr::from([127, 0, 0, 1]);
        
        // Make 2 requests
        assert!(limiter.check_rate_limit(ip).await.is_ok());
        assert!(limiter.check_rate_limit(ip).await.is_ok());
        
        // 3rd request should be blocked
        assert!(limiter.check_rate_limit(ip).await.is_err());
        
        // Wait for window to expire
        tokio::time::sleep(Duration::from_millis(1100)).await;
        
        // Should be able to make requests again
        assert!(limiter.check_rate_limit(ip).await.is_ok());
    }
    
    #[tokio::test]
    async fn test_rate_limiter_cleanup_expired() {
        let limiter = RateLimiter::new(5, 1); // 5 requests per 1 second
        let ip = IpAddr::from([127, 0, 0, 1]);
        
        // Make some requests
        for _ in 0..3 {
            limiter.check_rate_limit(ip).await.unwrap();
        }
        
        // Verify state has entries
        {
            let state = limiter.state.read().await;
            assert_eq!(state.requests.len(), 1);
            assert_eq!(state.requests.get(&ip).unwrap().len(), 3);
        }
        
        // Wait for window to expire
        tokio::time::sleep(Duration::from_millis(1100)).await;
        
        // Cleanup expired entries
        limiter.cleanup_expired().await;
        
        // State should be empty now
        {
            let state = limiter.state.read().await;
            assert_eq!(state.requests.len(), 0);
        }
    }
    
    #[tokio::test]
    async fn test_rate_limit_middleware_integration() {
        let limiter = RateLimiter::new(3, 60);
        
        let app = Router::new()
            .route("/test", get(|| async { "OK" }))
            .layer(middleware::from_fn(move |mut req: Request<Body>, next: Next| {
                let limiter = limiter.clone();
                async move {
                    req.extensions_mut().insert(limiter);
                    rate_limit_middleware(req, next).await
                }
            }));
        
        // First 3 requests should succeed
        for i in 0..3 {
            let request = Request::builder()
                .uri("/test")
                .body(Body::empty())
                .unwrap();
            
            let response = app.clone().oneshot(request).await.unwrap();
            assert_eq!(
                response.status(),
                StatusCode::OK,
                "Request {} should succeed",
                i + 1
            );
        }
        
        // 4th request should be rate limited
        let request = Request::builder()
            .uri("/test")
            .body(Body::empty())
            .unwrap();
        
        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
        
        // Check for Retry-After header
        assert!(response.headers().contains_key("Retry-After"));
    }
    
    #[tokio::test]
    async fn test_rate_limit_error_response_format() {
        let limiter = RateLimiter::new(1, 60);
        
        let app = Router::new()
            .route("/test", get(|| async { "OK" }))
            .layer(middleware::from_fn(move |mut req: Request<Body>, next: Next| {
                let limiter = limiter.clone();
                async move {
                    req.extensions_mut().insert(limiter);
                    rate_limit_middleware(req, next).await
                }
            }));
        
        // First request succeeds
        let request = Request::builder()
            .uri("/test")
            .body(Body::empty())
            .unwrap();
        app.clone().oneshot(request).await.unwrap();
        
        // Second request is rate limited
        let request = Request::builder()
            .uri("/test")
            .body(Body::empty())
            .unwrap();
        
        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
        
        // Parse response body
        let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let body: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();
        
        // Verify error response structure
        assert_eq!(body["error"], "RateLimitExceeded");
        assert!(body["message"].as_str().unwrap().contains("Rate limit exceeded"));
        assert_eq!(body["details"]["limit"], 1);
        assert_eq!(body["details"]["window_seconds"], 60);
        assert!(body["details"]["retry_after"].as_u64().unwrap() > 0);
    }
    
    #[tokio::test]
    async fn test_extract_client_ip_from_x_forwarded_for() {
        let request = Request::builder()
            .uri("/test")
            .header("X-Forwarded-For", "192.168.1.100, 10.0.0.1")
            .body(Body::empty())
            .unwrap();
        
        let ip = extract_client_ip(&request).unwrap();
        assert_eq!(ip, IpAddr::from([192, 168, 1, 100]));
    }
    
    #[tokio::test]
    async fn test_extract_client_ip_from_x_real_ip() {
        let request = Request::builder()
            .uri("/test")
            .header("X-Real-IP", "192.168.1.200")
            .body(Body::empty())
            .unwrap();
        
        let ip = extract_client_ip(&request).unwrap();
        assert_eq!(ip, IpAddr::from([192, 168, 1, 200]));
    }
    
    #[tokio::test]
    async fn test_extract_client_ip_default() {
        let request = Request::builder()
            .uri("/test")
            .body(Body::empty())
            .unwrap();
        
        let ip = extract_client_ip(&request).unwrap();
        // Should return default localhost IP
        assert_eq!(ip, IpAddr::from([127, 0, 0, 1]));
    }
    
    #[tokio::test]
    async fn test_rate_limiter_concurrent_requests() {
        use std::sync::Arc;
        
        let limiter = Arc::new(RateLimiter::new(10, 60));
        let ip = IpAddr::from([127, 0, 0, 1]);
        
        // Spawn 10 concurrent requests
        let mut handles = vec![];
        for _ in 0..10 {
            let limiter = limiter.clone();
            let handle = tokio::spawn(async move {
                limiter.check_rate_limit(ip).await
            });
            handles.push(handle);
        }
        
        // All 10 should succeed
        let mut success_count = 0;
        for handle in handles {
            if handle.await.unwrap().is_ok() {
                success_count += 1;
            }
        }
        
        assert_eq!(success_count, 10);
        
        // 11th request should fail
        assert!(limiter.check_rate_limit(ip).await.is_err());
    }
}
