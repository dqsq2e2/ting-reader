//! Plugin configuration management
//!
//! This module provides configuration storage, validation, and hot reload functionality
//! for plugins. Each plugin has an isolated configuration namespace.

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
    /// Plugin ID whose configuration changed
    pub plugin_id: PluginId,
    /// Plugin name
    pub plugin_name: String,
    /// Old configuration value (if any)
    pub old_config: Option<Value>,
    /// New configuration value
    pub new_config: Value,
    /// Timestamp of the change
    pub timestamp: i64,
}

/// Plugin configuration entry
#[derive(Debug, Clone, Serialize, Deserialize)]
struct PluginConfigEntry {
    /// Plugin ID (namespace)
    plugin_id: PluginId,
    /// Plugin name
    plugin_name: String,
    /// Configuration schema (JSON Schema)
    schema: Option<Value>,
    /// Configuration values
    config: Value,
    /// Encrypted fields (field paths that should be encrypted)
    #[serde(default)]
    encrypted_fields: Vec<String>,
    /// Last updated timestamp
    updated_at: i64,
}

/// Plugin configuration manager
///
/// Manages plugin configurations with isolated namespaces, schema validation,
/// hot reload notifications, and encryption for sensitive values.
///
/// **Validates: Requirements 11.1, 11.2, 11.3, 11.4, 11.5, 11.8**
pub struct PluginConfigManager {
    /// Configuration storage directory
    config_dir: PathBuf,
    
    /// In-memory configuration cache (plugin_id -> config entry)
    configs: Arc<RwLock<HashMap<PluginId, PluginConfigEntry>>>,
    
    /// Configuration change event subscribers
    subscribers: Arc<RwLock<Vec<Box<dyn Fn(ConfigChangeEvent) + Send + Sync>>>>,
    
    /// Encryption key for sensitive configuration values
    encryption_key: Arc<[u8; 32]>,
}

impl PluginConfigManager {
    /// Create a new plugin configuration manager
    ///
    /// # Arguments
    /// * `config_dir` - Directory to store configuration files
    /// * `encryption_key` - 32-byte key for encrypting sensitive values
    ///
    /// # Returns
    /// A new PluginConfigManager instance
    pub fn new(config_dir: PathBuf, encryption_key: [u8; 32]) -> Result<Self> {
        // Create config directory if it doesn't exist
        std::fs::create_dir_all(&config_dir).map_err(|e| {
            TingError::ConfigError(format!("Failed to create config directory: {}", e))
        })?;
        
        let manager = Self {
            config_dir,
            configs: Arc::new(RwLock::new(HashMap::new())),
            subscribers: Arc::new(RwLock::new(Vec::new())),
            encryption_key: Arc::new(encryption_key),
        };
        
        // Load existing configurations
        manager.load_all_configs()?;
        
        Ok(manager)
    }
    
    /// Initialize configuration for a plugin
    ///
    /// Creates an isolated configuration namespace for the plugin.
    ///
    /// **Validates: Requirement 11.1 - Configuration Isolation**
    ///
    /// # Arguments
    /// * `plugin_id` - Plugin ID (namespace)
    /// * `plugin_name` - Plugin name
    /// * `schema` - Configuration schema (JSON Schema format)
    /// * `default_config` - Default configuration values
    ///
    /// # Returns
    /// Ok if successful
    pub fn initialize_config(
        &self,
        plugin_id: PluginId,
        plugin_name: String,
        schema: Option<Value>,
        default_config: Value,
    ) -> Result<()> {
        tracing::info!(
            plugin_id = %plugin_id,
            plugin_name = %plugin_name,
            "Initializing plugin configuration"
        );
        
        // Validate default config against schema if provided
        if let Some(ref schema_value) = schema {
            self.validate_config(schema_value, &default_config)?;
        }
        
        // Identify encrypted fields from schema
        let encrypted_fields = if let Some(ref schema_value) = schema {
            self.extract_encrypted_fields(schema_value)
        } else {
            Vec::new()
        };
        
        // Encrypt sensitive fields in default config
        let encrypted_config = self.encrypt_sensitive_fields(&default_config, &encrypted_fields)?;
        
        let entry = PluginConfigEntry {
            plugin_id: plugin_id.clone(),
            plugin_name: plugin_name.clone(),
            schema,
            config: encrypted_config,
            encrypted_fields,
            updated_at: chrono::Utc::now().timestamp(),
        };
        
        // Store in memory
        {
            let mut configs = self.configs.write().map_err(|e| {
                TingError::ConfigError(format!("Failed to acquire config lock: {}", e))
            })?;
            
            configs.insert(plugin_id.clone(), entry.clone());
        }
        
        // Persist to disk
        self.save_config(&entry)?;
        
        tracing::info!(
            plugin_id = %plugin_id,
            "Plugin configuration initialized"
        );
        
        Ok(())
    }
    
    /// Get configuration for a plugin
    ///
    /// Returns the decrypted configuration values.
    ///
    /// **Validates: Requirement 11.3 - Configuration Read API**
    ///
    /// # Arguments
    /// * `plugin_id` - Plugin ID
    ///
    /// # Returns
    /// Configuration value with sensitive fields decrypted
    pub fn get_config(&self, plugin_id: &PluginId) -> Result<Value> {
        let configs = self.configs.read().map_err(|e| {
            TingError::ConfigError(format!("Failed to acquire config lock: {}", e))
        })?;
        
        let entry = configs.get(plugin_id).ok_or_else(|| {
            TingError::ConfigError(format!("Configuration not found for plugin: {}", plugin_id))
        })?;
        
        // Decrypt sensitive fields before returning
        self.decrypt_sensitive_fields(&entry.config, &entry.encrypted_fields)
    }
    
