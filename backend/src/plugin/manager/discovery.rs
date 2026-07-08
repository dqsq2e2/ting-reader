use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tracing::{error, info, warn};

use super::{FailedPlugin, PluginEntry, PluginManager};
use crate::core::error::{Result, TingError};
use crate::plugin::tr_package::{self, TrPackageSignatureIdentity};
use crate::plugin::types::metadata::has_plugin_manifest;
use crate::plugin::types::{Plugin, PluginMetadata, PluginState, PluginType};

const RESERVED_PLUGIN_DIRS: [&str; 3] = ["configs", "data", "temp"];

impl PluginManager {
    /// Discover and load all plugins from the plugin directory
    pub async fn discover_plugins(&self, plugin_dir: &Path) -> Result<Vec<PluginMetadata>> {
        info!("Discovering plugin directory: {}", plugin_dir.display());

        let mut discovered = Vec::new();

        if !plugin_dir.exists() {
            tokio::fs::create_dir_all(plugin_dir)
                .await
                .map_err(TingError::IoError)?;
        }

        let mut potential_plugins: HashMap<String, Vec<(PluginMetadata, PathBuf)>> = HashMap::new();
        let mut read_dir = tokio::fs::read_dir(plugin_dir)
            .await
            .map_err(TingError::IoError)?;

        while let Some(entry) = read_dir.next_entry().await.map_err(TingError::IoError)? {
            let path = entry.path();
            if !path.is_dir() || is_reserved_plugin_dir(&path) {
                continue;
            }

            if !has_plugin_manifest(&path) {
                let error = "plugin.yml/plugin.yaml not found";
                warn!(
                    "Failed to discover plugin from {}: {}",
                    path.display(),
                    error
                );
                self.register_failed_plugin_directory(&path, error.to_string())
                    .await;
                continue;
            }

            if !tr_package::has_installed_signature_metadata(&path) {
                warn!(
                    "Skipping plugin directory {} because it has no installed .tr signature metadata",
                    path.display()
                );
                continue;
            }

            if let Err(e) = tr_package::read_installed_signature_identity(&path) {
                error!(
                    "Skipping plugin directory {} because signature verification failed: {}",
                    path.display(),
                    e
                );
                continue;
            }

            match self.read_plugin_metadata(&path) {
                Ok(metadata) => {
                    let id = metadata.id.clone();
                    potential_plugins
                        .entry(id)
                        .or_default()
                        .push((metadata, path));
                }
                Err(e) => {
                    error!("Failed to read metadata from {}: {}", path.display(), e);
                    self.register_failed_plugin_directory(&path, e.to_string())
                        .await;
                }
            }
        }

        let mut latest_plugins = Vec::new();
        for (id, versions) in potential_plugins {
            if let Err(e) = ensure_discovered_versions_share_signature_identity(&id, &versions) {
                error!("{}", e);
                for (_, path) in versions {
                    self.register_failed_plugin_directory(&path, e.to_string())
                        .await;
                }
                continue;
            }

            let latest_version = versions
                .iter()
                .map(|(m, _)| m.version.clone())
                .max_by(|a, b| parse_ver(a).cmp(&parse_ver(b)));

            if let Some(latest_ver) = latest_version {
                if let Some((metadata, path)) =
                    versions.iter().find(|(m, _)| m.version == latest_ver)
                {
                    latest_plugins.push((metadata.clone(), path.clone()));
                }

                for (meta, p) in versions {
                    if meta.version != latest_ver {
                        info!(
                            "Found old version of {}: {} at {}. Cleaning up...",
                            id,
                            meta.version,
                            p.display()
                        );
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

        let load_order = resolve_discovery_load_order(&latest_plugins);
        for index in load_order {
            let (metadata, path) = &latest_plugins[index];
            info!(
                "Loading latest version: {}: {}",
                metadata.id, metadata.version
            );
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

        Ok(discovered)
    }

    /// Read plugin metadata from a directory (delegates to shared reader)
    pub(crate) fn read_plugin_metadata(&self, path: &Path) -> Result<PluginMetadata> {
        crate::plugin::types::metadata::read_plugin_metadata(path)
    }

    async fn register_failed_plugin_directory(&self, plugin_path: &Path, error: String) {
        let metadata = failed_plugin_metadata(plugin_path);
        let plugin_id = metadata.instance_id();
        let instance =
            Arc::new(FailedPlugin::new(metadata.clone(), error.clone())) as Arc<dyn Plugin>;

        {
            let mut registry = self.registry.write().await;
            let mut entry = PluginEntry::new(metadata, instance);
            entry.state = PluginState::Failed;
            entry.load_error = Some(error);
            registry.insert(plugin_id.clone(), entry);
        }

        {
            let mut cache = self.metadata_cache.write().await;
            cache.insert(plugin_id, plugin_path.to_path_buf());
        }
    }
}

fn is_reserved_plugin_dir(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(|name| {
            let name = name.to_ascii_lowercase();
            RESERVED_PLUGIN_DIRS.contains(&name.as_str())
        })
        .unwrap_or(false)
}

fn parse_ver(v: &str) -> Vec<u32> {
    v.trim_start_matches('v')
        .split('.')
        .filter_map(|s| s.parse::<u32>().ok())
        .collect()
}

fn ensure_discovered_versions_share_signature_identity(
    id: &str,
    versions: &[(PluginMetadata, PathBuf)],
) -> Result<()> {
    let mut expected: Option<TrPackageSignatureIdentity> = None;
    for (_, path) in versions {
        let identity = tr_package::read_installed_signature_identity(path)?;
        if let Some(expected) = &expected {
            if !expected.is_compatible_with(&identity) {
                return Err(TingError::PluginLoadError(format!(
                    "Plugin {} has multiple installed versions from different signature identities; uninstall the conflicting version before loading",
                    id
                )));
            }
        } else {
            expected = Some(identity);
        }
    }
    Ok(())
}

fn resolve_discovery_load_order(plugins: &[(PluginMetadata, PathBuf)]) -> Vec<usize> {
    let mut dependencies = vec![Vec::<usize>::new(); plugins.len()];

    for (plugin_index, (metadata, _)) in plugins.iter().enumerate() {
        for dependency in &metadata.dependencies {
            for (candidate_index, (candidate, _)) in plugins.iter().enumerate() {
                if candidate_index != plugin_index
                    && PluginManager::metadata_matches_dependency(candidate, dependency)
                    && !dependencies[plugin_index].contains(&candidate_index)
                {
                    dependencies[plugin_index].push(candidate_index);
                }
            }
        }
    }

    let mut states = vec![0u8; plugins.len()];
    let mut order = Vec::with_capacity(plugins.len());

    for index in 0..plugins.len() {
        if states[index] == 0
            && !visit_for_load_order(index, &dependencies, &mut states, &mut order)
        {
            warn!("Circular plugin dependency detected during discovery; using directory order");
            return (0..plugins.len()).collect();
        }
    }

    order
}

fn visit_for_load_order(
    index: usize,
    dependencies: &[Vec<usize>],
    states: &mut [u8],
    order: &mut Vec<usize>,
) -> bool {
    match states[index] {
        1 => return false,
        2 => return true,
        _ => {}
    }

    states[index] = 1;
    for dependency in &dependencies[index] {
        if !visit_for_load_order(*dependency, dependencies, states, order) {
            return false;
        }
    }

    states[index] = 2;
    order.push(index);
    true
}

fn failed_plugin_metadata(path: &Path) -> PluginMetadata {
    let dir_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .unwrap_or("unknown-plugin");

    let (id, version) = dir_name
        .rsplit_once('@')
        .filter(|(id, version)| !id.is_empty() && !version.is_empty())
        .map(|(id, version)| (id.to_string(), version.to_string()))
        .unwrap_or_else(|| (dir_name.to_string(), "0.0.0".to_string()));

    PluginMetadata::new(
        id.clone(),
        id,
        version,
        PluginType::Utility,
        "Unknown".to_string(),
        "Plugin failed to load".to_string(),
        String::new(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::manager::{PluginConfig, PluginManager};
    use crate::plugin::types::PluginDependency;
    use std::time::Duration;

    #[tokio::test]
    async fn manifestless_directory_is_registered_as_failed() {
        let temp_dir = tempfile::tempdir().unwrap();
        let plugin_dir = temp_dir.path().join("plugins");
        let missing_manifest_dir = plugin_dir.join("missing-manifest@1.2.3");
        tokio::fs::create_dir_all(&missing_manifest_dir)
            .await
            .unwrap();

        let manager = PluginManager::new(PluginConfig {
            plugin_dir: plugin_dir.clone(),
            enable_hot_reload: false,
            max_memory_per_plugin: 128 * 1024 * 1024,
            max_execution_time: Duration::from_secs(30),
        })
        .unwrap();

        manager.discover_plugins(&plugin_dir).await.unwrap();
        let plugins = manager.list_plugins().await;

        assert_eq!(plugins.len(), 1);
        assert_eq!(plugins[0].id, "missing-manifest@1.2.3");
        assert_eq!(plugins[0].state, PluginState::Failed);
        assert!(plugins[0]
            .error
            .as_deref()
            .unwrap_or_default()
            .contains("plugin.yml/plugin.yaml not found"));
    }

    #[test]
    fn discovery_load_order_places_dependencies_first() {
        let base = PluginMetadata::new(
            "base-plugin".to_string(),
            "Base Plugin".to_string(),
            "1.0.0".to_string(),
            PluginType::Utility,
            "Ting Reader".to_string(),
            "Base plugin".to_string(),
            "plugin.js".to_string(),
        );
        let mut dependent = PluginMetadata::new(
            "dependent-plugin".to_string(),
            "Dependent Plugin".to_string(),
            "1.0.0".to_string(),
            PluginType::Utility,
            "Ting Reader".to_string(),
            "Dependent plugin".to_string(),
            "plugin.js".to_string(),
        );
        dependent.dependencies.push(PluginDependency::new(
            "base-plugin".to_string(),
            "^1.0.0".to_string(),
        ));

        let plugins = vec![
            (dependent, PathBuf::from("dependent")),
            (base, PathBuf::from("base")),
        ];

        let order = resolve_discovery_load_order(&plugins);

        assert_eq!(order, vec![1, 0]);
    }
}
