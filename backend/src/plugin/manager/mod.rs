//! Plugin manager - core orchestration for the plugin system

pub mod enums;
pub mod discovery;
pub mod lifecycle;
pub mod dispatch;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::{RwLock, Semaphore};
use serde::{Serialize, Deserialize};
use tracing::info;

use crate::core::error::{Result, TingError};
use crate::plugin::config::PluginConfigManager;
use crate::plugin::types::{Plugin, PluginMetadata, PluginId, PluginType, PluginState, PluginContext, PluginDependency};
use crate::plugin::wasm::WasmRuntime;

pub use enums::{ScraperMethod, FormatMethod, UtilityMethod};

/// Configuration for the plugin manager
#[derive(Debug, Clone)]
pub struct PluginConfig {
    pub plugin_dir: PathBuf,
    pub enable_hot_reload: bool,
    pub max_memory_per_plugin: usize,
    pub max_execution_time: std::time::Duration,
}

/// Helper struct for plugin registry entries
pub(crate) struct PluginEntry {
    pub(crate) metadata: PluginMetadata,
    pub(crate) instance: Arc<dyn Plugin>,
    pub(crate) state: PluginState,
    pub(crate) load_error: Option<String>,
    pub(crate) _active_tasks: Arc<std::sync::atomic::AtomicUsize>,
}

impl PluginEntry {
    pub(crate) fn new(metadata: PluginMetadata, instance: Arc<dyn Plugin>) -> Self {
        Self {
            metadata,
            instance,
            state: PluginState::Loaded,
            load_error: None,
            _active_tasks: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
        }
    }

    pub(crate) fn set_state(&mut self, state: PluginState) {
        self.state = state;
    }
}

pub(crate) type PluginRegistry = HashMap<PluginId, PluginEntry>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginInfo {
    pub id: String,
    pub name: String,
    pub version: String,
    pub author: String,
    pub description: String,
    pub plugin_type: PluginType,
    pub state: PluginState,
    pub total_calls: u64,
    pub successful_calls: u64,
    pub failed_calls: u64,
    #[serde(default)]
    pub runtime: Option<String>,
    #[serde(default)]
    pub supported_extensions: Option<Vec<String>>,
    #[serde(default)]
    pub dependencies: Vec<PluginDependency>,
    pub error: Option<String>,
    #[serde(default)]
    pub config_schema: Option<serde_json::Value>,
    #[serde(default)]
    pub permissions: Vec<String>,
    #[serde(default)]
    pub license: Option<String>,
    #[serde(default)]
    pub homepage: Option<String>,
}

impl PluginInfo {
    pub fn from_metadata(metadata: &PluginMetadata, state: &PluginState) -> Self {
        Self {
            id: metadata.instance_id(),
            name: metadata.name.clone(),
            version: metadata.version.clone(),
            author: metadata.author.clone(),
            description: metadata.description.clone(),
            plugin_type: metadata.plugin_type,
            state: *state,
            total_calls: 0,
            successful_calls: 0,
            failed_calls: 0,
            runtime: metadata.runtime.clone(),
            supported_extensions: metadata.supported_extensions.clone(),
            dependencies: metadata.dependencies.clone(),
            error: None,
            config_schema: metadata.config_schema.clone(),
            permissions: metadata.permissions.iter().map(|p| p.to_string()).collect(),
            license: metadata.license.clone(),
            homepage: metadata.homepage.clone(),
        }
    }

    pub fn from_metadata_with_error(metadata: &PluginMetadata, state: &PluginState, error: String) -> Self {
        let mut info = Self::from_metadata(metadata, state);
        info.error = Some(error);
        info
    }
}

/// A placeholder plugin implementation for failed plugins
pub(crate) struct FailedPlugin {
    metadata: PluginMetadata,
    error: String,
}

impl FailedPlugin {
    pub(crate) fn new(metadata: PluginMetadata, error: String) -> Self {
        Self { metadata, error }
    }
}

#[async_trait::async_trait]
impl Plugin for FailedPlugin {
    fn metadata(&self) -> &PluginMetadata { &self.metadata }
    async fn initialize(&self, _context: &PluginContext) -> Result<()> {
        Err(TingError::PluginLoadError(self.error.clone()))
    }
    async fn shutdown(&self) -> Result<()> { Ok(()) }
    fn plugin_type(&self) -> PluginType { self.metadata.plugin_type }
    fn as_any(&self) -> &dyn std::any::Any { self }
}

/// Manager for the plugin system
pub struct PluginManager {
    pub(crate) config: PluginConfig,
    pub(crate) registry: Arc<RwLock<PluginRegistry>>,
    pub(crate) metadata_cache: Arc<RwLock<HashMap<PluginId, PathBuf>>>,
    pub(crate) wasm_runtime: Arc<WasmRuntime>,
    pub(crate) http_client: reqwest::Client,
    pub(crate) _event_subscribers: Arc<RwLock<Vec<Box<dyn Fn(crate::plugin::types::PluginStateEvent) + Send + Sync>>>>,
    pub(crate) load_semaphore: Arc<Semaphore>,
    pub(crate) store_cache: Arc<crate::plugin::store::PluginCache>,
    pub(crate) config_manager: std::sync::RwLock<Option<Arc<PluginConfigManager>>>,
}

