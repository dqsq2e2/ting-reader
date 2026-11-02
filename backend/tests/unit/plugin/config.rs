use super::*;
use tempfile::TempDir;

fn test_encryption_key() -> [u8; 32] {
    [0u8; 32]
}

#[test]
fn plugin_and_core_ciphertexts_are_cross_compatible() {
    let key = test_encryption_key();
    let plaintext = "配置密码 密钥";
    let plugin = encryption::encrypt_value(&key, plaintext).unwrap();
    let core = crate::core::security::crypto::encrypt(plaintext, &key).unwrap();
    assert_eq!(
        crate::core::security::crypto::decrypt(&plugin, &key).unwrap(),
        plaintext
    );
    assert_eq!(encryption::decrypt_value(&key, &core).unwrap(), plaintext);
    assert!(encryption::decrypt_value(&[1; 32], &core).is_err());
}

#[test]
fn upgrade_migration_preserves_known_secrets_and_defaults_but_drops_removed_fields() {
    let (manager, directory) = test_config_manager();
    let schema = serde_json::json!({
        "type": "object",
        "properties": {
            "password": {"type": "string", "format": "password"},
            "enabled": {"type": "boolean", "default": true},
        },
        "additionalProperties": false,
    });
    let previous = serde_json::json!({"password": "old-secret", "removed": "unused"});
    manager
        .migrate_config("new@2".into(), "New".into(), &schema, &previous)
        .unwrap();
    assert_eq!(
        manager.get_config(&"new@2".into()).unwrap(),
        serde_json::json!({"password": "old-secret", "enabled": true})
    );
    let bytes = std::fs::read_to_string(directory.path().join("new@2.json")).unwrap();
    assert!(!bytes.contains("old-secret"));
    let incompatible = serde_json::json!({"password": false});
    assert!(
        manager
            .migrate_config("bad@2".into(), "Bad".into(), &schema, &incompatible)
            .is_err()
    );
}

fn test_config_manager() -> (PluginConfigManager, TempDir) {
    let temp_dir = TempDir::new().unwrap();
    let manager =
        PluginConfigManager::new(temp_dir.path().to_path_buf(), test_encryption_key()).unwrap();
    (manager, temp_dir)
}

#[test]
fn test_initialize_config() {
    let (manager, _temp_dir) = test_config_manager();
    let plugin_id = "test-plugin@1.0.0".to_string();
    let config = serde_json::json!({"setting1": "value1", "setting2": 42});

    let result = manager.initialize_config(
        plugin_id.clone(),
        "Test Plugin".to_string(),
        None,
        config.clone(),
    );
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

    manager
        .initialize_config(
            plugin1_id.clone(),
            "Plugin 1".to_string(),
            None,
            plugin1_config.clone(),
        )
        .unwrap();
    manager
        .initialize_config(
            plugin2_id.clone(),
            "Plugin 2".to_string(),
            None,
            plugin2_config.clone(),
        )
        .unwrap();

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
    assert!(
        manager
            .initialize_config(
                plugin_id.clone(),
                "Test Plugin".to_string(),
                Some(schema.clone()),
                valid_config
            )
            .is_ok()
    );
    assert!(
        manager
            .update_config(&plugin_id, serde_json::json!({"port": 8080}))
            .is_err()
    );
    assert!(
        manager
            .update_config(
                &plugin_id,
                serde_json::json!({"port": "not a number", "host": "localhost"})
            )
            .is_err()
    );
}

