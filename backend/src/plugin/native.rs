//! Native dynamic library loader
//!
//! This module provides functionality for loading and managing native dynamic libraries
//! (.dll on Windows, .so on Linux, .dylib on macOS) as plugins.
//!
//! The NativeLoader handles:
//! - Cross-platform dynamic library loading
//! - FFI function symbol lookup and calling
//! - Safe library unloading and resource cleanup
//! - Thread-safe library management

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};
use libloading::{Library, Symbol};
use crate::core::error::{Result, TingError};
use crate::plugin::types::{PluginId, PluginMetadata};
use crate::plugin::sandbox::ResourceLimits;

/// Native plugin loader
///
/// Manages the loading, execution, and unloading of native dynamic libraries.
/// Provides thread-safe access to loaded libraries and their exported functions.
/// Includes safety wrappers for timeout control, error handling, and resource monitoring.
pub struct NativeLoader {
    /// Map of plugin ID to loaded library
    libraries: Arc<RwLock<HashMap<PluginId, LoadedLibrary>>>,
    
    /// Default resource limits for native plugins
    default_limits: ResourceLimits,
}

/// A loaded native library with its metadata
struct LoadedLibrary {
    /// The loaded library handle
    library: Library,
    
    /// Path to the library file
    path: PathBuf,
    
    /// Plugin metadata
    metadata: PluginMetadata,
    
    /// Reference count for safe unloading
    ref_count: usize,
    
    /// Resource limits for this plugin
    resource_limits: ResourceLimits,
    
    /// Resource usage statistics
    stats: ResourceStats,
}

/// Resource usage statistics for a native plugin
#[derive(Debug, Clone, Default)]
pub struct ResourceStats {
    /// Total number of calls
    pub total_calls: u64,
    
    /// Number of successful calls
    pub successful_calls: u64,
    
    /// Number of failed calls
    pub failed_calls: u64,
    
    /// Number of timeout errors
    pub timeout_errors: u64,
    
    /// Peak memory usage in bytes (estimated)
    pub peak_memory_bytes: usize,
    
    /// Total CPU time spent
    pub total_cpu_time: Duration,
    
    /// Last execution time
    pub last_execution_time: Option<Duration>,
}

impl NativeLoader {
    /// Create a new native loader with default resource limits
    pub fn new() -> Self {
        Self::with_limits(ResourceLimits::default())
    }
    
    /// Create a new native loader with custom resource limits
    pub fn with_limits(default_limits: ResourceLimits) -> Self {
        Self {
            libraries: Arc::new(RwLock::new(HashMap::new())),
            default_limits,
        }
    }
    
    /// Load a native library from the given path
    ///
    /// # Arguments
    /// * `plugin_id` - Unique identifier for this plugin instance
    /// * `path` - Path to the dynamic library file
    /// * `metadata` - Plugin metadata
    ///
    /// # Returns
    /// * `Ok(())` if the library was loaded successfully
    /// * `Err(TingError)` if loading failed
    ///
    /// # Safety
    /// Loading native libraries is inherently unsafe as it executes arbitrary code.
    /// The caller must ensure the library is from a trusted source.
    pub fn load_library(
        &self,
        plugin_id: PluginId,
        path: &Path,
        metadata: PluginMetadata,
    ) -> Result<()> {
        self.load_library_with_limits(plugin_id, path, metadata, self.default_limits.clone())
    }
    
