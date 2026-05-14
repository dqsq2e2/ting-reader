use super::*;
use tempfile::TempDir;

fn test_encryption_key() -> [u8; 32] {
    [0u8; 32]
}

fn test_config_manager() -> (PluginConfigManager, TempDir) {
    let temp_dir = TempDir::new().unwrap();
    let manager = PluginConfigManager::new(temp_dir.path().to_path_buf(), test_encryption_key()).unwrap();
    (manager, temp_dir)
}

#[test]
fn test_initialize_config() {
    let (manager, _temp_dir) = test_config_manager();
    let plugin_id = "test-plugin@1.0.0".to_string();
    let config = serde_json::json!({"setting1": "value1", "setting2": 42});

    let result = manager.initialize_config(plugin_id.clone(), "Test Plugin".to_string(), None, config.clone());
    assert!(result.is_ok());

    let retrieved = manager.get_config(&plugin_id).unwrap();
    assert_eq!(retrieved, config);
}

#[test]
fn test_config_isolation() {
    let (manager, _temp_dir) = test_config_manager();
    let plugin1_id = "plugin1@1.0.0".to_string();
    let plugin1_config = serde_json::json!({"key": "value1"});
    let plugin2_id = "plugin2@1.0.0".to_string();
    let plugin2_config = serde_json::json!({"key": "value2"});

    manager.initialize_config(plugin1_id.clone(), "Plugin 1".to_string(), None, plugin1_config.clone()).unwrap();
    manager.initialize_config(plugin2_id.clone(), "Plugin 2".to_string(), None, plugin2_config.clone()).unwrap();

    let retrieved1 = manager.get_config(&plugin1_id).unwrap();
    let retrieved2 = manager.get_config(&plugin2_id).unwrap();
    assert_eq!(retrieved1, plugin1_config);
    assert_eq!(retrieved2, plugin2_config);
    assert_ne!(retrieved1, retrieved2);
}

#[test]
fn test_config_validation() {
    let (manager, _temp_dir) = test_config_manager();
    let schema = serde_json::json!({
        "type": "object",
        "properties": {"port": {"type": "integer", "minimum": 1, "maximum": 65535}, "host": {"type": "string"}},
        "required": ["port", "host"]
    });
    let plugin_id = "test-plugin@1.0.0".to_string();

    let valid_config = serde_json::json!({"port": 8080, "host": "localhost"});
    assert!(manager.initialize_config(plugin_id.clone(), "Test Plugin".to_string(), Some(schema.clone()), valid_config).is_ok());
    assert!(manager.update_config(&plugin_id, serde_json::json!({"port": 8080})).is_err());
    assert!(manager.update_config(&plugin_id, serde_json::json!({"port": "not a number", "host": "localhost"})).is_err());
}

#[test]
fn test_sensitive_field_encryption() {
    let (manager, _temp_dir) = test_config_manager();
    let schema = serde_json::json!({
        "type": "object",
        "properties": {"api_key": {"type": "string", "x-encrypted": true}, "public_setting": {"type": "string"}}
    });
    let plugin_id = "test-plugin@1.0.0".to_string();
    let config = serde_json::json!({"api_key": "secret-key-12345", "public_setting": "public-value"});

    manager.initialize_config(plugin_id.clone(), "Test Plugin".to_string(), Some(schema), config.clone()).unwrap();

    let retrieved = manager.get_config(&plugin_id).unwrap();
    assert_eq!(retrieved, config);

    // Verify stored config has encrypted field
    let configs = manager.configs.read().unwrap();
    let entry = configs.get(&plugin_id).unwrap();
    let stored_api_key = entry.config.get("api_key").unwrap().as_str().unwrap();
    assert!(stored_api_key.starts_with("encrypted:"));
    assert_ne!(stored_api_key, "secret-key-12345");

    let stored_public = entry.config.get("public_setting").unwrap().as_str().unwrap();
    assert_eq!(stored_public, "public-value");
}

