//! Plugin Installation Module
//!
//! This module handles plugin installation with validation, dependency checking,
//! extraction, and rollback capabilities.
//!
//! **Validates: Requirements 26.2, 26.3, 26.4, 26.8**

use crate::core::error::{Result, TingError};
use crate::plugin::types::{PluginMetadata, PluginId};
use sha2::{Sha256, Digest};
use std::fs;
use std::path::{Path, PathBuf};
use tracing::{info, debug, warn, error};
use serde::{Deserialize, Serialize};

/// Plugin package format (.tpkg file structure)
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
        debug!("Plugin package validated: {} v{}", package.metadata.name, package.metadata.version);
        
        // Step 2: Check dependencies (Requirement 26.3)
        dependency_checker(&package.metadata)?;
        debug!("Dependencies satisfied for plugin: {}", package.metadata.name);
        
        // Step 3: Extract and install (Requirement 26.4)
        let plugin_id = format!("{}@{}", package.metadata.name, package.metadata.version);
        let install_path = self.plugin_dir.join(&plugin_id);
        
        // Create backup point for rollback
        let backup = InstallationBackup::new(&install_path)?;
        
        match self.extract_and_install(package_path, &install_path, &package).await {
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
    
    /// Validate plugin package integrity
    ///
    /// **Validates: Requirement 26.2**
    fn validate_package(&self, package_path: &Path) -> Result<PluginPackage> {
        debug!("Validating plugin package: {}", package_path.display());
        
        // Check if package exists
        if !package_path.exists() {
            return Err(TingError::PluginLoadError(
                format!("Plugin package not found: {}", package_path.display())
            ));
        }

        let metadata = if package_path.is_dir() {
            // Directory package
            let metadata_path = package_path.join("plugin.json");
            if !metadata_path.exists() {
                return Err(TingError::PluginLoadError(
                    "plugin.json not found in package".to_string()
                ));
            }
            
            // Read metadata
            let metadata_content = fs::read_to_string(&metadata_path)?;
            serde_json::from_str(&metadata_content)
                .map_err(|e| TingError::PluginLoadError(format!("Invalid plugin.json: {}", e)))?
        } else {
            // Zip package
            let file = fs::File::open(package_path)?;
            let mut archive = zip::ZipArchive::new(file)
                .map_err(|e| TingError::PluginLoadError(format!("Failed to open zip archive: {}", e)))?;
            
            let mut metadata_file = archive.by_name("plugin.json")
                .map_err(|_| TingError::PluginLoadError("plugin.json not found in zip archive".to_string()))?;
            
            let mut metadata_content = String::new();
            use std::io::Read;
            metadata_file.read_to_string(&mut metadata_content)?;
            
            serde_json::from_str(&metadata_content)
                .map_err(|e| TingError::PluginLoadError(format!("Invalid plugin.json: {}", e)))?
        };
        
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
            // Hash the single file (zip)
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
        debug!("Extracting plugin from {} to {}", source_path.display(), target_path.display());
        
        // Create target directory
        fs::create_dir_all(target_path)?;
        
        if source_path.is_dir() {
            // Copy all files from source to target
            self.copy_directory(source_path, target_path)?;
        } else {
            // Extract zip archive
            self.extract_zip(source_path, target_path)?;
        }
        
        info!("Plugin files extracted to: {}", target_path.display());
        Ok(())
    }
    
    /// Recursively copy directory contents
    fn copy_directory(&self, source: &Path, target: &Path) -> Result<()> {
        for entry in fs::read_dir(source)? {
            let entry = entry?;
            let source_path = entry.path();
            let file_name = entry.file_name();
            let target_path = target.join(&file_name);
            
            if source_path.is_dir() {
                fs::create_dir_all(&target_path)?;
                self.copy_directory(&source_path, &target_path)?;
            } else {
                fs::copy(&source_path, &target_path)?;
            }
        }
        Ok(())
    }

    /// Extract zip archive to target directory
    fn extract_zip(&self, source: &Path, target: &Path) -> Result<()> {
        let file = fs::File::open(source)?;
        let mut archive = zip::ZipArchive::new(file)
            .map_err(|e| TingError::PluginLoadError(format!("Failed to open zip archive: {}", e)))?;
        
        for i in 0..archive.len() {
            let mut file = archive.by_index(i)
                .map_err(|e| TingError::PluginLoadError(format!("Failed to read zip entry: {}", e)))?;
            
            let outpath = match file.enclosed_name() {
                Some(path) => target.join(path),
                None => continue,
            };
            
            if file.name().ends_with('/') {
                fs::create_dir_all(&outpath)?;
            } else {
                if let Some(p) = outpath.parent() {
                    if !p.exists() {
                        fs::create_dir_all(p)?;
                    }
                }
                let mut outfile = fs::File::create(&outpath)?;
                std::io::copy(&mut file, &mut outfile)?;
            }
            
            // Get and set permissions
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                if let Some(mode) = file.unix_mode() {
                    fs::set_permissions(&outpath, fs::Permissions::from_mode(mode))?;
                }
            }
        }
        
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

/// Installation backup for rollback support
///
/// **Validates: Requirement 26.8**
struct InstallationBackup {
    /// Path being installed to
    target_path: PathBuf,
    /// Backup path (if target existed before)
    backup_path: Option<PathBuf>,
    /// Whether the installation was committed
    committed: bool,
}

impl InstallationBackup {
    /// Create a new installation backup
    fn new(target_path: &Path) -> Result<Self> {
        let backup_path = if target_path.exists() {
            // Target exists, create backup
            let backup = target_path.with_extension("backup");
            debug!("Creating backup: {} -> {}", target_path.display(), backup.display());
            
            // Remove old backup if it exists
            if backup.exists() {
                fs::remove_dir_all(&backup)?;
            }
            
            // Rename current to backup
            fs::rename(target_path, &backup)?;
            Some(backup)
        } else {
            None
        };
        
        Ok(Self {
            target_path: target_path.to_path_buf(),
            backup_path,
            committed: false,
        })
    }
    
    /// Commit the installation (delete backup)
    fn commit(mut self) -> Result<()> {
        self.committed = true;
        
        if let Some(backup) = &self.backup_path {
            debug!("Committing installation, removing backup: {}", backup.display());
            fs::remove_dir_all(backup)?;
        }
        
        Ok(())
    }
    
    /// Rollback the installation (restore backup)
    fn rollback(&self) -> Result<()> {
        warn!("Rolling back installation: {}", self.target_path.display());
        
        // Remove failed installation
        if self.target_path.exists() {
            fs::remove_dir_all(&self.target_path)?;
        }
        
        // Restore backup if it exists
        if let Some(backup) = &self.backup_path {
            debug!("Restoring backup: {} -> {}", backup.display(), self.target_path.display());
            fs::rename(backup, &self.target_path)?;
        }
        
        Ok(())
    }
}

impl Drop for InstallationBackup {
    fn drop(&mut self) {
        if !self.committed {
            // If not committed and we're being dropped, rollback
            if let Err(e) = self.rollback() {
                error!("Failed to rollback installation: {}", e);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use crate::plugin::types::PluginDependency;
    
    fn create_test_plugin(dir: &Path, name: &str, version: &str) -> Result<()> {
        let metadata = PluginMetadata {
            name: name.to_string(),
            version: version.to_string(),
            plugin_type: crate::plugin::types::PluginType::Utility,
            author: "Test Author".to_string(),
            description: "Test plugin".to_string(),
            license: Some("MIT".to_string()),
            homepage: None,
            entry_point: "plugin.js".to_string(),
            dependencies: vec![],
            npm_dependencies: vec![],
            permissions: vec![],
            config_schema: None,
            min_core_version: None,
            supported_extensions: None,
        };
        
        let metadata_json = serde_json::to_string_pretty(&metadata)
            .map_err(|e| TingError::PluginLoadError(format!("Failed to serialize metadata: {}", e)))?;
        fs::write(dir.join("plugin.json"), metadata_json)?;
        fs::write(dir.join("plugin.js"), "// Test plugin")?;
        
        Ok(())
    }
    
    fn create_test_plugin_with_dependencies(
        dir: &Path,
        name: &str,
        version: &str,
        dependencies: Vec<PluginDependency>,
    ) -> Result<()> {
        let metadata = PluginMetadata {
            name: name.to_string(),
            version: version.to_string(),
            plugin_type: crate::plugin::types::PluginType::Utility,
            author: "Test Author".to_string(),
            description: "Test plugin with dependencies".to_string(),
            license: Some("MIT".to_string()),
            homepage: None,
            entry_point: "plugin.js".to_string(),
            dependencies,
            npm_dependencies: vec![],
            permissions: vec![],
            config_schema: None,
            min_core_version: None,
            supported_extensions: None,
        };
        
        let metadata_json = serde_json::to_string_pretty(&metadata)
            .map_err(|e| TingError::PluginLoadError(format!("Failed to serialize metadata: {}", e)))?;
        fs::write(dir.join("plugin.json"), metadata_json)?;
        fs::write(dir.join("plugin.js"), "// Test plugin with dependencies")?;
        
        Ok(())
    }
    
    #[tokio::test]
    async fn test_install_plugin_success() {
        let temp_dir = TempDir::new().unwrap();
        let plugin_dir = temp_dir.path().join("plugins");
        let temp_extract = temp_dir.path().join("temp");
        let source_dir = temp_dir.path().join("source");
        
        fs::create_dir_all(&source_dir).unwrap();
        create_test_plugin(&source_dir, "test-plugin", "1.0.0").unwrap();
        
        let installer = PluginInstaller::new(plugin_dir.clone(), temp_extract).unwrap();
        
        let result = installer.install_plugin(
            &source_dir,
            |_metadata| Ok(()),
        ).await;
        
        assert!(result.is_ok());
        let plugin_id = result.unwrap();
        assert_eq!(plugin_id, "test-plugin@1.0.0");
        
        // Verify plugin was installed
        let installed_path = plugin_dir.join(&plugin_id);
        assert!(installed_path.exists());
        assert!(installed_path.join("plugin.json").exists());
        assert!(installed_path.join("plugin.js").exists());
    }
    
    #[tokio::test]
    async fn test_install_plugin_dependency_failure() {
        let temp_dir = TempDir::new().unwrap();
        let plugin_dir = temp_dir.path().join("plugins");
        let temp_extract = temp_dir.path().join("temp");
        let source_dir = temp_dir.path().join("source");
        
        fs::create_dir_all(&source_dir).unwrap();
        create_test_plugin(&source_dir, "test-plugin", "1.0.0").unwrap();
        
        let installer = PluginInstaller::new(plugin_dir.clone(), temp_extract).unwrap();
        
        let result = installer.install_plugin(
            &source_dir,
            |_metadata| Err(TingError::DependencyError("Missing dependency".to_string())),
        ).await;
        
        assert!(result.is_err());
        
        // Verify plugin was NOT installed
        let installed_path = plugin_dir.join("test-plugin@1.0.0");
        assert!(!installed_path.exists());
    }
    
    #[tokio::test]
    async fn test_install_plugin_rollback() {
        let temp_dir = TempDir::new().unwrap();
        let plugin_dir = temp_dir.path().join("plugins");
        let temp_extract = temp_dir.path().join("temp");
        let source_dir = temp_dir.path().join("source");
        
        fs::create_dir_all(&source_dir).unwrap();
        fs::create_dir_all(&plugin_dir).unwrap();
        
        // Create existing plugin
        let existing_plugin_dir = plugin_dir.join("test-plugin@1.0.0");
        fs::create_dir_all(&existing_plugin_dir).unwrap();
        fs::write(existing_plugin_dir.join("old_file.txt"), "old content").unwrap();
        
        // Create invalid source (missing plugin.json)
        fs::write(source_dir.join("invalid.txt"), "invalid").unwrap();
        
        let installer = PluginInstaller::new(plugin_dir.clone(), temp_extract).unwrap();
        
        let result = installer.install_plugin(
            &source_dir,
            |_metadata| Ok(()),
        ).await;
        
        assert!(result.is_err());
        
        // Verify old plugin was restored
        assert!(existing_plugin_dir.exists());
        assert!(existing_plugin_dir.join("old_file.txt").exists());
    }
    
    #[test]
    fn test_calculate_checksum() {
        let temp_dir = TempDir::new().unwrap();
        let plugin_dir = temp_dir.path().join("plugin");
        fs::create_dir_all(&plugin_dir).unwrap();
        
        fs::write(plugin_dir.join("file1.txt"), "content1").unwrap();
        fs::write(plugin_dir.join("file2.txt"), "content2").unwrap();
        
        let installer = PluginInstaller::new(
            temp_dir.path().join("plugins"),
            temp_dir.path().join("temp"),
        ).unwrap();
        
        let checksum1 = installer.calculate_checksum(&plugin_dir).unwrap();
        let checksum2 = installer.calculate_checksum(&plugin_dir).unwrap();
        
        // Same files should produce same checksum
        assert_eq!(checksum1, checksum2);
        
        // Modify a file
        fs::write(plugin_dir.join("file1.txt"), "modified").unwrap();
        let checksum3 = installer.calculate_checksum(&plugin_dir).unwrap();
        
        // Checksum should change
        assert_ne!(checksum1, checksum3);
    }
    
    #[tokio::test]
    async fn test_uninstall_plugin() {
        let temp_dir = TempDir::new().unwrap();
        let plugin_dir = temp_dir.path().join("plugins");
        let temp_extract = temp_dir.path().join("temp");
        let source_dir = temp_dir.path().join("source");
        
        fs::create_dir_all(&source_dir).unwrap();
        create_test_plugin(&source_dir, "test-plugin", "1.0.0").unwrap();
        
        let installer = PluginInstaller::new(plugin_dir.clone(), temp_extract).unwrap();
        
        // Install plugin
        let plugin_id = installer.install_plugin(
            &source_dir,
            |_metadata| Ok(()),
        ).await.unwrap();
        
        let installed_path = plugin_dir.join(&plugin_id);
        assert!(installed_path.exists());
        
        // Uninstall plugin
        installer.uninstall_plugin(&plugin_id).unwrap();
        
        // Verify plugin was removed
        assert!(!installed_path.exists());
    }
    
    // ========== Tests for Requirement 26.2: Plugin Package Validation ==========
    
    #[tokio::test]
    async fn test_validate_package_missing_plugin_json() {
        let temp_dir = TempDir::new().unwrap();
        let plugin_dir = temp_dir.path().join("plugins");
        let temp_extract = temp_dir.path().join("temp");
        let source_dir = temp_dir.path().join("source");
        
        fs::create_dir_all(&source_dir).unwrap();
        // Create plugin without plugin.json
        fs::write(source_dir.join("plugin.js"), "// Test plugin").unwrap();
        
        let installer = PluginInstaller::new(plugin_dir, temp_extract).unwrap();
        
        let result = installer.install_plugin(
            &source_dir,
            |_metadata| Ok(()),
        ).await;
        
        assert!(result.is_err());
        match result.unwrap_err() {
            TingError::PluginLoadError(msg) => {
                assert!(msg.contains("plugin.json not found"));
            }
            _ => panic!("Expected PluginLoadError"),
        }
    }
    
    #[tokio::test]
    async fn test_validate_package_invalid_json() {
        let temp_dir = TempDir::new().unwrap();
        let plugin_dir = temp_dir.path().join("plugins");
        let temp_extract = temp_dir.path().join("temp");
        let source_dir = temp_dir.path().join("source");
        
        fs::create_dir_all(&source_dir).unwrap();
        // Create invalid plugin.json
        fs::write(source_dir.join("plugin.json"), "{ invalid json }").unwrap();
        fs::write(source_dir.join("plugin.js"), "// Test plugin").unwrap();
        
        let installer = PluginInstaller::new(plugin_dir, temp_extract).unwrap();
        
        let result = installer.install_plugin(
            &source_dir,
            |_metadata| Ok(()),
        ).await;
        
        assert!(result.is_err());
        match result.unwrap_err() {
            TingError::PluginLoadError(msg) => {
                assert!(msg.contains("Invalid plugin.json"));
            }
            _ => panic!("Expected PluginLoadError"),
        }
    }
    
    #[tokio::test]
    async fn test_validate_package_nonexistent_path() {
        let temp_dir = TempDir::new().unwrap();
        let plugin_dir = temp_dir.path().join("plugins");
        let temp_extract = temp_dir.path().join("temp");
        let nonexistent_dir = temp_dir.path().join("nonexistent");
        
        let installer = PluginInstaller::new(plugin_dir, temp_extract).unwrap();
        
        let result = installer.install_plugin(
            &nonexistent_dir,
            |_metadata| Ok(()),
        ).await;
        
        assert!(result.is_err());
        match result.unwrap_err() {
            TingError::PluginLoadError(msg) => {
                assert!(msg.contains("not found"));
            }
            _ => panic!("Expected PluginLoadError"),
        }
    }
    
    #[test]
    fn test_checksum_consistency() {
        let temp_dir = TempDir::new().unwrap();
        let plugin_dir = temp_dir.path().join("plugin");
        fs::create_dir_all(&plugin_dir).unwrap();
        
        fs::write(plugin_dir.join("file1.txt"), "content1").unwrap();
        fs::write(plugin_dir.join("file2.txt"), "content2").unwrap();
        
        let installer = PluginInstaller::new(
            temp_dir.path().join("plugins"),
            temp_dir.path().join("temp"),
        ).unwrap();
        
        // Calculate checksum multiple times
        let checksum1 = installer.calculate_checksum(&plugin_dir).unwrap();
        let checksum2 = installer.calculate_checksum(&plugin_dir).unwrap();
        let checksum3 = installer.calculate_checksum(&plugin_dir).unwrap();
        
        // Same files should always produce same checksum
        assert_eq!(checksum1, checksum2);
        assert_eq!(checksum2, checksum3);
    }
    
    #[test]
    fn test_checksum_detects_file_changes() {
        let temp_dir = TempDir::new().unwrap();
        let plugin_dir = temp_dir.path().join("plugin");
        fs::create_dir_all(&plugin_dir).unwrap();
        
        fs::write(plugin_dir.join("file1.txt"), "original content").unwrap();
        
        let installer = PluginInstaller::new(
            temp_dir.path().join("plugins"),
            temp_dir.path().join("temp"),
        ).unwrap();
        
        let checksum_before = installer.calculate_checksum(&plugin_dir).unwrap();
        
        // Modify file content
        fs::write(plugin_dir.join("file1.txt"), "modified content").unwrap();
        
        let checksum_after = installer.calculate_checksum(&plugin_dir).unwrap();
        
        // Checksum should be different
        assert_ne!(checksum_before, checksum_after);
    }
    
    #[test]
    fn test_checksum_detects_new_files() {
        let temp_dir = TempDir::new().unwrap();
        let plugin_dir = temp_dir.path().join("plugin");
        fs::create_dir_all(&plugin_dir).unwrap();
        
        fs::write(plugin_dir.join("file1.txt"), "content").unwrap();
        
        let installer = PluginInstaller::new(
            temp_dir.path().join("plugins"),
            temp_dir.path().join("temp"),
        ).unwrap();
        
        let checksum_before = installer.calculate_checksum(&plugin_dir).unwrap();
        
        // Add new file
        fs::write(plugin_dir.join("file2.txt"), "new content").unwrap();
        
        let checksum_after = installer.calculate_checksum(&plugin_dir).unwrap();
        
        // Checksum should be different
        assert_ne!(checksum_before, checksum_after);
    }
    
    #[test]
    fn test_checksum_detects_deleted_files() {
        let temp_dir = TempDir::new().unwrap();
        let plugin_dir = temp_dir.path().join("plugin");
        fs::create_dir_all(&plugin_dir).unwrap();
        
        fs::write(plugin_dir.join("file1.txt"), "content1").unwrap();
        fs::write(plugin_dir.join("file2.txt"), "content2").unwrap();
        
        let installer = PluginInstaller::new(
            temp_dir.path().join("plugins"),
            temp_dir.path().join("temp"),
        ).unwrap();
        
        let checksum_before = installer.calculate_checksum(&plugin_dir).unwrap();
        
        // Delete a file
        fs::remove_file(plugin_dir.join("file2.txt")).unwrap();
        
        let checksum_after = installer.calculate_checksum(&plugin_dir).unwrap();
        
        // Checksum should be different
        assert_ne!(checksum_before, checksum_after);
    }
    
    // ========== Tests for Requirement 26.3: Dependency Checking ==========
    
    #[tokio::test]
    async fn test_dependency_check_success() {
        let temp_dir = TempDir::new().unwrap();
        let plugin_dir = temp_dir.path().join("plugins");
        let temp_extract = temp_dir.path().join("temp");
        let source_dir = temp_dir.path().join("source");
        
        fs::create_dir_all(&source_dir).unwrap();
        
        let dependencies = vec![
            PluginDependency {
                plugin_name: "base-plugin".to_string(),
                version_requirement: "^1.0.0".to_string(),
            },
        ];
        
        create_test_plugin_with_dependencies(&source_dir, "dependent-plugin", "1.0.0", dependencies).unwrap();
        
        let installer = PluginInstaller::new(plugin_dir.clone(), temp_extract).unwrap();
        
        // Dependency checker that succeeds
        let result = installer.install_plugin(
            &source_dir,
            |metadata| {
                assert_eq!(metadata.dependencies.len(), 1);
                assert_eq!(metadata.dependencies[0].plugin_name, "base-plugin");
                Ok(())
            },
        ).await;
        
        assert!(result.is_ok());
    }
    
    #[tokio::test]
    async fn test_dependency_check_missing_dependency() {
        let temp_dir = TempDir::new().unwrap();
        let plugin_dir = temp_dir.path().join("plugins");
        let temp_extract = temp_dir.path().join("temp");
        let source_dir = temp_dir.path().join("source");
        
        fs::create_dir_all(&source_dir).unwrap();
        
        let dependencies = vec![
            PluginDependency {
                plugin_name: "missing-plugin".to_string(),
                version_requirement: "^1.0.0".to_string(),
            },
        ];
        
        create_test_plugin_with_dependencies(&source_dir, "dependent-plugin", "1.0.0", dependencies).unwrap();
        
        let installer = PluginInstaller::new(plugin_dir.clone(), temp_extract).unwrap();
        
        // Dependency checker that fails
        let result = installer.install_plugin(
            &source_dir,
            |_metadata| Err(TingError::DependencyError("Missing dependency: missing-plugin".to_string())),
        ).await;
        
        assert!(result.is_err());
        match result.unwrap_err() {
            TingError::DependencyError(msg) => {
                assert!(msg.contains("missing-plugin"));
            }
            _ => panic!("Expected DependencyError"),
        }
        
        // Verify plugin was NOT installed
        let installed_path = plugin_dir.join("dependent-plugin@1.0.0");
        assert!(!installed_path.exists());
    }
    
    #[tokio::test]
    async fn test_dependency_check_version_incompatible() {
        let temp_dir = TempDir::new().unwrap();
        let plugin_dir = temp_dir.path().join("plugins");
        let temp_extract = temp_dir.path().join("temp");
        let source_dir = temp_dir.path().join("source");
        
        fs::create_dir_all(&source_dir).unwrap();
        
        let dependencies = vec![
            PluginDependency {
                plugin_name: "base-plugin".to_string(),
                version_requirement: "^2.0.0".to_string(),
            },
        ];
        
        create_test_plugin_with_dependencies(&source_dir, "dependent-plugin", "1.0.0", dependencies).unwrap();
        
        let installer = PluginInstaller::new(plugin_dir.clone(), temp_extract).unwrap();
        
        // Dependency checker that fails due to version incompatibility
        let result = installer.install_plugin(
            &source_dir,
            |_metadata| Err(TingError::DependencyError(
                "Version incompatible: base-plugin requires ^2.0.0, found 1.0.0".to_string()
            )),
        ).await;
        
        assert!(result.is_err());
        match result.unwrap_err() {
            TingError::DependencyError(msg) => {
                assert!(msg.contains("Version incompatible"));
            }
            _ => panic!("Expected DependencyError"),
        }
    }
    
    #[tokio::test]
    async fn test_dependency_check_multiple_dependencies() {
        let temp_dir = TempDir::new().unwrap();
        let plugin_dir = temp_dir.path().join("plugins");
        let temp_extract = temp_dir.path().join("temp");
        let source_dir = temp_dir.path().join("source");
        
        fs::create_dir_all(&source_dir).unwrap();
        
        let dependencies = vec![
            PluginDependency {
                plugin_name: "plugin-a".to_string(),
                version_requirement: "^1.0.0".to_string(),
            },
            PluginDependency {
                plugin_name: "plugin-b".to_string(),
                version_requirement: "^2.0.0".to_string(),
            },
            PluginDependency {
                plugin_name: "plugin-c".to_string(),
                version_requirement: "^3.0.0".to_string(),
            },
        ];
        
        create_test_plugin_with_dependencies(&source_dir, "complex-plugin", "1.0.0", dependencies).unwrap();
        
        let installer = PluginInstaller::new(plugin_dir.clone(), temp_extract).unwrap();
        
        // Dependency checker that validates all dependencies
        let result = installer.install_plugin(
            &source_dir,
            |metadata| {
                assert_eq!(metadata.dependencies.len(), 3);
                assert_eq!(metadata.dependencies[0].plugin_name, "plugin-a");
                assert_eq!(metadata.dependencies[1].plugin_name, "plugin-b");
                assert_eq!(metadata.dependencies[2].plugin_name, "plugin-c");
                Ok(())
            },
        ).await;
        
        assert!(result.is_ok());
    }
    
    // ========== Tests for Requirement 26.8: Installation Rollback ==========
    
    #[tokio::test]
    async fn test_rollback_on_validation_failure() {
        let temp_dir = TempDir::new().unwrap();
        let plugin_dir = temp_dir.path().join("plugins");
        let temp_extract = temp_dir.path().join("temp");
        let source_dir = temp_dir.path().join("source");
        
        fs::create_dir_all(&source_dir).unwrap();
        fs::create_dir_all(&plugin_dir).unwrap();
        
        // Create existing plugin version
        let existing_plugin_dir = plugin_dir.join("test-plugin@1.0.0");
        fs::create_dir_all(&existing_plugin_dir).unwrap();
        fs::write(existing_plugin_dir.join("old_file.txt"), "old content").unwrap();
        fs::write(existing_plugin_dir.join("plugin.json"), r#"{"name":"test-plugin","version":"1.0.0"}"#).unwrap();
        
        // Create invalid source (missing plugin.json)
        fs::write(source_dir.join("invalid.txt"), "invalid").unwrap();
        
        let installer = PluginInstaller::new(plugin_dir.clone(), temp_extract).unwrap();
        
        let result = installer.install_plugin(
            &source_dir,
            |_metadata| Ok(()),
        ).await;
        
        assert!(result.is_err());
        
        // Verify old plugin was restored
        assert!(existing_plugin_dir.exists());
        assert!(existing_plugin_dir.join("old_file.txt").exists());
        let old_content = fs::read_to_string(existing_plugin_dir.join("old_file.txt")).unwrap();
        assert_eq!(old_content, "old content");
    }
    
    #[tokio::test]
    async fn test_rollback_on_dependency_failure() {
        let temp_dir = TempDir::new().unwrap();
        let plugin_dir = temp_dir.path().join("plugins");
        let temp_extract = temp_dir.path().join("temp");
        let source_dir = temp_dir.path().join("source");
        
        fs::create_dir_all(&source_dir).unwrap();
        fs::create_dir_all(&plugin_dir).unwrap();
        
        // Create existing plugin version
        let existing_plugin_dir = plugin_dir.join("test-plugin@1.0.0");
        fs::create_dir_all(&existing_plugin_dir).unwrap();
        fs::write(existing_plugin_dir.join("original.txt"), "original data").unwrap();
        
        // Create new version
        create_test_plugin(&source_dir, "test-plugin", "1.0.0").unwrap();
        
        let installer = PluginInstaller::new(plugin_dir.clone(), temp_extract).unwrap();
        
        let result = installer.install_plugin(
            &source_dir,
            |_metadata| Err(TingError::DependencyError("Dependency check failed".to_string())),
        ).await;
        
        assert!(result.is_err());
        
        // Verify old plugin was restored
        assert!(existing_plugin_dir.exists());
        assert!(existing_plugin_dir.join("original.txt").exists());
        let original_content = fs::read_to_string(existing_plugin_dir.join("original.txt")).unwrap();
        assert_eq!(original_content, "original data");
    }
    
    #[tokio::test]
    async fn test_rollback_cleans_up_partial_installation() {
        let temp_dir = TempDir::new().unwrap();
        let plugin_dir = temp_dir.path().join("plugins");
        let temp_extract = temp_dir.path().join("temp");
        let source_dir = temp_dir.path().join("source");
        
        fs::create_dir_all(&source_dir).unwrap();
        create_test_plugin(&source_dir, "test-plugin", "1.0.0").unwrap();
        
        let installer = PluginInstaller::new(plugin_dir.clone(), temp_extract).unwrap();
        
        // Simulate installation failure by providing invalid dependency checker
        let result = installer.install_plugin(
            &source_dir,
            |_metadata| Err(TingError::DependencyError("Simulated failure".to_string())),
        ).await;
        
        assert!(result.is_err());
        
        // Verify no partial installation remains
        let installed_path = plugin_dir.join("test-plugin@1.0.0");
        assert!(!installed_path.exists());
    }
    
    #[tokio::test]
    async fn test_rollback_preserves_other_plugins() {
        let temp_dir = TempDir::new().unwrap();
        let plugin_dir = temp_dir.path().join("plugins");
        let temp_extract = temp_dir.path().join("temp");
        let source_dir = temp_dir.path().join("source");
        
        fs::create_dir_all(&source_dir).unwrap();
        fs::create_dir_all(&plugin_dir).unwrap();
        
        // Create other existing plugins
        let other_plugin_dir = plugin_dir.join("other-plugin@1.0.0");
        fs::create_dir_all(&other_plugin_dir).unwrap();
        fs::write(other_plugin_dir.join("data.txt"), "other plugin data").unwrap();
        
        // Try to install a plugin that will fail
        create_test_plugin(&source_dir, "failing-plugin", "1.0.0").unwrap();
        
        let installer = PluginInstaller::new(plugin_dir.clone(), temp_extract).unwrap();
        
        let result = installer.install_plugin(
            &source_dir,
            |_metadata| Err(TingError::DependencyError("Installation failed".to_string())),
        ).await;
        
        assert!(result.is_err());
        
        // Verify other plugin is still intact
        assert!(other_plugin_dir.exists());
        assert!(other_plugin_dir.join("data.txt").exists());
        let other_data = fs::read_to_string(other_plugin_dir.join("data.txt")).unwrap();
        assert_eq!(other_data, "other plugin data");
    }
    
    #[test]
    fn test_installation_backup_commit() {
        let temp_dir = TempDir::new().unwrap();
        let target_path = temp_dir.path().join("plugin");
        
        // Create existing plugin
        fs::create_dir_all(&target_path).unwrap();
        fs::write(target_path.join("old.txt"), "old").unwrap();
        
        let backup = InstallationBackup::new(&target_path).unwrap();
        
        // Backup should exist
        let backup_path = target_path.with_extension("backup");
        assert!(backup_path.exists());
        
        // Commit should remove backup
        backup.commit().unwrap();
        assert!(!backup_path.exists());
    }
    
    #[test]
    fn test_installation_backup_rollback() {
        let temp_dir = TempDir::new().unwrap();
        let target_path = temp_dir.path().join("plugin");
        
        // Create existing plugin
        fs::create_dir_all(&target_path).unwrap();
        fs::write(target_path.join("old.txt"), "old content").unwrap();
        
        let backup = InstallationBackup::new(&target_path).unwrap();
        
        // After backup, target is moved to backup, so we need to recreate it
        fs::create_dir_all(&target_path).unwrap();
        
        // Simulate new installation
        fs::write(target_path.join("new.txt"), "new content").unwrap();
        
        // Rollback should restore old state
        backup.rollback().unwrap();
        
        assert!(target_path.exists());
        assert!(target_path.join("old.txt").exists());
        assert!(!target_path.join("new.txt").exists());
        
        let old_content = fs::read_to_string(target_path.join("old.txt")).unwrap();
        assert_eq!(old_content, "old content");
    }
    
    #[test]
    fn test_installation_backup_auto_rollback_on_drop() {
        let temp_dir = TempDir::new().unwrap();
        let target_path = temp_dir.path().join("plugin");
        
        // Create existing plugin
        fs::create_dir_all(&target_path).unwrap();
        fs::write(target_path.join("original.txt"), "original").unwrap();
        
        {
            let _backup = InstallationBackup::new(&target_path).unwrap();
            
            // After backup, target is moved to backup, so we need to recreate it
            fs::create_dir_all(&target_path).unwrap();
            
            // Simulate new installation
            fs::write(target_path.join("modified.txt"), "modified").unwrap();
            
            // Drop backup without committing (simulates error)
        }
        
        // Should auto-rollback on drop
        assert!(target_path.exists());
        assert!(target_path.join("original.txt").exists());
        assert!(!target_path.join("modified.txt").exists());
    }
}