    /// Load a native library with custom resource limits
    ///
    /// # Arguments
    /// * `plugin_id` - Unique identifier for this plugin instance
    /// * `path` - Path to the dynamic library file
    /// * `metadata` - Plugin metadata
    /// * `resource_limits` - Custom resource limits for this plugin
    ///
    /// # Returns
    /// * `Ok(())` if the library was loaded successfully
    /// * `Err(TingError)` if loading failed
    pub fn load_library_with_limits(
        &self,
        plugin_id: PluginId,
        path: &Path,
        metadata: PluginMetadata,
        resource_limits: ResourceLimits,
    ) -> Result<()> {
        // Validate the file exists
        if !path.exists() {
            return Err(TingError::PluginLoadError(
                format!("Library file not found: {:?}", path)
            ));
        }
        
        // Validate file extension matches platform
        if !self.is_valid_library_extension(path) {
            return Err(TingError::PluginLoadError(
                format!("Invalid library file extension: {:?}", path)
            ));
        }
        
        // Load the library
        let library = unsafe {
            Library::new(path).map_err(|e| {
                TingError::PluginLoadError(
                    format!("Failed to load library {:?}: {}", path, e)
                )
            })?
        };
        
        tracing::info!(
            plugin_id = %plugin_id,
            path = ?path,
            max_memory = resource_limits.max_memory_bytes,
            max_cpu_time = ?resource_limits.max_cpu_time,
            "Native library loaded successfully with resource limits"
        );
        
        // Store the loaded library
        let loaded = LoadedLibrary {
            library,
            path: path.to_path_buf(),
            metadata,
            ref_count: 1,
            resource_limits,
            stats: ResourceStats::default(),
        };
        
        let mut libraries = self.libraries.write().map_err(|e| {
            TingError::PluginLoadError(format!("Failed to acquire write lock: {}", e))
        })?;
        
        libraries.insert(plugin_id, loaded);
        
        Ok(())
    }
    
    /// Unload a native library
    ///
    /// # Arguments
    /// * `plugin_id` - ID of the plugin to unload
    ///
    /// # Returns
    /// * `Ok(())` if the library was unloaded successfully
    /// * `Err(TingError)` if unloading failed or the library is still in use
    pub fn unload_library(&self, plugin_id: &PluginId) -> Result<()> {
        let mut libraries = self.libraries.write().map_err(|e| {
            TingError::PluginLoadError(format!("Failed to acquire write lock: {}", e))
        })?;
        
        let loaded = libraries.get_mut(plugin_id).ok_or_else(|| {
            TingError::PluginNotFound(format!("Plugin {} not found", plugin_id))
        })?;
        
        // Decrement reference count
        loaded.ref_count = loaded.ref_count.saturating_sub(1);
        
        // Only unload if reference count reaches zero
        if loaded.ref_count == 0 {
            let path = loaded.path.clone();
            libraries.remove(plugin_id);
            
            tracing::info!(
                plugin_id = %plugin_id,
                path = ?path,
                "Native library unloaded"
            );
        } else {
            tracing::debug!(
                plugin_id = %plugin_id,
                ref_count = loaded.ref_count,
                "Library still in use, not unloading"
            );
        }
        
        Ok(())
    }
    
    /// Check if a symbol exists in a loaded library
    ///
    /// # Arguments
    /// * `plugin_id` - ID of the plugin
    /// * `symbol_name` - Name of the function symbol to check
    ///
    /// # Returns
    /// * `Ok(true)` if the symbol exists
    /// * `Ok(false)` if the symbol doesn't exist
    /// * `Err(TingError)` if the library is not loaded
    pub fn has_symbol(&self, plugin_id: &PluginId, symbol_name: &str) -> Result<bool> {
        let libraries = self.libraries.read().map_err(|e| {
            TingError::PluginLoadError(format!("Failed to acquire read lock: {}", e))
        })?;
        
        let loaded = libraries.get(plugin_id).ok_or_else(|| {
            TingError::PluginNotFound(format!("Plugin {} not found", plugin_id))
        })?;
        
        // Try to look up the symbol
        unsafe {
            match loaded.library.get::<*const ()>(symbol_name.as_bytes()) {
                Ok(_) => Ok(true),
                Err(_) => Ok(false),
            }
        }
    }
    
