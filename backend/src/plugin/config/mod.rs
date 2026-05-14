//! Plugin configuration management
//!
//! Provides configuration storage, validation, encryption, and hot reload for plugins.

mod encryption;
#[cfg(test)]
mod tests;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use crate::core::error::{Result, TingError};
use super::types::PluginId;

/// Configuration change event
#[derive(Debug, Clone)]
pub struct ConfigChangeEvent {
    pub plugin_id: PluginId,
    pub plugin_name: String,
    pub old_config: Option<Value>,
    pub new_config: Value,
    pub timestamp: i64,
}

/// Plugin configuration entry (private)
#[derive(Debug, Clone, Serialize, Deserialize)]
struct PluginConfigEntry {
    plugin_id: PluginId,
    plugin_name: String,
    schema: Option<Value>,
    config: Value,
    #[serde(default)]
    encrypted_fields: Vec<String>,
    updated_at: i64,
}

/// Plugin configuration manager
pub struct PluginConfigManager {
    config_dir: PathBuf,
    configs: Arc<RwLock<HashMap<PluginId, PluginConfigEntry>>>,
    subscribers: Arc<RwLock<Vec<Box<dyn Fn(ConfigChangeEvent) + Send + Sync>>>>,
    encryption_key: Arc<[u8; 32]>,
}

impl PluginConfigManager {
    pub fn new(config_dir: PathBuf, encryption_key: [u8; 32]) -> Result<Self> {
        std::fs::create_dir_all(&config_dir)
            .map_err(|e| TingError::ConfigError(format!("Failed to create config directory: {}", e)))?;

        let manager = Self {
            config_dir,
            configs: Arc::new(RwLock::new(HashMap::new())),
            subscribers: Arc::new(RwLock::new(Vec::new())),
            encryption_key: Arc::new(encryption_key),
        };

        manager.load_all_configs()?;
        Ok(manager)
    }

    pub fn initialize_config(
        &self,
        plugin_id: PluginId,
        plugin_name: String,
        schema: Option<Value>,
        default_config: Value,
    ) -> Result<()> {
        tracing::info!(plugin_id = %plugin_id, plugin_name = %plugin_name, "Initializing plugin configuration");

        if let Some(ref schema_value) = schema {
            encryption::validate_config(schema_value, &default_config)?;
        }

        let encrypted_fields = if let Some(ref schema_value) = schema {
            encryption::extract_encrypted_fields(schema_value)
        } else {
            Vec::new()
        };

        let encrypted_config =
            encryption::encrypt_sensitive_fields(&self.encryption_key, &default_config, &encrypted_fields)?;

        let entry = PluginConfigEntry {
            plugin_id: plugin_id.clone(),
            plugin_name: plugin_name.clone(),
            schema,
            config: encrypted_config,
            encrypted_fields,
            updated_at: chrono::Utc::now().timestamp(),
        };

        {
            let mut configs = self.configs.write()
                .map_err(|e| TingError::ConfigError(format!("Failed to acquire config lock: {}", e)))?;
            configs.insert(plugin_id.clone(), entry.clone());
        }

        self.save_config(&entry)?;
        tracing::info!(plugin_id = %plugin_id, "Plugin configuration initialized");
        Ok(())
    }

    pub fn get_config(&self, plugin_id: &PluginId) -> Result<Value> {
        let configs = self.configs.read()
            .map_err(|e| TingError::ConfigError(format!("Failed to acquire config lock: {}", e)))?;

        let entry = configs.get(plugin_id).ok_or_else(|| {
            TingError::ConfigError(format!("Configuration not found for plugin: {}", plugin_id))
        })?;

        encryption::decrypt_sensitive_fields(&self.encryption_key, &entry.config, &entry.encrypted_fields)
    }

