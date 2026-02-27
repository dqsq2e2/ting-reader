use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;
use crate::core::error::{Result, TingError};
use crate::plugin::types::*;
use crate::plugin::scraper::ScraperPlugin;
use crate::plugin::installer::PluginInstaller;
use crate::plugin::js_plugin::JavaScriptPluginLoader;
use crate::plugin::js_wrapper::JavaScriptPluginWrapper;
use crate::plugin::native::NativeLoader;
use crate::plugin::native_plugin::NativePlugin;
use crate::plugin::runtime::WasmRuntime;
use crate::plugin::runtime::WasmPlugin;
use tracing::{info, error};
use serde_json::Value;
use serde::{Serialize, Deserialize};

/// Configuration for the plugin manager
#[derive(Debug, Clone)]
pub struct PluginConfig {
    /// Directory where plugins are stored
    pub plugin_dir: PathBuf,
    /// Enable hot reloading of plugins
    pub enable_hot_reload: bool,
    /// Maximum memory usage per plugin (bytes)
    pub max_memory_per_plugin: usize,
    /// Maximum execution time for plugin operations
    pub max_execution_time: std::time::Duration,
}

/// Helper struct for plugin registry entries
struct PluginEntry {
    metadata: PluginMetadata,
    instance: Arc<dyn Plugin>,
    state: PluginState,
    _active_tasks: Arc<std::sync::atomic::AtomicUsize>,
}

impl PluginEntry {
    fn new(metadata: PluginMetadata, instance: Arc<dyn Plugin>) -> Self {
        Self {
            metadata,
            instance,
            state: PluginState::Loaded,
            _active_tasks: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
        }
    }
    
    fn set_state(&mut self, state: PluginState) {
        self.state = state;
    }
}

type PluginRegistry = HashMap<PluginId, PluginEntry>;

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
    pub supported_extensions: Option<Vec<String>>,
}

/// Event triggered when a plugin's state changes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginStateEvent {
    /// ID of the plugin
    pub plugin_id: PluginId,
    /// Name of the plugin
    pub plugin_name: String,
    /// Previous state (None if new)
    pub old_state: Option<PluginState>,
    /// New state
    pub new_state: PluginState,
    /// Timestamp of the event
    pub timestamp: i64,
}

/// Manager for the plugin system
pub struct PluginManager {
    config: PluginConfig,
    registry: Arc<RwLock<PluginRegistry>>,
    metadata_cache: Arc<RwLock<HashMap<PluginId, PathBuf>>>,
    wasm_runtime: Arc<WasmRuntime>,
    _event_subscribers: Arc<RwLock<Vec<Box<dyn Fn(PluginStateEvent) + Send + Sync>>>>,
}

impl PluginManager {
    /// Create a new plugin manager
    pub fn new(config: PluginConfig) -> Result<Self> {
        let wasm_runtime = Arc::new(WasmRuntime::new()?);
        Ok(Self {
            config,
            registry: Arc::new(RwLock::new(HashMap::new())),
            metadata_cache: Arc::new(RwLock::new(HashMap::new())),
            wasm_runtime,
            _event_subscribers: Arc::new(RwLock::new(Vec::new())),
        })
    }

    /// Discover and load all plugins from the plugin directory
    pub async fn discover_plugins(&self, plugin_dir: &Path) -> Result<Vec<PluginMetadata>> {
        info!("Discovering plugins in {}", plugin_dir.display());
        
        let mut discovered = Vec::new();
        
        // Ensure plugin directory exists
        if !plugin_dir.exists() {
            tokio::fs::create_dir_all(plugin_dir).await.map_err(TingError::IoError)?;
        }
        
        let mut read_dir = tokio::fs::read_dir(plugin_dir).await.map_err(TingError::IoError)?;
        
        while let Some(entry) = read_dir.next_entry().await.map_err(TingError::IoError)? {
            let path = entry.path();
            if path.is_dir() {
                // Check if it's a valid plugin package
                if path.join("plugin.json").exists() {
                    match self.load_plugin(&path).await {
                        Ok(id) => {
                            if let Some(entry) = self.registry.read().await.get(&id) {
                                discovered.push(entry.metadata.clone());
                            }
                        }
                        Err(e) => {
                            error!("Failed to load plugin from {}: {}", path.display(), e);
                        }
                    }
                }
            }
        }
        
        Ok(discovered)
    }
    
