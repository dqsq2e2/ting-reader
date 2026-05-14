use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use tracing::{info, error, warn};

use crate::core::error::{Result, TingError};
use crate::plugin::types::{Plugin, PluginId, PluginMetadata, PluginContext, PluginState};
use crate::plugin::js::{JavaScriptPluginLoader, JavaScriptPluginWrapper};
use crate::plugin::native::{NativeLoader, NativePlugin};
use crate::plugin::installer::PluginInstaller;
use super::{PluginManager, PluginEntry, FailedPlugin};

impl PluginManager {
    /// Load a plugin from a directory
    pub async fn load_plugin(&self, plugin_path: &Path) -> Result<PluginId> {
        let _permit = self.load_semaphore.acquire().await
            .map_err(|e| TingError::PluginLoadError(format!("Failed to acquire load permit: {}", e)))?;

        info!("Acquired plugin load permit for: {}", plugin_path.display());

        let metadata = self.read_plugin_metadata(plugin_path)?;
        let plugin_id = metadata.instance_id();

        info!("正在加载插件: {} from {}", plugin_id, plugin_path.display());

        {
            let registry = self.registry.read().await;
            if registry.contains_key(&plugin_id) {
                info!("Plugin {} already loaded, skipping", plugin_id);
                return Ok(plugin_id);
            }
        }

        let load_future = self.load_plugin_instance(plugin_path, &metadata);
        let (instance, state, error) = match tokio::time::timeout(
            Duration::from_secs(30),
            load_future
        ).await {
            Ok(Ok(inst)) => (inst, PluginState::Loaded, None),
            Ok(Err(e)) => {
                error!("Failed to load plugin {}: {}", plugin_id, e);
                (
                    Arc::new(FailedPlugin::new(metadata.clone(), e.to_string())) as Arc<dyn Plugin>,
                    PluginState::Failed,
                    Some(e.to_string())
                )
            }
            Err(_) => {
                let timeout_err = format!("Plugin load timeout after 30s");
                error!("{} for plugin {}", timeout_err, plugin_id);
                (
                    Arc::new(FailedPlugin::new(metadata.clone(), timeout_err.clone())) as Arc<dyn Plugin>,
                    PluginState::Failed,
                    Some(timeout_err)
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

        if state != PluginState::Failed {
            if let Err(e) = self.initialize_plugin(&plugin_id).await {
                error!("Failed to initialize plugin {}: {}", plugin_id, e);
                let mut registry = self.registry.write().await;
                if let Some(entry) = registry.get_mut(&plugin_id) {
                    entry.state = PluginState::Failed;
                    entry.load_error = Some(e.to_string());
                }
            }
        }

        info!("Plugin load completed for: {} (permit released)", plugin_id);
        Ok(plugin_id)
    }

    pub(crate) async fn load_plugin_instance(&self, plugin_path: &Path, metadata: &PluginMetadata) -> Result<Arc<dyn Plugin>> {
        if metadata.entry_point.ends_with(".js") {
            let loader = JavaScriptPluginLoader::new(plugin_path.to_path_buf())
                .map_err(|e| TingError::PluginLoadError(format!("Failed to create JS loader: {}", e)))?;
            let wrapper = JavaScriptPluginWrapper::new(loader)?;
            Ok(Arc::new(wrapper))
        } else if metadata.entry_point.ends_with(".wasm") {
            let wasm_path = plugin_path.join(&metadata.entry_point);
            let module = self.wasm_runtime.load_module_from_file(&wasm_path).await?;
            let instance = self.wasm_runtime.instantiate(module, metadata).await?;
            Ok(Arc::new(instance))
        } else if metadata.entry_point.ends_with(".dll") || metadata.entry_point.ends_with(".so") || metadata.entry_point.ends_with(".dylib") {
            let lib_path = plugin_path.join(&metadata.entry_point);
            let loader = Arc::new(NativeLoader::new());
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

        if let Err(e) = self.unload_plugin(plugin_id).await {
            if !matches!(e, TingError::PluginNotFound(_)) {
                tracing::warn!("卸载期间卸载插件出错: {}", e);
            }
        }

        // Clean up plugin configuration
        if let Some(cm) = self.config_manager.read().unwrap().as_ref() {
            let _ = cm.delete_config(plugin_id);
        }

        let installer = PluginInstaller::new(
            self.config.plugin_dir.clone(),
            self.config.plugin_dir.join("temp")
        )?;

        if let Err(e) = installer.uninstall_plugin(plugin_id) {
            warn!("Failed to uninstall plugin using standard ID path: {}. Searching for directory...", e);

            let mut found = false;
            let mut read_dir = tokio::fs::read_dir(&self.config.plugin_dir).await.map_err(TingError::IoError)?;

            while let Some(entry) = read_dir.next_entry().await.map_err(TingError::IoError)? {
                let path = entry.path();
                if path.is_dir() && path.join("plugin.json").exists() {
                    if let Ok(metadata) = self.read_plugin_metadata(&path) {
                        if &metadata.instance_id() == plugin_id {
                            info!("Found plugin directory for {}: {}", plugin_id, path.display());
                            if let Err(e) = tokio::fs::remove_dir_all(&path).await {
                                error!("Failed to remove plugin directory {}: {}", path.display(), e);
                            }
                            found = true;
                            break;
                        }
                    }
                }
            }

            if !found {
                return Err(e);
            }
        }

        {
            let mut cache = self.metadata_cache.write().await;
            cache.remove(plugin_id);
        }

        info!("Plugin uninstalled and files removed: {}", plugin_id);
        Ok(())
    }

    /// Reload a plugin
    pub async fn reload_plugin(&self, id: &PluginId) -> Result<()> {
        tracing::info!(plugin_id = %id, "正在重新加载插件");

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

        let new_metadata = self.read_plugin_metadata(&plugin_path)?;
        let new_id = new_metadata.instance_id();

        if new_id == *id {
            tracing::info!(plugin_id = %id, "重新加载相同版本，首先卸载旧实例");

            match self.load_plugin_instance(&plugin_path, &new_metadata).await {
                Ok(instance) => {
                    if let Err(e) = self.unload_plugin(id).await {
                        tracing::error!(plugin_id = %id, error = %e, "卸载旧版本失败");
                        return Err(e);
                    }

                    {
                        let mut registry = self.registry.write().await;
                        registry.insert(new_metadata.instance_id(), PluginEntry::new(new_metadata.clone(), instance));
                    }

                    self.initialize_plugin(&new_id).await?;

                    {
                        let mut cache = self.metadata_cache.write().await;
                        cache.insert(new_id.clone(), plugin_path);
                    }

                    tracing::info!(plugin_id = %new_id, "插件成功重新加载 (相同版本)");
                    Ok(())
                }
                Err(e) => {
                    tracing::error!(plugin_id = %id, error = %e, "加载新插件实例失败，正在中止重新加载");
                    Err(e)
                }
            }
        } else {
            tracing::info!(old_id = %id, new_id = %new_id, "随着版本更改而重新加载");

            match self.load_plugin(&plugin_path).await {
                Ok(loaded_id) => {
                    if let Err(e) = self.unload_plugin(id).await {
                        tracing::warn!(plugin_id = %id, error = %e, "升级后卸载旧版本失败");
                    }
                    tracing::info!(old_id = %id, new_id = %loaded_id, "插件升级成功");
                    Ok(())
                }
                Err(e) => {
                    tracing::error!(plugin_id = %id, error = %e, "加载新版本失败");
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

        let metadata = installer.get_package_metadata(package_path)?;
        let target_plugin_id = metadata.instance_id();

        let needs_unload = {
            let registry = self.registry.read().await;
            registry.contains_key(&target_plugin_id)
        };

        let old_versions_to_remove = {
            let registry = self.registry.read().await;
            let mut to_remove = Vec::new();
            for (id, entry) in registry.iter() {
                let is_same_plugin = entry.metadata.id == metadata.id ||
                    (entry.metadata.id == entry.metadata.name && entry.metadata.name == metadata.name);

                if is_same_plugin && id != &target_plugin_id {
                    to_remove.push(id.clone());
                }
            }
            to_remove
        };

        for old_id in old_versions_to_remove {
            info!("Found old version of plugin {}, removing: {}", metadata.id, old_id);
            if let Err(e) = self.uninstall_plugin(&old_id).await {
                tracing::warn!("卸载旧版本 {} 失败: {}", old_id, e);
            }
        }

        if needs_unload {
            info!("Plugin {} is already loaded, unloading before re-installation", target_plugin_id);
            if let Err(e) = self.unload_plugin(&target_plugin_id).await {
                tracing::warn!("安装前卸载插件 {} 失败: {}", target_plugin_id, e);
            }
        }

        let plugin_id = installer.install_plugin(package_path, |_| Ok(())).await?;

        let plugin_path = self.config.plugin_dir.join(&plugin_id);
        if let Err(e) = self.load_plugin(&plugin_path).await {
            tracing::error!("安装后自动加载插件失败: {}", e);
        }

        Ok(plugin_id)
    }

    /// Install a plugin from the store
    pub async fn install_plugin_from_store(&self, plugin_id: &str) -> Result<PluginId> {
        info!("Installing plugin from store: {}", plugin_id);

        let plugins = self.get_store_plugins().await?;
        let plugin = plugins.iter()
            .find(|p| p.id == plugin_id)
            .ok_or_else(|| TingError::PluginNotFound(format!("Plugin {} not found in store", plugin_id)))?;

        let download_url = crate::plugin::store::get_download_url(plugin)?;

        info!("Downloading plugin {} from {}", plugin_id, download_url);

        let temp_dir = self.config.plugin_dir.join("temp");
        if !temp_dir.exists() {
            tokio::fs::create_dir_all(&temp_dir).await.map_err(TingError::IoError)?;
        }

        let temp_path = crate::plugin::store::download_plugin(&self.http_client, &download_url, &temp_dir).await?;

        info!("Installing plugin package from {}", temp_path.display());
        let result = self.install_plugin_package(&temp_path).await;

        if let Err(e) = tokio::fs::remove_file(&temp_path).await {
            tracing::warn!("删除临时文件 {} 失败: {}", temp_path.display(), e);
        }

        result
    }

    // ── Lifecycle helpers ──

    pub(crate) async fn initialize_plugin(&self, plugin_id: &PluginId) -> Result<()> {
        let (metadata, config_schema) = {
            let registry = self.registry.read().await;
            let entry = registry.get(plugin_id).ok_or_else(|| TingError::PluginNotFound(plugin_id.clone()))?;
            (entry.metadata.clone(), entry.metadata.config_schema.clone())
        };

        // Initialize per-plugin config if plugin has a schema and no config exists yet
        if let Some(ref schema) = config_schema {
            if let Some(cm) = self.config_manager.read().unwrap().as_ref() {
                if cm.get_config(plugin_id).is_err() {
                    let default_config = extract_defaults_from_schema(schema);
                    let _ = cm.initialize_config(
                        plugin_id.clone(),
                        metadata.name.clone(),
                        Some(schema.clone()),
                        default_config,
                    );
                }
            }
        }

        let context = self.create_plugin_context(&metadata)?;

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

    pub(crate) async fn shutdown_plugin(&self, plugin_id: &PluginId) -> Result<()> {
        let instance = {
            let mut registry = self.registry.write().await;
            let entry = registry.get_mut(plugin_id).ok_or_else(|| TingError::PluginNotFound(plugin_id.clone()))?;
            entry.set_state(PluginState::Unloading);
            entry.instance.clone()
        };

        instance.shutdown().await?;
        Ok(())
    }

    pub(crate) fn create_plugin_context(&self, metadata: &PluginMetadata) -> Result<PluginContext> {
        let instance_id = metadata.instance_id();
        let config = if let Some(cm) = self.config_manager.read().unwrap().as_ref() {
            cm.get_config(&instance_id).unwrap_or_else(|_| {
                metadata.config_schema.as_ref()
                    .map(|s| extract_defaults_from_schema(s))
                    .unwrap_or(serde_json::json!({}))
            })
        } else {
            metadata.config_schema.as_ref()
                .map(|s| extract_defaults_from_schema(s))
                .unwrap_or(serde_json::json!({}))
        };

        Ok(PluginContext {
            config,
            data_dir: self.config.plugin_dir.join("data").join(&metadata.name),
            logger: Arc::new(crate::plugin::logger::DefaultPluginLogger::new(metadata.name.clone())),
            event_bus: Arc::new(crate::plugin::events::DefaultPluginEventBus::new()),
        })
    }
}

/// Extract default config values from a JSON Schema
fn extract_defaults_from_schema(schema: &serde_json::Value) -> serde_json::Value {
    let mut defaults = serde_json::json!({});
    if let Some(properties) = schema.get("properties").and_then(|p| p.as_object()) {
        let obj = defaults.as_object_mut().unwrap();
        for (key, prop) in properties {
            if let Some(default_val) = prop.get("default") {
                obj.insert(key.clone(), default_val.clone());
            }
        }
    }
    defaults
}
