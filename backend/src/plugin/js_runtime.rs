//! JavaScript Runtime Module
//!
//! This module provides JavaScript plugin execution using Deno Core.
//! It allows loading and executing JavaScript plugins with proper error handling
//! and sandboxing.

use anyhow::{Context, Result};
use deno_core::{JsRuntime, v8};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::time::Instant;
use tracing::{debug, info};

use super::js_bindings::create_js_runtime_with_bindings;
use super::sandbox::{ResourceLimits, Sandbox};
use super::types::PluginMetadata;

/// JavaScript Runtime wrapper for executing JavaScript plugins
pub struct JsRuntimeWrapper {
    /// The Deno Core runtime instance
    runtime: JsRuntime,
    /// Path to the plugin file
    plugin_path: PathBuf,
    /// Plugin metadata
    metadata: PluginMetadata,
    /// Security sandbox
    sandbox: Option<Sandbox>,
    /// Execution start time (for CPU time tracking)
    execution_start: Option<Instant>,
}

impl JsRuntimeWrapper {
    /// Create a new JavaScript runtime for a plugin
    ///
    /// # Arguments
    /// * `plugin_path` - Path to the JavaScript plugin file
    /// * `metadata` - Plugin metadata
    /// * `config` - Plugin configuration (optional)
    ///
    /// # Returns
    /// A new JsRuntimeWrapper instance
    pub fn new(plugin_path: PathBuf, metadata: PluginMetadata, config: Option<Value>) -> Result<Self> {
        debug!("Creating JavaScript runtime for plugin: {}", metadata.name);

        // Create sandbox from plugin permissions
        let sandbox = if !metadata.permissions.is_empty() {
            let resource_limits = ResourceLimits::default();
            Some(Sandbox::new(metadata.permissions.clone(), resource_limits))
        } else {
            None
        };

        // Create runtime with plugin bindings and sandbox
        let config = config.unwrap_or(Value::Object(serde_json::Map::new()));
        let runtime = create_js_runtime_with_bindings(
            metadata.name.clone(),
            config,
            sandbox.as_ref(),
        )?;

        Ok(Self {
            runtime,
            plugin_path,
            metadata,
            sandbox,
            execution_start: None,
        })
    }

    /// Load and initialize the JavaScript module
    ///
    /// # Returns
    /// Result indicating success or failure
    pub async fn load_module(&mut self) -> Result<()> {
        info!(
            "Loading JavaScript module from: {}",
            self.plugin_path.display()
        );

        // Read the JavaScript file
        let code = std::fs::read_to_string(&self.plugin_path)
            .with_context(|| {
                format!(
                    "Failed to read JavaScript plugin file: {}",
                    self.plugin_path.display()
                )
            })?;

        // Create a module name from the file path
        let module_name = self.plugin_path
            .file_name()
            .and_then(|n| n.to_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| "plugin.js".to_string());

        debug!("Module name: {}", module_name);

        // Execute the script with a static module name
        self.runtime
            .execute_script("<plugin_module>", code.into())
            .with_context(|| {
                format!(
                    "Failed to execute JavaScript module: {}",
                    self.plugin_path.display()
                )
            })?;

        info!("JavaScript module loaded successfully");
        Ok(())
    }