    /// List all installed plugins
    pub async fn list_plugins(&self) -> Vec<PluginInfo> {
        let registry = self.registry.read().await;
        registry.values().map(|entry| {
            PluginInfo {
                id: entry.metadata.id(),
                name: entry.metadata.name.clone(),
                version: entry.metadata.version.clone(),
                author: entry.metadata.author.clone(),
                description: entry.metadata.description.clone(),
                plugin_type: entry.metadata.plugin_type,
                state: entry.state,
                total_calls: 0,
                successful_calls: 0,
                failed_calls: 0,
                supported_extensions: entry.metadata.supported_extensions.clone(),
            }
        }).collect()
    }

    /// Load a plugin from a directory
    pub async fn load_plugin(&self, plugin_path: &Path) -> Result<PluginId> {
        let metadata = self.read_plugin_metadata(plugin_path)?;
        let plugin_id = format!("{}@{}", metadata.name, metadata.version);
        
        info!("Loading plugin: {}", plugin_id);
        
        // Check if already loaded
        {
            let registry = self.registry.read().await;
            if registry.contains_key(&plugin_id) {
                return Ok(plugin_id);
            }
        }
        
        // Load plugin instance
        let instance = self.load_plugin_instance(plugin_path, &metadata).await?;
        
        // Register plugin
        {
            let mut registry = self.registry.write().await;
            registry.insert(plugin_id.clone(), PluginEntry::new(metadata.clone(), instance));
        }
        
        // Update cache
        {
            let mut cache = self.metadata_cache.write().await;
            cache.insert(plugin_id.clone(), plugin_path.to_path_buf());
        }
        
        // Initialize plugin
        self.initialize_plugin(&plugin_id).await?;
        
        Ok(plugin_id)
    }

    async fn load_plugin_instance(&self, plugin_path: &Path, metadata: &PluginMetadata) -> Result<Arc<dyn Plugin>> {
        if metadata.entry_point.ends_with(".js") {
            // JS Plugin
            let loader = JavaScriptPluginLoader::new(plugin_path.to_path_buf())
                .map_err(|e| TingError::PluginLoadError(format!("Failed to create JS loader: {}", e)))?;
            let wrapper = JavaScriptPluginWrapper::new(loader)?;
            
            Ok(Arc::new(wrapper))
        } else if metadata.entry_point.ends_with(".wasm") {
            // WASM Plugin
            let wasm_path = plugin_path.join(&metadata.entry_point);
            let module = self.wasm_runtime.load_module_from_file(&wasm_path).await?;
            let instance = self.wasm_runtime.instantiate(module, metadata).await?;
            Ok(Arc::new(instance))
        } else if metadata.entry_point.ends_with(".dll") || metadata.entry_point.ends_with(".so") || metadata.entry_point.ends_with(".dylib") {
            // Native Plugin
            let lib_path = plugin_path.join(&metadata.entry_point);
            
            // Create native loader
            let loader = Arc::new(NativeLoader::new());
            // Load library
            loader.load_library(metadata.id(), &lib_path, metadata.clone())?;
            
            let plugin = NativePlugin::new(metadata.id(), metadata.clone(), loader);
            Ok(Arc::new(plugin))
        } else {
            Err(TingError::PluginLoadError(format!("Unsupported entry point: {}", metadata.entry_point)))
        }
    }

    /// Unload a plugin
    pub async fn unload_plugin(&self, plugin_id: &PluginId) -> Result<()> {
        info!("Unloading plugin: {}", plugin_id);
        
        // Shutdown plugin first
        self.shutdown_plugin(plugin_id).await?;
        
        let mut registry = self.registry.write().await;
        if registry.remove(plugin_id).is_some() {
            info!("Plugin unloaded: {}", plugin_id);
            Ok(())
        } else {
            Err(TingError::PluginNotFound(plugin_id.clone()))
        }
    }

    /// Uninstall a plugin (Unload and delete files)
    pub async fn uninstall_plugin(&self, plugin_id: &PluginId) -> Result<()> {
        info!("Uninstalling plugin: {}", plugin_id);
        
        // 1. Unload plugin first
        // We ignore "not found" error from unload because it might be already unloaded
        // But if shutdown fails, we might still want to proceed with deletion?
        // Let's try to unload, log error if any, but proceed.
        if let Err(e) = self.unload_plugin(plugin_id).await {
            // Only ignore PluginNotFound, others might be important but shouldn't block uninstallation
            if !matches!(e, TingError::PluginNotFound(_)) {
                tracing::warn!("Error unloading plugin during uninstall: {}", e);
            }
        }
        
        // 2. Remove files using Installer
        let installer = PluginInstaller::new(
            self.config.plugin_dir.clone(),
            self.config.plugin_dir.join("temp")
        )?;
        
        installer.uninstall_plugin(plugin_id)?;
        
        // 3. Remove from metadata cache
        {
            let mut cache = self.metadata_cache.write().await;
            cache.remove(plugin_id);
        }
        
        info!("Plugin uninstalled and files removed: {}", plugin_id);
        Ok(())
    }