impl PluginManager {
    /// Create a new plugin manager
    pub fn new(config: PluginConfig) -> Result<Self> {
        let wasm_runtime = Arc::new(WasmRuntime::new()?);
        let http_client = reqwest::Client::builder()
            .user_agent("TingReader/1.0")
            .build()
            .map_err(|e| TingError::NetworkError(e.to_string()))?;

        Ok(Self {
            config,
            registry: Arc::new(RwLock::new(HashMap::new())),
            metadata_cache: Arc::new(RwLock::new(HashMap::new())),
            wasm_runtime,
            http_client,
            _event_subscribers: Arc::new(RwLock::new(Vec::new())),
            load_semaphore: Arc::new(Semaphore::new(2)),
            store_cache: Arc::new(crate::plugin::store::PluginCache::new()),
            config_manager: std::sync::RwLock::new(None),
        })
    }

    /// Set the plugin config manager (called after construction since it's created later)
    pub fn set_config_manager(&self, cm: Arc<PluginConfigManager>) {
        let mut lock = self.config_manager.write().unwrap();
        *lock = Some(cm);
    }

    /// Trigger garbage collection on all plugins
    pub async fn garbage_collect_all(&self) {
        info!("Triggering garbage collection for all plugins");

        let registry = self.registry.read().await;
        for entry in registry.values() {
            if let Err(e) = entry.instance.garbage_collect().await {
                tracing::warn!("垃圾回收插件 {} 失败: {}", entry.metadata.name, e);
            }
        }

        tokio::task::spawn_blocking(|| {
            crate::core::utils::release_memory();
        }).await.unwrap_or_else(|e| tracing::warn!("释放内存失败: {}", e));
    }

    /// List all installed plugins
    pub async fn list_plugins(&self) -> Vec<PluginInfo> {
        let registry = self.registry.read().await;
        registry.values().map(|entry| {
            if entry.state == PluginState::Failed {
                PluginInfo::from_metadata_with_error(
                    &entry.metadata, &entry.state,
                    entry.load_error.clone().unwrap_or_default()
                )
            } else {
                PluginInfo::from_metadata(&entry.metadata, &entry.state)
            }
        }).collect()
    }

    /// Get the list of plugins from the store
    pub async fn get_store_plugins(&self) -> Result<Vec<crate::plugin::store::StorePlugin>> {
        crate::plugin::store::fetch_store_plugins_cached(&self.http_client, &self.store_cache).await
    }

    pub fn get_plugin(&self, id: &PluginId) -> Result<PluginMetadata> {
        let registry = futures::executor::block_on(self.registry.read());
        let entry = registry.get(id).ok_or_else(|| TingError::PluginNotFound(id.clone()))?;
        Ok(entry.metadata.clone())
    }

    pub async fn find_plugins_by_type(&self, plugin_type: PluginType) -> Vec<PluginInfo> {
        let registry = self.registry.read().await;
        registry.values()
            .filter(|e| e.metadata.plugin_type == plugin_type)
            .map(|entry| {
                if entry.state == PluginState::Failed {
                    PluginInfo::from_metadata_with_error(
                        &entry.metadata, &entry.state,
                        entry.load_error.clone().unwrap_or_default()
                    )
                } else {
                    PluginInfo::from_metadata(&entry.metadata, &entry.state)
                }
            })
            .collect()
    }

    pub fn is_system_supported_format(extension: &str) -> bool {
        matches!(extension.to_lowercase().as_str(),
            "mp3" | "m4a" | "wav" | "ogg" | "flac" | "aac" | "wma" | "opus" | "m4b"
        )
    }

    pub async fn find_plugin_for_format(&self, file_path: &Path) -> Option<PluginInfo> {
        let extension = file_path.extension()?.to_string_lossy().to_lowercase();

        if Self::is_system_supported_format(&extension) {
            return None;
        }

        let registry = self.registry.read().await;

        registry.values()
            .filter(|e| e.metadata.plugin_type == PluginType::Format)
            .find(|e| {
                e.metadata.supported_extensions.as_ref()
                    .map(|exts| exts.contains(&extension))
                    .unwrap_or(false)
            })
            .map(|entry| {
                if entry.state == PluginState::Failed {
                    PluginInfo::from_metadata_with_error(
                        &entry.metadata, &entry.state,
                        entry.load_error.clone().unwrap_or_default()
                    )
                } else {
                    PluginInfo::from_metadata(&entry.metadata, &entry.state)
                }
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_send_sync<T: Send + Sync>() {}

    #[test]
    fn test_plugin_manager_send_sync() {
        assert_send_sync::<PluginManager>();
    }
}
