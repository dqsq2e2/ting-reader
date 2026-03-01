//! Native plugin wrapper
//!
//! This module provides a wrapper that implements the Plugin trait for native dynamic libraries.
//! It bridges the NativeLoader functionality with the Plugin interface.

use std::sync::{Arc, RwLock};
use serde_json::Value;
use crate::core::error::{Result, TingError};
use super::types::{Plugin, PluginMetadata, PluginType, PluginContext};
use super::native::NativeLoader;

/// Native plugin wrapper that implements the Plugin trait
pub struct NativePlugin {
    /// Plugin metadata
    metadata: PluginMetadata,
    
    /// Plugin ID (name@version)
    plugin_id: String,
    
    /// Reference to the native loader
    native_loader: Arc<NativeLoader>,
    
    /// Initialization state
    initialized: RwLock<bool>,
}

impl NativePlugin {
    /// Create a new native plugin wrapper
    ///
    /// # Arguments
    /// * `plugin_id` - Unique plugin ID (name@version)
    /// * `metadata` - Plugin metadata
    /// * `native_loader` - Reference to the native loader that loaded this plugin
    pub fn new(
        plugin_id: String,
        metadata: PluginMetadata,
        native_loader: Arc<NativeLoader>,
    ) -> Self {
        Self {
            metadata,
            plugin_id,
            native_loader,
            initialized: RwLock::new(false),
        }
    }
    
    /// Call a method on the native plugin
    ///
    /// # Arguments
    /// * `method` - Method name to invoke
    /// * `params` - JSON parameters for the method
    ///
    /// # Returns
    /// JSON result from the plugin
    pub async fn call_method(&self, method: &str, params: Value) -> Result<Value> {
        // Check if initialized
        let is_initialized = *self.initialized.read().map_err(|e| {
            TingError::PluginExecutionError(format!("Failed to check initialization state: {}", e))
        })?;
        
        if !is_initialized {
            return Err(TingError::PluginExecutionError(
                format!("Plugin {} is not initialized", self.plugin_id)
            ));
        }
        
        // Call the native function through the loader in a blocking task
        let loader = self.native_loader.clone();
        let plugin_id = self.plugin_id.clone();
        let method = method.to_string();
        
        // Offload to blocking thread pool to avoid blocking the async runtime
        tokio::task::spawn_blocking(move || {
            loader.call_function(&plugin_id, &method, params)
        })
        .await
        .map_err(|e| TingError::PluginExecutionError(format!("Task join error: {}", e)))?
    }
}

#[async_trait::async_trait]
impl Plugin for NativePlugin {
    fn metadata(&self) -> &PluginMetadata {
        &self.metadata
    }
    
    async fn initialize(&self, context: &PluginContext) -> Result<()> {
        tracing::info!(
            plugin_id = %self.plugin_id,
            "Initializing native plugin"
        );
        
        // Call the plugin's initialize method if it exists
        let init_params = serde_json::json!({
            "config": context.config,
            "data_dir": context.data_dir.to_string_lossy(),
        });
        
        // Try to call initialize method (optional for plugins)
        // Since native calls are blocking, we rely on them being fast
        // or wrap in spawn_blocking if needed. For init/shutdown, direct call is usually fine.
        let loader = self.native_loader.clone();
        let plugin_id = self.plugin_id.clone();
        
        // Offload to blocking thread pool
        let result = tokio::task::spawn_blocking(move || {
            loader.call_function(&plugin_id, "initialize", init_params)
        })
        .await
        .map_err(|e| TingError::PluginExecutionError(format!("Task join error: {}", e)))?;
        
        match result {
            Ok(_) => {
                tracing::debug!(
                    plugin_id = %self.plugin_id,
                    "Native plugin initialize method called successfully"
                );
            }
            Err(e) => {
                // If the plugin doesn't have an initialize method, that's okay
                tracing::debug!(
                    plugin_id = %self.plugin_id,
                    error = %e,
                    "Native plugin initialize method not found or failed (this is optional)"
                );
            }
        }
        
        // Mark as initialized
        let mut initialized = self.initialized.write().map_err(|e| {
            TingError::PluginExecutionError(format!("Failed to update initialization state: {}", e))
        })?;
        *initialized = true;
        
        tracing::info!(
            plugin_id = %self.plugin_id,
            "Native plugin initialized successfully"
        );
        
        Ok(())
    }
    
    async fn shutdown(&self) -> Result<()> {
        tracing::info!(
            plugin_id = %self.plugin_id,
            "Shutting down native plugin"
        );
        
        // Call the plugin's shutdown method if it exists
        let shutdown_params = serde_json::json!({});
        
        // Try to call shutdown method (optional for plugins)
        let loader = self.native_loader.clone();
        let plugin_id = self.plugin_id.clone();
        
        // Offload to blocking thread pool
        let result = tokio::task::spawn_blocking(move || {
            loader.call_function(&plugin_id, "shutdown", shutdown_params)
        })
        .await
        .map_err(|e| TingError::PluginExecutionError(format!("Task join error: {}", e)))?;

        match result {
            Ok(_) => {
                tracing::debug!(
                    plugin_id = %self.plugin_id,
                    "Native plugin shutdown method called successfully"
                );
            }
            Err(e) => {
                // If the plugin doesn't have a shutdown method, that's okay
                tracing::debug!(
                    plugin_id = %self.plugin_id,
                    error = %e,
                    "Native plugin shutdown method not found or failed (this is optional)"
                );
            }
        }
        
        // Mark as not initialized
        let mut initialized = self.initialized.write().map_err(|e| {
            TingError::PluginExecutionError(format!("Failed to update initialization state: {}", e))
        })?;
        *initialized = false;
        
        tracing::info!(
            plugin_id = %self.plugin_id,
            "Native plugin shut down successfully"
        );
        
        Ok(())
    }
    
    async fn garbage_collect(&self) -> Result<()> {
        // Native plugins manage their own memory.
        tracing::debug!(plugin_id = %self.plugin_id, "Garbage collection requested for native plugin");
        
        // Try to call the plugin's garbage_collect method if it exists
        // We use spawn_blocking because this is a native call
        let loader = self.native_loader.clone();
        let plugin_id = self.plugin_id.clone();
        
        let _ = tokio::task::spawn_blocking(move || {
            if let Ok(true) = loader.has_symbol(&plugin_id, "plugin_invoke") {
                // We don't check if "garbage_collect" is supported by the invoke dispatch,
                // we just try to call it. If the plugin doesn't handle it, it should return an error
                // or ignore it.
                let _ = loader.call_function(
                    &plugin_id, 
                    "garbage_collect", 
                    serde_json::json!({})
                );
            }
        }).await;
        
        Ok(())
    }

    fn plugin_type(&self) -> PluginType {
        self.metadata.plugin_type
    }
    
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

// Ensure NativePlugin is thread-safe
unsafe impl Send for NativePlugin {}
unsafe impl Sync for NativePlugin {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_native_plugin_creation() {
        let metadata = PluginMetadata::new(
            "test-plugin".to_string(),
            "1.0.0".to_string(),
            crate::plugin::types::PluginType::Format,
            "Test Author".to_string(),
            "Test plugin".to_string(),
            "plugin.dll".to_string(),
        );
        
        let loader = Arc::new(NativeLoader::new());
        let plugin = NativePlugin::new("test-plugin@1.0.0".to_string(), metadata, loader);
        
        assert_eq!(plugin.metadata().name, "test-plugin");
    }
}
