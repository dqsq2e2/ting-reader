use semver::{Version, VersionReq};
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use tracing::{error, info, warn};

use super::{FailedPlugin, PluginEntry, PluginManager};
use crate::core::error::{Result, TingError};
use crate::plugin::installer::PluginInstaller;
use crate::plugin::js::{JavaScriptPluginLoader, JavaScriptPluginWrapper};
use crate::plugin::native::{NativeLoader, NativePlugin};
use crate::plugin::tr_package::{self, TrPackageSignatureIdentity};
use crate::plugin::types::metadata::has_plugin_manifest;
use crate::plugin::types::{Plugin, PluginContext, PluginId, PluginMetadata, PluginState};

impl PluginManager {
    /// Load a plugin from a directory
    pub async fn load_plugin(&self, plugin_path: &Path) -> Result<PluginId> {
        let _permit = self.load_semaphore.acquire().await.map_err(|e| {
            TingError::PluginLoadError(format!("Failed to acquire load permit: {}", e))
        })?;

        info!("Acquired plugin load permit for: {}", plugin_path.display());

        let metadata = self.read_plugin_metadata(plugin_path)?;
        let plugin_id = metadata.instance_id();

        info!(
            "Loading plugin: {} from {}",
            plugin_id,
            plugin_path.display()
        );

        {
            let registry = self.registry.read().await;
            if registry.contains_key(&plugin_id) {
                info!("Plugin {} already loaded, skipping", plugin_id);
                return Ok(plugin_id);
            }
        }

        let preflight_result = match Self::validate_core_compatibility(&metadata) {
            Ok(()) => self.validate_plugin_dependencies(&metadata, None).await,
            Err(e) => Err(e),
        };

        let (instance, state, error) = match preflight_result {
            Err(e) => {
                error!("Plugin preflight failed {}: {}", plugin_id, e);
                (
                    Arc::new(FailedPlugin::new(metadata.clone(), e.to_string())) as Arc<dyn Plugin>,
                    PluginState::Failed,
                    Some(e.to_string()),
                )
            }
            Ok(()) => match tokio::time::timeout(
                Duration::from_secs(30),
                self.load_plugin_instance(plugin_path, &metadata),
            )
            .await
            {
                Ok(Ok(inst)) => (inst, PluginState::Loaded, None),
                Ok(Err(e)) => {
                    error!("Failed to load plugin {}: {}", plugin_id, e);
                    (
                        Arc::new(FailedPlugin::new(metadata.clone(), e.to_string()))
                            as Arc<dyn Plugin>,
                        PluginState::Failed,
                        Some(e.to_string()),
                    )
                }
                Err(_) => {
                    let timeout_err = format!("Plugin load timeout after 30s");
                    error!("{} for plugin {}", timeout_err, plugin_id);
                    (
                        Arc::new(FailedPlugin::new(metadata.clone(), timeout_err.clone()))
                            as Arc<dyn Plugin>,
                        PluginState::Failed,
                        Some(timeout_err),
                    )
                }
            },
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

    pub(crate) async fn load_plugin_instance(
        &self,
        plugin_path: &Path,
        metadata: &PluginMetadata,
    ) -> Result<Arc<dyn Plugin>> {
        if metadata.entry_point.ends_with(".js") {
            let loader = JavaScriptPluginLoader::new(plugin_path.to_path_buf()).map_err(|e| {
                TingError::PluginLoadError(format!("Failed to create JS loader: {}", e))
            })?;
            loader
                .install_npm_dependencies(&self.npm_manager)
                .map_err(|e| {
                    TingError::PluginLoadError(format!("Failed to install npm dependencies: {}", e))
                })?;
            let wrapper = JavaScriptPluginWrapper::new_with_host_gateway(
                loader,
                Some(self.host_gateway_handle()),
            )?;
            Ok(Arc::new(wrapper))
        } else if metadata.entry_point.ends_with(".wasm") {
            let wasm_path = plugin_path.join(&metadata.entry_point);
            let module = self.wasm_runtime.load_module_from_file(&wasm_path).await?;
            let instance = self
                .wasm_runtime
                .instantiate_with_host_gateway(module, metadata, Some(self.host_gateway_handle()))
                .await?;
            Ok(Arc::new(instance))
        } else if metadata.entry_point.ends_with(".dll")
            || metadata.entry_point.ends_with(".so")
            || metadata.entry_point.ends_with(".dylib")
        {
            let lib_path = plugin_path.join(&metadata.entry_point);
            let loader = Arc::new(NativeLoader::new());
            loader.load_library(metadata.instance_id(), &lib_path, metadata.clone())?;
            let plugin = NativePlugin::new(
                metadata.instance_id(),
                metadata.clone(),
                loader,
                plugin_path.to_path_buf(),
                Some(self.host_gateway_handle()),
            );
            Ok(Arc::new(plugin))
        } else {
            Err(TingError::PluginLoadError(format!(
                "Unsupported entry point: {}",
                metadata.entry_point
            )))
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

        let dependents = match self.find_plugin_dependents(plugin_id).await {
            Ok(dependents) => dependents,
            Err(TingError::PluginNotFound(_)) => Vec::new(),
            Err(e) => return Err(e),
        };

        if !dependents.is_empty() {
            return Err(TingError::DependencyError(format!(
                "Cannot uninstall plugin {} because these plugins depend on it: {}",
                plugin_id,
                dependents.join(", ")
            )));
        }

        let cached_plugin_path = {
            let cache = self.metadata_cache.read().await;
            cache.get(plugin_id).cloned()
        };

        if let Err(e) = self.unload_plugin(plugin_id).await {
            if !matches!(e, TingError::PluginNotFound(_)) {
                tracing::warn!(
                    plugin_id = %plugin_id,
                    error = %e,
                    message_key = "plugin.unload_during_uninstall_failed",
                    message_params = %serde_json::json!({
                        "plugin_id": plugin_id,
                        "error": e.to_string(),
                    }),
                    "Failed to unload plugin during uninstall"
                );
            }
        }

        // Clean up plugin configuration
        if let Some(cm) = self.config_manager.read().unwrap().as_ref() {
            let _ = cm.delete_config(plugin_id);
        }

        let installer = PluginInstaller::new(
            self.config.plugin_dir.clone(),
            self.config.plugin_dir.join("temp"),
        )?;

        if let Err(e) = installer.uninstall_plugin(plugin_id) {
            warn!(
                "Failed to uninstall plugin using standard ID path: {}. Searching for directory...",
                e
            );

            let mut found = false;
            if let Some(cached_path) = cached_plugin_path.as_ref() {
                let root = tokio::fs::canonicalize(&self.config.plugin_dir).await;
                let target = tokio::fs::canonicalize(cached_path).await;

                match (root, target) {
                    (Ok(root), Ok(target)) if target.starts_with(&root) && target != root => {
                        info!(
                            "Removing cached plugin directory for {}: {}",
                            plugin_id,
                            cached_path.display()
                        );
                        if let Err(e) = tokio::fs::remove_dir_all(cached_path).await {
                            error!(
                                "Failed to remove cached plugin directory {}: {}",
                                cached_path.display(),
                                e
                            );
                            return Err(TingError::IoError(e));
                        }
                        found = true;
                    }
                    (Ok(_), Ok(target)) => {
                        warn!(
                            "Cached plugin path for {} is outside plugin directory: {}",
                            plugin_id,
                            target.display()
                        );
                    }
                    _ => {}
                }
            }

            let mut read_dir = tokio::fs::read_dir(&self.config.plugin_dir)
                .await
                .map_err(TingError::IoError)?;

            if !found {
                while let Some(entry) = read_dir.next_entry().await.map_err(TingError::IoError)? {
                    let path = entry.path();
                    if path.is_dir() && has_plugin_manifest(&path) {
                        if let Ok(metadata) = self.read_plugin_metadata(&path) {
                            if &metadata.instance_id() == plugin_id {
                                info!(
                                    "Found plugin directory for {}: {}",
                                    plugin_id,
                                    path.display()
                                );
                                if let Err(e) = tokio::fs::remove_dir_all(&path).await {
                                    error!(
                                        "Failed to remove plugin directory {}: {}",
                                        path.display(),
                                        e
                                    );
                                }
                                found = true;
                                break;
                            }
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
        tracing::info!(
            plugin_id = %id,
            message_key = "plugin.reload.started",
            message_params = %serde_json::json!({ "plugin_id": id }),
            "Reloading plugin"
        );

        let (plugin_path, _old_metadata) = {
            let cache = self.metadata_cache.read().await;
            let path = cache.get(id).cloned().ok_or_else(|| {
                TingError::PluginNotFound(format!("Plugin {} not found in cache", id))
            })?;

            let registry = self.registry.read().await;
            let metadata = registry
                .get(id)
                .map(|e| e.metadata.clone())
                .ok_or_else(|| TingError::PluginNotFound(id.clone()))?;

            (path, metadata)
        };

        let new_metadata = self.read_plugin_metadata(&plugin_path)?;
        Self::validate_core_compatibility(&new_metadata)?;
        self.validate_plugin_dependencies(&new_metadata, Some(id))
            .await?;
        let new_id = new_metadata.instance_id();

        if new_id == *id {
            tracing::info!(
                plugin_id = %id,
                message_key = "plugin.reload.same_version",
                message_params = %serde_json::json!({ "plugin_id": id }),
                "Reloading same plugin version"
            );

            match self.load_plugin_instance(&plugin_path, &new_metadata).await {
                Ok(instance) => {
                    if let Err(e) = self.unload_plugin(id).await {
                        tracing::error!(
                            plugin_id = %id,
                            error = %e,
                            message_key = "plugin.unload_old_failed",
                            message_params = %serde_json::json!({
                                "plugin_id": id,
                                "error": e.to_string(),
                            }),
                            "Failed to unload old plugin version"
                        );
                        return Err(e);
                    }

                    {
                        let mut registry = self.registry.write().await;
                        registry.insert(
                            new_metadata.instance_id(),
                            PluginEntry::new(new_metadata.clone(), instance),
                        );
                    }

                    self.initialize_plugin(&new_id).await?;

                    {
                        let mut cache = self.metadata_cache.write().await;
                        cache.insert(new_id.clone(), plugin_path);
                    }

                    tracing::info!(
                        plugin_id = %new_id,
                        message_key = "plugin.reload.completed",
                        message_params = %serde_json::json!({ "plugin_id": new_id }),
                        "Plugin reloaded"
                    );
                    Ok(())
                }
                Err(e) => {
                    tracing::error!(
                        plugin_id = %id,
                        error = %e,
                        message_key = "plugin.reload_new_instance_failed",
                        message_params = %serde_json::json!({
                            "plugin_id": id,
                            "error": e.to_string(),
                        }),
                        "Failed to load new plugin instance; aborting reload"
                    );
                    Err(e)
                }
            }
        } else {
            tracing::info!(
                old_id = %id,
                new_id = %new_id,
                message_key = "plugin.reload.version_changed",
                message_params = %serde_json::json!({
                    "old_id": id,
                    "new_id": new_id,
                }),
                "Reloading plugin with version change"
            );

            match self.load_plugin(&plugin_path).await {
                Ok(loaded_id) => {
                    if let Err(e) = self.unload_plugin(id).await {
                        tracing::warn!(
                            plugin_id = %id,
                            error = %e,
                            message_key = "plugin.unload_after_upgrade_failed",
                            message_params = %serde_json::json!({
                                "plugin_id": id,
                                "error": e.to_string(),
                            }),
                            "Failed to unload old plugin version after upgrade"
                        );
                    }
                    tracing::info!(
                        old_id = %id,
                        new_id = %loaded_id,
                        message_key = "plugin.upgrade.completed",
                        message_params = %serde_json::json!({
                            "old_id": id,
                            "new_id": loaded_id,
                        }),
                        "Plugin upgraded"
                    );
                    Ok(())
                }
                Err(e) => {
                    tracing::error!(
                        plugin_id = %id,
                        error = %e,
                        message_key = "plugin.load_new_version_failed",
                        message_params = %serde_json::json!({
                            "plugin_id": id,
                            "error": e.to_string(),
                        }),
                        "Failed to load new plugin version"
                    );
                    Err(e)
                }
            }
        }
    }

    /// Install a plugin package
    pub async fn install_plugin_package(&self, package_path: &Path) -> Result<PluginId> {
        let installer = PluginInstaller::new(
            self.config.plugin_dir.clone(),
            self.config.plugin_dir.join("temp"),
        )?;

        let metadata = installer.get_package_metadata(package_path)?;
        Self::validate_core_compatibility(&metadata)?;
        self.validate_plugin_dependencies(&metadata, None).await?;
        self.ensure_install_signature_identity_allowed(package_path, &metadata)
            .await?;
        let target_plugin_id = metadata.instance_id();

        let needs_unload = {
            let registry = self.registry.read().await;
            registry.contains_key(&target_plugin_id)
        };

        let old_versions_to_remove = {
            let registry = self.registry.read().await;
            let mut to_remove = Vec::new();
            for (id, entry) in registry.iter() {
                let is_same_plugin = entry.metadata.id == metadata.id;

                if is_same_plugin && id != &target_plugin_id {
                    to_remove.push(id.clone());
                }
            }
            to_remove
        };

        for old_id in old_versions_to_remove {
            info!(
                "Found old version of plugin {}, removing: {}",
                metadata.id, old_id
            );
            if let Err(e) = self.uninstall_plugin(&old_id).await {
                tracing::warn!(
                    plugin_id = %old_id,
                    error = %e,
                    message_key = "plugin.uninstall_old_failed",
                    message_params = %serde_json::json!({
                        "plugin_id": old_id,
                        "error": e.to_string(),
                    }),
                    "Failed to uninstall old plugin version"
                );
            }
        }

        if needs_unload {
            info!(
                "Plugin {} is already loaded, unloading before re-installation",
                target_plugin_id
            );
            if let Err(e) = self.unload_plugin(&target_plugin_id).await {
                tracing::warn!(
                    plugin_id = %target_plugin_id,
                    error = %e,
                    message_key = "plugin.unload_before_install_failed",
                    message_params = %serde_json::json!({
                        "plugin_id": target_plugin_id,
                        "error": e.to_string(),
                    }),
                    "Failed to unload plugin before install"
                );
            }
        }

        let plugin_id = installer.install_plugin(package_path, |_| Ok(())).await?;

        let plugin_path = self.config.plugin_dir.join(&plugin_id);
        if let Err(e) = self.load_plugin(&plugin_path).await {
            tracing::error!(
                plugin_id = %plugin_id,
                error = %e,
                message_key = "plugin.auto_load_after_install_failed",
                message_params = %serde_json::json!({
                    "plugin_id": plugin_id,
                    "error": e.to_string(),
                }),
                "Failed to auto-load plugin after install"
            );
        }

        Ok(plugin_id)
    }

    /// Install bundled plugin packages before regular discovery.
    pub async fn install_preinstalled_packages(&self, preinstalled_dir: &Path) -> Result<()> {
        if preinstalled_dir.as_os_str().is_empty() || !preinstalled_dir.exists() {
            return Ok(());
        }

        if !preinstalled_dir.is_dir() {
            warn!(
                "Preinstalled plugin path is not a directory: {}",
                preinstalled_dir.display()
            );
            return Ok(());
        }

        info!(
            "Scanning preinstalled plugin packages from {}",
            preinstalled_dir.display()
        );

        let installer = PluginInstaller::new(
            self.config.plugin_dir.clone(),
            self.config.plugin_dir.join("temp"),
        )?;
        let mut read_dir = tokio::fs::read_dir(preinstalled_dir)
            .await
            .map_err(TingError::IoError)?;

        while let Some(entry) = read_dir.next_entry().await.map_err(TingError::IoError)? {
            let package_path = entry.path();
            let is_tr_package = package_path
                .extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| ext.eq_ignore_ascii_case("tr"))
                .unwrap_or(false);
            if !package_path.is_file() || !is_tr_package {
                continue;
            }

            match self
                .install_preinstalled_package(&installer, &package_path)
                .await
            {
                Ok(Some(plugin_id)) => info!(
                    "Installed preinstalled plugin package {} as {}",
                    package_path.display(),
                    plugin_id
                ),
                Ok(None) => info!(
                    "Skipped preinstalled plugin package {} because an equal or newer version is already installed",
                    package_path.display()
                ),
                Err(e) => warn!(
                    "Failed to install preinstalled plugin package {}: {}",
                    package_path.display(),
                    e
                ),
            }
        }

        Ok(())
    }

    async fn install_preinstalled_package(
        &self,
        installer: &PluginInstaller,
        package_path: &Path,
    ) -> Result<Option<PluginId>> {
        let metadata = installer.get_package_metadata(package_path)?;
        Self::validate_core_compatibility(&metadata)?;
        self.ensure_install_signature_identity_allowed(package_path, &metadata)
            .await?;

        if self.has_same_or_newer_installed_version(&metadata).await? {
            return Ok(None);
        }

        installer
            .install_plugin(package_path, |_| Ok(()))
            .await
            .map(Some)
    }

    /// Install a plugin from the store
    pub async fn install_plugin_from_store(&self, plugin_id: &str) -> Result<PluginId> {
        let temp_path = self.download_plugin_from_store(plugin_id).await?;

        info!("Installing plugin package from {}", temp_path.display());
        let result = self.install_plugin_package(&temp_path).await;

        if let Err(e) = tokio::fs::remove_file(&temp_path).await {
            tracing::warn!(
                path = %temp_path.display(),
                error = %e,
                message_key = "plugin.temp_file.delete_failed",
                message_params = %serde_json::json!({
                    "path": temp_path.display().to_string(),
                    "error": e.to_string(),
                }),
                "Failed to delete temporary plugin file"
            );
        }

        result
    }

    /// Download a plugin package from the configured store to a temporary file.
    pub async fn download_plugin_from_store(&self, plugin_id: &str) -> Result<std::path::PathBuf> {
        info!("Downloading plugin from store: {}", plugin_id);

        let plugins = self.get_store_plugins().await?;
        let plugin = plugins.iter().find(|p| p.id == plugin_id).ok_or_else(|| {
            TingError::PluginNotFound(format!("Plugin {} not found in store", plugin_id))
        })?;

        let download_url = crate::plugin::store::get_download_url(plugin)?;

        info!("Downloading plugin {} from {}", plugin_id, download_url);

        let temp_dir = self.config.plugin_dir.join("temp");
        if !temp_dir.exists() {
            tokio::fs::create_dir_all(&temp_dir)
                .await
                .map_err(TingError::IoError)?;
        }

        crate::plugin::store::download_plugin(&self.http_client, &download_url, &temp_dir).await
    }

    // ── Lifecycle helpers ──

    async fn ensure_install_signature_identity_allowed(
        &self,
        package_path: &Path,
        metadata: &PluginMetadata,
    ) -> Result<()> {
        let candidate_identity = plugin_install_signature_identity(package_path)?;
        let candidate_path = std::fs::canonicalize(package_path).ok();

        let mut read_dir = tokio::fs::read_dir(&self.config.plugin_dir)
            .await
            .map_err(TingError::IoError)?;

        while let Some(entry) = read_dir.next_entry().await.map_err(TingError::IoError)? {
            let path = entry.path();
            if !path.is_dir() || !has_plugin_manifest(&path) {
                continue;
            }

            if let (Some(candidate_path), Ok(installed_path)) =
                (&candidate_path, std::fs::canonicalize(&path))
            {
                if &installed_path == candidate_path {
                    continue;
                }
            }

            let installed_metadata = match self.read_plugin_metadata(&path) {
                Ok(installed_metadata) => installed_metadata,
                Err(_) => continue,
            };

            if installed_metadata.id != metadata.id {
                continue;
            }

            if !tr_package::has_installed_signature_metadata(&path) {
                continue;
            }

            let installed_identity = tr_package::read_installed_signature_identity(&path)?;
            if !installed_identity.is_compatible_with(&candidate_identity) {
                return Err(TingError::PluginLoadError(format!(
                    "Plugin {} is already installed with signature identity '{}', but the candidate package uses '{}'. Uninstall the existing plugin before installing a package from a different publisher.",
                    metadata.id,
                    installed_identity.label(),
                    candidate_identity.label()
                )));
            }
        }

        Ok(())
    }

    async fn has_same_or_newer_installed_version(
        &self,
        candidate: &PluginMetadata,
    ) -> Result<bool> {
        let mut read_dir = tokio::fs::read_dir(&self.config.plugin_dir)
            .await
            .map_err(TingError::IoError)?;

        while let Some(entry) = read_dir.next_entry().await.map_err(TingError::IoError)? {
            let path = entry.path();
            if !path.is_dir()
                || !has_plugin_manifest(&path)
                || !tr_package::has_installed_signature_metadata(&path)
            {
                continue;
            }

            let installed = match self.read_plugin_metadata(&path) {
                Ok(installed) => installed,
                Err(_) => continue,
            };

            if installed.id == candidate.id
                && plugin_version_is_same_or_newer(&installed.version, &candidate.version)
            {
                return Ok(true);
            }
        }

        Ok(false)
    }

    pub(crate) async fn initialize_plugin(&self, plugin_id: &PluginId) -> Result<()> {
        let (metadata, config_schema) = {
            let registry = self.registry.read().await;
            let entry = registry
                .get(plugin_id)
                .ok_or_else(|| TingError::PluginNotFound(plugin_id.clone()))?;
            (entry.metadata.clone(), entry.metadata.config_schema.clone())
        };

        // Initialize per-plugin config if plugin has a schema and no config exists yet
        if let Some(ref schema) = config_schema {
            if let Some(cm) = self.config_manager.read().unwrap().as_ref() {
                let default_config = extract_defaults_from_schema(schema);
                let _ = cm.ensure_config(
                    plugin_id.clone(),
                    metadata.name.clone(),
                    Some(schema.clone()),
                    default_config,
                );
            }
        }

        let context = self.create_plugin_context(&metadata)?;

        let instance = {
            let mut registry = self.registry.write().await;
            let entry = registry
                .get_mut(plugin_id)
                .ok_or_else(|| TingError::PluginNotFound(plugin_id.clone()))?;
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
            let entry = registry
                .get_mut(plugin_id)
                .ok_or_else(|| TingError::PluginNotFound(plugin_id.clone()))?;
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
                metadata
                    .config_schema
                    .as_ref()
                    .map(|s| extract_defaults_from_schema(s))
                    .unwrap_or(serde_json::json!({}))
            })
        } else {
            metadata
                .config_schema
                .as_ref()
                .map(|s| extract_defaults_from_schema(s))
                .unwrap_or(serde_json::json!({}))
        };

        Ok(PluginContext {
            config,
            data_dir: self.config.plugin_dir.join("data").join(&metadata.name),
            logger: Arc::new(crate::plugin::logger::DefaultPluginLogger::new(
                metadata.name.clone(),
            )),
            event_bus: Arc::new(crate::plugin::events::DefaultPluginEventBus::new()),
        })
    }

    fn validate_core_compatibility(metadata: &PluginMetadata) -> Result<()> {
        Self::validate_core_compatibility_for_version(metadata, env!("CARGO_PKG_VERSION"))
    }

    fn validate_core_compatibility_for_version(
        metadata: &PluginMetadata,
        current_core_version: &str,
    ) -> Result<()> {
        let Some(min_core_version) = metadata
            .min_core_version
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        else {
            return Ok(());
        };

        let current = parse_core_version(current_core_version, "current core version")?;
        let required = parse_core_version(min_core_version, "min_core_version")?;

        if current < required {
            return Err(TingError::PluginLoadError(format!(
                "Plugin {} requires Ting Reader core >= {}, current core version is {}",
                metadata.instance_id(),
                min_core_version,
                current_core_version
            )));
        }

        Ok(())
    }

    pub(crate) async fn validate_plugin_dependencies(
        &self,
        metadata: &PluginMetadata,
        ignored_plugin_id: Option<&PluginId>,
    ) -> Result<()> {
        let registry = self.registry.read().await;
        let mut missing_deps = Vec::new();

        for dep in &metadata.dependencies {
            let matching_plugins: Vec<_> = registry
                .iter()
                .filter(|(id, entry)| {
                    ignored_plugin_id
                        .map(|ignored| ignored != *id)
                        .unwrap_or(true)
                        && entry.state != PluginState::Failed
                        && Self::metadata_matches_dependency(&entry.metadata, dep)
                })
                .collect();

            if matching_plugins.is_empty() {
                missing_deps.push(format!("{} ({})", dep.plugin_name, dep.version_requirement));
            }
        }

        if !missing_deps.is_empty() {
            return Err(TingError::DependencyError(format!(
                "Missing or incompatible dependencies for plugin {}: {}",
                metadata.instance_id(),
                missing_deps.join(", ")
            )));
        }

        Ok(())
    }

    pub(crate) async fn find_plugin_dependents(
        &self,
        plugin_id: &PluginId,
    ) -> Result<Vec<PluginId>> {
        let registry = self.registry.read().await;
        let target = registry
            .get(plugin_id)
            .ok_or_else(|| TingError::PluginNotFound(plugin_id.clone()))?;

        let dependents = registry
            .iter()
            .filter(|(id, entry)| {
                *id != plugin_id
                    && entry.state != PluginState::Failed
                    && entry
                        .metadata
                        .dependencies
                        .iter()
                        .any(|dep| Self::metadata_matches_dependency(&target.metadata, dep))
            })
            .map(|(id, _)| id.clone())
            .collect();

        Ok(dependents)
    }

    pub(crate) fn metadata_matches_dependency(
        metadata: &PluginMetadata,
        dependency: &crate::plugin::types::PluginDependency,
    ) -> bool {
        let dependency_name = dependency.plugin_name.trim();
        if dependency_name.is_empty() {
            return false;
        }

        let name_matches = metadata.id == dependency_name
            || metadata.name == dependency_name
            || metadata.instance_id() == dependency_name;

        name_matches
            && version_requirement_matches(&metadata.version, &dependency.version_requirement)
    }
}

fn parse_core_version(version: &str, label: &str) -> Result<Version> {
    Version::parse(version.trim().trim_start_matches('v'))
        .map_err(|e| TingError::PluginLoadError(format!("Invalid {} '{}': {}", label, version, e)))
}

fn plugin_install_signature_identity(package_path: &Path) -> Result<TrPackageSignatureIdentity> {
    if package_path.is_dir() {
        return tr_package::read_installed_signature_identity(package_path);
    }

    if tr_package::has_tr_magic(package_path)? {
        return tr_package::read_package_signature_identity(package_path);
    }

    Err(TingError::PluginLoadError(format!(
        "{} is not a valid Ting Reader .tr package",
        package_path.display()
    )))
}

fn version_requirement_matches(version: &str, requirement: &str) -> bool {
    let Ok(version) = Version::parse(version.trim().trim_start_matches('v')) else {
        return false;
    };

    let requirement = requirement.trim();
    if requirement.is_empty() || requirement == "*" {
        return true;
    }

    VersionReq::parse(requirement)
        .map(|req| req.matches(&version))
        .unwrap_or_else(|_| version.to_string() == requirement.trim_start_matches('v'))
}

fn plugin_version_is_same_or_newer(installed: &str, candidate: &str) -> bool {
    match (
        Version::parse(installed.trim().trim_start_matches('v')),
        Version::parse(candidate.trim().trim_start_matches('v')),
    ) {
        (Ok(installed), Ok(candidate)) => installed >= candidate,
        _ => installed.trim().trim_start_matches('v') == candidate.trim().trim_start_matches('v'),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::config::PluginConfigManager;
    use crate::plugin::manager::PluginConfig;
    use crate::plugin::types::{PluginDependency, PluginType};
    use serde_json::json;
    use std::sync::Arc;
    use std::time::Duration;

    struct TestPlugin {
        metadata: PluginMetadata,
    }

    #[async_trait::async_trait]
    impl Plugin for TestPlugin {
        fn metadata(&self) -> &PluginMetadata {
            &self.metadata
        }

        async fn initialize(&self, _context: &PluginContext) -> Result<()> {
            Ok(())
        }

        async fn shutdown(&self) -> Result<()> {
            Ok(())
        }

        fn plugin_type(&self) -> PluginType {
            self.metadata.plugin_type
        }

        fn as_any(&self) -> &dyn std::any::Any {
            self
        }
    }

    fn test_metadata(id: &str, name: &str, version: &str) -> PluginMetadata {
        let mut metadata = PluginMetadata::new(
            id.to_string(),
            name.to_string(),
            version.to_string(),
            PluginType::Utility,
            "Ting Reader".to_string(),
            "Test plugin".to_string(),
            "plugin.js".to_string(),
        );
        metadata.min_core_version = Some(env!("CARGO_PKG_VERSION").to_string());
        metadata
    }

    fn test_entry(metadata: PluginMetadata) -> PluginEntry {
        PluginEntry::new(
            metadata.clone(),
            Arc::new(TestPlugin { metadata }) as Arc<dyn Plugin>,
        )
    }

    #[test]
    fn create_context_uses_persisted_config_when_available() {
        let temp_dir = tempfile::tempdir().unwrap();
        let plugin_dir = temp_dir.path().join("plugins");
        let manager = PluginManager::new(PluginConfig {
            plugin_dir: plugin_dir.clone(),
            enable_hot_reload: false,
            max_memory_per_plugin: 128 * 1024 * 1024,
            max_execution_time: Duration::from_secs(30),
        })
        .unwrap();

        let config_manager =
            Arc::new(PluginConfigManager::new(plugin_dir.join("configs"), [7u8; 32]).unwrap());
        manager.set_config_manager(config_manager.clone());

        let metadata = PluginMetadata::new(
            "config-plugin".to_string(),
            "Config Plugin".to_string(),
            "1.0.0".to_string(),
            PluginType::Utility,
            "Ting Reader".to_string(),
            "Config plugin".to_string(),
            "plugin.js".to_string(),
        )
        .with_config_schema(json!({
            "type": "object",
            "properties": {
                "api_key": {
                    "type": "string",
                    "default": "schema-default"
                },
                "enabled": {
                    "type": "boolean",
                    "default": false
                }
            }
        }));

        let plugin_id = metadata.instance_id();
        config_manager
            .initialize_config(
                plugin_id,
                metadata.name.clone(),
                metadata.config_schema.clone(),
                json!({
                    "api_key": "persisted-value",
                    "enabled": true
                }),
            )
            .unwrap();

        let context = manager.create_plugin_context(&metadata).unwrap();

        assert_eq!(context.config["api_key"], "persisted-value");
        assert_eq!(context.config["enabled"], true);
    }

    #[test]
    fn validate_core_compatibility_rejects_future_core_requirement() {
        let mut metadata = PluginMetadata::new(
            "future-plugin".to_string(),
            "Future Plugin".to_string(),
            "1.0.0".to_string(),
            PluginType::Utility,
            "Ting Reader".to_string(),
            "Future plugin".to_string(),
            "plugin.js".to_string(),
        );
        metadata.min_core_version = Some("999.0.0".to_string());

        let error = PluginManager::validate_core_compatibility(&metadata).unwrap_err();

        assert!(error
            .to_string()
            .contains("requires Ting Reader core >= 999.0.0"));
    }

    #[test]
    fn validate_core_compatibility_accepts_v_prefixed_versions() {
        let mut metadata = PluginMetadata::new(
            "current-plugin".to_string(),
            "Current Plugin".to_string(),
            "1.0.0".to_string(),
            PluginType::Utility,
            "Ting Reader".to_string(),
            "Current plugin".to_string(),
            "plugin.js".to_string(),
        );
        metadata.min_core_version = Some("v1.4.8".to_string());

        PluginManager::validate_core_compatibility(&metadata).unwrap();
    }

    #[test]
    fn validate_core_compatibility_ignores_missing_core_requirement() {
        let metadata = PluginMetadata::new(
            "missing-core-plugin".to_string(),
            "Missing Core Plugin".to_string(),
            "1.0.0".to_string(),
            PluginType::Utility,
            "Ting Reader".to_string(),
            "Missing core requirement".to_string(),
            "plugin.js".to_string(),
        );

        PluginManager::validate_core_compatibility(&metadata).unwrap();
    }

    #[test]
    fn validate_core_compatibility_accepts_older_core_requirement_when_current_satisfies_it() {
        let mut metadata = PluginMetadata::new(
            "old-core-plugin".to_string(),
            "Old Core Plugin".to_string(),
            "1.0.0".to_string(),
            PluginType::Utility,
            "Ting Reader".to_string(),
            "Old core requirement".to_string(),
            "plugin.js".to_string(),
        );
        metadata.min_core_version = Some("1.4.7".to_string());

        PluginManager::validate_core_compatibility_for_version(&metadata, "1.4.8").unwrap();
    }

    #[test]
    fn validate_core_compatibility_rejects_when_current_core_is_too_old() {
        let mut metadata = PluginMetadata::new(
            "new-core-plugin".to_string(),
            "New Core Plugin".to_string(),
            "1.0.0".to_string(),
            PluginType::Utility,
            "Ting Reader".to_string(),
            "New core requirement".to_string(),
            "plugin.js".to_string(),
        );
        metadata.min_core_version = Some("1.4.7".to_string());

        let error =
            PluginManager::validate_core_compatibility_for_version(&metadata, "1.4.6").unwrap_err();

        assert!(error
            .to_string()
            .contains("requires Ting Reader core >= 1.4.7"));
        assert!(error.to_string().contains("current core version is 1.4.6"));
    }

    #[tokio::test]
    async fn validate_plugin_dependencies_requires_loaded_dependency() {
        let temp_dir = tempfile::tempdir().unwrap();
        let manager = PluginManager::new(PluginConfig {
            plugin_dir: temp_dir.path().join("plugins"),
            enable_hot_reload: false,
            max_memory_per_plugin: 128 * 1024 * 1024,
            max_execution_time: Duration::from_secs(30),
        })
        .unwrap();

        let mut dependent = test_metadata("dependent-plugin", "Dependent Plugin", "1.0.0");
        dependent.dependencies.push(PluginDependency::new(
            "base-plugin".to_string(),
            "^1.0.0".to_string(),
        ));

        assert!(manager
            .validate_plugin_dependencies(&dependent, None)
            .await
            .is_err());

        let base = test_metadata("base-plugin", "Base Plugin", "1.2.0");
        manager
            .registry
            .write()
            .await
            .insert(base.instance_id(), test_entry(base));

        manager
            .validate_plugin_dependencies(&dependent, None)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn uninstall_plugin_is_blocked_when_other_plugins_depend_on_it() {
        let temp_dir = tempfile::tempdir().unwrap();
        let manager = PluginManager::new(PluginConfig {
            plugin_dir: temp_dir.path().join("plugins"),
            enable_hot_reload: false,
            max_memory_per_plugin: 128 * 1024 * 1024,
            max_execution_time: Duration::from_secs(30),
        })
        .unwrap();

        let base = test_metadata("base-plugin", "Base Plugin", "1.0.0");
        let base_id = base.instance_id();
        let mut dependent = test_metadata("dependent-plugin", "Dependent Plugin", "1.0.0");
        dependent.dependencies.push(PluginDependency::new(
            "base-plugin".to_string(),
            "^1.0.0".to_string(),
        ));

        let mut registry = manager.registry.write().await;
        registry.insert(base_id.clone(), test_entry(base));
        registry.insert(dependent.instance_id(), test_entry(dependent));
        drop(registry);

        let error = manager.uninstall_plugin(&base_id).await.unwrap_err();

        assert!(matches!(error, TingError::DependencyError(_)));
        assert!(error.to_string().contains("dependent-plugin@1.0.0"));
    }
}
