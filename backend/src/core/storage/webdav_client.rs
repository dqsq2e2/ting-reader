use crate::core::error::{Result, TingError};
use reqwest::Client;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, Semaphore};
use tracing::{debug, warn};

/// WebDAV client with built-in rate limiting and wind control measures
pub struct WebDavClient {
    client: Client,
    /// Semaphore to limit concurrent requests
    request_limiter: Arc<Semaphore>,
    /// Track last request time for rate limiting
    last_request: Arc<Mutex<Instant>>,
    /// Minimum interval between requests
    min_interval: Duration,
}

impl WebDavClient {
    /// Create a new WebDAV client with rate limiting
    pub fn new(max_concurrent: usize, min_interval_ms: u64) -> Result<Self> {
        let client = Client::builder()
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| TingError::NetworkError(e.to_string()))?;

        Ok(Self {
            client,
            request_limiter: Arc::new(Semaphore::new(max_concurrent)),
            last_request: Arc::new(Mutex::new(Instant::now() - Duration::from_secs(1))),
            min_interval: Duration::from_millis(min_interval_ms),
        })
    }

    /// Make a PROPFIND request with rate limiting
    pub async fn propfind(
        &self,
        url: &str,
        depth: &str,
        username: Option<&str>,
        password: Option<&str>,
    ) -> Result<String> {
        // Acquire semaphore permit to limit concurrent requests
        let _permit = self.request_limiter.acquire().await.map_err(|e| {
            TingError::NetworkError(format!("Failed to acquire request permit: {}", e))
        })?;

        // Ensure minimum interval between requests
        {
            let mut last = self.last_request.lock().await;
            let elapsed = last.elapsed();
            if elapsed < self.min_interval {
                let sleep_time = self.min_interval - elapsed;
                debug!("Rate limiting: sleeping for {:?}", sleep_time);
                tokio::time::sleep(sleep_time).await;
            }
            *last = Instant::now();
        }

        // Build and send request with browser-like headers
        let mut req = self.client
            .request(reqwest::Method::from_bytes(b"PROPFIND").unwrap(), url)
            .header("Depth", depth)
            .header("Accept", "text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,image/apng,*/*;q=0.8")
            .header("Accept-Language", "zh-CN,zh;q=0.9,en;q=0.8")
            .header("Accept-Encoding", "gzip, deflate, br")
            .header("Connection", "keep-alive")
            .header("Cache-Control", "no-cache")
            .header("Pragma", "no-cache");

        if let (Some(u), Some(p)) = (username, password) {
            req = req.basic_auth(u, Some(p));
        }

        let response = req
            .send()
            .await
            .map_err(|e| TingError::NetworkError(format!("PROPFIND request failed: {}", e)))?;

        if response.status().is_success() || response.status().as_u16() == 207 {
            let xml = response
                .text()
                .await
                .map_err(|e| TingError::NetworkError(format!("Failed to read response: {}", e)))?;
            Ok(xml)
        } else if response.status().as_u16() == 401 {
            Err(TingError::AuthenticationError(
                "WebDAV authentication failed".to_string(),
            ))
        } else if response.status().as_u16() == 429 {
            warn!("WebDAV server returned 429 Too Many Requests, implementing backoff");
            // Implement exponential backoff for 429 responses
            tokio::time::sleep(Duration::from_secs(2)).await;
            Err(TingError::NetworkError(
                "Rate limited by server".to_string(),
            ))
        } else {
            Err(TingError::NetworkError(format!(
                "WebDAV request failed with status: {}",
                response.status()
            )))
        }
    }

    /// Make a GET request with rate limiting (for file downloads)
    pub async fn get(
        &self,
        url: &str,
        username: Option<&str>,
        password: Option<&str>,
        range: Option<&str>,
    ) -> Result<reqwest::Response> {
        // Acquire semaphore permit
        let _permit = self.request_limiter.acquire().await.map_err(|e| {
            TingError::NetworkError(format!("Failed to acquire request permit: {}", e))
        })?;

        // Rate limiting
        {
            let mut last = self.last_request.lock().await;
            let elapsed = last.elapsed();
            if elapsed < self.min_interval {
                let sleep_time = self.min_interval - elapsed;
                tokio::time::sleep(sleep_time).await;
            }
            *last = Instant::now();
        }

        // Build request with browser-like headers
        let mut req = self
            .client
            .get(url)
            .header("Accept", "*/*")
            .header("Accept-Language", "zh-CN,zh;q=0.9,en;q=0.8")
            .header("Accept-Encoding", "gzip, deflate, br")
            .header("Connection", "keep-alive")
            .header("Cache-Control", "no-cache");

        if let (Some(u), Some(p)) = (username, password) {
            req = req.basic_auth(u, Some(p));
        }

        if let Some(range_header) = range {
            req = req.header("Range", range_header);
        }

        let response = req
            .send()
            .await
            .map_err(|e| TingError::NetworkError(format!("GET request failed: {}", e)))?;

        if response.status().is_success() || response.status().as_u16() == 206 {
            Ok(response)
        } else if response.status().as_u16() == 401 {
            Err(TingError::AuthenticationError(
                "WebDAV authentication failed".to_string(),
            ))
        } else if response.status().as_u16() == 429 {
            warn!("WebDAV server returned 429 Too Many Requests");
            tokio::time::sleep(Duration::from_secs(2)).await;
            Err(TingError::NetworkError(
                "Rate limited by server".to_string(),
            ))
        } else {
            Err(TingError::NetworkError(format!(
                "GET request failed with status: {}",
                response.status()
            )))
        }
    }

    /// Get the underlying reqwest client (for compatibility)
    pub fn client(&self) -> &Client {
        &self.client
    }
}

impl Default for WebDavClient {
    fn default() -> Self {
        Self::new(3, 200).expect("Failed to create default WebDAV client")
    }
}