    /// Execute a JavaScript function with arguments
    ///
    /// # Arguments
    /// * `function_name` - Name of the function to call
    /// * `args` - JSON-serializable arguments
    ///
    /// # Returns
    /// Result containing the function's return value as JSON
    pub async fn call_function<T, R>(&mut self, function_name: &str, args: T) -> Result<R>
    where
        T: Serialize,
        R: for<'de> Deserialize<'de>,
    {
        debug!("Calling JavaScript function: {}", function_name);

        // Start tracking execution time
        self.start_execution();

        // Serialize arguments to JSON
        let args_json = serde_json::to_string(&args)
            .context("Failed to serialize function arguments")?;

        // Call _ting_invoke using V8 API to avoid compiling new scripts for arguments
        {
            let scope = &mut self.runtime.handle_scope();
            let context = scope.get_current_context();
            let global = context.global(scope);

            // Get _ting_invoke function
            let invoke_name = v8::String::new(scope, "_ting_invoke").unwrap();
            let invoke_val = global.get(scope, invoke_name.into())
                .ok_or_else(|| anyhow::anyhow!("_ting_invoke not found"))?;
            let invoke_func = v8::Local::<v8::Function>::try_from(invoke_val)
                .map_err(|_| anyhow::anyhow!("_ting_invoke is not a function"))?;

            // Prepare arguments: [function_name, args_value]
            let func_name_v8 = v8::String::new(scope, function_name).unwrap();
            
            // Parse args JSON to V8 value
            let args_json_v8 = v8::String::new(scope, &args_json).unwrap();
            let args_val = v8::json::parse(scope, args_json_v8)
                .ok_or_else(|| anyhow::anyhow!("Failed to parse arguments JSON in V8"))?;

            let recv = v8::undefined(scope).into();
            let args = [func_name_v8.into(), args_val];

            // Call _ting_invoke
            if invoke_func.call(scope, recv, &args).is_none() {
                return Err(anyhow::anyhow!("Failed to call _ting_invoke"));
            }
        }

        // Drive the event loop until completion
        self.runtime.run_event_loop(Default::default()).await
            .context("Failed to run event loop")?;

        let (status, result_or_error) = {
            let scope = &mut self.runtime.handle_scope();
            let context = scope.get_current_context();
            let global = context.global(scope);

            // Helper to get string from global object
            let get_global_string = |scope: &mut deno_core::v8::HandleScope, key: &str| -> Option<String> {
                let key_str = deno_core::v8::String::new(scope, key)?;
                let val = global.get(scope, key_str.into())?;
                if val.is_undefined() || val.is_null() {
                    return None;
                }
                Some(val.to_string(scope)?.to_rust_string_lossy(scope))
            };

            let status = get_global_string(scope, "_ting_status")
                .ok_or_else(|| anyhow::anyhow!("Failed to retrieve execution status"))?;

            let result = match status.as_str() {
                "success" => {
                    let res = get_global_string(scope, "_ting_result")
                        .ok_or_else(|| anyhow::anyhow!("Function finished successfully but returned no result"))?;
                    Ok(res)
                },
                "error" => {
                    let err = get_global_string(scope, "_ting_error")
                        .unwrap_or_else(|| "Unknown error".to_string());
                    Err(err)
                },
                "pending" => {
                    Err("Event loop finished but function is still pending".to_string())
                },
                s => {
                    Err(format!("Invalid execution status: {}", s))
                }
            };
            (status, result)
        };

        // Cleanup global variables to free memory
        // This is crucial to prevent memory leaks as _ting_result can hold large JSON strings
        let _ = self.runtime.execute_script(
            "<cleanup>",
            r#"
            globalThis._ting_result = undefined;
            globalThis._ting_error = undefined;
            globalThis._ting_status = undefined;
            "#.to_string().into()
        );

        // Stop tracking execution time
        self.stop_execution();

        match result_or_error {
            Ok(result_str) => {
                // Deserialize the result
                let result: R = serde_json::from_str(&result_str)
                    .with_context(|| format!("Failed to deserialize function result: {}", result_str))?;
                
                debug!("Function call completed successfully");
                Ok(result)
            },
            Err(err_msg) => {
                if status == "pending" {
                     Err(anyhow::anyhow!(err_msg))
                } else if status == "error" {
                     Err(JsError::FunctionCallError(err_msg).into())
                } else {
                     Err(anyhow::anyhow!(err_msg))
                }
            }
        }
    }

    /// Execute arbitrary JavaScript code
    ///
    /// # Arguments
    /// * `code` - JavaScript code to execute
    ///
    /// # Returns
    /// Result indicating success or failure
    pub fn execute_script(&mut self, code: &str) -> Result<()> {
        debug!("Executing JavaScript code");

        self.runtime
            .execute_script("<execute_script>", code.to_string().into())
            .context("Failed to execute JavaScript code")?;

        debug!("JavaScript code executed successfully");
        Ok(())
    }

    /// Get the plugin metadata
    pub fn metadata(&self) -> &PluginMetadata {
        &self.metadata
    }

    /// Get the plugin path
    pub fn plugin_path(&self) -> &Path {
        &self.plugin_path
    }

    /// Get the sandbox (if any)
    pub fn sandbox(&self) -> Option<&Sandbox> {
        self.sandbox.as_ref()
    }

    /// Start tracking execution time
    fn start_execution(&mut self) {
        self.execution_start = Some(Instant::now());
    }