    /// Reload a plugin
    pub async fn reload_plugin(&self, id: &PluginId) -> Result<()> {
        tracing::info!(plugin_id = %id, "Reloading plugin");
        
        // Get the plugin path and metadata before reloading
        let (plugin_path, _old_metadata) = {
            let cache = self.metadata_cache.read().await;
            let path = cache.get(id).cloned().ok_or_else(|| {
                TingError::PluginNotFound(format!("Plugin {} not found in cache", id))
            })?;
            
            let registry = self.registry.read().await;
            let metadata = registry.get(id)
                .map(|e| e.metadata.clone())
                .ok_or_else(|| TingError::PluginNotFound(id.clone()))?;
            
            (path, metadata)
        };
        
        // Read new metadata first to check version
        let new_metadata = self.read_plugin_metadata(&plugin_path)?;
        let new_id = format!("{}@{}", new_metadata.name, new_metadata.version);
        
        if new_id == *id {
            // Same version reload - must unload first to avoid ID conflict
            tracing::info!(plugin_id = %id, "Reloading same version, unloading old instance first");
            
            // 1. Try to load the new instance first (without registering) to verify it works
            match self.load_plugin_instance(&plugin_path, &new_metadata).await {
                Ok(instance) => {
                    // 2. Unload old version
                    if let Err(e) = self.unload_plugin(id).await {
                        tracing::error!(plugin_id = %id, error = %e, "Failed to unload old version");
                        return Err(e);
                    }
                    
                    // 3. Register new instance
                    {
                        let mut registry = self.registry.write().await;
                        registry.insert(new_metadata.clone().id(), PluginEntry::new(new_metadata.clone(), instance));
                    }
                    
                    // 4. Initialize
                    self.initialize_plugin(&new_id).await?;
                    
                    // 5. Update cache
                    {
                        let mut cache = self.metadata_cache.write().await;
                        cache.insert(new_id.clone(), plugin_path);
                    }
                    
                    tracing::info!(plugin_id = %new_id, "Plugin reloaded successfully (same version)");
                    Ok(())
                }
                Err(e) => {
                    tracing::error!(plugin_id = %id, error = %e, "Failed to load new plugin instance, aborting reload");
                    Err(e)
                }
            }
        } else {
            // Different version - can use standard atomic reload
            tracing::info!(old_id = %id, new_id = %new_id, "Reloading with version change");
            
            match self.load_plugin(&plugin_path).await {
                Ok(loaded_id) => {
                    if let Err(e) = self.unload_plugin(id).await {
                        tracing::warn!(plugin_id = %id, error = %e, "Failed to unload old version after upgrade");
                    }
                    tracing::info!(old_id = %id, new_id = %loaded_id, "Plugin upgraded successfully");
                    Ok(())
                }
                Err(e) => {
                    tracing::error!(plugin_id = %id, error = %e, "Failed to load new version");
                    Err(e)
                }
            }
        }
    }

    /// Install a plugin package
    pub async fn install_plugin_package(&self, package_path: &Path) -> Result<PluginId> {
        let installer = PluginInstaller::new(
            self.config.plugin_dir.clone(),
            self.config.plugin_dir.join("temp")
        )?;
        
        let plugin_id = installer.install_plugin(package_path, |_| Ok(())).await?;
        
        // Automatically load the plugin after installation
        let plugin_path = self.config.plugin_dir.join(&plugin_id);
        if let Err(e) = self.load_plugin(&plugin_path).await {
            tracing::error!("Failed to auto-load plugin after installation: {}", e);
            // We return success for installation even if loading fails, 
            // but log the error. User can try to reload manually.
        }
        
        Ok(plugin_id)
    }

