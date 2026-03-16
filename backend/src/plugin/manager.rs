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
use tracing::{info, error, warn};
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
    load_error: Option<String>,
    _active_tasks: Arc<std::sync::atomic::AtomicUsize>,
}

impl PluginEntry {
    fn new(metadata: PluginMetadata, instance: Arc<dyn Plugin>) -> Self {
        Self {
            metadata,
            instance,
            state: PluginState::Loaded,
            load_error: None,
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
    #[serde(default)]
    pub dependencies: Vec<PluginDependency>,
    pub error: Option<String>,
}

/// A placeholder plugin implementation for failed plugins
struct FailedPlugin {
    metadata: PluginMetadata,
    error: String,
}

impl FailedPlugin {
    fn new(metadata: PluginMetadata, error: String) -> Self {
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
    http_client: reqwest::Client,
    _event_subscribers: Arc<RwLock<Vec<Box<dyn Fn(PluginStateEvent) + Send + Sync>>>>,
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
        })
    }

    /// Trigger garbage collection on all plugins
    pub async fn garbage_collect_all(&self) {
        info!("Triggering garbage collection for all plugins");
        
        // 1. Collect plugins first
        let registry = self.registry.read().await;
        for entry in registry.values() {
            if let Err(e) = entry.instance.garbage_collect().await {
                tracing::warn!("Failed to garbage collect plugin {}: {}", entry.metadata.name, e);
            }
        }
        
        // 2. Release memory to OS (Linux specific)
        // This helps with allocator fragmentation after large allocations (like image processing)
        // Run in blocking task since malloc_trim can be slow
        tokio::task::spawn_blocking(|| {
            crate::core::utils::release_memory();
        }).await.unwrap_or_else(|e| tracing::warn!("Failed to release memory: {}", e));
    }