    pub fn update_config(&self, plugin_id: &PluginId, new_config: Value) -> Result<()> {
        tracing::info!(plugin_id = %plugin_id, "Updating plugin configuration");

        let (old_config, schema, encrypted_fields, plugin_name) = {
            let configs = self.configs.read()
                .map_err(|e| TingError::ConfigError(format!("Failed to acquire config lock: {}", e)))?;
            let entry = configs.get(plugin_id).ok_or_else(|| {
                TingError::ConfigError(format!("Configuration not found for plugin: {}", plugin_id))
            })?;
            (entry.config.clone(), entry.schema.clone(), entry.encrypted_fields.clone(), entry.plugin_name.clone())
        };

        if let Some(ref schema_value) = schema {
            encryption::validate_config(schema_value, &new_config)?;
        }

        let encrypted_config =
            encryption::encrypt_sensitive_fields(&self.encryption_key, &new_config, &encrypted_fields)?;

        {
            let mut configs = self.configs.write()
                .map_err(|e| TingError::ConfigError(format!("Failed to acquire config lock: {}", e)))?;
            if let Some(entry) = configs.get_mut(plugin_id) {
                entry.config = encrypted_config.clone();
                entry.updated_at = chrono::Utc::now().timestamp();
            }
        }

        let entry = {
            let configs = self.configs.read()
                .map_err(|e| TingError::ConfigError(format!("Failed to acquire config lock: {}", e)))?;
            configs.get(plugin_id).cloned().ok_or_else(|| {
                TingError::ConfigError(format!("Configuration not found for plugin: {}", plugin_id))
            })?
        };
        self.save_config(&entry)?;

        self.publish_config_change(plugin_id.clone(), plugin_name, Some(old_config), new_config);
        tracing::info!(plugin_id = %plugin_id, "Plugin configuration updated");
        Ok(())
    }

    pub fn delete_config(&self, plugin_id: &PluginId) -> Result<()> {
        tracing::info!(plugin_id = %plugin_id, "Deleting plugin configuration");

        {
            let mut configs = self.configs.write()
                .map_err(|e| TingError::ConfigError(format!("Failed to acquire config lock: {}", e)))?;
            configs.remove(plugin_id);
        }

        let config_file = self.get_config_file_path(plugin_id);
        if config_file.exists() {
            std::fs::remove_file(&config_file)
                .map_err(|e| TingError::ConfigError(format!("Failed to delete config file: {}", e)))?;
        }

        tracing::info!(plugin_id = %plugin_id, "Plugin configuration deleted");
        Ok(())
    }

    pub fn subscribe_to_changes<F>(&self, callback: F) -> Result<()>
    where
        F: Fn(ConfigChangeEvent) + Send + Sync + 'static,
    {
        let mut subscribers = self.subscribers.write()
            .map_err(|e| TingError::ConfigError(format!("Failed to acquire subscribers lock: {}", e)))?;
        subscribers.push(Box::new(callback));
        Ok(())
    }

    pub fn export_config(&self, plugin_id: &PluginId) -> Result<Value> {
        tracing::info!(plugin_id = %plugin_id, "Exporting plugin configuration");

        let configs = self.configs.read()
            .map_err(|e| TingError::ConfigError(format!("Failed to acquire config lock: {}", e)))?;

        let entry = configs.get(plugin_id).ok_or_else(|| {
            TingError::ConfigError(format!("Configuration not found for plugin: {}", plugin_id))
        })?;

        let decrypted_config =
            encryption::decrypt_sensitive_fields(&self.encryption_key, &entry.config, &entry.encrypted_fields)?;

        let export = serde_json::json!({
            "plugin_id": entry.plugin_id,
            "plugin_name": entry.plugin_name,
            "schema": entry.schema,
            "config": decrypted_config,
            "exported_at": chrono::Utc::now().timestamp(),
        });

        tracing::info!(plugin_id = %plugin_id, "Plugin configuration exported");
        Ok(export)
    }