    /// Update configuration for a plugin
    ///
    /// Validates the new configuration against the schema and notifies the plugin
    /// of the change for hot reload.
    ///
    /// **Validates: Requirements 11.3, 11.4, 11.5 - Configuration Update, Hot Reload, Validation**
    ///
    /// # Arguments
    /// * `plugin_id` - Plugin ID
    /// * `new_config` - New configuration values
    ///
    /// # Returns
    /// Ok if successful
    pub fn update_config(&self, plugin_id: &PluginId, new_config: Value) -> Result<()> {
        tracing::info!(
            plugin_id = %plugin_id,
            "Updating plugin configuration"
        );
        
        // Get current entry
        let (old_config, schema, encrypted_fields, plugin_name) = {
            let configs = self.configs.read().map_err(|e| {
                TingError::ConfigError(format!("Failed to acquire config lock: {}", e))
            })?;
            
            let entry = configs.get(plugin_id).ok_or_else(|| {
                TingError::ConfigError(format!("Configuration not found for plugin: {}", plugin_id))
            })?;
            
            (
                entry.config.clone(),
                entry.schema.clone(),
                entry.encrypted_fields.clone(),
                entry.plugin_name.clone(),
            )
        };
        
        // Validate new config against schema (Requirement 11.5)
        if let Some(ref schema_value) = schema {
            self.validate_config(schema_value, &new_config)?;
        }
        
        // Encrypt sensitive fields
        let encrypted_config = self.encrypt_sensitive_fields(&new_config, &encrypted_fields)?;
        
        // Update in memory
        {
            let mut configs = self.configs.write().map_err(|e| {
                TingError::ConfigError(format!("Failed to acquire config lock: {}", e))
            })?;
            
            if let Some(entry) = configs.get_mut(plugin_id) {
                entry.config = encrypted_config.clone();
                entry.updated_at = chrono::Utc::now().timestamp();
            }
        }
        
        // Persist to disk
        let entry = {
            let configs = self.configs.read().map_err(|e| {
                TingError::ConfigError(format!("Failed to acquire config lock: {}", e))
            })?;
            
            configs.get(plugin_id).cloned().ok_or_else(|| {
                TingError::ConfigError(format!("Configuration not found for plugin: {}", plugin_id))
            })?
        };
        
        self.save_config(&entry)?;
        
        // Notify subscribers for hot reload (Requirement 11.4)
        self.publish_config_change(
            plugin_id.clone(),
            plugin_name,
            Some(old_config),
            new_config,
        );
        
        tracing::info!(
            plugin_id = %plugin_id,
            "Plugin configuration updated"
        );
        
        Ok(())
    }
    
    /// Delete configuration for a plugin
    ///
    /// # Arguments
    /// * `plugin_id` - Plugin ID
    ///
    /// # Returns
    /// Ok if successful
    pub fn delete_config(&self, plugin_id: &PluginId) -> Result<()> {
        tracing::info!(
            plugin_id = %plugin_id,
            "Deleting plugin configuration"
        );
        
        // Remove from memory
        {
            let mut configs = self.configs.write().map_err(|e| {
                TingError::ConfigError(format!("Failed to acquire config lock: {}", e))
            })?;
            
            configs.remove(plugin_id);
        }
        
        // Delete from disk
        let config_file = self.get_config_file_path(plugin_id);
        if config_file.exists() {
            std::fs::remove_file(&config_file).map_err(|e| {
                TingError::ConfigError(format!("Failed to delete config file: {}", e))
            })?;
        }
        
        tracing::info!(
            plugin_id = %plugin_id,
            "Plugin configuration deleted"
        );
        
        Ok(())
    }
    
    /// Subscribe to configuration change events
    ///
    /// The callback will be invoked whenever a plugin configuration is updated,
    /// allowing plugins to hot reload their configuration.
    ///
    /// **Validates: Requirement 11.4 - Configuration Hot Reload Notification**
    ///
    /// # Arguments
    /// * `callback` - Function to call when configuration changes
    ///
    /// # Returns
    /// Ok if successful
    pub fn subscribe_to_changes<F>(&self, callback: F) -> Result<()>
    where
        F: Fn(ConfigChangeEvent) + Send + Sync + 'static,
    {
        let mut subscribers = self.subscribers.write().map_err(|e| {
            TingError::ConfigError(format!("Failed to acquire subscribers lock: {}", e))
        })?;
        
        subscribers.push(Box::new(callback));
        Ok(())
    }
    
    /// Export configuration for a plugin to JSON
    ///
    /// Returns the plugin's configuration as a JSON value with decrypted sensitive fields.
    ///
    /// **Validates: Requirement 11.7 - Configuration Import/Export**
    ///
    /// # Arguments
    /// * `plugin_id` - Plugin ID
    ///
    /// # Returns
    /// JSON value containing plugin metadata and configuration
    pub fn export_config(&self, plugin_id: &PluginId) -> Result<Value> {
        tracing::info!(
            plugin_id = %plugin_id,
            "Exporting plugin configuration"
        );
        
        let configs = self.configs.read().map_err(|e| {
            TingError::ConfigError(format!("Failed to acquire config lock: {}", e))
        })?;
        
        let entry = configs.get(plugin_id).ok_or_else(|| {
            TingError::ConfigError(format!("Configuration not found for plugin: {}", plugin_id))
        })?;
        
        // Decrypt sensitive fields before exporting
        let decrypted_config = self.decrypt_sensitive_fields(&entry.config, &entry.encrypted_fields)?;
        
        // Create export structure with metadata
        let export = serde_json::json!({
            "plugin_id": entry.plugin_id,
            "plugin_name": entry.plugin_name,
            "schema": entry.schema,
            "config": decrypted_config,
            "exported_at": chrono::Utc::now().timestamp(),
        });
        
        tracing::info!(
            plugin_id = %plugin_id,
            "Plugin configuration exported successfully"
        );
        
        Ok(export)
    }
    