#[test]
fn localized_config_schema_survives_initialize_update_import_and_reload() {
    let (manager, directory) = test_config_manager();
    let plugin_id = "localized@1.0.0".to_string();
    let schema = serde_json::json!({
        "type": "object",
        "title": {"zh": "AI", "en": "AI"},
        "description": {"zh": "配置", "en": "Configuration"},
        "properties": {
            "api_key": {
                "type": "string", "format": "secret", "minLength": 1,
                "title": {"zh": "密钥", "en": "API key"},
                "description": {"zh": "服务密钥", "en": "Service key"},
                "placeholder": {"zh": "输入密钥", "en": "Enter key"}
            },
            "mode": {
                "type": "string", "enum": ["fast", "accurate"],
                "default": "fast",
                "enum_labels": {"fast": {"zh": "快速", "en": "Fast"}}
            }
        },
        "required": ["api_key", "mode"],
        "additionalProperties": false
    });
    let initial = serde_json::json!({"api_key": "initial-secret", "mode": "fast"});
    manager
        .initialize_config(
            plugin_id.clone(),
            "AI".into(),
            Some(schema.clone()),
            initial,
        )
        .unwrap();
    let updated = serde_json::json!({"api_key": "updated-secret", "mode": "accurate"});
    manager.update_config(&plugin_id, updated.clone()).unwrap();
    assert!(
        manager
            .update_config(&plugin_id, serde_json::json!({"mode": "fast"}))
            .is_err()
    );
    assert!(
        manager
            .update_config(
                &plugin_id,
                serde_json::json!({"api_key": "", "mode": "fast"})
            )
            .is_err()
    );
    assert!(
        manager
            .import_config(
                &plugin_id,
                serde_json::json!({"config": {"api_key": "key", "mode": "invalid"}})
            )
            .is_err()
    );
    let exported = manager.export_config(&plugin_id).unwrap();
    assert_eq!(exported["schema"], schema);
    manager.import_config(&plugin_id, exported).unwrap();
    drop(manager);

    let manager =
        PluginConfigManager::new(directory.path().to_path_buf(), test_encryption_key()).unwrap();
    manager
        .ensure_config(
            plugin_id.clone(),
            "AI".into(),
            Some(schema.clone()),
            serde_json::json!({}),
        )
        .unwrap();
    assert_eq!(manager.get_config(&plugin_id).unwrap(), updated);
    assert_eq!(manager.export_config(&plugin_id).unwrap()["schema"], schema);
    assert!(
        !std::fs::read_to_string(directory.path().join("localized@1.0.0.json"))
            .unwrap()
            .contains("updated-secret")
    );
}

#[test]
fn localized_config_schema_supports_nested_arrays_combinators_and_references() {
    let schema = serde_json::json!({
        "type": "object",
        "definitions": {
            "endpoint": {
                "title": {"en": "Endpoint"},
                "type": "string", "minLength": 1
            }
        },
        "properties": {
            "endpoints": {
                "title": {"zh": "地址"},
                "type": "array",
                "items": {
                    "description": {"en": "Endpoint settings"},
                    "allOf": [{"$ref": "#/definitions/endpoint"}]
                }
            },
            "options": {
                "type": "object",
                "patternProperties": {"^x-": {"title": {"en": "Option"}, "type": "boolean"}},
                "additionalProperties": {"description": {"en": "Extra"}, "type": "integer"}
            }
        }
    });
    let original = schema.clone();
    assert!(encryption::validate_config(&schema, &serde_json::json!({"endpoints": ["https://example.com"], "options": {"x-on": true, "count": 2}})).is_ok());
    assert!(encryption::validate_config(&schema, &serde_json::json!({"endpoints": [""]})).is_err());
    assert!(
        encryption::validate_config(&schema, &serde_json::json!({"options": {"x-on": 1}})).is_err()
    );
    assert_eq!(schema, original);
}

#[test]
fn localized_config_schema_handles_subschema_keyword_shapes() {
    for keyword in [
        "not",
        "if",
        "then",
        "else",
        "contains",
        "propertyNames",
        "additionalProperties",
        "additionalItems",
    ] {
        let schema = serde_json::json!({(keyword): {"title": {"en": "Label"}}});
        let result = encryption::validate_config(&schema, &serde_json::json!({}));
        if keyword == "not" {
            assert!(
                result
                    .unwrap_err()
                    .to_string()
                    .contains("Configuration validation failed")
            );
        } else {
            result.unwrap();
        }
    }
    for keyword in ["allOf", "anyOf", "oneOf", "items"] {
        let schema = serde_json::json!({(keyword): [{"description": {"en": "Label"}}]});
        encryption::validate_config(&schema, &serde_json::json!([])).unwrap();
    }
    let schema = serde_json::json!({
        "$schema": "http://json-schema.org/draft-07/schema#",
        "dependencies": {"enabled": {"title": {"en": "Dependency"}, "required": ["endpoint"]}}
    });
    encryption::validate_config(
        &schema,
        &serde_json::json!({"enabled": true, "endpoint": "url"}),
    )
    .unwrap();
    assert!(encryption::validate_config(&schema, &serde_json::json!({"enabled": true})).is_err());
    let schema = serde_json::json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$defs": {"value": {"title": {"en": "Value"}, "type": "integer"}},
        "prefixItems": [{"$ref": "#/$defs/value"}],
        "items": {"description": {"en": "Other values"}, "type": "string"},
        "unevaluatedItems": {"title": {"en": "Remaining items"}},
        "unevaluatedProperties": {"title": {"en": "Remaining properties"}},
        "dependentSchemas": {"enabled": {"title": {"en": "Dependency"}}},
        "contentSchema": {"title": {"en": "Content"}}
    });
    encryption::validate_config(&schema, &serde_json::json!([1, "two"])).unwrap();
    assert!(encryption::validate_config(&schema, &serde_json::json!(["wrong"])).is_err());
}