    pub fn import_config(&self, plugin_id: &PluginId, import_data: Value) -> Result<()> {
        tracing::info!(plugin_id = %plugin_id, "Importing plugin configuration");

        let config = import_data.get("config").ok_or_else(|| {
            TingError::ConfigError("Import data missing 'config' field".to_string())
        })?.clone();

        let (schema, _encrypted_fields, _plugin_name) = {
            let configs = self.configs.read()
                .map_err(|e| TingError::ConfigError(format!("Failed to acquire config lock: {}", e)))?;
            let entry = configs.get(plugin_id).ok_or_else(|| {
                TingError::ConfigError(format!("Configuration not found for plugin: {}", plugin_id))
            })?;
            (entry.schema.clone(), entry.encrypted_fields.clone(), entry.plugin_name.clone())
        };

        if let Some(ref schema_value) = schema {
            encryption::validate_config(schema_value, &config)?;
        }

        self.update_config(plugin_id, config)?;
        tracing::info!(plugin_id = %plugin_id, "Plugin configuration imported");
        Ok(())
    }

    pub fn export_all_configs(&self) -> Result<Value> {
        tracing::info!("Exporting all plugin configurations");

        let configs = self.configs.read()
            .map_err(|e| TingError::ConfigError(format!("Failed to acquire config lock: {}", e)))?;

        let mut exports = serde_json::Map::new();
        for (plugin_id, entry) in configs.iter() {
            let decrypted_config =
                encryption::decrypt_sensitive_fields(&self.encryption_key, &entry.config, &entry.encrypted_fields)?;
            let export = serde_json::json!({
                "plugin_id": entry.plugin_id,
                "plugin_name": entry.plugin_name,
                "schema": entry.schema,
                "config": decrypted_config,
                "exported_at": chrono::Utc::now().timestamp(),
            });
            exports.insert(plugin_id.clone(), export);
        }

        tracing::info!(count = configs.len(), "All plugin configurations exported");
        Ok(Value::Object(exports))
    }

    pub fn import_all_configs(&self, import_data: Value) -> Result<()> {
        tracing::info!("Importing all plugin configurations");

        let imports = import_data.as_object().ok_or_else(|| {
            TingError::ConfigError("Import data must be a JSON object".to_string())
        })?;

        let mut imported_count = 0;
        for (plugin_id, plugin_data) in imports.iter() {
            match self.import_config(plugin_id, plugin_data.clone()) {
                Ok(_) => { imported_count += 1; }
                Err(e) => {
                    tracing::warn!(plugin_id = %plugin_id, error = %e, "Failed to import plugin configuration, skipping");
                }
            }
        }

        tracing::info!(imported = imported_count, total = imports.len(), "Configurations import completed");
        Ok(())
    }

    pub fn backup_config(&self, plugin_id: &PluginId) -> Result<PathBuf> {
        tracing::info!(plugin_id = %plugin_id, "Creating configuration backup");

        let backup_dir = self.config_dir.join("backups");
        std::fs::create_dir_all(&backup_dir)
            .map_err(|e| TingError::ConfigError(format!("Failed to create backup directory: {}", e)))?;

        let entry = {
            let configs = self.configs.read()
                .map_err(|e| TingError::ConfigError(format!("Failed to acquire config lock: {}", e)))?;
            configs.get(plugin_id).ok_or_else(|| {
                TingError::ConfigError(format!("Configuration not found for plugin: {}", plugin_id))
            })?.clone()
        };

        let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
        let safe_plugin_id = plugin_id.replace(['/', '\\', ':', '*', '?', '"', '<', '>', '|'], "_");
        let backup_path = backup_dir.join(format!("{}_{}.json", safe_plugin_id, timestamp));

        let backup_content = serde_json::to_string_pretty(&entry)
            .map_err(|e| TingError::ConfigError(format!("Failed to serialize backup: {}", e)))?;
        std::fs::write(&backup_path, backup_content)
            .map_err(|e| TingError::ConfigError(format!("Failed to write backup file: {}", e)))?;

        tracing::info!(plugin_id = %plugin_id, backup_path = ?backup_path, "Configuration backup created");
        Ok(backup_path)
    }