    /// Discover and load all plugins from the plugin directory
    pub async fn discover_plugins(&self, plugin_dir: &Path) -> Result<Vec<PluginMetadata>> {
        info!("Discovering plugins in {}", plugin_dir.display());
        
        let mut discovered = Vec::new();
        
        // Ensure plugin directory exists
        if !plugin_dir.exists() {
            tokio::fs::create_dir_all(plugin_dir).await.map_err(TingError::IoError)?;
        }
        
        // First pass: scan all potential plugins
        let mut potential_plugins: HashMap<String, Vec<(PluginMetadata, PathBuf)>> = HashMap::new();
        let mut read_dir = tokio::fs::read_dir(plugin_dir).await.map_err(TingError::IoError)?;
        
        while let Some(entry) = read_dir.next_entry().await.map_err(TingError::IoError)? {
            let path = entry.path();
            if path.is_dir() && path.join("plugin.json").exists() {
                match self.read_plugin_metadata(&path) {
                    Ok(metadata) => {
                         // Group by plugin ID (or name for legacy)
                         let id = metadata.id.clone();
                         potential_plugins.entry(id).or_default().push((metadata, path));
                    }
                    Err(e) => {
                        error!("Failed to read metadata from {}: {}", path.display(), e);
                    }
                }
            }
        }
        
        // Second pass: load only the latest version for each plugin ID
        for (id, versions) in potential_plugins {
            // Helper to parse version string
            fn parse_ver(v: &str) -> Vec<u32> {
                 v.trim_start_matches('v')
                  .split('.')
                  .filter_map(|s| s.parse::<u32>().ok())
                  .collect()
            }

            // Find the latest version
            // Use clone to avoid borrow issues since we need to iterate again for cleanup
            let latest_version = versions.iter()
                .map(|(m, _)| m.version.clone())
                .max_by(|a, b| parse_ver(a).cmp(&parse_ver(b)));
            
            if let Some(latest_ver) = latest_version {
                 // Find the path for the latest version
                 if let Some((metadata, path)) = versions.iter().find(|(m, _)| m.version == latest_ver) {
                     info!("Loading latest version for {}: {}", id, metadata.version);
                     match self.load_plugin(path).await {
                        Ok(plugin_id) => {
                            if let Some(plugin_entry) = self.registry.read().await.get(&plugin_id) {
                                discovered.push(plugin_entry.metadata.clone());
                            }
                        }
                        Err(e) => {
                            error!("Failed to load plugin from {}: {}", path.display(), e);
                        }
                     }
                 }
                 
                 // Cleanup old versions
                 for (meta, p) in versions {
                     if meta.version != latest_ver {
                         info!("Found old version of {}: {} at {}. Cleaning up...", id, meta.version, p.display());
                         // Try to remove old directory
                         // Clone path for error logging to avoid borrow after move
                         let p_display = p.display().to_string();
                         if let Err(e) = tokio::fs::remove_dir_all(p).await {
                             warn!("Failed to remove old plugin directory {}: {}", p_display, e);
                         } else {
                             info!("Removed old plugin directory: {}", p_display);
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
            let error = if entry.state == PluginState::Failed {
                entry.load_error.clone()
            } else {
                None
            };
            
            PluginInfo {
                id: entry.metadata.instance_id(),
                name: entry.metadata.name.clone(),
                version: entry.metadata.version.clone(),
                author: entry.metadata.author.clone(),
                description: entry.metadata.description.clone(),
                plugin_type: entry.metadata.plugin_type,
                state: entry.state.clone(),
                total_calls: 0,
                successful_calls: 0,
                failed_calls: 0,
                supported_extensions: entry.metadata.supported_extensions.clone(),
                dependencies: entry.metadata.dependencies.clone(),
                error,
            }
        }).collect()
    }

    /// Load a plugin from a directory
    pub async fn load_plugin(&self, plugin_path: &Path) -> Result<PluginId> {
        let metadata = self.read_plugin_metadata(plugin_path)?;
        let plugin_id = metadata.instance_id();
        
        info!("Loading plugin: {} from {}", plugin_id, plugin_path.display());
        
        // Check if already loaded
        {
            let registry = self.registry.read().await;
            if registry.contains_key(&plugin_id) {
                return Ok(plugin_id);
            }
        }
        
        // Load plugin instance
        let (instance, state, error) = match self.load_plugin_instance(plugin_path, &metadata).await {
            Ok(inst) => (inst, PluginState::Loaded, None),
            Err(e) => {
                error!("Failed to load plugin {}: {}", plugin_id, e);
                (
                    Arc::new(FailedPlugin::new(metadata.clone(), e.to_string())) as Arc<dyn Plugin>,
                    PluginState::Failed,
                    Some(e.to_string())
                )
            }
        };
        
        // Register plugin
        {
            let mut registry = self.registry.write().await;
            let mut entry = PluginEntry::new(metadata.clone(), instance);
            entry.state = state.clone();
            entry.load_error = error;
            registry.insert(plugin_id.clone(), entry);
        }
        
        // Update cache
        {
            let mut cache = self.metadata_cache.write().await;
            cache.insert(plugin_id.clone(), plugin_path.to_path_buf());
        }
        
        // Initialize plugin if not failed
        if state != PluginState::Failed {
            self.initialize_plugin(&plugin_id).await?;
        }
        
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
            loader.load_library(metadata.instance_id(), &lib_path, metadata.clone())?;
            
            let plugin = NativePlugin::new(metadata.instance_id(), metadata.clone(), loader, plugin_path.to_path_buf());
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
        
        // Try to uninstall using ID-based directory name first (new style)
        if let Err(e) = installer.uninstall_plugin(plugin_id) {
            // Fallback: try Name-based directory name (old style)
            // We need to parse ID to get name and version if possible, or iterate directories
            // Since we don't have the name here easily if it's already unloaded...
            // We should try to find directory that contains plugin.json with this ID
            
            warn!("Failed to uninstall plugin using standard ID path: {}. Searching for directory...", e);
            
            // Search for the plugin directory manually
            let mut found = false;
            let mut read_dir = tokio::fs::read_dir(&self.config.plugin_dir).await.map_err(TingError::IoError)?;
            
            while let Some(entry) = read_dir.next_entry().await.map_err(TingError::IoError)? {
                let path = entry.path();
                if path.is_dir() {
                    // Check if plugin.json exists before reading
                    if path.join("plugin.json").exists() {
                         if let Ok(metadata) = self.read_plugin_metadata(&path) {
                             if &metadata.instance_id() == plugin_id {
                                 info!("Found plugin directory for {}: {}", plugin_id, path.display());
                                 if let Err(e) = tokio::fs::remove_dir_all(&path).await {
                                     error!("Failed to remove plugin directory {}: {}", path.display(), e);
                                     // Don't return error here, try to continue cleanup
                                 }
                                 found = true;
                                 break;
                             }
                         }
                    }
                }
            }
            
            if !found {
                 // Re-return the original error if we couldn't find it manually either
                 return Err(e);
            }
        }
        
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
        let new_id = new_metadata.instance_id();
        
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
                        registry.insert(new_metadata.instance_id(), PluginEntry::new(new_metadata.clone(), instance));
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
        
        // Get metadata to check if plugin is already loaded
        // This helps with "overwrite install" scenario where we need to unload first to release file locks
        let metadata = installer.get_package_metadata(package_path)?;
        let target_plugin_id = metadata.instance_id();
        
        // Check if this specific version is already loaded
        let needs_unload = {
            let registry = self.registry.read().await;
            registry.contains_key(&target_plugin_id)
        };
        
        // Check for other versions of the same plugin to clean up
        let old_versions_to_remove = {
            let registry = self.registry.read().await;
            let mut to_remove = Vec::new();
            for (id, entry) in registry.iter() {
                // Same plugin ID (name/id match) but different version
                // Check using ID field first (new style), then name (old style)
                let is_same_plugin = entry.metadata.id == metadata.id || 
                                     (entry.metadata.id == entry.metadata.name && entry.metadata.name == metadata.name);
                
                if is_same_plugin && id != &target_plugin_id {
                    to_remove.push(id.clone());
                }
            }
            to_remove
        };

        // Unload old versions first
        for old_id in old_versions_to_remove {
            info!("Found old version of plugin {}, removing: {}", metadata.id, old_id);
            if let Err(e) = self.uninstall_plugin(&old_id).await {
                tracing::warn!("Failed to uninstall old version {}: {}", old_id, e);
            }
        }
        
        if needs_unload {
            info!("Plugin {} is already loaded, unloading before re-installation", target_plugin_id);
            if let Err(e) = self.unload_plugin(&target_plugin_id).await {
                tracing::warn!("Failed to unload plugin {} before installation: {}", target_plugin_id, e);
                // Continue anyway, installer handles backup/restore
            }
        }
        
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

    /// Get the list of plugins from the store
    pub async fn get_store_plugins(&self) -> Result<Vec<crate::plugin::store::StorePlugin>> {
        crate::plugin::store::fetch_store_plugins(&self.http_client).await
    }
    
    /// Install a plugin from the store
    pub async fn install_plugin_from_store(&self, plugin_id: &str) -> Result<PluginId> {
        info!("Installing plugin from store: {}", plugin_id);
        
        // 1. Fetch plugin info to get URL
        let plugins = self.get_store_plugins().await?;
        let plugin = plugins.iter()
            .find(|p| p.id == plugin_id)
            .ok_or_else(|| TingError::PluginNotFound(format!("Plugin {} not found in store", plugin_id)))?;
            
        let download_url = crate::plugin::store::get_download_url(plugin)?;
        
        info!("Downloading plugin {} from {}", plugin_id, download_url);
        
        // 2. Download to temp file
        let temp_dir = self.config.plugin_dir.join("temp");
        if !temp_dir.exists() {
             tokio::fs::create_dir_all(&temp_dir).await.map_err(TingError::IoError)?;
        }
        
        let temp_path = crate::plugin::store::download_plugin(&self.http_client, &download_url, &temp_dir).await?;
        
        // 3. Install
        info!("Installing plugin package from {}", temp_path.display());
        let result = self.install_plugin_package(&temp_path).await;
        
        // 4. Cleanup temp file
        if let Err(e) = tokio::fs::remove_file(&temp_path).await {
            tracing::warn!("Failed to remove temp file {}: {}", temp_path.display(), e);
        }
        
        result
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
                     let author = params.get("author").and_then(|v| v.as_str());
                     let narrator = params.get("narrator").and_then(|v| v.as_str());
                     let page = params.get("page").and_then(|v| v.as_u64()).unwrap_or(1) as u32;
                     let result = wasm_plugin.search(query, author, narrator, page).await?;
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
            FormatMethod::GetStreamUrl => "get_stream_url",
            FormatMethod::WriteMetadata => "write_metadata",
        };
        
        if let Some(native) = instance.as_any().downcast_ref::<NativePlugin>() {
             native.call_method(method_name, params).await
        } else if let Some(wrapper) = instance.as_any().downcast_ref::<JavaScriptPluginWrapper>() {
             wrapper.call_function(method_name, params).await
        } else {
             Err(TingError::PluginExecutionError("Unknown plugin type".to_string()))
        }
    }

    /// Call a utility plugin method
    pub async fn call_utility(&self, id: &PluginId, method: UtilityMethod, params: Value) -> Result<Value> {
        let instance = {
            let registry = self.registry.read().await;
            let entry = registry.get(id).ok_or_else(|| TingError::PluginNotFound(id.clone()))?;
            if entry.metadata.plugin_type != PluginType::Utility {
                return Err(TingError::PluginExecutionError(format!("Plugin {} is not a utility plugin", id)));
            }
            entry.instance.clone()
        };

        let method_name = match method {
            UtilityMethod::GetFfmpegPath => "get_ffmpeg_path",
            UtilityMethod::GetFfprobePath => "get_ffprobe_path",
            UtilityMethod::CheckVersion => "check_version",
        };
        
        if let Some(native) = instance.as_any().downcast_ref::<NativePlugin>() {
             native.call_method(method_name, params).await
        } else if let Some(wrapper) = instance.as_any().downcast_ref::<JavaScriptPluginWrapper>() {
             wrapper.call_function(method_name, params).await
        } else {
             Err(TingError::PluginExecutionError("Unknown plugin type".to_string()))
        }
    }

    /// Helper to find and call ffmpeg-utils to get ffmpeg path
    pub async fn get_ffmpeg_path(&self) -> Option<String> {
        let registry = self.registry.read().await;
        // Find plugin with name "FFmpeg Provider"
        let plugin_id = registry.values()
            .find(|e| e.metadata.name == "FFmpeg Provider")
            .map(|e| e.metadata.instance_id());
            
        drop(registry); // Release lock
        
        if let Some(id) = plugin_id {
            if let Ok(result) = self.call_utility(&id, UtilityMethod::GetFfmpegPath, serde_json::json!({})).await {
                return result.get("path").and_then(|v| v.as_str()).map(|s| s.to_string());
            }
        }
        
        // Fallback: Check system path
        // We assume ffmpeg might be in PATH
        None
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
            .map(|entry| {
                let error = if entry.state == PluginState::Failed {
                    entry.load_error.clone()
                } else {
                    None
                };

                PluginInfo {
                    id: entry.metadata.instance_id(),
                    name: entry.metadata.name.clone(),
                    version: entry.metadata.version.clone(),
                    author: entry.metadata.author.clone(),
                    description: entry.metadata.description.clone(),
                    plugin_type: entry.metadata.plugin_type,
                    state: entry.state.clone(),
                    total_calls: 0,
                    successful_calls: 0,
                    failed_calls: 0,
                    supported_extensions: entry.metadata.supported_extensions.clone(),
                    dependencies: entry.metadata.dependencies.clone(),
                    error,
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
            .map(|entry| {
                let error = if entry.state == PluginState::Failed {
                    entry.load_error.clone()
                } else {
                    None
                };

                PluginInfo {
                    id: entry.metadata.instance_id(),
                    name: entry.metadata.name.clone(),
                    version: entry.metadata.version.clone(),
                    author: entry.metadata.author.clone(),
                    description: entry.metadata.description.clone(),
                    plugin_type: entry.metadata.plugin_type,
                    state: entry.state.clone(),
                    total_calls: 0,
                    successful_calls: 0,
                    failed_calls: 0,
                    supported_extensions: entry.metadata.supported_extensions.clone(),
                    dependencies: entry.metadata.dependencies.clone(),
                    error,
                }
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
    GetStreamUrl,
    WriteMetadata,
}

#[derive(Debug, Clone, Copy)]
pub enum UtilityMethod {
    GetFfmpegPath,
    GetFfprobePath,
    CheckVersion,
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