#[test]
fn localized_config_schema_does_not_rewrite_instance_data_or_property_names() {
    let literal = serde_json::json!({"title": {"en": "Literal"}, "properties": {"title": {"title": {"en": "Nested literal"}}}});
    let schema = serde_json::json!({
        "type": "object",
        "properties": {
            "title": {"type": "string", "title": {"en": "Title"}},
            "description": {"type": "string"},
            "literal": {"const": literal, "default": literal, "enum": [literal], "examples": [literal]}
        },
        "required": ["title", "literal"]
    });
    let original = schema.clone();
    encryption::validate_config(
        &schema,
        &serde_json::json!({"title": "Book", "literal": literal}),
    )
    .unwrap();
    assert!(
        encryption::validate_config(
            &schema,
            &serde_json::json!({"title": 1, "literal": literal})
        )
        .is_err()
    );
    assert_eq!(schema, original);
}

#[test]
fn localized_config_schema_still_rejects_invalid_schemas() {
    for invalid in [
        serde_json::json!({"title": 1}),
        serde_json::json!({"title": {"en": 1}}),
        serde_json::json!({"description": {}}),
        serde_json::json!({"title": {"en": "Title"}, "type": "invalid"}),
        serde_json::json!({"properties": {"field": {"title": {"en": "Field"}, "minimum": "invalid"}}}),
        serde_json::json!({"additionalProperties": [{"title": {"en": "Field"}}]}),
    ] {
        let error = encryption::validate_config(&invalid, &serde_json::json!({})).unwrap_err();
        assert!(
            error.to_string().contains("Invalid configuration schema"),
            "{error}"
        );
    }
}

#[test]
fn test_sensitive_field_encryption() {
    let (manager, _temp_dir) = test_config_manager();
    let schema = serde_json::json!({
        "type": "object",
        "properties": {"api_key": {"type": "string", "x-encrypted": true}, "public_setting": {"type": "string"}}
    });
    let plugin_id = "test-plugin@1.0.0".to_string();
    let config =
        serde_json::json!({"api_key": "secret-key-12345", "public_setting": "public-value"});

    manager
        .initialize_config(
            plugin_id.clone(),
            "Test Plugin".to_string(),
            Some(schema),
            config.clone(),
        )
        .unwrap();

    let retrieved = manager.get_config(&plugin_id).unwrap();
    assert_eq!(retrieved, config);

    // Verify stored config has encrypted field
    let configs = manager.configs.read().unwrap();
    let entry = configs.get(&plugin_id).unwrap();
    let stored_api_key = entry.config.get("api_key").unwrap().as_str().unwrap();
    assert!(stored_api_key.starts_with("encrypted:"));
    assert_ne!(stored_api_key, "secret-key-12345");

    let stored_public = entry
        .config
        .get("public_setting")
        .unwrap()
        .as_str()
        .unwrap();
    assert_eq!(stored_public, "public-value");
}