#[test]
fn test_config_hot_reload_notification() {
    let (manager, _temp_dir) = test_config_manager();
    let plugin_id = "test-plugin@1.0.0".to_string();
    let initial_config = serde_json::json!({"setting": "initial"});

    manager.initialize_config(plugin_id.clone(), "Test Plugin".to_string(), None, initial_config.clone()).unwrap();

    let notified = Arc::new(RwLock::new(false));
    let notified_clone = Arc::clone(&notified);
    manager.subscribe_to_changes(move |event| {
        assert_eq!(event.plugin_id, "test-plugin@1.0.0");
        assert_eq!(event.old_config.unwrap(), serde_json::json!({"setting": "initial"}));
        assert_eq!(event.new_config, serde_json::json!({"setting": "updated"}));
        *notified_clone.write().unwrap() = true;
    }).unwrap();

    manager.update_config(&plugin_id, serde_json::json!({"setting": "updated"})).unwrap();
    assert!(*notified.read().unwrap());
}

#[test]
fn test_config_persistence() {
    let temp_dir = TempDir::new().unwrap();
    let config_dir = temp_dir.path().to_path_buf();
    let plugin_id = "test-plugin@1.0.0".to_string();
    let config = serde_json::json!({"key": "value"});

    {
        let manager = PluginConfigManager::new(config_dir.clone(), test_encryption_key()).unwrap();
        manager.initialize_config(plugin_id.clone(), "Test Plugin".to_string(), None, config.clone()).unwrap();
    }

    {
        let manager = PluginConfigManager::new(config_dir, test_encryption_key()).unwrap();
        let retrieved = manager.get_config(&plugin_id).unwrap();
        assert_eq!(retrieved, config);
    }
}

#[test]
fn test_export_config() {
    let (manager, _temp_dir) = test_config_manager();
    let plugin_id = "test-plugin@1.0.0".to_string();
    let config = serde_json::json!({"setting1": "value1", "setting2": 42});

    manager.initialize_config(plugin_id.clone(), "Test Plugin".to_string(), None, config.clone()).unwrap();

    let exported = manager.export_config(&plugin_id).unwrap();
    assert_eq!(exported["plugin_id"], plugin_id);
    assert_eq!(exported["plugin_name"], "Test Plugin");
    assert_eq!(exported["config"], config);
    assert!(exported["exported_at"].is_number());
}

#[test]
fn test_import_config() {
    let (manager, _temp_dir) = test_config_manager();
    let plugin_id = "test-plugin@1.0.0".to_string();

    manager.initialize_config(plugin_id.clone(), "Test Plugin".to_string(), None, serde_json::json!({"setting": "initial"})).unwrap();

    let import_data = serde_json::json!({"config": {"setting": "updated"}});
    manager.import_config(&plugin_id, import_data).unwrap();

    let retrieved = manager.get_config(&plugin_id).unwrap();
    assert_eq!(retrieved, serde_json::json!({"setting": "updated"}));
}

#[test]
fn test_export_import_round_trip() {
    let (manager, _temp_dir) = test_config_manager();
    let plugin_id = "test-plugin@1.0.0".to_string();
    let config = serde_json::json!({"string_value": "test", "number_value": 123, "boolean_value": true, "array_value": [1, 2, 3], "object_value": {"nested": "value"}});

    manager.initialize_config(plugin_id.clone(), "Test Plugin".to_string(), None, config.clone()).unwrap();

    let exported = manager.export_config(&plugin_id).unwrap();
    manager.import_config(&plugin_id, exported).unwrap();

    let retrieved = manager.get_config(&plugin_id).unwrap();
    assert_eq!(retrieved, config);
}

#[test]
fn test_export_all_configs() {
    let (manager, _temp_dir) = test_config_manager();
    let plugin1_id = "plugin1@1.0.0".to_string();
    let plugin2_id = "plugin2@1.0.0".to_string();

    manager.initialize_config(plugin1_id.clone(), "Plugin 1".to_string(), None, serde_json::json!({"key": "value1"})).unwrap();
    manager.initialize_config(plugin2_id.clone(), "Plugin 2".to_string(), None, serde_json::json!({"key": "value2"})).unwrap();

    let exported = manager.export_all_configs().unwrap();
    let exports = exported.as_object().unwrap();
    assert_eq!(exports.len(), 2);
    assert!(exports.contains_key(&plugin1_id));
    assert!(exports.contains_key(&plugin2_id));
    assert_eq!(exports[&plugin1_id]["config"], serde_json::json!({"key": "value1"}));
    assert_eq!(exports[&plugin2_id]["config"], serde_json::json!({"key": "value2"}));
}

