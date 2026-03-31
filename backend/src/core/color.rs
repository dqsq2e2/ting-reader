
use crate::core::error::{Result, TingError};
use std::path::Path;

/// Calculate the dominant color from image bytes
pub async fn calculate_theme_color_from_bytes(bytes: &[u8]) -> Result<Option<String>> {
    if bytes.is_empty() {
        return Ok(None);
    }

    let bytes_vec = bytes.to_vec();

    // Decode image in a blocking task to avoid blocking the async runtime
    let result = tokio::task::spawn_blocking(move || {
        tracing::debug!("颜色提取：从内存加载图像 ({} 字节)", bytes_vec.len());
        match image::load_from_memory(&bytes_vec) {
            Ok(img) => {
                // Get palette
                let buffer = img.to_rgba8();
                let pixels = buffer.as_raw();
                
                // Use max_colors=5 to match legacy behavior (colorthief.js default for getColor)
                // Quality=10 is also default
                match color_thief::get_palette(pixels, color_thief::ColorFormat::Rgba, 10, 5) {
                    Ok(palette) => {
                        // Explicitly drop large buffers
                        drop(buffer);
                        drop(img);
                        // bytes_vec dropped at end of scope
                        
                        if let Some(dominant) = palette.first() {
                            // Return rgba string with 0.1 alpha for UI background use
                            // Matches the behavior of the old backend
                            Some(format!("rgba({}, {}, {}, 0.1)", dominant.r, dominant.g, dominant.b))
                        } else {
                            None
                        }
                    }
                    Err(e) => {
                        tracing::warn!("提取颜色失败: {:?}", e);
                        None
                    }
                }
            }
            Err(e) => {
                tracing::warn!("解码图像失败: {}", e);
                None
            }
        }
    }).await;

    match result {
        Ok(opt) => Ok(opt),
        Err(e) => Err(TingError::PluginExecutionError(format!("Task join error: {}", e))),
    }
}

/// Calculate the dominant color from an image URL or file path.
/// Returns a CSS rgba string.
pub async fn calculate_theme_color(url_or_path: &str) -> Result<Option<String>> {
    if url_or_path.is_empty() {
        return Ok(None);
    }

    // Skip embedded covers for now as they are hard to extract without more context
    if url_or_path.starts_with("embedded://") {
        return Ok(None);
    }
    
    // Check if we need to remove hash referer (since this might be called with the original url from DB)
    let mut clean_url_or_path = url_or_path.to_string();
    let mut referer = "".to_string();
    if let Some(idx) = clean_url_or_path.find("#referer=") {
        referer = clean_url_or_path[idx + 9..].to_string();
        clean_url_or_path = clean_url_or_path[..idx].to_string();
    }

    // 1. Get image bytes
    let bytes = if clean_url_or_path.starts_with("http://") || clean_url_or_path.starts_with("https://") || clean_url_or_path.starts_with("//") {
        let fetch_url = if clean_url_or_path.starts_with("//") {
            format!("https:{}", clean_url_or_path)
        } else {
            clean_url_or_path.to_string()
        };

        // Fetch from URL
        let client = reqwest::Client::new();
        let mut req = client.get(&fetch_url)
            .header(reqwest::header::USER_AGENT, "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36");
            
        if !referer.is_empty() {
            req = req.header(reqwest::header::REFERER, referer);
        }

        match req.send().await {
            Ok(response) => {
                match response.bytes().await {
                    Ok(b) => b.to_vec(),
                    Err(e) => {
                        tracing::warn!("下载封面图像失败: {}", e);
                        return Ok(None);
                    }
                }
            }
            Err(e) => {
                tracing::warn!("获取封面图像失败: {}", e);
                return Ok(None);
            }
        }
    } else {
        // Read from local file
        let path = Path::new(url_or_path);
        if !path.exists() {
             return Ok(None);
        }
        match tokio::fs::read(path).await {
            Ok(b) => b,
            Err(e) => {
                tracing::warn!("读取本地封面图像失败: {}", e);
                return Ok(None);
            }
        }
    };

    calculate_theme_color_from_bytes(&bytes).await
}

/// Calculate the dominant color using an existing reqwest Client
pub async fn calculate_theme_color_with_client(url_or_path: &str, client: &reqwest::Client) -> Result<Option<String>> {
    if url_or_path.is_empty() {
        return Ok(None);
    }

    if url_or_path.starts_with("embedded://") {
        return Ok(None);
    }

    let mut clean_url_or_path = url_or_path.to_string();
    let mut referer = "".to_string();
    if let Some(idx) = clean_url_or_path.find("#referer=") {
        referer = clean_url_or_path[idx + 9..].to_string();
        clean_url_or_path = clean_url_or_path[..idx].to_string();
    }

    // 1. Get image bytes
    let bytes = if clean_url_or_path.starts_with("http://") || clean_url_or_path.starts_with("https://") || clean_url_or_path.starts_with("//") {
        let fetch_url = if clean_url_or_path.starts_with("//") {
            format!("https:{}", clean_url_or_path)
        } else {
            clean_url_or_path.to_string()
        };

        // Fetch from URL using provided client
        let mut req = client.get(&fetch_url)
            .header(reqwest::header::USER_AGENT, "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36");

        if !referer.is_empty() {
            req = req.header(reqwest::header::REFERER, referer);
        }

        match req.send().await {
            Ok(response) => {
                match response.bytes().await {
                    Ok(b) => b.to_vec(),
                    Err(e) => {
                        tracing::warn!("下载封面图像失败: {}", e);
                        return Ok(None);
                    }
                }
            }
            Err(e) => {
                tracing::warn!("获取封面图像失败: {}", e);
                return Ok(None);
            }
        }
    } else {
        // Read from local file
        let path = Path::new(url_or_path);
        if !path.exists() {
             return Ok(None);
        }
        match tokio::fs::read(path).await {
            Ok(b) => b,
            Err(e) => {
                tracing::warn!("读取本地封面图像失败: {}", e);
                return Ok(None);
            }
        }
    };

    calculate_theme_color_from_bytes(&bytes).await
}