#[test]
fn test_sensitive_field_encryption_aliases() {
    let (manager, _temp_dir) = test_config_manager();
    let schema = serde_json::json!({
        "type": "object",
        "properties": {
            "api_key": {"type": "string", "encrypted": true},
            "password": {"type": "string", "format": "password"},
            "secret": {"type": "string", "format": "secret"},
            "public_setting": {"type": "string"}
        }
    });
    let plugin_id = "test-plugin@1.0.0".to_string();
    let config = serde_json::json!({
        "api_key": "api-key",
        "password": "password-value",
        "secret": "secret-value",
        "public_setting": "public-value"
    });

    manager
        .initialize_config(
            plugin_id.clone(),
            "Test Plugin".to_string(),
            Some(schema),
            config.clone(),
        )
        .unwrap();

    assert_eq!(manager.get_config(&plugin_id).unwrap(), config);

    let configs = manager.configs.read().unwrap();
    let entry = configs.get(&plugin_id).unwrap();
    for field in ["api_key", "password", "secret"] {
        let stored = entry.config.get(field).unwrap().as_str().unwrap();
        assert!(
            stored.starts_with("encrypted:"),
            "{} was not encrypted",
            field
        );
        assert_ne!(stored, config[field].as_str().unwrap());
    }
    assert_eq!(entry.config["public_setting"], "public-value");
}

#[test]
fn test_ensure_config_syncs_schema_and_encrypts_existing_sensitive_values() {
    let (manager, _temp_dir) = test_config_manager();
    let plugin_id = "test-plugin@1.0.0".to_string();

    manager
        .initialize_config(
            plugin_id.clone(),
            "Test Plugin".to_string(),
            None,
            serde_json::json!({"api_key": "plain-existing-value"}),
        )
        .unwrap();

    let schema = serde_json::json!({
        "type": "object",
        "properties": {
            "api_key": {"type": "string", "format": "password"},
            "region": {"type": "string", "default": "cn"}
        }
    });

    manager
        .ensure_config(
            plugin_id.clone(),
            "Test Plugin".to_string(),
            Some(schema),
            serde_json::json!({"region": "cn"}),
        )
        .unwrap();

    assert_eq!(
        manager.get_config(&plugin_id).unwrap(),
        serde_json::json!({"api_key": "plain-existing-value", "region": "cn"})
    );

    let configs = manager.configs.read().unwrap();
    let entry = configs.get(&plugin_id).unwrap();
    assert_eq!(entry.encrypted_fields, vec!["api_key".to_string()]);
    assert!(
        entry.config["api_key"]
            .as_str()
            .unwrap()
            .starts_with("encrypted:")
    );
}

#[test]
fn test_preserves_sensitive_field_when_placeholder_is_saved() {
    let (manager, _temp_dir) = test_config_manager();
    let schema = serde_json::json!({
        "type": "object",
        "properties": {
            "api_key": {"type": "string", "x-encrypted": true},
            "public_setting": {"type": "string"}
        }
    });
    let plugin_id = "test-plugin@1.0.0".to_string();

    manager
        .initialize_config(
            plugin_id.clone(),
            "Test Plugin".to_string(),
            Some(schema),
            serde_json::json!({
                "api_key": "secret-key",
                "public_setting": "before"
            }),
        )
        .unwrap();

    let merged = manager
        .merge_preserved_sensitive_fields(
            &plugin_id,
            serde_json::json!({
                "api_key": SECRET_UNCHANGED_PLACEHOLDER,
                "public_setting": "after"
            }),
        )
        .unwrap();
    manager.update_config(&plugin_id, merged).unwrap();

    assert_eq!(
        manager.get_config(&plugin_id).unwrap(),
        serde_json::json!({
            "api_key": "secret-key",
            "public_setting": "after"
        })
    );
}