    /// Call a scraper plugin method
    pub async fn call_scraper(&self, id: &PluginId, method: ScraperMethod, params: Value) -> Result<Value> {
        let instance = {
            let registry = self.registry.read().await;
            let entry = registry.get(id).ok_or_else(|| TingError::PluginNotFound(id.clone()))?;
            if entry.metadata.plugin_type != PluginType::Scraper {
                return Err(TingError::PluginExecutionError(format!("Plugin {} is not a scraper", id)));
            }
            entry.instance.clone()
        };

        let method_name = match method {
            ScraperMethod::Search => "search",
            ScraperMethod::GetDetail => "getDetail",
            ScraperMethod::GetChapterList => "getChapters",
            ScraperMethod::GetChapterDetail => "getChapterDetail", // Not common
            ScraperMethod::DownloadCover => "downloadCover",
            ScraperMethod::GetAudioUrl => "getAudioUrl",
        };

        if let Some(wrapper) = instance.as_any().downcast_ref::<JavaScriptPluginWrapper>() {
            wrapper.call_function(method_name, params).await
        } else if let Some(wasm_plugin) = instance.as_any().downcast_ref::<WasmPlugin>() {
             match method {
                 ScraperMethod::Search => {
                     let query = params.get("query").and_then(|v| v.as_str()).unwrap_or("");
                     let page = params.get("page").and_then(|v| v.as_u64()).unwrap_or(1) as u32;
                     let result = wasm_plugin.search(query, page).await?;
                     Ok(serde_json::to_value(result).map_err(|e| TingError::PluginExecutionError(format!("Serialization error: {}", e)))?)
                 },
                 ScraperMethod::GetDetail => {
                     let id = params.get("id").and_then(|v| v.as_str()).unwrap_or("");
                     let result = wasm_plugin.get_detail(id).await?;
                     Ok(serde_json::to_value(result).map_err(|e| TingError::PluginExecutionError(format!("Serialization error: {}", e)))?)
                 },
                 ScraperMethod::GetChapterList => {
                     let id = params.get("id").and_then(|v| v.as_str()).unwrap_or("");
                     let result = wasm_plugin.get_chapters(id).await?;
                     Ok(serde_json::to_value(result).map_err(|e| TingError::PluginExecutionError(format!("Serialization error: {}", e)))?)
                 },
                 ScraperMethod::DownloadCover => {
                     let url = params.get("url").and_then(|v| v.as_str()).unwrap_or("");
                     let result = wasm_plugin.download_cover(url).await?;
                     use base64::Engine;
                     let b64 = base64::engine::general_purpose::STANDARD.encode(result);
                     Ok(serde_json::json!({ "data": b64 }))
                 },
                 ScraperMethod::GetAudioUrl => {
                     let id = params.get("id").and_then(|v| v.as_str()).unwrap_or("");
                     let result = wasm_plugin.get_audio_url(id).await?;
                     Ok(serde_json::json!({ "url": result }))
                 },
                 _ => Err(TingError::PluginExecutionError(format!("Unsupported method for WASM: {:?}", method))),
             }
        } else {
             Err(TingError::PluginExecutionError("Native scrapers not supported yet".to_string()))
        }
    }

    /// Call a format plugin method
    pub async fn call_format(&self, id: &PluginId, method: FormatMethod, params: Value) -> Result<Value> {
        let instance = {
            let registry = self.registry.read().await;
            let entry = registry.get(id).ok_or_else(|| TingError::PluginNotFound(id.clone()))?;
            if entry.metadata.plugin_type != PluginType::Format {
                return Err(TingError::PluginExecutionError(format!("Plugin {} is not a format plugin", id)));
            }
            entry.instance.clone()
        };

        let method_name = match method {
            FormatMethod::Detect => "detect",
            FormatMethod::ExtractMetadata => "extract_metadata",
            FormatMethod::Decode => "decode",
            FormatMethod::Encode => "encode",
            FormatMethod::Decrypt => "decrypt",
            FormatMethod::DecryptChunk => "decrypt_chunk",
            FormatMethod::GetMetadataReadSize => "get_metadata_read_size",
            FormatMethod::GetDecryptionPlan => "get_decryption_plan",
        };
        
        if let Some(native) = instance.as_any().downcast_ref::<NativePlugin>() {
             native.call_method(method_name, params).await
        } else if let Some(wrapper) = instance.as_any().downcast_ref::<JavaScriptPluginWrapper>() {
             wrapper.call_function(method_name, params).await
        } else {
             Err(TingError::PluginExecutionError("Unknown plugin type".to_string()))
        }
    }

    // Lifecycle methods

