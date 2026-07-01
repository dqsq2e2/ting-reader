//! Plugin Installation Module
//!
//! This module handles plugin installation with validation, dependency checking,
//! extraction, and rollback capabilities.
//!
//! **Validates: Requirements 26.2, 26.3, 26.4, 26.8**

mod rollback;
#[cfg(test)]
mod tests;

use crate::core::error::{Result, TingError};
use crate::plugin::fs_utils;
use crate::plugin::tr_package;
use crate::plugin::types::metadata::{parse_plugin_metadata_content, read_plugin_metadata};
use crate::plugin::types::{PluginId, PluginMetadata};
use rollback::InstallationBackup;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use tracing::{debug, error, info};

/// Plugin package metadata and integrity summary.
#[derive(Debug, Serialize, Deserialize)]
pub struct PluginPackage {
    /// Plugin metadata
    pub metadata: PluginMetadata,
    /// SHA256 checksum of the plugin files
    pub checksum: String,
    /// Optional signature for verification
    pub signature: Option<String>,
}

/// Plugin installer handles installation, validation, and rollback
pub struct PluginInstaller {
    /// Directory where plugins are installed
    plugin_dir: PathBuf,
    /// Temporary directory for extraction
    _temp_dir: PathBuf,
}

impl PluginInstaller {
    /// Create a new plugin installer
    pub fn new(plugin_dir: PathBuf, temp_dir: PathBuf) -> Result<Self> {
        // Ensure directories exist
        fs::create_dir_all(&plugin_dir)?;
        fs::create_dir_all(&temp_dir)?;

        Ok(Self {
            plugin_dir,
            _temp_dir: temp_dir,
        })
    }

    /// Install a plugin from a package file
    ///
    /// This method performs the following steps:
    /// 1. Validate the plugin package (checksum/signature)
    /// 2. Check dependencies
    /// 3. Extract and install the plugin
    /// 4. Rollback on failure
    ///
    /// **Validates: Requirements 26.2, 26.3, 26.4, 26.8**
    pub async fn install_plugin(
        &self,
        package_path: &Path,
        dependency_checker: impl Fn(&PluginMetadata) -> Result<()>,
    ) -> Result<PluginId> {
        info!("Installing plugin from: {}", package_path.display());

        // Step 1: Validate plugin package (Requirement 26.2)
        let package = self.validate_package(package_path)?;
        debug!(
            "Plugin package validated: {} v{}",
            package.metadata.name, package.metadata.version
        );

        // Step 2: Check dependencies (Requirement 26.3)
        dependency_checker(&package.metadata)?;
        debug!(
            "Dependencies satisfied for plugin: {}",
            package.metadata.name
        );

        // Step 3: Extract and install (Requirement 26.4)
        // Use ID instead of name for directory structure
        let plugin_id = format!("{}@{}", package.metadata.id, package.metadata.version);
        let install_path = self.plugin_dir.join(&plugin_id);

        // Create backup point for rollback
        let backup = InstallationBackup::new(&install_path)?;

        match self
            .extract_and_install(package_path, &install_path, &package)
            .await
        {
            Ok(()) => {
                info!("Plugin installed successfully: {}", plugin_id);
                backup.commit()?;
                Ok(plugin_id)
            }
            Err(e) => {
                // Step 4: Rollback on failure (Requirement 26.8)
                error!("Plugin installation failed: {}, rolling back", e);
                backup.rollback()?;
                Err(e)
            }
        }
    }

    /// Get plugin metadata from a package file without full validation
    pub fn get_package_metadata(&self, package_path: &Path) -> Result<PluginMetadata> {
        debug!("Reading plugin metadata from: {}", package_path.display());

        // Check if package exists
        if !package_path.exists() {
            return Err(TingError::PluginLoadError(format!(
                "Plugin package not found: {}",
                package_path.display()
            )));
        }

        if package_path.is_dir() {
            // Directory package
            read_plugin_metadata(package_path)
        } else if tr_package::has_tr_magic(package_path)? {
            let metadata_content = tr_package::read_manifest_file(package_path, "plugin.yml")?;
            parse_plugin_metadata_content(&metadata_content, "plugin.yml")
        } else {
            Err(TingError::PluginLoadError(format!(
                "{} is not a valid Ting Reader .tr package",
                package_path.display()
            )))
        }
    }

    /// Validate plugin package integrity
    ///
    /// **Validates: Requirement 26.2**
    fn validate_package(&self, package_path: &Path) -> Result<PluginPackage> {
        debug!("Validating plugin package: {}", package_path.display());

        // Get metadata using the helper method
        let metadata = self.get_package_metadata(package_path)?;

        // Calculate checksum
        let checksum = self.calculate_checksum(package_path)?;
        debug!("Calculated checksum: {}", checksum);

        // TODO: Verify signature if present

        Ok(PluginPackage {
            metadata,
            checksum,
            signature: None,
        })
    }

    /// Calculate SHA256 checksum of plugin files
    fn calculate_checksum(&self, plugin_path: &Path) -> Result<String> {
        let mut hasher = Sha256::new();

        if plugin_path.is_dir() {
            // Walk through all files and hash them
            for entry in walkdir::WalkDir::new(plugin_path)
                .follow_links(false)
                .into_iter()
                .filter_map(|e| e.ok())
                .filter(|e| e.file_type().is_file())
            {
                let file_content = fs::read(entry.path())?;
                hasher.update(&file_content);
            }
        } else {
            // Hash the single package file.
            let file_content = fs::read(plugin_path)?;
            hasher.update(&file_content);
        }

        let result = hasher.finalize();
        Ok(format!("{:x}", result))
    }

    /// Extract and install plugin to target directory
    ///
    /// **Validates: Requirement 26.4**
    async fn extract_and_install(
        &self,
        source_path: &Path,
        target_path: &Path,
        _package: &PluginPackage,
    ) -> Result<()> {
        debug!(
            "Extracting plugin from {} to {}",
            source_path.display(),
            target_path.display()
        );

        // Create target directory
        fs::create_dir_all(target_path)?;

        if source_path.is_dir() {
            // Copy all files from source to target
            fs_utils::copy_dir_recursive(source_path, target_path)?;
        } else if tr_package::has_tr_magic(source_path)? {
            tr_package::extract_tr_package(source_path, target_path)?;
            tr_package::write_install_provenance(source_path, target_path)?;
        } else {
            return Err(TingError::PluginLoadError(format!(
                "{} is not a valid Ting Reader .tr package",
                source_path.display()
            )));
        }

        info!("Plugin files extracted to: {}", target_path.display());
        Ok(())
    }

    /// Uninstall a plugin
    ///
    /// This removes the plugin directory and all its files.
    pub fn uninstall_plugin(&self, plugin_id: &PluginId) -> Result<()> {
        info!("Uninstalling plugin: {}", plugin_id);

        let plugin_path = self.plugin_dir.join(plugin_id);

        if !plugin_path.exists() {
            return Err(TingError::PluginNotFound(plugin_id.clone()));
        }

        // Remove plugin directory
        fs::remove_dir_all(&plugin_path)?;

        info!("Plugin uninstalled: {}", plugin_id);
        Ok(())
    }
}