    /// Import configuration for a plugin from JSON
    ///
    /// Validates the configuration against the schema and encrypts sensitive fields.
    /// Triggers hot reload notification after import.
    ///
    /// **Validates: Requirement 11.7 - Configuration Import/Export**
    ///
    /// # Arguments
    /// * `plugin_id` - Plugin ID
    /// * `import_data` - JSON value containing configuration to import
    ///
    /// # Returns
    /// Ok if successful
    pub fn import_config(&self, plugin_id: &PluginId, import_data: Value) -> Result<()> {
        tracing::info!(
            plugin_id = %plugin_id,
            "Importing plugin configuration"
        );
        
        // Extract configuration from import data
        let config = import_data.get("config").ok_or_else(|| {
            TingError::ConfigError("Import data missing 'config' field".to_string())
        })?.clone();
        
        // Get current entry for schema and encrypted fields
        let (schema, _encrypted_fields, _plugin_name) = {
            let configs = self.configs.read().map_err(|e| {
                TingError::ConfigError(format!("Failed to acquire config lock: {}", e))
            })?;
            
            let entry = configs.get(plugin_id).ok_or_else(|| {
                TingError::ConfigError(format!("Configuration not found for plugin: {}", plugin_id))
            })?;
            
            (
                entry.schema.clone(),
                entry.encrypted_fields.clone(),
                entry.plugin_name.clone(),
            )
        };
        
        // Validate against schema if available
        if let Some(ref schema_value) = schema {
            self.validate_config(schema_value, &config)?;
        }
        
        // Use update_config to handle encryption and hot reload
        self.update_config(plugin_id, config)?;
        
        tracing::info!(
            plugin_id = %plugin_id,
            "Plugin configuration imported successfully"
        );
        
        Ok(())
    }
    
    /// Export all plugin configurations to JSON
    ///
    /// Returns a map of plugin IDs to their exported configurations.
    ///
    /// **Validates: Requirement 33.7 - Configuration Export/Import**
    ///
    /// # Returns
    /// JSON object mapping plugin IDs to their configurations
    pub fn export_all_configs(&self) -> Result<Value> {
        tracing::info!("Exporting all plugin configurations");
        
        let configs = self.configs.read().map_err(|e| {
            TingError::ConfigError(format!("Failed to acquire config lock: {}", e))
        })?;
        
        let mut exports = serde_json::Map::new();
        
        for (plugin_id, entry) in configs.iter() {
            // Decrypt sensitive fields before exporting
            let decrypted_config = self.decrypt_sensitive_fields(&entry.config, &entry.encrypted_fields)?;
            
            let export = serde_json::json!({
                "plugin_id": entry.plugin_id,
                "plugin_name": entry.plugin_name,
                "schema": entry.schema,
                "config": decrypted_config,
                "exported_at": chrono::Utc::now().timestamp(),
            });
            
            exports.insert(plugin_id.clone(), export);
        }
        
        let result = Value::Object(exports);
        
        tracing::info!(
            count = configs.len(),
            "All plugin configurations exported successfully"
        );
        
        Ok(result)
    }
    
    /// Import all plugin configurations from JSON
    ///
    /// Imports configurations for multiple plugins from a JSON object.
    /// Each plugin's configuration is validated and encrypted as needed.
    ///
    /// **Validates: Requirement 33.7 - Configuration Export/Import**
    ///
    /// # Arguments
    /// * `import_data` - JSON object mapping plugin IDs to their configurations
    ///
    /// # Returns
    /// Ok if successful
    pub fn import_all_configs(&self, import_data: Value) -> Result<()> {
        tracing::info!("Importing all plugin configurations");
        
        let imports = import_data.as_object().ok_or_else(|| {
            TingError::ConfigError("Import data must be a JSON object".to_string())
        })?;
        
        let mut imported_count = 0;
        
        for (plugin_id, plugin_data) in imports.iter() {
            // Import each plugin's configuration
            match self.import_config(plugin_id, plugin_data.clone()) {
                Ok(_) => {
                    imported_count += 1;
                    tracing::debug!(
                        plugin_id = %plugin_id,
                        "Plugin configuration imported"
                    );
                }
                Err(e) => {
                    tracing::warn!(
                        plugin_id = %plugin_id,
                        error = %e,
                        "Failed to import plugin configuration, skipping"
                    );
                }
            }
        }
        
        tracing::info!(
            imported = imported_count,
            total = imports.len(),
            "Plugin configurations import completed"
        );
        
        Ok(())
    }
    