#[test]
fn test_config_hot_reload_notification() {
    let (manager, _temp_dir) = test_config_manager();
    let plugin_id = "test-plugin@1.0.0".to_string();
    let initial_config = serde_json::json!({"setting": "initial"});

    manager
        .initialize_config(
            plugin_id.clone(),
            "Test Plugin".to_string(),
            None,
            initial_config.clone(),
        )
        .unwrap();

    let notified = Arc::new(RwLock::new(false));
    let notified_clone = Arc::clone(&notified);
    manager
        .subscribe_to_changes(move |event| {
            assert_eq!(event.plugin_id, "test-plugin@1.0.0");
            assert_eq!(
                event.old_config.unwrap(),
                serde_json::json!({"setting": "initial"})
            );
            assert_eq!(event.new_config, serde_json::json!({"setting": "updated"}));
            *notified_clone.write().unwrap() = true;
        })
        .unwrap();

    manager
        .update_config(&plugin_id, serde_json::json!({"setting": "updated"}))
        .unwrap();
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
        manager
            .initialize_config(
                plugin_id.clone(),
                "Test Plugin".to_string(),
                None,
                config.clone(),
            )
            .unwrap();
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

    manager
        .initialize_config(
            plugin_id.clone(),
            "Test Plugin".to_string(),
            None,
            config.clone(),
        )
        .unwrap();

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

    manager
        .initialize_config(
            plugin_id.clone(),
            "Test Plugin".to_string(),
            None,
            serde_json::json!({"setting": "initial"}),
        )
        .unwrap();

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

    manager
        .initialize_config(
            plugin_id.clone(),
            "Test Plugin".to_string(),
            None,
            config.clone(),
        )
        .unwrap();

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

    manager
        .initialize_config(
            plugin1_id.clone(),
            "Plugin 1".to_string(),
            None,
            serde_json::json!({"key": "value1"}),
        )
        .unwrap();
    manager
        .initialize_config(
            plugin2_id.clone(),
            "Plugin 2".to_string(),
            None,
            serde_json::json!({"key": "value2"}),
        )
        .unwrap();

    let exported = manager.export_all_configs().unwrap();
    let exports = exported.as_object().unwrap();
    assert_eq!(exports.len(), 2);
    assert!(exports.contains_key(&plugin1_id));
    assert!(exports.contains_key(&plugin2_id));
    assert_eq!(
        exports[&plugin1_id]["config"],
        serde_json::json!({"key": "value1"})
    );
    assert_eq!(
        exports[&plugin2_id]["config"],
        serde_json::json!({"key": "value2"})
    );
}

#[test]
fn test_backup_config() {
    let (manager, _temp_dir) = test_config_manager();
    let plugin_id = "test-plugin@1.0.0".to_string();

    manager
        .initialize_config(
            plugin_id.clone(),
            "Test Plugin".to_string(),
            None,
            serde_json::json!({"key": "value"}),
        )
        .unwrap();

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

    manager
        .initialize_config(
            plugin_id.clone(),
            "Test Plugin".to_string(),
            None,
            original_config.clone(),
        )
        .unwrap();
    let backup_path = manager.backup_config(&plugin_id).unwrap();

    manager
        .update_config(&plugin_id, serde_json::json!({"key": "modified"}))
        .unwrap();
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
    let config =
        serde_json::json!({"api_key": "secret-key-12345", "public_setting": "public-value"});

    manager
        .initialize_config(
            plugin_id.clone(),
            "Test Plugin".to_string(),
            Some(schema),
            config.clone(),
        )
        .unwrap();
    let backup_path = manager.backup_config(&plugin_id).unwrap();

    manager
        .update_config(
            &plugin_id,
            serde_json::json!({"api_key": "different-key", "public_setting": "different-value"}),
        )
        .unwrap();
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

    manager
        .initialize_config(
            plugin_id.clone(),
            "Test Plugin".to_string(),
            Some(schema),
            serde_json::json!({"port": 8080}),
        )
        .unwrap();

    assert!(
        manager
            .import_config(&plugin_id, serde_json::json!({"config": {}}))
            .is_err()
    );
    assert!(
        manager
            .import_config(
                &plugin_id,
                serde_json::json!({"config": {"port": "not a number"}})
            )
            .is_err()
    );

    assert!(
        manager
            .import_config(&plugin_id, serde_json::json!({"config": {"port": 9000}}))
            .is_ok()
    );
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
    manager
        .initialize_config(
            plugin_id.clone(),
            "Test Plugin".to_string(),
            None,
            serde_json::json!({"key": "value"}),
        )
        .unwrap();

    let result = manager.import_config(
        &plugin_id,
        serde_json::json!({"plugin_id": plugin_id, "plugin_name": "Test Plugin"}),
    );
    assert!(result.is_err());
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("missing 'config' field")
    );
}