    /// Check CPU time limit
    pub fn check_cpu_time_limit(&self) -> Result<()> {
        if let (Some(sandbox), Some(start_time)) = (&self.sandbox, self.execution_start) {
            let elapsed = start_time.elapsed();
            sandbox.check_cpu_time(elapsed)?;
        }
        Ok(())
    }

    /// Stop tracking execution time
    fn stop_execution(&mut self) {
        self.execution_start = None;
    }

    /// Check file access permission
    pub fn check_file_access(&self, path: &Path, access: super::sandbox::FileAccess) -> Result<()> {
        if let Some(sandbox) = &self.sandbox {
            sandbox.check_file_access(path, access)?;
        }
        Ok(())
    }

    /// Check network access permission
    pub fn check_network_access(&self, url: &str) -> Result<()> {
        if let Some(sandbox) = &self.sandbox {
            sandbox.check_network_access(url)?;
        }
        Ok(())
    }

    /// Check memory limit
    pub fn check_memory_limit(&self, current_bytes: usize) -> Result<()> {
        if let Some(sandbox) = &self.sandbox {
            sandbox.check_memory_limit(current_bytes)?;
        }
        Ok(())
    }

    /// Request garbage collection
    pub fn garbage_collect(&mut self) -> Result<()> {
        debug!("Requesting garbage collection");
        self.runtime.v8_isolate().low_memory_notification();
        Ok(())
    }
}

/// JavaScript plugin error wrapper
#[derive(Debug, thiserror::Error)]
pub enum JsError {
    #[error("JavaScript execution error: {0}")]
    ExecutionError(String),

    #[error("JavaScript module load error: {0}")]
    ModuleLoadError(String),

    #[error("JavaScript function call error: {0}")]
    FunctionCallError(String),

    #[error("JavaScript serialization error: {0}")]
    SerializationError(String),

    #[error("JavaScript runtime error: {0}")]
    RuntimeError(String),
}