    async fn initialize_plugin(&self, plugin_id: &PluginId) -> Result<()> {
        let context = {
            let registry = self.registry.read().await;
            let entry = registry.get(plugin_id).ok_or_else(|| TingError::PluginNotFound(plugin_id.clone()))?;
            self.create_plugin_context(&entry.metadata)?
        };

        let instance = {
            let mut registry = self.registry.write().await;
            let entry = registry.get_mut(plugin_id).ok_or_else(|| TingError::PluginNotFound(plugin_id.clone()))?;
            entry.set_state(PluginState::Initializing);
            entry.instance.clone()
        };

        instance.initialize(&context).await?;

        {
            let mut registry = self.registry.write().await;
            if let Some(entry) = registry.get_mut(plugin_id) {
                entry.set_state(PluginState::Active);
            }
        }
        
        Ok(())
    }

    async fn shutdown_plugin(&self, plugin_id: &PluginId) -> Result<()> {
        let instance = {
            let mut registry = self.registry.write().await;
            let entry = registry.get_mut(plugin_id).ok_or_else(|| TingError::PluginNotFound(plugin_id.clone()))?;
            entry.set_state(PluginState::Unloading);
            entry.instance.clone()
        };

        instance.shutdown().await?;
        
        Ok(())
    }

    fn create_plugin_context(&self, metadata: &PluginMetadata) -> Result<PluginContext> {
        // Default configuration
        let default_config = serde_json::json!({
            "enable_streaming": true,
            "buffer_size": 8192
        });
        
        Ok(PluginContext {
            // plugin_id removed as it's not in struct
            config: default_config,
            data_dir: self.config.plugin_dir.join("data").join(&metadata.name),
            logger: Arc::new(crate::plugin::logger::DefaultPluginLogger::new(metadata.name.clone())),
            event_bus: Arc::new(crate::plugin::events::DefaultPluginEventBus::new()),
        })
    }

    fn read_plugin_metadata(&self, path: &Path) -> Result<PluginMetadata> {
        let metadata_path = path.join("plugin.json");
        let content = std::fs::read_to_string(metadata_path).map_err(TingError::IoError)?;
        serde_json::from_str(&content).map_err(|e| TingError::PluginLoadError(format!("Invalid metadata: {}", e)))
    }
    
    pub fn get_plugin(&self, id: &PluginId) -> Result<PluginMetadata> {
        // Implementation for get_plugin
        let registry = futures::executor::block_on(self.registry.read());
        let entry = registry.get(id).ok_or_else(|| TingError::PluginNotFound(id.clone()))?;
        Ok(entry.metadata.clone())
    }

    pub async fn find_plugins_by_type(&self, plugin_type: PluginType) -> Vec<PluginInfo> {
        let registry = self.registry.read().await;
        registry.values()
            .filter(|e| e.metadata.plugin_type == plugin_type)
            .map(|entry| PluginInfo {
                id: entry.metadata.id(),
                name: entry.metadata.name.clone(),
                version: entry.metadata.version.clone(),
                author: entry.metadata.author.clone(),
                description: entry.metadata.description.clone(),
                plugin_type: entry.metadata.plugin_type,
                state: entry.state,
                total_calls: 0,
                successful_calls: 0,
                failed_calls: 0,
                supported_extensions: entry.metadata.supported_extensions.clone(),
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
        
        // Check if it is a system supported format first
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
            .map(|entry| PluginInfo {
                id: entry.metadata.id(),
                name: entry.metadata.name.clone(),
                version: entry.metadata.version.clone(),
                author: entry.metadata.author.clone(),
                description: entry.metadata.description.clone(),
                plugin_type: entry.metadata.plugin_type,
                state: entry.state,
                total_calls: 0,
                successful_calls: 0,
                failed_calls: 0,
                supported_extensions: entry.metadata.supported_extensions.clone(),
            })
    }
}

// Enums for call_scraper/call_format
#[derive(Debug, Clone, Copy)]
pub enum ScraperMethod {
    Search,
    GetDetail, // Renamed from GetBookDetail
    GetChapterList,
    GetChapterDetail,
    DownloadCover,
    GetAudioUrl,
}

#[derive(Debug, Clone, Copy)]
pub enum FormatMethod {
    Detect,
    ExtractMetadata,
    Decode,
    Encode,
    Decrypt,
    DecryptChunk,
    GetMetadataReadSize,
    GetDecryptionPlan,
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
