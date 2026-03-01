
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
        tracing::debug!("Color extraction: Loading image from memory ({} bytes)", bytes_vec.len());
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
                        tracing::warn!("Failed to extract colors: {:?}", e);
                        None
                    }
                }
            }
            Err(e) => {
                tracing::warn!("Failed to decode image: {}", e);
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

    // 1. Get image bytes
    let bytes = if url_or_path.starts_with("http://") || url_or_path.starts_with("https://") {
        // Fetch from URL
        match reqwest::get(url_or_path).await {
            Ok(response) => {
                match response.bytes().await {
                    Ok(b) => b.to_vec(),
                    Err(e) => {
                        tracing::warn!("Failed to download cover image: {}", e);
                        return Ok(None);
                    }
                }
            }
            Err(e) => {
                tracing::warn!("Failed to fetch cover image: {}", e);
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
                tracing::warn!("Failed to read local cover image: {}", e);
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

    // 1. Get image bytes
    let bytes = if url_or_path.starts_with("http://") || url_or_path.starts_with("https://") {
        // Fetch from URL using provided client
        match client.get(url_or_path).send().await {
            Ok(response) => {
                match response.bytes().await {
                    Ok(b) => b.to_vec(),
                    Err(e) => {
                        tracing::warn!("Failed to download cover image: {}", e);
                        return Ok(None);
                    }
                }
            }
            Err(e) => {
                tracing::warn!("Failed to fetch cover image: {}", e);
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
                tracing::warn!("Failed to read local cover image: {}", e);
                return Ok(None);
            }
        }
    };

    calculate_theme_color_from_bytes(&bytes).await
}