impl From<anyhow::Error> for JsError {
    fn from(err: anyhow::Error) -> Self {
        JsError::RuntimeError(err.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;
    use std::io::Write;

    #[tokio::test]
    async fn test_js_runtime_creation() {
        let metadata = PluginMetadata::new(
            "test-plugin".to_string(),
            "1.0.0".to_string(),
            super::super::types::PluginType::Utility,
            "Test Author".to_string(),
            "Test plugin".to_string(),
            "plugin.js".to_string(),
        );

        let temp_file = NamedTempFile::new().unwrap();
        let runtime = JsRuntimeWrapper::new(temp_file.path().to_path_buf(), metadata, None);
        assert!(runtime.is_ok());
    }

    #[tokio::test]
    async fn test_execute_simple_script() {
        let metadata = PluginMetadata::new(
            "test-plugin".to_string(),
            "1.0.0".to_string(),
            super::super::types::PluginType::Utility,
            "Test Author".to_string(),
            "Test plugin".to_string(),
            "plugin.js".to_string(),
        );

        let temp_file = NamedTempFile::new().unwrap();
        let mut runtime = JsRuntimeWrapper::new(temp_file.path().to_path_buf(), metadata, None).unwrap();

        let result = runtime.execute_script("const x = 1 + 1;");
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_load_module() {
        let metadata = PluginMetadata::new(
            "test-plugin".to_string(),
            "1.0.0".to_string(),
            super::super::types::PluginType::Utility,
            "Test Author".to_string(),
            "Test plugin".to_string(),
            "plugin.js".to_string(),
        );

        // Create a temporary JavaScript file
        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(temp_file, "function hello() {{ return 'Hello, World!'; }}").unwrap();
        temp_file.flush().unwrap();

        let mut runtime = JsRuntimeWrapper::new(temp_file.path().to_path_buf(), metadata, None).unwrap();
        let result = runtime.load_module().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_execute_script_with_error() {
        let metadata = PluginMetadata::new(
            "test-plugin".to_string(),
            "1.0.0".to_string(),
            super::super::types::PluginType::Utility,
            "Test Author".to_string(),
            "Test plugin".to_string(),
            "plugin.js".to_string(),
        );

        let temp_file = NamedTempFile::new().unwrap();
        let mut runtime = JsRuntimeWrapper::new(temp_file.path().to_path_buf(), metadata, None).unwrap();

        // This should fail due to syntax error
        let result = runtime.execute_script("const x = ;");
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_sandbox_network_access_check() {
        use super::super::sandbox::Permission;
        
        let mut metadata = PluginMetadata::new(
            "test-plugin".to_string(),
            "1.0.0".to_string(),
            super::super::types::PluginType::Utility,
            "Test Author".to_string(),
            "Test plugin".to_string(),
            "plugin.js".to_string(),
        );
        
        // Add network permission
        metadata.permissions = vec![
            Permission::NetworkAccess("*.example.com".to_string()),
        ];

        let temp_file = NamedTempFile::new().unwrap();
        let runtime = JsRuntimeWrapper::new(temp_file.path().to_path_buf(), metadata, None).unwrap();

        // Check allowed URL
        let result = runtime.check_network_access("https://api.example.com/data");
        assert!(result.is_ok());

        // Check disallowed URL
        let result = runtime.check_network_access("https://evil.com/data");
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_sandbox_file_access_check() {
        use super::super::sandbox::{Permission, FileAccess};
        use std::path::PathBuf;
        
        let mut metadata = PluginMetadata::new(
            "test-plugin".to_string(),
            "1.0.0".to_string(),
            super::super::types::PluginType::Utility,
            "Test Author".to_string(),
            "Test plugin".to_string(),
            "plugin.js".to_string(),
        );
        
        // Add file permissions
        metadata.permissions = vec![
            Permission::FileRead(PathBuf::from("./data/cache")),
            Permission::FileWrite(PathBuf::from("./data/output")),
        ];

        let temp_file = NamedTempFile::new().unwrap();
        let runtime = JsRuntimeWrapper::new(temp_file.path().to_path_buf(), metadata, None).unwrap();

        // Check allowed read
        let result = runtime.check_file_access(
            &PathBuf::from("./data/cache/file.txt"),
            FileAccess::Read
        );
        assert!(result.is_ok());

        // Check allowed write
        let result = runtime.check_file_access(
            &PathBuf::from("./data/output/file.txt"),
            FileAccess::Write
        );
        assert!(result.is_ok());

        // Check disallowed read (wrong path)
        let result = runtime.check_file_access(
            &PathBuf::from("./data/secret/file.txt"),
            FileAccess::Read
        );
        assert!(result.is_err());

        // Check disallowed write (read-only path)
        let result = runtime.check_file_access(
            &PathBuf::from("./data/cache/file.txt"),
            FileAccess::Write
        );
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_sandbox_memory_limit_check() {
        use super::super::sandbox::Permission;
        
        let mut metadata = PluginMetadata::new(
            "test-plugin".to_string(),
            "1.0.0".to_string(),
            super::super::types::PluginType::Utility,
            "Test Author".to_string(),
            "Test plugin".to_string(),
            "plugin.js".to_string(),
        );
        
        // Add a permission to trigger sandbox creation
        metadata.permissions = vec![Permission::NetworkAccess("example.com".to_string())];

        let temp_file = NamedTempFile::new().unwrap();
        let runtime = JsRuntimeWrapper::new(temp_file.path().to_path_buf(), metadata, None).unwrap();

        // Check within limit
        let result = runtime.check_memory_limit(100 * 1024 * 1024); // 100 MB
        assert!(result.is_ok());

        // Check exceeding limit
        let result = runtime.check_memory_limit(1024 * 1024 * 1024); // 1 GB (exceeds default 512 MB)
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_sandbox_cpu_time_tracking() {
        use std::time::Duration;
        use super::super::sandbox::Permission;
        
        let mut metadata = PluginMetadata::new(
            "test-plugin".to_string(),
            "1.0.0".to_string(),
            super::super::types::PluginType::Utility,
            "Test Author".to_string(),
            "Test plugin".to_string(),
            "plugin.js".to_string(),
        );

        // Add a permission to trigger sandbox creation
        metadata.permissions = vec![Permission::NetworkAccess("example.com".to_string())];

        let temp_file = NamedTempFile::new().unwrap();
        let mut runtime = JsRuntimeWrapper::new(temp_file.path().to_path_buf(), metadata, None).unwrap();

        // Start tracking
        runtime.start_execution();
        
        // Simulate some work
        std::thread::sleep(Duration::from_millis(10));
        
        // Check CPU time (should be OK for short duration)
        let result = runtime.check_cpu_time_limit();
        assert!(result.is_ok());
        
        // Stop tracking
        runtime.stop_execution();
    }
}