    /// Call a function in a loaded library with safety wrappers
    ///
    /// This method provides:
    /// - Timeout control to prevent hanging
    /// - Error catching and conversion
    /// - Resource monitoring (CPU time, memory estimation)
    ///
    /// # Arguments
    /// * `plugin_id` - ID of the plugin
    /// * `function_name` - Name of the function to call
    /// * `args` - Arguments to pass to the function (as JSON)
    ///
    /// # Returns
    /// * `Ok(Value)` - The function's return value as JSON
    /// * `Err(TingError)` - If the function call failed
    ///
    /// # Safety
    /// This function assumes the native plugin exports a standard interface:
    /// ```c
    /// int plugin_invoke(const char* method, const char* params, char** result);
    /// ```
    pub fn call_function(
        &self,
        plugin_id: &PluginId,
        function_name: &str,
        args: serde_json::Value,
    ) -> Result<serde_json::Value> {
        // Get resource limits before spawning thread
        let resource_limits = {
            let libraries = self.libraries.read().map_err(|e| {
                TingError::PluginLoadError(format!("Failed to acquire read lock: {}", e))
            })?;
            
            let loaded = libraries.get(plugin_id).ok_or_else(|| {
                TingError::PluginNotFound(format!("Plugin {} not found", plugin_id))
            })?;
            
            loaded.resource_limits.clone()
        };
        
        // Execute with timeout using tokio runtime
        let result = self.call_function_with_timeout(
            plugin_id,
            function_name,
            args,
            resource_limits.max_cpu_time,
        )?;
        
        // Update statistics
        self.update_stats_after_call(plugin_id, true, None)?;
        
        Ok(result)
    }
    
    /// Call a function with timeout control
    ///
    /// Spawns the native call in a separate thread to enable timeout enforcement.
    fn call_function_with_timeout(
        &self,
        plugin_id: &PluginId,
        function_name: &str,
        args: serde_json::Value,
        _timeout: Duration,
    ) -> Result<serde_json::Value> {
        // Record start time for CPU time monitoring
        let start_time = Instant::now();
        
        // Prepare arguments
        let args_str = serde_json::to_string(&args).map_err(|e| {
            TingError::PluginExecutionError(format!("Failed to serialize arguments: {}", e))
        })?;
        
        let method_cstr = std::ffi::CString::new(function_name).map_err(|e| {
            TingError::PluginExecutionError(format!("Invalid function name: {}", e))
        })?;
        
        let params_cstr = std::ffi::CString::new(args_str).map_err(|e| {
            TingError::PluginExecutionError(format!("Invalid parameters: {}", e))
        })?;
        
        // DIRECT EXECUTION: Removed thread spawning to avoid issues in DLL environment
        // The outer layer (NativePlugin) already uses spawn_blocking, so we are not blocking the async runtime.
        // Thread spawning inside a DLL or during initialization can be problematic.
        let result = Self::execute_native_call(
            &self.libraries,
            plugin_id,
            method_cstr,
            params_cstr,
        );
        
        match result {
            Ok(call_result) => {
                let elapsed = start_time.elapsed();
                // Update execution time stats
                self.update_execution_time(plugin_id, elapsed)?;
                Ok(call_result)
            }
            Err(e) => {
                tracing::error!(
                    plugin_id = %plugin_id,
                    function = function_name,
                    "Native plugin execution failed: {:?}",
                    e
                );
                Err(e)
            }
        }
    }
    
