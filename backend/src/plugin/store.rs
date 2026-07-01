use crate::core::error::{Result, TingError};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::info;

use super::types::{LocalizedText, PluginCapability};

/// Cache entry for store plugins
#[derive(Debug, Clone)]
struct CacheEntry {
    key: String,
    plugins: Vec<StorePlugin>,
    timestamp: Instant,
}

/// Cache for store plugins with 1 hour TTL
pub struct PluginCache {
    cache: Arc<RwLock<Option<CacheEntry>>>,
    ttl: Duration,
}

impl PluginCache {
    pub fn new() -> Self {
        Self {
            cache: Arc::new(RwLock::new(None)),
            ttl: Duration::from_secs(3600), // 1 hour
        }
    }

    pub async fn get(&self, key: &str) -> Option<Vec<StorePlugin>> {
        let cache = self.cache.read().await;
        if let Some(entry) = cache.as_ref() {
            if entry.key != key {
                info!("Plugin cache miss for key {}", key);
                return None;
            }
            if entry.timestamp.elapsed() < self.ttl {
                info!("Plugin cache hit for key {}", key);
                return Some(entry.plugins.clone());
            }
            info!("Plugin cache expired for key {}", key);
        }
        None
    }

    pub async fn set(&self, key: String, plugins: Vec<StorePlugin>) {
        let mut cache = self.cache.write().await;
        *cache = Some(CacheEntry {
            key,
            plugins,
            timestamp: Instant::now(),
        });
        info!("Plugin cache updated");
    }

    pub async fn clear(&self) {
        let mut cache = self.cache.write().await;
        *cache = None;
        info!("Plugin cache cleared");
    }
}

impl Default for PluginCache {
    fn default() -> Self {
        Self::new()
    }
}

/// Plugin information from the store
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorePlugin {
    pub id: String,
    pub name: String,
    pub description: String,
    pub long_description: Option<String>,
    pub icon: Option<String>,
    pub repo: Option<String>,
    pub version: String,
    pub download_url: serde_json::Value, // String or Map<String, String>
    pub size: Option<serde_json::Value>, // String or Map<String, String>
    pub date: Option<String>,
    pub downloads: Option<Vec<StoreDownload>>,
    pub dependencies: Option<Vec<String>>,
    /// Runtime type: "wasm", "javascript", or "native"
    #[serde(default)]
    pub runtime: Option<String>,
    /// License identifier (e.g., "MIT")
    #[serde(default)]
    pub license: Option<String>,
    /// Plugin author
    #[serde(default)]
    pub author: Option<String>,
    /// Localized descriptions keyed by locale, e.g. zh/en/ja
    #[serde(default)]
    pub description_i18n: LocalizedText,
    /// Required permissions
    #[serde(default)]
    pub permissions: Option<Vec<String>>,
    /// Configuration schema (JSON Schema format)
    #[serde(default)]
    pub config_schema: Option<serde_json::Value>,
    /// Minimum core version required
    #[serde(default)]
    pub min_core_version: Option<String>,
    /// Minimum Flutter client version required for client-facing plugins
    #[serde(default)]
    pub min_flutter_version: Option<String>,
    /// Capability declarations used by the plugin base.
    #[serde(default)]
    pub capabilities: Vec<PluginCapability>,
}

impl StorePlugin {
    pub fn normalize_i18n(&mut self) {
        self.description_i18n
            .entry("zh".to_string())
            .or_insert_with(|| self.description.clone());
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoreDownload {
    pub name: String,
    pub url: String,
}

/// Parse the plugin-store provider response into store plugins.
pub fn parse_store_plugins_response(value: serde_json::Value) -> Result<Vec<StorePlugin>> {
    let payload = value.get("plugins").cloned().unwrap_or(value);
    let mut plugins: Vec<StorePlugin> = serde_json::from_value(payload).map_err(|e| {
        TingError::SerializationError(format!("Failed to parse store provider response: {}", e))
    })?;
    for plugin in &mut plugins {
        plugin.normalize_i18n();
    }
    Ok(plugins)
}

/// Get the download URL for the current platform
pub fn get_download_url(plugin: &StorePlugin) -> Result<String> {
    // Check if download_url is a string (universal or direct package plugin)
    if let Some(url) = plugin.download_url.as_str() {
        return Ok(url.to_string());
    }

    // Check if it's a map (platform specific for native plugins)
    if let Some(map) = plugin.download_url.as_object() {
        let platform_key = get_platform_key();

        if let Some(url) = map.get(platform_key).and_then(|v| v.as_str()) {
            return Ok(url.to_string());
        }

        // Direct package plugins may not have a repo, so provide a clearer error message.
        if plugin.repo.as_ref().map_or(true, |r| r.is_empty()) {
            return Err(TingError::PluginLoadError(format!(
                "Plugin {} is not available for platform '{}'. This plugin uses direct package downloads with limited platform support.",
                plugin.id, platform_key
            )));
        }

        return Err(TingError::PluginLoadError(format!(
            "No download URL found for platform '{}' for plugin {}",
            platform_key, plugin.id
        )));
    }

    Err(TingError::PluginLoadError(format!(
        "Invalid download_url format for plugin {}",
        plugin.id
    )))
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
pub async fn download_plugin(
    client: &reqwest::Client,
    url: &str,
    temp_dir: &std::path::Path,
) -> Result<std::path::PathBuf> {
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|e| TingError::NetworkError(format!("Failed to download plugin: {}", e)))?;

    if !response.status().is_success() {
        return Err(TingError::NetworkError(format!(
            "Download returned status: {}",
            response.status()
        )));
    }

    // Create a temporary file
    let file_name = url.split('/').last().unwrap_or("plugin.tr");
    let temp_path = temp_dir.join(file_name);

    let content = response
        .bytes()
        .await
        .map_err(|e| TingError::NetworkError(format!("Failed to read download content: {}", e)))?;

    tokio::fs::write(&temp_path, content)
        .await
        .map_err(TingError::IoError)?;

    Ok(temp_path)
}
