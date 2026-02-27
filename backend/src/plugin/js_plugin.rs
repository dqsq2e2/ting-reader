//! JavaScript Plugin Loader
//!
//! This module provides the JavaScript plugin loading and lifecycle management.
//! 
//! **Important Note on Thread Safety:**
//! JavaScript plugins using Deno Core cannot implement the Plugin trait directly
//! because Deno's JsRuntime is not Send + Sync (V8 isolates are single-threaded).
//! 
//! Instead, this module provides:
//! 1. JavaScriptPluginLoader - for loading and managing JS plugin metadata
//! 2. JavaScriptPluginExecutor - for executing JS plugins in a single-threaded context
//! 
//! The plugin manager should handle JS plugins specially, executing them on a
//! dedicated single-threaded runtime.

use anyhow::{Context, Result};
use serde_json::Value;
use std::path::{Path, PathBuf};
use tracing::{info, warn};

use super::js_runtime::JsRuntimeWrapper;
use super::types::{PluginMetadata, PluginType};
use crate::core::error::TingError;

/// JavaScript plugin loader
///
/// This struct handles loading JavaScript plugin metadata and creating executors.
/// It does NOT implement the Plugin trait due to thread safety constraints.
#[derive(Debug, Clone)]
pub struct JavaScriptPluginLoader {
    /// Plugin metadata
    metadata: PluginMetadata,
    
    /// Plugin directory path
    plugin_dir: PathBuf,
}

impl JavaScriptPluginLoader {
    /// Create a new JavaScript plugin loader from a plugin directory
    ///
    /// # Arguments
    /// * `plugin_dir` - Path to the plugin directory containing plugin.json and .js files
    ///
    /// # Returns
    /// A new JavaScriptPluginLoader instance
    ///
    /// # Errors
    /// Returns an error if:
    /// - plugin.json cannot be read or parsed
    /// - The runtime field is not "javascript"
    /// - The entry point file doesn't exist
    pub fn new(plugin_dir: PathBuf) -> Result<Self> {
        info!("Loading JavaScript plugin from: {}", plugin_dir.display());
        
        // Read and parse plugin.json
        let metadata = Self::read_metadata(&plugin_dir)?;
        
        // Verify this is a JavaScript plugin
        Self::verify_runtime(&metadata, &plugin_dir)?;
        
        // Get the entry point file path
        let entry_point = plugin_dir.join(&metadata.entry_point);
        if !entry_point.exists() {
            return Err(TingError::PluginLoadError(format!(
                "Entry point file not found: {}",
                entry_point.display()
            ))
            .into());
        }
        
        Ok(Self {
            metadata,
            plugin_dir,
        })
    }
    
    /// Get the plugin metadata
    pub fn metadata(&self) -> &PluginMetadata {
        &self.metadata
    }
    
    /// Get the plugin directory
    pub fn plugin_dir(&self) -> &Path {
        &self.plugin_dir
    }
    
    /// Get the plugin type
    pub fn plugin_type(&self) -> PluginType {
        self.metadata.plugin_type
    }
    
    /// Create an executor for this plugin
    ///
    /// The executor must be used in a single-threaded context (e.g., LocalSet)
    pub fn create_executor(&self) -> Result<JavaScriptPluginExecutor> {
        JavaScriptPluginExecutor::new(self.plugin_dir.clone(), self.metadata.clone())
    }
    
    /// Install npm dependencies for this plugin
    ///
    /// This method generates a package.json and runs npm install if the plugin
    /// has npm dependencies declared.
    ///
    /// # Arguments
    /// * `npm_manager` - The npm manager instance to use
    ///
    /// # Returns
    /// Result indicating success or failure
    pub fn install_npm_dependencies(&self, npm_manager: &super::npm_manager::NpmManager) -> Result<()> {
        // Check if plugin has npm dependencies
        if self.metadata.npm_dependencies.is_empty() {
            info!("Plugin {} has no npm dependencies, skipping npm install", self.metadata.name);
            return Ok(());
        }
        
        info!("Installing npm dependencies for plugin: {}", self.metadata.name);
        
        // Generate package.json
        npm_manager.generate_package_json(
            &self.plugin_dir,
            &self.metadata.name,
            &self.metadata.version,
            Some(&self.metadata.description),
            Some(&self.metadata.author),
            self.metadata.license.as_deref(),
            &self.metadata.npm_dependencies,
        )?;
        
        // Install dependencies
        npm_manager.install_dependencies(&self.plugin_dir)?;
        
        info!("npm dependencies installed successfully for plugin: {}", self.metadata.name);
        Ok(())
    }
    