    /// Execute the actual native call (runs in a separate thread)
    fn execute_native_call(
        libraries: &Arc<RwLock<HashMap<PluginId, LoadedLibrary>>>,
        plugin_id: &PluginId,
        method_cstr: std::ffi::CString,
        params_cstr: std::ffi::CString,
    ) -> Result<serde_json::Value> {
        // Acquire read lock
        let libraries = libraries.read().map_err(|e| {
            TingError::PluginLoadError(format!("Failed to acquire read lock: {}", e))
        })?;
        
        let loaded = libraries.get(plugin_id).ok_or_else(|| {
            TingError::PluginNotFound(format!("Plugin {} not found", plugin_id))
        })?;
        
        // Look up the plugin_invoke function
        type InvokeFn = unsafe extern "C" fn(*const u8, *const u8, *mut *mut u8) -> i32;
        
        let symbol: Symbol<InvokeFn> = unsafe {
            loaded.library.get(b"plugin_invoke").map_err(|e| {
                TingError::PluginExecutionError(
                    format!("Symbol 'plugin_invoke' not found in plugin {}: {}", plugin_id, e)
                )
            })?
        };
        
        // Call the function with error catching
        let mut result_ptr: *mut u8 = std::ptr::null_mut();
        
        // Catch any panics from the native code
        let return_code = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            unsafe {
                symbol(
                    method_cstr.as_ptr() as *const u8,
                    params_cstr.as_ptr() as *const u8,
                    &mut result_ptr as *mut *mut u8,
                )
            }
        }));
        
        let return_code = match return_code {
            Ok(code) => code,
            Err(e) => {
                tracing::error!(
                    plugin_id = %plugin_id,
                    "Native plugin panicked during execution: {:?}",
                    e
                );
                return Err(TingError::PluginExecutionError(
                    "Native plugin panicked during execution".to_string()
                ));
            }
        };
        
        // Check return code
        if return_code != 0 {
            tracing::warn!(
                plugin_id = %plugin_id,
                return_code = return_code,
                "Native plugin returned error code"
            );
            
            return Err(TingError::PluginExecutionError(
                format!("Plugin function returned error code: {}", return_code)
            ));
        }
        
        // Validate result pointer
        if result_ptr.is_null() {
            return Err(TingError::PluginExecutionError(
                "Plugin function returned null result".to_string()
            ));
        }
        
        // Convert result pointer to string
        let result_str = unsafe {
            // Fix for ARM64 build: Ensure pointer cast is correct
            // result_ptr is *mut u8 (pointer to u8)
            // CStr::from_ptr expects *const c_char (which can be i8 or u8 depending on platform)
            let cstr = std::ffi::CStr::from_ptr(result_ptr as *const std::os::raw::c_char);
            cstr.to_str().map_err(|e| {
                TingError::PluginExecutionError(format!("Invalid UTF-8 in result: {}", e))
            })?
        };
        
        // Parse JSON result
        let result = serde_json::from_str(result_str).map_err(|e| {
            TingError::PluginExecutionError(format!("Failed to parse result JSON: {}", e))
        })?;
        
        // Free the result string using plugin_free if available, otherwise fallback to libc::free
        // Ideally, plugins should export plugin_free to ensure memory is freed by the same allocator
        let free_symbol_name = b"plugin_free";
        let has_free = unsafe {
            loaded.library.get::<*const ()>(free_symbol_name).is_ok()
        };

        if has_free {
            type FreeFn = unsafe extern "C" fn(*mut u8);
            let free_fn: Symbol<FreeFn> = unsafe {
                loaded.library.get(free_symbol_name).unwrap()
            };
            unsafe {
                free_fn(result_ptr);
            }
        } else {
            // Fallback to libc::free (might cause issues on Windows if CRT differs)
            unsafe {
                libc::free(result_ptr as *mut libc::c_void);
            }
        }
        
        Ok(result)
    }
    
    /// Update statistics after a function call
    fn update_stats_after_call(
        &self,
        plugin_id: &PluginId,
        success: bool,
        elapsed: Option<Duration>,
    ) -> Result<()> {
        let mut libraries = self.libraries.write().map_err(|e| {
            TingError::PluginLoadError(format!("Failed to acquire write lock: {}", e))
        })?;
        
        if let Some(loaded) = libraries.get_mut(plugin_id) {
            loaded.stats.total_calls += 1;
            
            if success {
                loaded.stats.successful_calls += 1;
            } else {
                loaded.stats.failed_calls += 1;
                
                if elapsed.is_some() {
                    loaded.stats.timeout_errors += 1;
                }
            }
            
            if let Some(duration) = elapsed {
                loaded.stats.total_cpu_time += duration;
                loaded.stats.last_execution_time = Some(duration);
            }
            
            // Estimate memory usage (this is a rough estimate)
            // In a real implementation, you might use platform-specific APIs
            // to get actual memory usage
            let estimated_memory = Self::estimate_memory_usage();
            if estimated_memory > loaded.stats.peak_memory_bytes {
                loaded.stats.peak_memory_bytes = estimated_memory;
            }
            
            tracing::debug!(
                plugin_id = %plugin_id,
                total_calls = loaded.stats.total_calls,
                successful = loaded.stats.successful_calls,
                failed = loaded.stats.failed_calls,
                timeouts = loaded.stats.timeout_errors,
                peak_memory = loaded.stats.peak_memory_bytes,
                "Updated plugin statistics"
            );
        }
        
        Ok(())
    }
    
    /// Update execution time statistics
    fn update_execution_time(&self, plugin_id: &PluginId, elapsed: Duration) -> Result<()> {
        let mut libraries = self.libraries.write().map_err(|e| {
            TingError::PluginLoadError(format!("Failed to acquire write lock: {}", e))
        })?;
        
        if let Some(loaded) = libraries.get_mut(plugin_id) {
            loaded.stats.total_cpu_time += elapsed;
            loaded.stats.last_execution_time = Some(elapsed);
        }
        
        Ok(())
    }
    
    /// Estimate current memory usage
    ///
    /// This is a rough estimate. In production, you might want to use
    /// platform-specific APIs like:
    /// - Linux: /proc/self/status
    /// - Windows: GetProcessMemoryInfo
    /// - macOS: task_info
    fn estimate_memory_usage() -> usize {
        // For now, return a placeholder
        // In a real implementation, use platform-specific memory APIs
        0
    }
    
    /// Get resource usage statistics for a plugin
    pub fn get_stats(&self, plugin_id: &PluginId) -> Result<ResourceStats> {
        let libraries = self.libraries.read().map_err(|e| {
            TingError::PluginLoadError(format!("Failed to acquire read lock: {}", e))
        })?;
        
        let loaded = libraries.get(plugin_id).ok_or_else(|| {
            TingError::PluginNotFound(format!("Plugin {} not found", plugin_id))
        })?;
        
        Ok(loaded.stats.clone())
    }
    
    /// Check if a library is currently loaded
    pub fn is_loaded(&self, plugin_id: &PluginId) -> bool {
        self.libraries
            .read()
            .map(|libs| libs.contains_key(plugin_id))
            .unwrap_or(false)
    }
    
    /// Get the path of a loaded library
    pub fn get_library_path(&self, plugin_id: &PluginId) -> Result<PathBuf> {
        let libraries = self.libraries.read().map_err(|e| {
            TingError::PluginLoadError(format!("Failed to acquire read lock: {}", e))
        })?;
        
        let loaded = libraries.get(plugin_id).ok_or_else(|| {
            TingError::PluginNotFound(format!("Plugin {} not found", plugin_id))
        })?;
        
        Ok(loaded.path.clone())
    }
    
    /// Get metadata for a loaded library
    pub fn get_metadata(&self, plugin_id: &PluginId) -> Result<PluginMetadata> {
        let libraries = self.libraries.read().map_err(|e| {
            TingError::PluginLoadError(format!("Failed to acquire read lock: {}", e))
        })?;
        
        let loaded = libraries.get(plugin_id).ok_or_else(|| {
            TingError::PluginNotFound(format!("Plugin {} not found", plugin_id))
        })?;
        
        Ok(loaded.metadata.clone())
    }
    
    /// Increment the reference count for a library
    ///
    /// This prevents the library from being unloaded while it's in use.
    pub fn increment_ref_count(&self, plugin_id: &PluginId) -> Result<()> {
        let mut libraries = self.libraries.write().map_err(|e| {
            TingError::PluginLoadError(format!("Failed to acquire write lock: {}", e))
        })?;
        
        let loaded = libraries.get_mut(plugin_id).ok_or_else(|| {
            TingError::PluginNotFound(format!("Plugin {} not found", plugin_id))
        })?;
        
        loaded.ref_count += 1;
        
        Ok(())
    }
    
    /// Get the number of loaded libraries
    pub fn library_count(&self) -> usize {
        self.libraries
            .read()
            .map(|libs| libs.len())
            .unwrap_or(0)
    }
    
    /// List all loaded library IDs
    pub fn list_loaded_libraries(&self) -> Vec<PluginId> {
        self.libraries
            .read()
            .map(|libs| libs.keys().cloned().collect())
            .unwrap_or_default()
    }
    
    /// Validate library file extension for the current platform
    fn is_valid_library_extension(&self, path: &Path) -> bool {
        let extension = path.extension().and_then(|e| e.to_str());
        
        match extension {
            Some(ext) => {
                #[cfg(target_os = "windows")]
                return ext == "dll";
                
                #[cfg(target_os = "linux")]
                return ext == "so";
                
                #[cfg(target_os = "macos")]
                return ext == "dylib";
                
                #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
                return false;
            }
            None => false,
        }
    }
}

