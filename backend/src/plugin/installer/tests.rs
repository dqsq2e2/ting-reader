use super::*;
use crate::plugin::types::PluginDependency;
use std::process::Command;
use tempfile::TempDir;

fn create_test_plugin(dir: &Path, name: &str, version: &str) -> Result<()> {
    let metadata = serde_json::json!({
        "id": name,
        "name": name,
        "version": version,
        "min_core_version": "1.4.8",
        "author": "Test Author",
        "description": "Test plugin",
        "license": "MIT",
        "entry_point": "plugin.js",
        "runtime": "javascript",
        "dependencies": [],
        "permissions": [],
        "capabilities": test_capabilities(),
    });

    let metadata_yaml = serde_yaml::to_string(&metadata)
        .map_err(|e| TingError::PluginLoadError(format!("Failed to serialize metadata: {}", e)))?;
    fs::write(dir.join("plugin.yml"), metadata_yaml)?;
    fs::write(dir.join("plugin.js"), "// Test plugin")?;

    Ok(())
}

fn create_test_plugin_with_dependencies(
    dir: &Path,
    name: &str,
    version: &str,
    dependencies: Vec<PluginDependency>,
) -> Result<()> {
    let metadata = serde_json::json!({
        "id": name,
        "name": name,
        "version": version,
        "min_core_version": "1.4.8",
        "author": "Test Author",
        "description": "Test plugin with dependencies",
        "license": "MIT",
        "entry_point": "plugin.js",
        "runtime": "javascript",
        "dependencies": dependencies,
        "permissions": [],
        "capabilities": test_capabilities(),
    });

    let metadata_yaml = serde_yaml::to_string(&metadata)
        .map_err(|e| TingError::PluginLoadError(format!("Failed to serialize metadata: {}", e)))?;
    fs::write(dir.join("plugin.yml"), metadata_yaml)?;
    fs::write(dir.join("plugin.js"), "// Test plugin with dependencies")?;

    Ok(())
}

fn test_capabilities() -> serde_json::Value {
    serde_json::json!([
        {
            "id": "test.tools",
            "kind": "tool_provider",
            "invoke": "execute"
        }
    ])
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

    let result = installer
        .install_plugin(&source_dir, |_metadata| Ok(()))
        .await;

    assert!(result.is_ok(), "install failed: {:?}", result);
    let plugin_id = result.unwrap();
    assert_eq!(plugin_id, "test-plugin@1.0.0");

    // Verify plugin was installed
    let installed_path = plugin_dir.join(&plugin_id);
    assert!(installed_path.exists());
    assert!(installed_path.join("plugin.yml").exists());
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

    let result = installer
        .install_plugin(&source_dir, |_metadata| {
            Err(TingError::DependencyError("Missing dependency".to_string()))
        })
        .await;

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

    // Create invalid source (missing plugin.yml)
    fs::write(source_dir.join("invalid.txt"), "invalid").unwrap();

    let installer = PluginInstaller::new(plugin_dir.clone(), temp_extract).unwrap();

    let result = installer
        .install_plugin(&source_dir, |_metadata| Ok(()))
        .await;

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
    )
    .unwrap();

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
    let plugin_id = installer
        .install_plugin(&source_dir, |_metadata| Ok(()))
        .await
        .unwrap();

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
    // Create plugin without plugin.yml
    fs::write(source_dir.join("plugin.js"), "// Test plugin").unwrap();

    let installer = PluginInstaller::new(plugin_dir, temp_extract).unwrap();

    let result = installer
        .install_plugin(&source_dir, |_metadata| Ok(()))
        .await;

    assert!(result.is_err());
    match result.unwrap_err() {
        TingError::PluginLoadError(msg) => {
            assert!(msg.contains("plugin.yml not found"));
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
    // Create invalid plugin.yml
    fs::write(source_dir.join("plugin.yml"), "{ invalid yaml").unwrap();
    fs::write(source_dir.join("plugin.js"), "// Test plugin").unwrap();

    let installer = PluginInstaller::new(plugin_dir, temp_extract).unwrap();

    let result = installer
        .install_plugin(&source_dir, |_metadata| Ok(()))
        .await;

    assert!(result.is_err());
    match result.unwrap_err() {
        TingError::PluginLoadError(msg) => {
            assert!(msg.contains("Invalid YAML"));
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

    let result = installer
        .install_plugin(&nonexistent_dir, |_metadata| Ok(()))
        .await;

    assert!(result.is_err());
    match result.unwrap_err() {
        TingError::PluginLoadError(msg) => {
            assert!(msg.contains("not found"));
        }
        _ => panic!("Expected PluginLoadError"),
    }
}

#[tokio::test]
async fn test_install_tr_package_success() {
    let Some(trpack) = local_trpack_binary() else {
        eprintln!("skipping .tr install test: trpack binary not found");
        return;
    };

    let temp_dir = TempDir::new().unwrap();
    let plugin_dir = temp_dir.path().join("plugins");
    let temp_extract = temp_dir.path().join("temp");
    let source_dir = temp_dir.path().join("source");
    let package_path = temp_dir.path().join("test-plugin.tr");

    fs::create_dir_all(&source_dir).unwrap();
    create_test_plugin(&source_dir, "test-plugin", "1.0.0").unwrap();

    let output = Command::new(trpack)
        .arg("build")
        .arg(&source_dir)
        .arg("--output")
        .arg(&package_path)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "trpack build failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let installer = PluginInstaller::new(plugin_dir.clone(), temp_extract).unwrap();
    let plugin_id = installer
        .install_plugin(&package_path, |_metadata| Ok(()))
        .await
        .unwrap();

    assert_eq!(plugin_id, "test-plugin@1.0.0");
    assert!(plugin_dir.join(&plugin_id).join("plugin.yml").exists());
    assert!(plugin_dir.join(&plugin_id).join("plugin.js").exists());
}

#[tokio::test]
async fn test_renamed_zip_tr_package_is_rejected() {
    let temp_dir = TempDir::new().unwrap();
    let plugin_dir = temp_dir.path().join("plugins");
    let temp_extract = temp_dir.path().join("temp");
    let package_path = temp_dir.path().join("renamed.tr");

    fs::write(&package_path, b"PK\x03\x04not really a tr package").unwrap();

    let installer = PluginInstaller::new(plugin_dir, temp_extract).unwrap();
    let result = installer
        .install_plugin(&package_path, |_metadata| Ok(()))
        .await;

    assert!(result.is_err());
    let error = result.unwrap_err().to_string();
    assert!(
        error.contains("is not a valid Ting Reader .tr package"),
        "{error}"
    );
}

fn local_trpack_binary() -> Option<std::path::PathBuf> {
    let exe = if cfg!(windows) {
        "trpack.exe"
    } else {
        "trpack"
    };
    let candidate = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()?
        .parent()?
        .join("trpack")
        .join("target")
        .join("debug")
        .join(exe);
    candidate.exists().then_some(candidate)
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
    )
    .unwrap();

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
    )
    .unwrap();

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
    )
    .unwrap();

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
    )
    .unwrap();

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

    let dependencies = vec![PluginDependency {
        plugin_name: "base-plugin".to_string(),
        version_requirement: "^1.0.0".to_string(),
    }];

    create_test_plugin_with_dependencies(&source_dir, "dependent-plugin", "1.0.0", dependencies)
        .unwrap();

    let installer = PluginInstaller::new(plugin_dir.clone(), temp_extract).unwrap();

    // Dependency checker that succeeds
    let result = installer
        .install_plugin(&source_dir, |metadata| {
            assert_eq!(metadata.dependencies.len(), 1);
            assert_eq!(metadata.dependencies[0].plugin_name, "base-plugin");
            Ok(())
        })
        .await;

    assert!(result.is_ok(), "install failed: {:?}", result);
}