    /// Check if npm dependencies are installed
    pub fn has_npm_dependencies_installed(&self, npm_manager: &super::npm_manager::NpmManager) -> bool {
        if self.metadata.npm_dependencies.is_empty() {
            return true; // No dependencies means nothing to install
        }
        npm_manager.has_node_modules(&self.plugin_dir)
    }
    
    /// Read plugin metadata from plugin.json
    fn read_metadata(plugin_dir: &Path) -> Result<PluginMetadata> {
        let metadata_path = plugin_dir.join("plugin.json");
        
        if !metadata_path.exists() {
            return Err(TingError::PluginLoadError(format!(
                "plugin.json not found in: {}",
                plugin_dir.display()
            ))
            .into());
        }
        
        let content = std::fs::read_to_string(&metadata_path).with_context(|| {
            format!("Failed to read plugin.json from: {}", metadata_path.display())
        })?;
        
        // Parse the JSON
        let json: Value = serde_json::from_str(&content).with_context(|| {
            format!("Failed to parse plugin.json from: {}", metadata_path.display())
        })?;
        
        // Extract metadata fields
        let name = json["name"]
            .as_str()
            .ok_or_else(|| TingError::PluginLoadError("Missing 'name' field in plugin.json".to_string()))?
            .to_string();
        
        let version = json["version"]
            .as_str()
            .ok_or_else(|| TingError::PluginLoadError("Missing 'version' field in plugin.json".to_string()))?
            .to_string();
        
        let plugin_type_str = json["plugin_type"]
            .as_str()
            .ok_or_else(|| TingError::PluginLoadError("Missing 'plugin_type' field in plugin.json".to_string()))?;
        
        let plugin_type = match plugin_type_str {
            "scraper" => PluginType::Scraper,
            "format" => PluginType::Format,
            "utility" => PluginType::Utility,
            _ => return Err(TingError::PluginLoadError(format!(
                "Invalid plugin_type: {}. Must be 'scraper', 'format', or 'utility'",
                plugin_type_str
            ))
            .into()),
        };
        
        let author = json["author"]
            .as_str()
            .ok_or_else(|| TingError::PluginLoadError("Missing 'author' field in plugin.json".to_string()))?
            .to_string();
        
        let description = json["description"]
            .as_str()
            .ok_or_else(|| TingError::PluginLoadError("Missing 'description' field in plugin.json".to_string()))?
            .to_string();
        
        let entry_point = json["entry_point"]
            .as_str()
            .ok_or_else(|| TingError::PluginLoadError("Missing 'entry_point' field in plugin.json".to_string()))?
            .to_string();
        
        // Optional fields
        let license = json["license"].as_str().map(|s| s.to_string());
        let homepage = json["homepage"].as_str().map(|s| s.to_string());
        let config_schema = json.get("config_schema").cloned();
        let min_core_version = json["min_core_version"].as_str().map(|s| s.to_string());
        
        let supported_extensions = json["supported_extensions"].as_array().map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        });

        // Parse dependencies
        let dependencies = if let Some(deps_array) = json["dependencies"].as_array() {
            deps_array
                .iter()
                .filter_map(|dep| {
                    let plugin_name = dep["plugin_name"].as_str()?.to_string();
                    let version_requirement = dep["version_requirement"].as_str()?.to_string();
                    Some(super::types::PluginDependency {
                        plugin_name,
                        version_requirement,
                    })
                })
                .collect()
        } else {
            Vec::new()
        };
        
        // Parse npm dependencies using NpmManager
        let npm_dependencies = super::npm_manager::NpmManager::parse_dependencies(&json);
        
        // Parse permissions
        let permissions = if let Some(perms_array) = json["permissions"].as_array() {
            perms_array
                .iter()
                .filter_map(|perm| {
                    let perm_type = perm["type"].as_str()?;
                    let value = perm["value"].as_str()?;
                    
                    match perm_type {
                        "network_access" => Some(super::sandbox::Permission::NetworkAccess(value.to_string())),
                        "file_read" => Some(super::sandbox::Permission::FileRead(PathBuf::from(value))),
                        "file_write" => Some(super::sandbox::Permission::FileWrite(PathBuf::from(value))),
                        "database_read" => Some(super::sandbox::Permission::DatabaseRead),
                        "database_write" => Some(super::sandbox::Permission::DatabaseWrite),
                        "event_publish" => Some(super::sandbox::Permission::EventPublish),
                        _ => {
                            warn!("Unknown permission type: {}", perm_type);
                            None
                        }
                    }
                })
                .collect()
        } else {
            Vec::new()
        };
        
        let metadata = PluginMetadata {
            name,
            version,
            plugin_type,
            author,
            description,
            license,
            homepage,
            entry_point,
            dependencies,
            npm_dependencies,
            permissions,
            config_schema,
            min_core_version,
            supported_extensions,
        };
        
        Ok(metadata)
    }
    
    /// Verify that the plugin metadata specifies JavaScript runtime
    fn verify_runtime(metadata: &PluginMetadata, plugin_dir: &Path) -> Result<()> {
        // Read plugin.json again to check for runtime field
        let metadata_path = plugin_dir.join("plugin.json");
        let content = std::fs::read_to_string(&metadata_path)?;
        let json: Value = serde_json::from_str(&content)?;
        
        // Check if runtime field exists and is "javascript"
        if let Some(runtime) = json.get("runtime") {
            if let Some(runtime_str) = runtime.as_str() {
                if runtime_str != "javascript" {
                    return Err(TingError::PluginLoadError(format!(
                        "Plugin runtime is '{}', expected 'javascript'",
                        runtime_str
                    ))
                    .into());
                }
            } else {
                return Err(TingError::PluginLoadError(
                    "Plugin 'runtime' field must be a string".to_string()
                )
                .into());
            }
        } else {
            // If no runtime field, check if entry_point is a .js file
            if !metadata.entry_point.ends_with(".js") {
                return Err(TingError::PluginLoadError(format!(
                    "Plugin entry_point '{}' is not a .js file and no 'runtime' field specified",
                    metadata.entry_point
                ))
                .into());
            }
        }
        
        Ok(())
    }
}