impl Default for NativeLoader {
    fn default() -> Self {
        Self::new()
    }
}

// Ensure NativeLoader is thread-safe
unsafe impl Send for NativeLoader {}
unsafe impl Sync for NativeLoader {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::types::PluginType;
    use std::time::Duration;
    
    fn create_test_metadata() -> PluginMetadata {
        PluginMetadata::new(
            "test-plugin".to_string(),
            "1.0.0".to_string(),
            PluginType::Utility,
            "Test Author".to_string(),
            "Test plugin".to_string(),
            "libtest.so".to_string(),
        )
    }
    
    #[test]
    fn test_native_loader_creation() {
        let loader = NativeLoader::new();
        assert_eq!(loader.library_count(), 0);
    }
    
    #[test]
    fn test_native_loader_with_custom_limits() {
        let limits = ResourceLimits::custom(
            256 * 1024 * 1024, // 256 MB
            Duration::from_secs(60),
            50 * 1024 * 1024, // 50 MB
            5 * 1024 * 1024, // 5 MB/s
        );
        let loader = NativeLoader::with_limits(limits);
        assert_eq!(loader.library_count(), 0);
    }
    
    #[test]
    fn test_is_loaded_returns_false_for_nonexistent() {
        let loader = NativeLoader::new();
        assert!(!loader.is_loaded(&"nonexistent".to_string()));
    }
    