    /// Backup configuration for a plugin
    ///
    /// Creates a timestamped backup file in the backup directory.
    /// The backup preserves encryption for sensitive fields.
    ///
    /// **Validates: Requirement 33.7 - Configuration Backup**
    ///
    /// # Arguments
    /// * `plugin_id` - Plugin ID
    ///
    /// # Returns
    /// Path to the created backup file
    pub fn backup_config(&self, plugin_id: &PluginId) -> Result<PathBuf> {
        tracing::info!(
            plugin_id = %plugin_id,
            "Creating configuration backup"
        );
        
        // Create backup directory if it doesn't exist
        let backup_dir = self.config_dir.join("backups");
        std::fs::create_dir_all(&backup_dir).map_err(|e| {
            TingError::ConfigError(format!("Failed to create backup directory: {}", e))
        })?;
        
        // Get current configuration entry
        let entry = {
            let configs = self.configs.read().map_err(|e| {
                TingError::ConfigError(format!("Failed to acquire config lock: {}", e))
            })?;
            
            configs.get(plugin_id).ok_or_else(|| {
                TingError::ConfigError(format!("Configuration not found for plugin: {}", plugin_id))
            })?.clone()
        };
        
        // Create backup filename with timestamp
        let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
        let safe_plugin_id = plugin_id.replace(['/', '\\', ':', '*', '?', '"', '<', '>', '|'], "_");
        let backup_filename = format!("{}_{}.json", safe_plugin_id, timestamp);
        let backup_path = backup_dir.join(backup_filename);
        
        // Serialize entry (with encrypted fields preserved)
        let backup_content = serde_json::to_string_pretty(&entry).map_err(|e| {
            TingError::ConfigError(format!("Failed to serialize backup: {}", e))
        })?;
        
        // Write backup file
        std::fs::write(&backup_path, backup_content).map_err(|e| {
            TingError::ConfigError(format!("Failed to write backup file: {}", e))
        })?;
        
        tracing::info!(
            plugin_id = %plugin_id,
            backup_path = ?backup_path,
            "Configuration backup created successfully"
        );
        
        Ok(backup_path)
    }
    
    /// Restore configuration from a backup file
    ///
    /// Validates the backup file and restores the configuration.
    /// Triggers hot reload notification after restore.
    ///
    /// **Validates: Requirement 33.7 - Configuration Restore**
    ///
    /// # Arguments
    /// * `backup_path` - Path to the backup file
    ///
    /// # Returns
    /// Ok if successful
    pub fn restore_config(&self, backup_path: &Path) -> Result<()> {
        tracing::info!(
            backup_path = ?backup_path,
            "Restoring configuration from backup"
        );
        
        // Validate backup file exists
        if !backup_path.exists() {
            return Err(TingError::ConfigError(format!(
                "Backup file not found: {}",
                backup_path.display()
            )));
        }
        
        // Load backup file
        let backup_content = std::fs::read_to_string(backup_path).map_err(|e| {
            TingError::ConfigError(format!("Failed to read backup file: {}", e))
        })?;
        
        // Parse backup entry
        let entry: PluginConfigEntry = serde_json::from_str(&backup_content).map_err(|e| {
            TingError::ConfigError(format!("Failed to parse backup file: {}", e))
        })?;
        
        let plugin_id = entry.plugin_id.clone();
        let plugin_name = entry.plugin_name.clone();
        
        // Get old config for hot reload notification
        let old_config = {
            let configs = self.configs.read().map_err(|e| {
                TingError::ConfigError(format!("Failed to acquire config lock: {}", e))
            })?;
            
            configs.get(&plugin_id).map(|e| e.config.clone())
        };
        
        // Update in memory
        {
            let mut configs = self.configs.write().map_err(|e| {
                TingError::ConfigError(format!("Failed to acquire config lock: {}", e))
            })?;
            
            configs.insert(plugin_id.clone(), entry.clone());
        }
        
        // Persist to disk
        self.save_config(&entry)?;
        
        // Notify subscribers for hot reload
        if let Some(old_cfg) = old_config {
            // Decrypt both configs for notification
            let old_decrypted = self.decrypt_sensitive_fields(&old_cfg, &entry.encrypted_fields)?;
            let new_decrypted = self.decrypt_sensitive_fields(&entry.config, &entry.encrypted_fields)?;
            
            self.publish_config_change(
                plugin_id.clone(),
                plugin_name,
                Some(old_decrypted),
                new_decrypted,
            );
        }
        
        tracing::info!(
            plugin_id = %plugin_id,
            "Configuration restored successfully from backup"
        );
        
        Ok(())
    }
    
    // Private helper methods
    
    /// Load all configurations from disk
    fn load_all_configs(&self) -> Result<()> {
        if !self.config_dir.exists() {
            return Ok(());
        }
        
        let entries = std::fs::read_dir(&self.config_dir).map_err(|e| {
            TingError::ConfigError(format!("Failed to read config directory: {}", e))
        })?;
        
        for entry in entries {
            let entry = entry.map_err(|e| {
                TingError::ConfigError(format!("Failed to read directory entry: {}", e))
            })?;
            
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("json") {
                match self.load_config(&path) {
                    Ok(config_entry) => {
                        let mut configs = self.configs.write().map_err(|e| {
                            TingError::ConfigError(format!("Failed to acquire config lock: {}", e))
                        })?;
                        
                        configs.insert(config_entry.plugin_id.clone(), config_entry);
                    }
                    Err(e) => {
                        tracing::warn!(
                            path = ?path,
                            error = %e,
                            "Failed to load config file, skipping"
                        );
                    }
                }
            }
        }
        
        Ok(())
    }
    
    /// Load configuration from a file
    fn load_config(&self, path: &Path) -> Result<PluginConfigEntry> {
        let content = std::fs::read_to_string(path).map_err(|e| {
            TingError::ConfigError(format!("Failed to read config file: {}", e))
        })?;
        
        let entry: PluginConfigEntry = serde_json::from_str(&content).map_err(|e| {
            TingError::ConfigError(format!("Failed to parse config file: {}", e))
        })?;
        
        Ok(entry)
    }
    