/// JavaScript plugin executor
///
/// This struct wraps a JavaScript runtime and provides execution methods.
/// It must be used in a single-threaded context (e.g., tokio::task::LocalSet).
pub struct JavaScriptPluginExecutor {
    /// The JavaScript runtime wrapper
    runtime: JsRuntimeWrapper,
    
    /// Plugin metadata
    metadata: PluginMetadata,
    
    /// Plugin directory path
    plugin_dir: PathBuf,
    
    /// Whether the plugin has been initialized
    initialized: bool,
}

impl JavaScriptPluginExecutor {
    /// Create a new JavaScript plugin executor
    fn new(plugin_dir: PathBuf, metadata: PluginMetadata) -> Result<Self> {
        let entry_point = plugin_dir.join(&metadata.entry_point);
        let runtime = JsRuntimeWrapper::new(entry_point, metadata.clone(), None)?;
        
        Ok(Self {
            runtime,
            metadata,
            plugin_dir,
            initialized: false,
        })
    }
    
    /// Load the JavaScript module
    pub async fn load_module(&mut self) -> Result<()> {
        self.runtime.load_module().await
    }
    
    /// Initialize the plugin
    pub async fn initialize(&mut self, config: Value, data_dir: PathBuf) -> Result<()> {
        if self.initialized {
            return Ok(());
        }
        
        info!("Initializing JavaScript plugin: {}", self.metadata.name);
        
        // Call the initialize function if it exists
        let init_code = format!(
            r#"
            if (typeof initialize === 'function') {{
                const context = {};
                initialize(context);
            }}
            "#,
            serde_json::to_string(&serde_json::json!({
                "config": config,
                "data_dir": data_dir.to_string_lossy(),
            }))
            .unwrap_or_else(|_| "{}".to_string())
        );
        
        self.runtime.execute_script(&init_code)?;
        self.initialized = true;
        
        info!("JavaScript plugin initialized: {}", self.metadata.name);
        Ok(())
    }
    
    /// Shutdown the plugin
    pub fn shutdown(&mut self) -> Result<()> {
        if !self.initialized {
            return Ok(());
        }
        
        info!("Shutting down JavaScript plugin: {}", self.metadata.name);
        
        // Call the shutdown function if it exists
        let shutdown_code = r#"
            if (typeof shutdown === 'function') {
                shutdown();
            }
        "#;
        
        self.runtime.execute_script(shutdown_code)?;
        self.initialized = false;
        
        info!("JavaScript plugin shut down: {}", self.metadata.name);
        Ok(())
    }
    
    /// Call a JavaScript function
    pub async fn call_function<T, R>(&mut self, function_name: &str, args: T) -> Result<R>
    where
        T: serde::Serialize,
        R: for<'de> serde::Deserialize<'de>,
    {
        self.runtime.call_function(function_name, args).await
    }
    
    /// Get the plugin metadata
    pub fn metadata(&self) -> &PluginMetadata {
        &self.metadata
    }
    