#[test]
fn test_backup_config() {
    let (manager, _temp_dir) = test_config_manager();
    let plugin_id = "test-plugin@1.0.0".to_string();

    manager.initialize_config(plugin_id.clone(), "Test Plugin".to_string(), None, serde_json::json!({"key": "value"})).unwrap();

    let backup_path = manager.backup_config(&plugin_id).unwrap();
    assert!(backup_path.exists());

    let backup_content = std::fs::read_to_string(&backup_path).unwrap();
    let backup_entry: PluginConfigEntry = serde_json::from_str(&backup_content).unwrap();
    assert_eq!(backup_entry.plugin_id, plugin_id);
    assert_eq!(backup_entry.plugin_name, "Test Plugin");
}

#[test]
fn test_restore_config() {
    let (manager, _temp_dir) = test_config_manager();
    let plugin_id = "test-plugin@1.0.0".to_string();
    let original_config = serde_json::json!({"key": "original"});

    manager.initialize_config(plugin_id.clone(), "Test Plugin".to_string(), None, original_config.clone()).unwrap();
    let backup_path = manager.backup_config(&plugin_id).unwrap();

    manager.update_config(&plugin_id, serde_json::json!({"key": "modified"})).unwrap();
    assert_eq!(manager.get_config(&plugin_id).unwrap()["key"], "modified");

    manager.restore_config(&backup_path).unwrap();
    assert_eq!(manager.get_config(&plugin_id).unwrap(), original_config);
}

#[test]
fn test_backup_restore_with_encryption() {
    let (manager, _temp_dir) = test_config_manager();
    let schema = serde_json::json!({
        "type": "object",
        "properties": {"api_key": {"type": "string", "x-encrypted": true}, "public_setting": {"type": "string"}}
    });
    let plugin_id = "test-plugin@1.0.0".to_string();
    let config = serde_json::json!({"api_key": "secret-key-12345", "public_setting": "public-value"});

    manager.initialize_config(plugin_id.clone(), "Test Plugin".to_string(), Some(schema), config.clone()).unwrap();
    let backup_path = manager.backup_config(&plugin_id).unwrap();

    manager.update_config(&plugin_id, serde_json::json!({"api_key": "different-key", "public_setting": "different-value"})).unwrap();
    manager.restore_config(&backup_path).unwrap();

    assert_eq!(manager.get_config(&plugin_id).unwrap(), config);
}

#[test]
fn test_import_with_validation() {
    let (manager, _temp_dir) = test_config_manager();
    let schema = serde_json::json!({
        "type": "object",
        "properties": {"port": {"type": "integer", "minimum": 1, "maximum": 65535}},
        "required": ["port"]
    });
    let plugin_id = "test-plugin@1.0.0".to_string();

    manager.initialize_config(plugin_id.clone(), "Test Plugin".to_string(), Some(schema), serde_json::json!({"port": 8080})).unwrap();

    assert!(manager.import_config(&plugin_id, serde_json::json!({"config": {}})).is_err());
    assert!(manager.import_config(&plugin_id, serde_json::json!({"config": {"port": "not a number"}})).is_err());

    assert!(manager.import_config(&plugin_id, serde_json::json!({"config": {"port": 9000}})).is_ok());
    assert_eq!(manager.get_config(&plugin_id).unwrap()["port"], 9000);
}

#[test]
fn test_restore_nonexistent_backup() {
    let (manager, temp_dir) = test_config_manager();
    let result = manager.restore_config(&temp_dir.path().join("nonexistent_backup.json"));
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("not found"));
}

#[test]
fn test_export_nonexistent_plugin() {
    let (manager, _temp_dir) = test_config_manager();
    let result = manager.export_config(&"nonexistent@1.0.0".to_string());
    assert!(result.is_err());
}

#[test]
fn test_import_missing_config_field() {
    let (manager, _temp_dir) = test_config_manager();
    let plugin_id = "test-plugin@1.0.0".to_string();
    manager.initialize_config(plugin_id.clone(), "Test Plugin".to_string(), None, serde_json::json!({"key": "value"})).unwrap();

    let result = manager.import_config(&plugin_id, serde_json::json!({"plugin_id": plugin_id, "plugin_name": "Test Plugin"}));
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("missing 'config' field"));
}