    /// Save configuration to disk
    fn save_config(&self, entry: &PluginConfigEntry) -> Result<()> {
        let config_file = self.get_config_file_path(&entry.plugin_id);
        
        let content = serde_json::to_string_pretty(entry).map_err(|e| {
            TingError::ConfigError(format!("Failed to serialize config: {}", e))
        })?;
        
        std::fs::write(&config_file, content).map_err(|e| {
            TingError::ConfigError(format!("Failed to write config file: {}", e))
        })?;
        
        Ok(())
    }
    
    /// Get the file path for a plugin's configuration
    fn get_config_file_path(&self, plugin_id: &PluginId) -> PathBuf {
        // Use plugin ID as filename (sanitized)
        let filename = plugin_id.replace(['/', '\\', ':', '*', '?', '"', '<', '>', '|'], "_");
        self.config_dir.join(format!("{}.json", filename))
    }
    
    /// Validate configuration against JSON Schema
    ///
    /// **Validates: Requirement 11.5 - Configuration Validation**
    fn validate_config(&self, schema: &Value, config: &Value) -> Result<()> {
        // Use jsonschema crate for validation
        let compiled_schema = jsonschema::JSONSchema::compile(schema).map_err(|e| {
            TingError::ConfigError(format!("Invalid configuration schema: {}", e))
        })?;
        
        if let Err(errors) = compiled_schema.validate(config) {
            let error_messages: Vec<String> = errors
                .map(|e| format!("{}", e))
                .collect();
            
            return Err(TingError::ConfigError(format!(
                "Configuration validation failed: {}",
                error_messages.join(", ")
            )));
        }
        
        Ok(())
    }
    
    /// Extract encrypted field paths from schema
    ///
    /// Looks for fields marked with "x-encrypted": true in the schema.
    fn extract_encrypted_fields(&self, schema: &Value) -> Vec<String> {
        let mut encrypted_fields = Vec::new();
        
        if let Some(properties) = schema.get("properties").and_then(|p| p.as_object()) {
            for (field_name, field_schema) in properties {
                if let Some(true) = field_schema.get("x-encrypted").and_then(|v| v.as_bool()) {
                    encrypted_fields.push(field_name.clone());
                }
            }
        }
        
        encrypted_fields
    }
    
    /// Encrypt sensitive fields in configuration
    ///
    /// **Validates: Requirement 11.8 - Sensitive Configuration Encryption**
    fn encrypt_sensitive_fields(&self, config: &Value, encrypted_fields: &[String]) -> Result<Value> {
        if encrypted_fields.is_empty() {
            return Ok(config.clone());
        }
        
        let mut encrypted_config = config.clone();
        
        if let Some(obj) = encrypted_config.as_object_mut() {
            for field_name in encrypted_fields {
                if let Some(field_value) = obj.get(field_name) {
                    // Convert value to string for encryption
                    let value_str = if field_value.is_string() {
                        field_value.as_str().unwrap().to_string()
                    } else {
                        field_value.to_string()
                    };
                    
                    // Encrypt the value
                    let encrypted = self.encrypt_value(&value_str)?;
                    
                    // Store as base64-encoded string with prefix
                    obj.insert(
                        field_name.clone(),
                        Value::String(format!("encrypted:{}", encrypted)),
                    );
                }
            }
        }
        
        Ok(encrypted_config)
    }
    
    /// Decrypt sensitive fields in configuration
    fn decrypt_sensitive_fields(&self, config: &Value, encrypted_fields: &[String]) -> Result<Value> {
        if encrypted_fields.is_empty() {
            return Ok(config.clone());
        }
        
        let mut decrypted_config = config.clone();
        
        if let Some(obj) = decrypted_config.as_object_mut() {
            for field_name in encrypted_fields {
                if let Some(field_value) = obj.get(field_name) {
                    if let Some(encrypted_str) = field_value.as_str() {
                        // Check if it's encrypted
                        if let Some(encrypted_data) = encrypted_str.strip_prefix("encrypted:") {
                            // Decrypt the value
                            let decrypted = self.decrypt_value(encrypted_data)?;
                            
                            // Always return as string since we encrypted it as a string
                            obj.insert(field_name.clone(), Value::String(decrypted));
                        }
                    }
                }
            }
        }
        
        Ok(decrypted_config)
    }
    
    /// Encrypt a value using AES-256-GCM
    fn encrypt_value(&self, value: &str) -> Result<String> {
        use aes_gcm::{
            aead::{Aead, KeyInit, OsRng},
            Aes256Gcm, Nonce,
        };
        use base64::{Engine as _, engine::general_purpose};
        
        let cipher = Aes256Gcm::new(self.encryption_key.as_ref().into());
        
        // Generate random nonce
        let mut nonce_bytes = [0u8; 12];
        use aes_gcm::aead::rand_core::RngCore;
        OsRng.fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);
        
        // Encrypt
        let ciphertext = cipher.encrypt(nonce, value.as_bytes())
            .map_err(|e| TingError::ConfigError(format!("Encryption failed: {}", e)))?;
        
        // Combine nonce + ciphertext and encode as base64
        let mut combined = nonce_bytes.to_vec();
        combined.extend_from_slice(&ciphertext);
        