#[tokio::test]
async fn test_dependency_check_missing_dependency() {
    let temp_dir = TempDir::new().unwrap();
    let plugin_dir = temp_dir.path().join("plugins");
    let temp_extract = temp_dir.path().join("temp");
    let source_dir = temp_dir.path().join("source");

    fs::create_dir_all(&source_dir).unwrap();

    let dependencies = vec![PluginDependency {
        plugin_name: "missing-plugin".to_string(),
        version_requirement: "^1.0.0".to_string(),
    }];

    create_test_plugin_with_dependencies(&source_dir, "dependent-plugin", "1.0.0", dependencies)
        .unwrap();

    let installer = PluginInstaller::new(plugin_dir.clone(), temp_extract).unwrap();

    // Dependency checker that fails
    let result = installer
        .install_plugin(&source_dir, |_metadata| {
            Err(TingError::DependencyError(
                "Missing dependency: missing-plugin".to_string(),
            ))
        })
        .await;

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

    let dependencies = vec![PluginDependency {
        plugin_name: "base-plugin".to_string(),
        version_requirement: "^2.0.0".to_string(),
    }];

    create_test_plugin_with_dependencies(&source_dir, "dependent-plugin", "1.0.0", dependencies)
        .unwrap();

    let installer = PluginInstaller::new(plugin_dir.clone(), temp_extract).unwrap();

    // Dependency checker that fails due to version incompatibility
    let result = installer
        .install_plugin(&source_dir, |_metadata| {
            Err(TingError::DependencyError(
                "Version incompatible: base-plugin requires ^2.0.0, found 1.0.0".to_string(),
            ))
        })
        .await;

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

    create_test_plugin_with_dependencies(&source_dir, "complex-plugin", "1.0.0", dependencies)
        .unwrap();

    let installer = PluginInstaller::new(plugin_dir.clone(), temp_extract).unwrap();

    // Dependency checker that validates all dependencies
    let result = installer
        .install_plugin(&source_dir, |metadata| {
            assert_eq!(metadata.dependencies.len(), 3);
            assert_eq!(metadata.dependencies[0].plugin_name, "plugin-a");
            assert_eq!(metadata.dependencies[1].plugin_name, "plugin-b");
            assert_eq!(metadata.dependencies[2].plugin_name, "plugin-c");
            Ok(())
        })
        .await;

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
    fs::write(
        existing_plugin_dir.join("plugin.yml"),
        r#"{"name":"test-plugin","version":"1.0.0"}"#,
    )
    .unwrap();

    // Create invalid source (missing plugin.yml)
    fs::write(source_dir.join("invalid.txt"), "invalid").unwrap();

    let installer = PluginInstaller::new(plugin_dir.clone(), temp_extract).unwrap();

    let result = installer
        .install_plugin(&source_dir, |_metadata| Ok(()))
        .await;

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

    let result = installer
        .install_plugin(&source_dir, |_metadata| {
            Err(TingError::DependencyError(
                "Dependency check failed".to_string(),
            ))
        })
        .await;

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
    let result = installer
        .install_plugin(&source_dir, |_metadata| {
            Err(TingError::DependencyError("Simulated failure".to_string()))
        })
        .await;

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

    let result = installer
        .install_plugin(&source_dir, |_metadata| {
            Err(TingError::DependencyError(
                "Installation failed".to_string(),
            ))
        })
        .await;

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
