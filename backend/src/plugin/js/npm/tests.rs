#[cfg(test)]
use super::*;

#[cfg(test)]
use tempfile::TempDir;

#[test]
fn test_npm_dependency_creation() {
    let dep = NpmDependency::new("axios".to_string(), "^1.6.0".to_string());
    assert_eq!(dep.name, "axios");
    assert_eq!(dep.version, "^1.6.0");
}

#[test]
fn test_vulnerability_severity_ordering() {
    assert!(VulnerabilitySeverity::Low < VulnerabilitySeverity::Moderate);
    assert!(VulnerabilitySeverity::Moderate < VulnerabilitySeverity::High);
    assert!(VulnerabilitySeverity::High < VulnerabilitySeverity::Critical);
}

#[test]
fn test_vulnerability_severity_from_str() {
    assert_eq!(VulnerabilitySeverity::from_str("low"), Some(VulnerabilitySeverity::Low));
    assert_eq!(VulnerabilitySeverity::from_str("moderate"), Some(VulnerabilitySeverity::Moderate));
    assert_eq!(VulnerabilitySeverity::from_str("high"), Some(VulnerabilitySeverity::High));
    assert_eq!(VulnerabilitySeverity::from_str("critical"), Some(VulnerabilitySeverity::Critical));
    assert_eq!(VulnerabilitySeverity::from_str("invalid"), None);
}

#[test]
fn test_security_config_default() {
    let config = NpmSecurityConfig::default();
    assert!(config.whitelist.is_empty());
    assert!(config.enforce_version_lock);
    assert!(!config.enable_audit);
    assert!(!config.fail_on_audit_vulnerabilities);
    assert_eq!(config.max_vulnerability_severity, VulnerabilitySeverity::High);
}

#[test]
fn test_security_config_with_whitelist() {
    use std::collections::HashSet;
    let mut whitelist = HashSet::new();
    whitelist.insert("axios".to_string());
    whitelist.insert("cheerio".to_string());

    let config = NpmSecurityConfig {
        whitelist,
        enforce_version_lock: true,
        enable_audit: true,
        fail_on_audit_vulnerabilities: true,
        max_vulnerability_severity: VulnerabilitySeverity::Moderate,
    };

    assert_eq!(config.whitelist.len(), 2);
    assert!(config.whitelist.contains("axios"));
    assert!(config.whitelist.contains("cheerio"));
}

#[test]
fn test_parse_dependencies_from_json() {
    let plugin_json = serde_json::json!({
        "name": "test-plugin",
        "npm_dependencies": {
            "axios": "^1.6.0",
            "cheerio": "^1.0.0"
        }
    });

    let deps = NpmManager::parse_dependencies(&plugin_json);
    assert_eq!(deps.len(), 2);
    let dep_names: Vec<String> = deps.iter().map(|d| d.name.clone()).collect();
    assert!(dep_names.contains(&"axios".to_string()));
    assert!(dep_names.contains(&"cheerio".to_string()));
}

#[test]
fn test_parse_dependencies_empty() {
    let plugin_json = serde_json::json!({ "name": "test-plugin" });
    let deps = NpmManager::parse_dependencies(&plugin_json);
    assert_eq!(deps.len(), 0);
}

#[test]
fn test_package_json_creation() {
    let deps = vec![
        NpmDependency::new("axios".to_string(), "^1.6.0".to_string()),
        NpmDependency::new("cheerio".to_string(), "^1.0.0".to_string()),
    ];

    let package_json = PackageJson::from_plugin_metadata(
        "test-plugin", "1.0.0",
        Some("Test plugin"), Some("Test Author"), Some("MIT"), &deps,
    );

    assert_eq!(package_json.name, "test-plugin");
    assert_eq!(package_json.version, "1.0.0");
    assert_eq!(package_json.description, Some("Test plugin".to_string()));
    assert_eq!(package_json.author, Some("Test Author".to_string()));
    assert_eq!(package_json.license, Some("MIT".to_string()));
    assert_eq!(package_json.dependencies.len(), 2);
    assert_eq!(package_json.dependencies.get("axios"), Some(&"^1.6.0".to_string()));
    assert_eq!(package_json.dependencies.get("cheerio"), Some(&"^1.0.0".to_string()));
    assert!(package_json.private);
}

#[test]
fn test_package_json_write_and_read() {
    let temp_dir = TempDir::new().unwrap();
    let package_json_path = temp_dir.path().join("package.json");

    let deps = vec![NpmDependency::new("axios".to_string(), "^1.6.0".to_string())];
    let package_json = PackageJson::from_plugin_metadata(
        "test-plugin", "1.0.0",
        Some("Test plugin"), Some("Test Author"), Some("MIT"), &deps,
    );

    package_json.write_to_file(&package_json_path).unwrap();
    assert!(package_json_path.exists());

    let read_package_json = PackageJson::read_from_file(&package_json_path).unwrap();
    assert_eq!(read_package_json.name, "test-plugin");
    assert_eq!(read_package_json.version, "1.0.0");
    assert_eq!(read_package_json.dependencies.len(), 1);
}

#[test]
fn test_npm_manager_creation() {
    let manager = NpmManager::default();
    // Can't access private fields directly; test public API
    assert!(!manager.is_cached("axios", "1.6.0"));
}

#[test]
fn test_npm_manager_with_security() {
    use std::collections::HashSet;
    let mut whitelist = HashSet::new();
    whitelist.insert("axios".to_string());

    let security_config = NpmSecurityConfig {
        whitelist,
        enforce_version_lock: true,
        enable_audit: true,
        fail_on_audit_vulnerabilities: false,
        max_vulnerability_severity: VulnerabilitySeverity::High,
    };

    let log_dir = PathBuf::from("/tmp/npm_logs");
    let manager = NpmManager::with_security(None, None, security_config, Some(log_dir));
    // Test public API
    assert!(!manager.is_cached("axios", "1.6.0"));
}

