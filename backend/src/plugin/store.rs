use serde::{Deserialize, Serialize};
use crate::core::error::{Result, TingError};

/// Plugin information from the store
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorePlugin {
    pub id: String,
    pub name: String,
    pub description: String,
    #[serde(rename = "longDescription")]
    pub long_description: Option<String>,
    pub icon: Option<String>,
    pub repo: Option<String>,
    pub plugin_type: String,
    pub version: String,
    #[serde(rename = "downloadUrl")]
    pub download_url: serde_json::Value, // String or Map<String, String>
    pub size: Option<serde_json::Value>, // String or Map<String, String>
    pub date: Option<String>,
    pub downloads: Option<Vec<StoreDownload>>,
    pub dependencies: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoreDownload {
    pub name: String,
    pub url: String,
}

/// Fetch the list of plugins from the store
pub async fn fetch_store_plugins(client: &reqwest::Client) -> Result<Vec<StorePlugin>> {
    let url = "https://www.tingreader.cn/api/plugins";
    let response = client.get(url)
        .send()
        .await
        .map_err(|e| TingError::NetworkError(format!("Failed to fetch store plugins: {}", e)))?;
        
    if !response.status().is_success() {
        return Err(TingError::NetworkError(format!("Store API returned status: {}", response.status())));
    }
    
    let plugins: Vec<StorePlugin> = response.json()
        .await
        .map_err(|e| TingError::SerializationError(format!("Failed to parse store response: {}", e)))?;
        
    Ok(plugins)
}

/// Get the download URL for the current platform
pub fn get_download_url(plugin: &StorePlugin) -> Result<String> {
    // Check if download_url is a string (universal)
    if let Some(url) = plugin.download_url.as_str() {
        return Ok(url.to_string());
    }
    
    // Check if it's a map (platform specific)
    if let Some(map) = plugin.download_url.as_object() {
        let platform_key = get_platform_key();
        
        if let Some(url) = map.get(platform_key).and_then(|v| v.as_str()) {
            return Ok(url.to_string());
        }
        
        return Err(TingError::PluginLoadError(format!(
            "No download URL found for platform '{}' for plugin {}", 
            platform_key, plugin.id
        )));
    }
    
    Err(TingError::PluginLoadError(format!("Invalid downloadUrl format for plugin {}", plugin.id)))
}

/// Get the platform key for the current system
fn get_platform_key() -> &'static str {
    #[cfg(target_os = "windows")]
    return "windows-x86_64";
    
    #[cfg(target_os = "linux")]
    {
        #[cfg(target_arch = "aarch64")]
        return "linux-aarch64";
        
        #[cfg(not(target_arch = "aarch64"))]
        return "linux-x86_64";
    }
    
    #[cfg(target_os = "macos")]
    {
        #[cfg(target_arch = "aarch64")]
        return "macos-aarch64";
        
        #[cfg(not(target_arch = "aarch64"))]
        return "macos-x86_64";
    }

    #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
    "unknown"
}

/// Download a plugin to a temporary file
pub async fn download_plugin(client: &reqwest::Client, url: &str, temp_dir: &std::path::Path) -> Result<std::path::PathBuf> {
    let response = client.get(url)
        .send()
        .await
        .map_err(|e| TingError::NetworkError(format!("Failed to download plugin: {}", e)))?;
        
    if !response.status().is_success() {
        return Err(TingError::NetworkError(format!("Download returned status: {}", response.status())));
    }
    
    // Create a temporary file
    let file_name = url.split('/').last().unwrap_or("plugin.zip");
    let temp_path = temp_dir.join(file_name);
    
    let content = response.bytes()
        .await
        .map_err(|e| TingError::NetworkError(format!("Failed to read download content: {}", e)))?;
        
    tokio::fs::write(&temp_path, content)
        .await
        .map_err(TingError::IoError)?;
        
    Ok(temp_path)
}
