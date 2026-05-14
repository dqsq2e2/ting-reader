use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tracing::{info, error, warn};

use crate::core::error::{Result, TingError};
use crate::plugin::types::PluginMetadata;
use super::PluginManager;

impl PluginManager {
    /// Discover and load all plugins from the plugin directory
    pub async fn discover_plugins(&self, plugin_dir: &Path) -> Result<Vec<PluginMetadata>> {
        info!("正在发现插件目录: {}", plugin_dir.display());

        let mut discovered = Vec::new();

        if !plugin_dir.exists() {
            tokio::fs::create_dir_all(plugin_dir).await.map_err(TingError::IoError)?;
        }

        let mut potential_plugins: HashMap<String, Vec<(PluginMetadata, PathBuf)>> = HashMap::new();
        let mut read_dir = tokio::fs::read_dir(plugin_dir).await.map_err(TingError::IoError)?;

        while let Some(entry) = read_dir.next_entry().await.map_err(TingError::IoError)? {
            let path = entry.path();
            if path.is_dir() && path.join("plugin.json").exists() {
                match self.read_plugin_metadata(&path) {
                    Ok(metadata) => {
                        let id = metadata.id.clone();
                        potential_plugins.entry(id).or_default().push((metadata, path));
                    }
                    Err(e) => {
                        error!("Failed to read metadata from {}: {}", path.display(), e);
                    }
                }
            }
        }

        for (id, versions) in potential_plugins {
            fn parse_ver(v: &str) -> Vec<u32> {
                v.trim_start_matches('v')
                    .split('.')
                    .filter_map(|s| s.parse::<u32>().ok())
                    .collect()
            }

            let latest_version = versions.iter()
                .map(|(m, _)| m.version.clone())
                .max_by(|a, b| parse_ver(a).cmp(&parse_ver(b)));

            if let Some(latest_ver) = latest_version {
                if let Some((metadata, path)) = versions.iter().find(|(m, _)| m.version == latest_ver) {
                    info!("正在加载最新版本: {}: {}", id, metadata.version);
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

                for (meta, p) in versions {
                    if meta.version != latest_ver {
                        info!("Found old version of {}: {} at {}. Cleaning up...", id, meta.version, p.display());
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

    /// Read plugin metadata from a directory (delegates to shared reader)
    pub(crate) fn read_plugin_metadata(&self, path: &Path) -> Result<PluginMetadata> {
        crate::plugin::types::metadata::read_plugin_metadata(path)
    }
}