    pub fn restore_config(&self, backup_path: &Path) -> Result<()> {
        tracing::info!(backup_path = ?backup_path, "Restoring configuration from backup");

        if !backup_path.exists() {
            return Err(TingError::ConfigError(format!("Backup file not found: {}", backup_path.display())));
        }

        let backup_content = std::fs::read_to_string(backup_path)
            .map_err(|e| TingError::ConfigError(format!("Failed to read backup file: {}", e)))?;

        let entry: PluginConfigEntry = serde_json::from_str(&backup_content)
            .map_err(|e| TingError::ConfigError(format!("Failed to parse backup file: {}", e)))?;

        let plugin_id = entry.plugin_id.clone();
        let plugin_name = entry.plugin_name.clone();

        let old_config = {
            let configs = self.configs.read()
                .map_err(|e| TingError::ConfigError(format!("Failed to acquire config lock: {}", e)))?;
            configs.get(&plugin_id).map(|e| e.config.clone())
        };

        {
            let mut configs = self.configs.write()
                .map_err(|e| TingError::ConfigError(format!("Failed to acquire config lock: {}", e)))?;
            configs.insert(plugin_id.clone(), entry.clone());
        }

        self.save_config(&entry)?;

        if let Some(old_cfg) = old_config {
            let old_decrypted =
                encryption::decrypt_sensitive_fields(&self.encryption_key, &old_cfg, &entry.encrypted_fields)?;
            let new_decrypted =
                encryption::decrypt_sensitive_fields(&self.encryption_key, &entry.config, &entry.encrypted_fields)?;
            self.publish_config_change(plugin_id.clone(), plugin_name, Some(old_decrypted), new_decrypted);
        }

        tracing::info!(plugin_id = %plugin_id, "Configuration restored from backup");
        Ok(())
    }

    // ── Private helpers ──

    fn load_all_configs(&self) -> Result<()> {
        if !self.config_dir.exists() {
            return Ok(());
        }

        let entries = std::fs::read_dir(&self.config_dir)
            .map_err(|e| TingError::ConfigError(format!("Failed to read config directory: {}", e)))?;

        for entry in entries {
            let entry = entry
                .map_err(|e| TingError::ConfigError(format!("Failed to read directory entry: {}", e)))?;
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("json") {
                match self.load_config(&path) {
                    Ok(config_entry) => {
                        let mut configs = self.configs.write()
                            .map_err(|e| TingError::ConfigError(format!("Failed to acquire config lock: {}", e)))?;
                        configs.insert(config_entry.plugin_id.clone(), config_entry);
                    }
                    Err(e) => {
                        tracing::warn!(path = ?path, error = %e, "Failed to load config file, skipping");
                    }
                }
            }
        }
        Ok(())
    }

    fn load_config(&self, path: &Path) -> Result<PluginConfigEntry> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| TingError::ConfigError(format!("Failed to read config file: {}", e)))?;
        serde_json::from_str(&content)
            .map_err(|e| TingError::ConfigError(format!("Failed to parse config file: {}", e)))
    }

    fn save_config(&self, entry: &PluginConfigEntry) -> Result<()> {
        let config_file = self.get_config_file_path(&entry.plugin_id);
        let content = serde_json::to_string_pretty(entry)
            .map_err(|e| TingError::ConfigError(format!("Failed to serialize config: {}", e)))?;
        std::fs::write(&config_file, content)
            .map_err(|e| TingError::ConfigError(format!("Failed to write config file: {}", e)))?;
        Ok(())
    }

    fn get_config_file_path(&self, plugin_id: &PluginId) -> PathBuf {
        let filename = plugin_id.replace(['/', '\\', ':', '*', '?', '"', '<', '>', '|'], "_");
        self.config_dir.join(format!("{}.json", filename))
    }

    fn publish_config_change(
        &self,
        plugin_id: PluginId,
        plugin_name: String,
        old_config: Option<Value>,
        new_config: Value,
    ) {
        let event = ConfigChangeEvent {
            plugin_id: plugin_id.clone(),
            plugin_name,
            old_config,
            new_config,
            timestamp: chrono::Utc::now().timestamp(),
        };

        tracing::debug!(plugin_id = %plugin_id, "Publishing configuration change event");

        if let Ok(subscribers) = self.subscribers.read() {
            for subscriber in subscribers.iter() {
                subscriber(event.clone());
            }
        }
    }
}