#[test]
fn test_get_node_modules_path() {
    let manager = NpmManager::default();
    let plugin_dir = PathBuf::from("/path/to/plugin");
    let node_modules_path = manager.get_node_modules_path(&plugin_dir);
    assert_eq!(node_modules_path, PathBuf::from("/path/to/plugin/node_modules"));
}

#[test]
fn test_has_node_modules() {
    let temp_dir = TempDir::new().unwrap();
    let manager = NpmManager::default();
    assert!(!manager.has_node_modules(temp_dir.path()));

    let node_modules_path = temp_dir.path().join("node_modules");
    std::fs::create_dir(&node_modules_path).unwrap();
    assert!(manager.has_node_modules(temp_dir.path()));
}

#[test]
fn test_clean_node_modules() {
    let temp_dir = TempDir::new().unwrap();
    let manager = NpmManager::default();

    let node_modules_path = temp_dir.path().join("node_modules");
    std::fs::create_dir(&node_modules_path).unwrap();
    std::fs::write(node_modules_path.join("test.txt"), "test").unwrap();

    let result = manager.clean_node_modules(temp_dir.path());
    assert!(result.is_ok());
    assert!(!manager.has_node_modules(temp_dir.path()));
}

#[test]
fn test_clean_node_modules_not_exists() {
    let temp_dir = TempDir::new().unwrap();
    let manager = NpmManager::default();
    let result = manager.clean_node_modules(temp_dir.path());
    assert!(result.is_ok());
}

#[test]
fn test_dependency_install_log_serialization() {
    let deps = vec![NpmDependency::new("axios".to_string(), "^1.6.0".to_string())];
    let log = DependencyInstallLog {
        timestamp: "2024-01-01T00:00:00Z".to_string(),
        plugin_name: "test-plugin".to_string(),
        dependencies: deps,
        success: true,
        error: None,
        audit_result: None,
    };

    let json = serde_json::to_string(&log).unwrap();
    assert!(json.contains("test-plugin"));
    assert!(json.contains("axios"));

    let deserialized: DependencyInstallLog = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.plugin_name, "test-plugin");
    assert!(deserialized.success);
}

#[test]
fn test_cache_key_generation() {
    let key = cache::get_cache_key("axios", "1.6.0");
    assert_eq!(key, "axios@1.6.0");
}

#[test]
fn test_cache_entry_creation() {
    use std::collections::HashSet;
    let mut used_by = HashSet::new();
    used_by.insert("plugin1".to_string());

    let entry = CacheEntry {
        package_name: "axios".to_string(),
        version: "1.6.0".to_string(),
        cache_path: PathBuf::from("/cache/axios@1.6.0"),
        used_by,
        last_accessed: "2024-01-01T00:00:00Z".to_string(),
        size_bytes: 1024,
    };

    assert_eq!(entry.package_name, "axios");
    assert_eq!(entry.version, "1.6.0");
    assert_eq!(entry.used_by.len(), 1);
    assert!(entry.used_by.contains("plugin1"));
}

#[test]
fn test_cache_statistics_default() {
    let stats = CacheStatistics::default();
    assert_eq!(stats.total_packages, 0);
    assert_eq!(stats.hit_rate, 0.0);
}

#[test]
fn test_cache_statistics_hit_rate() {
    let stats = CacheStatistics {
        total_packages: 5,
        total_size_bytes: 5120,
        cache_hits: 8,
        cache_misses: 2,
        hit_rate: 0.8,
        plugins_count: 3,
        last_cleanup: None,
    };
    assert_eq!(stats.cache_hits, 8);
    assert_eq!(stats.cache_misses, 2);
    assert_eq!(stats.hit_rate, 0.8);
}

#[test]
fn test_get_cache_statistics() {
    let temp_dir = TempDir::new().unwrap();
    let cache_dir = temp_dir.path().join("cache");
    let manager = NpmManager::new(None, Some(cache_dir));

    let stats = manager.get_cache_statistics();
    assert_eq!(stats.total_packages, 0);
    assert_eq!(stats.cache_hits, 0);
    assert_eq!(stats.cache_misses, 0);
    assert_eq!(stats.hit_rate, 0.0);
}

#[test]
fn test_clear_cache() {
    let temp_dir = TempDir::new().unwrap();
    let cache_dir = temp_dir.path().join("cache");
    std::fs::create_dir_all(&cache_dir).unwrap();

    let manager = NpmManager::new(None, Some(cache_dir.clone()));
    let result = manager.clear_cache();
    assert!(result.is_ok());

    let stats = manager.get_cache_statistics();
    assert_eq!(stats.total_packages, 0);
    assert!(stats.last_cleanup.is_some());
}

#[test]
fn test_cleanup_all_unused() {
    let temp_dir = TempDir::new().unwrap();
    let cache_dir = temp_dir.path().join("cache");
    std::fs::create_dir_all(&cache_dir).unwrap();

    let manager = NpmManager::new(None, Some(cache_dir));
    let removed = manager.cleanup_all_unused().unwrap();
    assert_eq!(removed, 0);
}

#[test]
fn test_cache_hit_rate_update() {
    let mut stats = CacheStatistics { cache_hits: 8, cache_misses: 2, ..Default::default() };
    cache::update_hit_rate(&mut stats);
    assert_eq!(stats.hit_rate, 0.8);

    let mut stats2 = CacheStatistics::default();
    cache::update_hit_rate(&mut stats2);
    assert_eq!(stats2.hit_rate, 0.0);
}