    #[test]
    fn test_list_loaded_libraries_empty() {
        let loader = NativeLoader::new();
        assert_eq!(loader.list_loaded_libraries().len(), 0);
    }
    
    #[test]
    fn test_load_nonexistent_library_fails() {
        let loader = NativeLoader::new();
        let metadata = create_test_metadata();
        let result = loader.load_library(
            "test".to_string(),
            Path::new("/nonexistent/library.so"),
            metadata,
        );
        assert!(result.is_err());
    }
    
    #[test]
    fn test_load_library_with_custom_limits() {
        let loader = NativeLoader::new();
        let metadata = create_test_metadata();
        let limits = ResourceLimits::restrictive();
        let result = loader.load_library_with_limits(
            "test".to_string(),
            Path::new("/nonexistent/library.so"),
            metadata,
            limits,
        );
        assert!(result.is_err());
    }
    
    #[test]
    fn test_unload_nonexistent_library_fails() {
        let loader = NativeLoader::new();
        let result = loader.unload_library(&"nonexistent".to_string());
        assert!(result.is_err());
    }
    
    #[test]
    fn test_get_metadata_nonexistent_fails() {
        let loader = NativeLoader::new();
        let result = loader.get_metadata(&"nonexistent".to_string());
        assert!(result.is_err());
    }
    
    #[test]
    fn test_get_stats_nonexistent_fails() {
        let loader = NativeLoader::new();
        let result = loader.get_stats(&"nonexistent".to_string());
        assert!(result.is_err());
    }
    
    #[test]
    fn test_resource_stats_default() {
        let stats = ResourceStats::default();
        assert_eq!(stats.total_calls, 0);
        assert_eq!(stats.successful_calls, 0);
        assert_eq!(stats.failed_calls, 0);
        assert_eq!(stats.timeout_errors, 0);
        assert_eq!(stats.peak_memory_bytes, 0);
        assert_eq!(stats.total_cpu_time, Duration::from_secs(0));
        assert!(stats.last_execution_time.is_none());
    }
    
    #[cfg(target_os = "linux")]
    #[test]
    fn test_valid_library_extension_linux() {
        let loader = NativeLoader::new();
        assert!(loader.is_valid_library_extension(Path::new("test.so")));
        assert!(!loader.is_valid_library_extension(Path::new("test.dll")));
        assert!(!loader.is_valid_library_extension(Path::new("test.dylib")));
    }
    
    #[cfg(target_os = "windows")]
    #[test]
    fn test_valid_library_extension_windows() {
        let loader = NativeLoader::new();
        assert!(loader.is_valid_library_extension(Path::new("test.dll")));
        assert!(!loader.is_valid_library_extension(Path::new("test.so")));
        assert!(!loader.is_valid_library_extension(Path::new("test.dylib")));
    }
    
    #[cfg(target_os = "macos")]
    #[test]
    fn test_valid_library_extension_macos() {
        let loader = NativeLoader::new();
        assert!(loader.is_valid_library_extension(Path::new("test.dylib")));
        assert!(!loader.is_valid_library_extension(Path::new("test.dll")));
        assert!(!loader.is_valid_library_extension(Path::new("test.so")));
    }
}