    /// Get the plugin directory
    pub fn plugin_dir(&self) -> &Path {
        &self.plugin_dir
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;
    
    fn create_test_plugin_dir(name: &str, runtime: &str) -> TempDir {
        let temp_dir = TempDir::new().unwrap();
        let plugin_dir = temp_dir.path().join(name);
        fs::create_dir(&plugin_dir).unwrap();
        
        // Create plugin.json
        let metadata = serde_json::json!({
            "name": name,
            "version": "1.0.0",
            "plugin_type": "utility",
            "author": "Test Author",
            "description": "Test JavaScript plugin",
            "runtime": runtime,
            "entry_point": "plugin.js",
            "dependencies": [],
            "permissions": []
        });
        
        fs::write(
            plugin_dir.join("plugin.json"),
            serde_json::to_string_pretty(&metadata).unwrap(),
        )
        .unwrap();
        
        // Create a simple JavaScript file
        fs::write(
            plugin_dir.join("plugin.js"),
            r#"
            function initialize(context) {
                console.log("Plugin initialized");
            }
            
            function shutdown() {
                console.log("Plugin shut down");
            }
            
            function hello(args) {
                return { message: "Hello, " + args.name + "!" };
            }
            "#,
        )
        .unwrap();
        
        temp_dir
    }
    
    #[test]
    fn test_read_metadata() {
        let temp_dir = create_test_plugin_dir("test-plugin", "javascript");
        let plugin_dir = temp_dir.path().join("test-plugin");
        
        let metadata = JavaScriptPluginLoader::read_metadata(&plugin_dir).unwrap();
        
        assert_eq!(metadata.name, "test-plugin");
        assert_eq!(metadata.version, "1.0.0");
        assert_eq!(metadata.plugin_type, PluginType::Utility);
        assert_eq!(metadata.author, "Test Author");
        assert_eq!(metadata.entry_point, "plugin.js");
    }
    
    #[test]
    fn test_verify_runtime_javascript() {
        let temp_dir = create_test_plugin_dir("test-plugin", "javascript");
        let plugin_dir = temp_dir.path().join("test-plugin");
        
        let metadata = JavaScriptPluginLoader::read_metadata(&plugin_dir).unwrap();
        let result = JavaScriptPluginLoader::verify_runtime(&metadata, &plugin_dir);
        
        assert!(result.is_ok());
    }
    
    #[test]
    fn test_verify_runtime_wrong_runtime() {
        let temp_dir = create_test_plugin_dir("test-plugin", "wasm");
        let plugin_dir = temp_dir.path().join("test-plugin");
        
        let metadata = JavaScriptPluginLoader::read_metadata(&plugin_dir).unwrap();
        let result = JavaScriptPluginLoader::verify_runtime(&metadata, &plugin_dir);
        
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("expected 'javascript'"));
    }
    
    #[test]
    fn test_new_javascript_plugin_loader() {
        let temp_dir = create_test_plugin_dir("test-plugin", "javascript");
        let plugin_dir = temp_dir.path().join("test-plugin");
        
        let loader = JavaScriptPluginLoader::new(plugin_dir);
        
        assert!(loader.is_ok());
        let loader = loader.unwrap();
        assert_eq!(loader.metadata().name, "test-plugin");
        assert_eq!(loader.plugin_type(), PluginType::Utility);
    }
    
    #[test]
    fn test_new_missing_entry_point() {
        let temp_dir = TempDir::new().unwrap();
        let plugin_dir = temp_dir.path().join("test-plugin");
        fs::create_dir(&plugin_dir).unwrap();
        
        // Create plugin.json but no plugin.js
        let metadata = serde_json::json!({
            "name": "test-plugin",
            "version": "1.0.0",
            "plugin_type": "utility",
            "author": "Test Author",
            "description": "Test plugin",
            "runtime": "javascript",
            "entry_point": "plugin.js",
        });
        
        fs::write(
            plugin_dir.join("plugin.json"),
            serde_json::to_string_pretty(&metadata).unwrap(),
        )
        .unwrap();
        
        let result = JavaScriptPluginLoader::new(plugin_dir);
        
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Entry point file not found"));
    }
    
    #[tokio::test]
    async fn test_create_executor_and_load() {
        let temp_dir = create_test_plugin_dir("test-plugin", "javascript");
        let plugin_dir = temp_dir.path().join("test-plugin");
        
        let loader = JavaScriptPluginLoader::new(plugin_dir).unwrap();
        let mut executor = loader.create_executor().unwrap();
        
        let result = executor.load_module().await;
        assert!(result.is_ok());
    }
}