        Ok(general_purpose::STANDARD.encode(&combined))
    }
    
    /// Decrypt a value using AES-256-GCM
    fn decrypt_value(&self, encrypted: &str) -> Result<String> {
        use aes_gcm::{
            aead::{Aead, KeyInit},
            Aes256Gcm, Nonce,
        };
        use base64::{Engine as _, engine::general_purpose};
        
        let cipher = Aes256Gcm::new(self.encryption_key.as_ref().into());
        
        // Decode from base64
        let combined = general_purpose::STANDARD.decode(encrypted)
            .map_err(|e| TingError::ConfigError(format!("Invalid encrypted data: {}", e)))?;
        
        if combined.len() < 12 {
            return Err(TingError::ConfigError("Invalid encrypted data length".to_string()));
        }
        
        // Split nonce and ciphertext
        let (nonce_bytes, ciphertext) = combined.split_at(12);
        let nonce = Nonce::from_slice(nonce_bytes);
        
        // Decrypt
        let plaintext = cipher.decrypt(nonce, ciphertext)
            .map_err(|e| TingError::ConfigError(format!("Decryption failed: {}", e)))?;
        
        String::from_utf8(plaintext)
            .map_err(|e| TingError::ConfigError(format!("Invalid UTF-8 in decrypted data: {}", e)))
    }
    
    /// Publish configuration change event
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
        
        tracing::debug!(
            plugin_id = %plugin_id,
            "Publishing configuration change event"
        );
        
        // Notify all subscribers
        if let Ok(subscribers) = self.subscribers.read() {
            for subscriber in subscribers.iter() {
                subscriber(event.clone());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    
    fn test_encryption_key() -> [u8; 32] {
        [0u8; 32] // Simple key for testing
    }
    
    fn test_config_manager() -> (PluginConfigManager, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let manager = PluginConfigManager::new(
            temp_dir.path().to_path_buf(),
            test_encryption_key(),
        ).unwrap();
        (manager, temp_dir)
    }
    
    #[test]
    fn test_initialize_config() {
        let (manager, _temp_dir) = test_config_manager();
        
        let plugin_id = "test-plugin@1.0.0".to_string();
        let plugin_name = "Test Plugin".to_string();
        let config = serde_json::json!({
            "setting1": "value1",
            "setting2": 42
        });
        
        let result = manager.initialize_config(
            plugin_id.clone(),
            plugin_name,
            None,
            config.clone(),
        );
        
        assert!(result.is_ok());
        
        // Verify config can be retrieved
        let retrieved = manager.get_config(&plugin_id).unwrap();
        assert_eq!(retrieved, config);
    }
    
    #[test]
    fn test_config_isolation() {
        let (manager, _temp_dir) = test_config_manager();
        
        // Initialize two plugins with different configs
        let plugin1_id = "plugin1@1.0.0".to_string();
        let plugin1_config = serde_json::json!({"key": "value1"});
        
        let plugin2_id = "plugin2@1.0.0".to_string();
        let plugin2_config = serde_json::json!({"key": "value2"});
        
        manager.initialize_config(
            plugin1_id.clone(),
            "Plugin 1".to_string(),
            None,
            plugin1_config.clone(),
        ).unwrap();
        
        manager.initialize_config(
            plugin2_id.clone(),
            "Plugin 2".to_string(),
            None,
            plugin2_config.clone(),
        ).unwrap();
        
        // Verify configs are isolated
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
            "properties": {
                "port": {
                    "type": "integer",
                    "minimum": 1,
                    "maximum": 65535
                },
                "host": {
                    "type": "string"
                }
            },
            "required": ["port", "host"]
        });
        
        let plugin_id = "test-plugin@1.0.0".to_string();
        
        // Valid config
        let valid_config = serde_json::json!({
            "port": 8080,
            "host": "localhost"
        });
        
        let result = manager.initialize_config(
            plugin_id.clone(),
            "Test Plugin".to_string(),
            Some(schema.clone()),
            valid_config,
        );
        assert!(result.is_ok());
        
        // Invalid config (missing required field)
        let invalid_config = serde_json::json!({
            "port": 8080
        });
        
        let result = manager.update_config(&plugin_id, invalid_config);
        assert!(result.is_err());
        
        // Invalid config (wrong type)
        let invalid_config = serde_json::json!({
            "port": "not a number",
            "host": "localhost"
        });
        
        let result = manager.update_config(&plugin_id, invalid_config);
        assert!(result.is_err());
    }
    
    #[test]
    fn test_sensitive_field_encryption() {
        let (manager, _temp_dir) = test_config_manager();
        
        let schema = serde_json::json!({
            "type": "object",
            "properties": {
                "api_key": {
                    "type": "string",
                    "x-encrypted": true
                },
                "public_setting": {
                    "type": "string"
                }
            }
        });
        
        let plugin_id = "test-plugin@1.0.0".to_string();
        let config = serde_json::json!({
            "api_key": "secret-key-12345",
            "public_setting": "public-value"
        });
        
        manager.initialize_config(
            plugin_id.clone(),
            "Test Plugin".to_string(),
            Some(schema),
            config.clone(),
        ).unwrap();
        
        // Retrieve config - should be decrypted
        let retrieved = manager.get_config(&plugin_id).unwrap();
        assert_eq!(retrieved, config);
        
        // Check that the stored config has encrypted field
        let configs = manager.configs.read().unwrap();
        let entry = configs.get(&plugin_id).unwrap();
        let stored_api_key = entry.config.get("api_key").unwrap().as_str().unwrap();
        
        // Should be encrypted (prefixed with "encrypted:")
        assert!(stored_api_key.starts_with("encrypted:"));
        assert_ne!(stored_api_key, "secret-key-12345");
        
        // Public setting should not be encrypted
        let stored_public = entry.config.get("public_setting").unwrap().as_str().unwrap();
        assert_eq!(stored_public, "public-value");
    }
    
    #[test]
    fn test_config_hot_reload_notification() {
        let (manager, _temp_dir) = test_config_manager();
        
        let plugin_id = "test-plugin@1.0.0".to_string();
        let initial_config = serde_json::json!({"setting": "initial"});
        
        manager.initialize_config(
            plugin_id.clone(),
            "Test Plugin".to_string(),
            None,
            initial_config.clone(),
        ).unwrap();
        
        // Subscribe to changes
        let notified = Arc::new(RwLock::new(false));
        let notified_clone = Arc::clone(&notified);
        
        manager.subscribe_to_changes(move |event| {
            assert_eq!(event.plugin_id, "test-plugin@1.0.0");
            assert_eq!(event.old_config.unwrap(), serde_json::json!({"setting": "initial"}));
            assert_eq!(event.new_config, serde_json::json!({"setting": "updated"}));
            *notified_clone.write().unwrap() = true;
        }).unwrap();
        
        // Update config
        let new_config = serde_json::json!({"setting": "updated"});
        manager.update_config(&plugin_id, new_config).unwrap();
        
        // Verify notification was received
        assert!(*notified.read().unwrap());
    }
    
    #[test]
    fn test_config_persistence() {
        let temp_dir = TempDir::new().unwrap();
        let config_dir = temp_dir.path().to_path_buf();
        
        let plugin_id = "test-plugin@1.0.0".to_string();
        let config = serde_json::json!({"key": "value"});
        
        // Create manager and initialize config
        {
            let manager = PluginConfigManager::new(
                config_dir.clone(),
                test_encryption_key(),
            ).unwrap();
            
            manager.initialize_config(
                plugin_id.clone(),
                "Test Plugin".to_string(),
                None,
                config.clone(),
            ).unwrap();
        }
        
        // Create new manager - should load persisted config
        {
            let manager = PluginConfigManager::new(
                config_dir,
                test_encryption_key(),
            ).unwrap();
            
            let retrieved = manager.get_config(&plugin_id).unwrap();
            assert_eq!(retrieved, config);
        }
    }
    
    #[test]
    fn test_export_config() {
        let (manager, _temp_dir) = test_config_manager();
        
        let plugin_id = "test-plugin@1.0.0".to_string();
        let plugin_name = "Test Plugin".to_string();
        let config = serde_json::json!({
            "setting1": "value1",
            "setting2": 42
        });
        
        manager.initialize_config(
            plugin_id.clone(),
            plugin_name.clone(),
            None,
            config.clone(),
        ).unwrap();
        
        // Export configuration
        let exported = manager.export_config(&plugin_id).unwrap();
        
        // Verify export structure
        assert_eq!(exported["plugin_id"], plugin_id);
        assert_eq!(exported["plugin_name"], plugin_name);
        assert_eq!(exported["config"], config);
        assert!(exported["exported_at"].is_number());
    }
    
    #[test]
    fn test_import_config() {
        let (manager, _temp_dir) = test_config_manager();
        
        let plugin_id = "test-plugin@1.0.0".to_string();
        let initial_config = serde_json::json!({"setting": "initial"});
        
        manager.initialize_config(
            plugin_id.clone(),
            "Test Plugin".to_string(),
            None,
            initial_config,
        ).unwrap();
        
        // Create import data
        let new_config = serde_json::json!({"setting": "updated"});
        let import_data = serde_json::json!({
            "plugin_id": plugin_id,
            "plugin_name": "Test Plugin",
            "config": new_config,
        });
        
        // Import configuration
        manager.import_config(&plugin_id, import_data).unwrap();
        
        // Verify config was updated
        let retrieved = manager.get_config(&plugin_id).unwrap();
        assert_eq!(retrieved, new_config);
    }
    
    #[test]
    fn test_export_import_round_trip() {
        let (manager, _temp_dir) = test_config_manager();
        
        let plugin_id = "test-plugin@1.0.0".to_string();
        let original_config = serde_json::json!({
            "string_value": "test",
            "number_value": 123,
            "boolean_value": true,
            "array_value": [1, 2, 3],
            "object_value": {"nested": "value"}
        });
        
        manager.initialize_config(
            plugin_id.clone(),
            "Test Plugin".to_string(),
            None,
            original_config.clone(),
        ).unwrap();
        
        // Export and import
        let exported = manager.export_config(&plugin_id).unwrap();
        manager.import_config(&plugin_id, exported).unwrap();
        
        // Verify config is unchanged
        let retrieved = manager.get_config(&plugin_id).unwrap();
        assert_eq!(retrieved, original_config);
    }
    
    #[test]
    fn test_export_all_configs() {
        let (manager, _temp_dir) = test_config_manager();
        
        // Initialize multiple plugins
        let plugin1_id = "plugin1@1.0.0".to_string();
        let plugin1_config = serde_json::json!({"key": "value1"});
        
        let plugin2_id = "plugin2@1.0.0".to_string();
        let plugin2_config = serde_json::json!({"key": "value2"});
        
        manager.initialize_config(
            plugin1_id.clone(),
            "Plugin 1".to_string(),
            None,
            plugin1_config.clone(),
        ).unwrap();
        
        manager.initialize_config(
            plugin2_id.clone(),
            "Plugin 2".to_string(),
            None,
            plugin2_config.clone(),
        ).unwrap();
        
        // Export all configurations
        let exported = manager.export_all_configs().unwrap();
        
        // Verify both plugins are exported
        assert!(exported.is_object());
        let exports = exported.as_object().unwrap();
        assert_eq!(exports.len(), 2);
        assert!(exports.contains_key(&plugin1_id));
        assert!(exports.contains_key(&plugin2_id));
        
        // Verify config values
        assert_eq!(exports[&plugin1_id]["config"], plugin1_config);
        assert_eq!(exports[&plugin2_id]["config"], plugin2_config);
    }
    
    #[test]
    fn test_backup_config() {
        let (manager, _temp_dir) = test_config_manager();
        
        let plugin_id = "test-plugin@1.0.0".to_string();
        let config = serde_json::json!({"key": "value"});
        
        manager.initialize_config(
            plugin_id.clone(),
            "Test Plugin".to_string(),
            None,
            config.clone(),
        ).unwrap();
        
        // Create backup
        let backup_path = manager.backup_config(&plugin_id).unwrap();
        
        // Verify backup file exists
        assert!(backup_path.exists());
        
        // Verify backup path contains sanitized plugin ID (@ replaced with _)
        let backup_filename = backup_path.file_name().unwrap().to_str().unwrap();
        assert!(backup_filename.contains("test-plugin"));
        assert!(backup_filename.ends_with(".json"));
        
        // Verify backup content
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
        
        manager.initialize_config(
            plugin_id.clone(),
            "Test Plugin".to_string(),
            None,
            original_config.clone(),
        ).unwrap();
        
        // Create backup
        let backup_path = manager.backup_config(&plugin_id).unwrap();
        
        // Modify config
        let modified_config = serde_json::json!({"key": "modified"});
        manager.update_config(&plugin_id, modified_config).unwrap();
        
        // Verify config was modified
        let current = manager.get_config(&plugin_id).unwrap();
        assert_eq!(current["key"], "modified");
        
        // Restore from backup
        manager.restore_config(&backup_path).unwrap();
        
        // Verify config was restored
        let restored = manager.get_config(&plugin_id).unwrap();
        assert_eq!(restored, original_config);
    }
    
    #[test]
    fn test_backup_restore_with_encryption() {
        let (manager, _temp_dir) = test_config_manager();
        
        let schema = serde_json::json!({
            "type": "object",
            "properties": {
                "api_key": {
                    "type": "string",
                    "x-encrypted": true
                },
                "public_setting": {
                    "type": "string"
                }
            }
        });
        
        let plugin_id = "test-plugin@1.0.0".to_string();
        let config = serde_json::json!({
            "api_key": "secret-key-12345",
            "public_setting": "public-value"
        });
        
        manager.initialize_config(
            plugin_id.clone(),
            "Test Plugin".to_string(),
            Some(schema),
            config.clone(),
        ).unwrap();
        
        // Create backup
        let backup_path = manager.backup_config(&plugin_id).unwrap();
        
        // Modify config
        let modified_config = serde_json::json!({
            "api_key": "different-key",
            "public_setting": "different-value"
        });
        manager.update_config(&plugin_id, modified_config).unwrap();
        
        // Restore from backup
        manager.restore_config(&backup_path).unwrap();
        
        // Verify config was restored with decrypted values
        let restored = manager.get_config(&plugin_id).unwrap();
        assert_eq!(restored, config);
    }
    
    #[test]
    fn test_import_with_validation() {
        let (manager, _temp_dir) = test_config_manager();
        
        let schema = serde_json::json!({
            "type": "object",
            "properties": {
                "port": {
                    "type": "integer",
                    "minimum": 1,
                    "maximum": 65535
                }
            },
            "required": ["port"]
        });
        
        let plugin_id = "test-plugin@1.0.0".to_string();
        let valid_config = serde_json::json!({"port": 8080});
        
        manager.initialize_config(
            plugin_id.clone(),
            "Test Plugin".to_string(),
            Some(schema),
            valid_config,
        ).unwrap();
        
        // Try to import invalid config (missing required field)
        let invalid_import = serde_json::json!({
            "config": {}
        });
        
        let result = manager.import_config(&plugin_id, invalid_import);
        assert!(result.is_err());
        
        // Try to import invalid config (wrong type)
        let invalid_import = serde_json::json!({
            "config": {"port": "not a number"}
        });
        
        let result = manager.import_config(&plugin_id, invalid_import);
        assert!(result.is_err());
        
        // Import valid config
        let valid_import = serde_json::json!({
            "config": {"port": 9000}
        });
        
        let result = manager.import_config(&plugin_id, valid_import);
        assert!(result.is_ok());
        
        let retrieved = manager.get_config(&plugin_id).unwrap();
        assert_eq!(retrieved["port"], 9000);
    }
    
    #[test]
    fn test_restore_nonexistent_backup() {
        let (manager, temp_dir) = test_config_manager();
        
        let nonexistent_path = temp_dir.path().join("nonexistent_backup.json");
        let result = manager.restore_config(&nonexistent_path);
        
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }
    
    #[test]
    fn test_export_nonexistent_plugin() {
        let (manager, _temp_dir) = test_config_manager();
        
        let result = manager.export_config(&"nonexistent@1.0.0".to_string());
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }
    
    #[test]
    fn test_import_missing_config_field() {
        let (manager, _temp_dir) = test_config_manager();
        
        let plugin_id = "test-plugin@1.0.0".to_string();
        manager.initialize_config(
            plugin_id.clone(),
            "Test Plugin".to_string(),
            None,
            serde_json::json!({"key": "value"}),
        ).unwrap();
        
        // Import data without 'config' field
        let invalid_import = serde_json::json!({
            "plugin_id": plugin_id,
            "plugin_name": "Test Plugin"
        });
        
        let result = manager.import_config(&plugin_id, invalid_import);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("missing 'config' field"));
    }
}

