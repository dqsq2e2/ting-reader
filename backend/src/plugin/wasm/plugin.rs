use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use wasmtime::*;
use wasmtime_wasi::preview2::{WasiCtx, WasiView, ResourceTable};
use wasmtime_wasi::preview2::preview1::{WasiPreview1View, WasiPreview1Adapter};
use crate::core::error::{Result, TingError};
use super::runtime::WasmRuntime;
use crate::plugin::types::{PluginId, PluginMetadata, Plugin, PluginContext};
use crate::plugin::scraper::{ScraperPlugin, SearchResult, BookDetail, Chapter};

impl Default for WasmRuntime {
    fn default() -> Self {
        Self::new().expect("Failed to create default WASM runtime")
    }
}

/// WASM plugin instance
/// 
/// Represents a loaded and instantiated WASM plugin with its execution context.
pub struct WasmPlugin {
    pub(crate) plugin_id: PluginId,
    
    /// Inner state protected by mutex for concurrent access
    pub(crate) inner: Arc<tokio::sync::Mutex<WasmPluginInner>>,
    
    /// Plugin metadata
    pub(crate) metadata: Option<PluginMetadata>,
}

/// Inner state of WASM plugin
pub(crate) struct WasmPluginInner {
    /// WASM instance
    pub(crate) instance: Instance,
    
    /// WASM store (execution context)
    pub(crate) store: Store<PluginState>,
    
    /// Exported functions from the WASM module
    pub(crate) exports: WasmExports,
    
    /// Original module (for re-instantiation if needed)
    pub(crate) _module: Module,
}

impl WasmPlugin {
    /// Call a WASM function by name with arguments
    /// 
    /// # Arguments
    /// * `function_name` - Name of the exported function to call
    /// * `args` - Arguments to pass to the function
    /// 
    /// # Returns
    /// The result value from the function call
    pub async fn call(&self, function_name: &str, args: &[Val]) -> Result<Vec<Val>> {
        let start_time = Instant::now();
        let mut inner = self.inner.lock().await;
        let instance = inner.instance;
        
        // Get the function from exports
        let func = instance
            .get_func(&mut inner.store, function_name)
            .ok_or_else(|| TingError::PluginExecutionError(format!("Function '{}' not found", function_name)))?;
        
        // Prepare result buffer
        let mut results = vec![Val::I32(0); func.ty(&inner.store).results().len()];
        
        // Call the function with timeout
        let call_result = tokio::time::timeout(
            Duration::from_secs(300), // 5 minutes default timeout
            func.call_async(&mut inner.store, args, &mut results)
        ).await;
        
        // Clean up any lingering HTTP responses to prevent memory leaks
        if !inner.store.data().http_responses.is_empty() {
            let count = inner.store.data().http_responses.len();
            tracing::warn!(
                plugin_id = %self.plugin_id,
                function = function_name,
                count = count,
                "Cleaning up leaked HTTP responses after function call"
            );
            inner.store.data_mut().http_responses.clear();
        }

        match call_result {
            Ok(Ok(())) => {
                let elapsed = start_time.elapsed();
                tracing::debug!(
                    plugin_id = %self.plugin_id,
                    function = function_name,
                    elapsed_ms = elapsed.as_millis(),
                    "WASM function call completed"
                );
                Ok(results)
            }
            Ok(Err(e)) => {
                Err(TingError::PluginExecutionError(format!("WASM function call failed: {}", e)))
            }
            Err(_) => {
                Err(TingError::Timeout(format!("WASM function call timed out: {}", function_name)))
            }
        }
    }
    
    /// Call the initialize function
    pub async fn initialize_wasm(&self) -> Result<i32> {
        let mut inner = self.inner.lock().await;
        let exports = inner.exports.initialize;
        let results = exports.call_async(&mut inner.store, ()).await
            .map_err(|e| TingError::PluginExecutionError(format!("Initialize failed: {}", e)))?;
        Ok(results)
    }
    
    /// Call the shutdown function
    pub async fn shutdown_wasm(&self) -> Result<i32> {
        let mut inner = self.inner.lock().await;
        let exports = inner.exports.shutdown;
        let results = exports.call_async(&mut inner.store, ()).await
            .map_err(|e| TingError::PluginExecutionError(format!("Shutdown failed: {}", e)))?;
        Ok(results)
    }
    
    /// Call the invoke function with method and parameters
    /// 
    /// # Arguments
    /// * `method_ptr` - Pointer to method name in WASM memory
    /// * `params_ptr` - Pointer to parameters JSON in WASM memory
    /// 
    /// # Returns
    /// Pointer to result JSON in WASM memory
    pub async fn invoke(&self, method_ptr: i32, params_ptr: i32) -> Result<i32> {
        let mut inner = self.inner.lock().await;
        let exports = inner.exports.invoke;
        let results = exports.call_async(&mut inner.store, (method_ptr, params_ptr)).await
            .map_err(|e| TingError::PluginExecutionError(format!("Invoke failed: {}", e)))?;
        Ok(results)
    }
    
    /// Get access to the WASM memory
    /// 
    /// # Returns
    /// Reference to the WASM linear memory
    pub async fn memory(&self) -> Result<Memory> {
        // This is tricky because Memory belongs to Store which is locked.
        // We can't return Memory without keeping the lock.
        // So we should expose helper methods instead of returning Memory directly.
        Err(TingError::PluginExecutionError("Cannot access memory directly, use helper methods".to_string()))
    }
    
    /// Read data from WASM memory
    /// 
    /// # Arguments
    /// * `ptr` - Pointer to data in WASM memory
    /// * `len` - Length of data to read
    /// 
    /// # Returns
    /// Vector of bytes read from memory
    pub async fn read_memory(&self, ptr: usize, len: usize) -> Result<Vec<u8>> {
        let mut inner = self.inner.lock().await;
        let instance = inner.instance;
        let memory = instance
            .get_memory(&mut inner.store, "memory")
            .ok_or_else(|| TingError::PluginExecutionError("Memory export not found".to_string()))?;
            
        let mut buffer = vec![0u8; len];
        memory.read(&inner.store, ptr, &mut buffer)
            .map_err(|e| TingError::PluginExecutionError(format!("Failed to read memory: {}", e)))?;
        Ok(buffer)
    }
    
    /// Write data to WASM memory
    /// 
    /// # Arguments
    /// * `ptr` - Pointer to location in WASM memory
    /// * `data` - Data to write
    pub async fn write_memory(&self, ptr: usize, data: &[u8]) -> Result<()> {
        let mut inner = self.inner.lock().await;
        let instance = inner.instance;
        let memory = instance
            .get_memory(&mut inner.store, "memory")
            .ok_or_else(|| TingError::PluginExecutionError("Memory export not found".to_string()))?;
            
        memory.write(&mut inner.store, ptr, data)
            .map_err(|e| TingError::PluginExecutionError(format!("Failed to write memory: {}", e)))?;
        Ok(())
    }
    
    /// Get the current memory usage in bytes
    pub async fn memory_usage(&self) -> usize {
        let mut inner = self.inner.lock().await;
        let instance = inner.instance;
        if let Some(memory) = instance.get_memory(&mut inner.store, "memory") {
            memory.data_size(&inner.store)
        } else {
            0
        }
    }
    
    /// Get the plugin ID
    pub fn plugin_id(&self) -> &PluginId {
        &self.plugin_id
    }

    /// Allocate memory in WASM
    async fn alloc(&self, len: usize) -> Result<i32> {
        // We need to call the exported 'alloc' function
        // But since we don't have it in WasmExports (it's custom), we need to look it up dynamically
        let mut inner = self.inner.lock().await;
        let instance = inner.instance;
        let func = instance.get_func(&mut inner.store, "alloc")
            .ok_or_else(|| TingError::PluginExecutionError("Function 'alloc' not found".to_string()))?;
            
        let mut results = vec![Val::I32(0)];
        func.call_async(&mut inner.store, &[Val::I32(len as i32)], &mut results).await
            .map_err(|e| TingError::PluginExecutionError(format!("Alloc failed: {}", e)))?;
            
        match results[0] {
            Val::I32(ptr) => Ok(ptr),
            _ => Err(TingError::PluginExecutionError("Alloc returned non-i32".to_string())),
        }
    }

    /// Write string to WASM memory
    async fn write_string(&self, s: &str) -> Result<i32> {
        let bytes = s.as_bytes();
        // Allocate space for string + null terminator
        let ptr = self.alloc(bytes.len() + 1).await?;
        
        // Write bytes
        self.write_memory(ptr as usize, bytes).await?;
        // Write null terminator
        self.write_memory(ptr as usize + bytes.len(), &[0]).await?;
        
        Ok(ptr)
    }

    /// Write method and params to WASM memory
    async fn write_args(&self, method: &str, params: &str) -> Result<(i32, i32)> {
        let method_ptr = self.write_string(method).await?;
        let params_ptr = self.write_string(params).await?;
        Ok((method_ptr, params_ptr))
    }

    /// Read C-string from WASM memory
    async fn read_string(&self, ptr: i32) -> Result<String> {
        // Read until null terminator
        let mut bytes = Vec::new();
        let mut offset = 0;
        loop {
            let chunk = self.read_memory(ptr as usize + offset, 1).await?;
            if chunk[0] == 0 {
                break;
            }
            bytes.push(chunk[0]);
            offset += 1;
        }
        
        String::from_utf8(bytes)
            .map_err(|e| TingError::PluginExecutionError(format!("Invalid UTF-8 string: {}", e)))
    }
}

#[async_trait::async_trait]
impl Plugin for WasmPlugin {
    fn metadata(&self) -> &PluginMetadata {
        self.metadata.as_ref().expect("Metadata should be set for instantiated plugin")
    }
    
    async fn initialize(&self, _context: &PluginContext) -> Result<()> {
        let res = self.initialize_wasm().await?;
        if res != 0 {
            return Err(TingError::PluginExecutionError(format!("Initialize returned error code: {}", res)));
        }
        Ok(())
    }
    
    async fn shutdown(&self) -> Result<()> {
        let res = self.shutdown_wasm().await?;
        if res != 0 {
            return Err(TingError::PluginExecutionError(format!("Shutdown returned error code: {}", res)));
        }
        Ok(())
    }
    
    async fn garbage_collect(&self) -> Result<()> {
        let mut inner = self.inner.lock().await;
        inner.store.gc();
        Ok(())
    }

    fn plugin_type(&self) -> crate::plugin::types::PluginType {
        self.metadata().plugin_type
    }
    
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[async_trait::async_trait]
impl ScraperPlugin for WasmPlugin {
    async fn search(&self, query: &str, author: Option<&str>, narrator: Option<&str>, page: u32) -> Result<SearchResult> {
        let params = serde_json::json!({ 
            "query": query, 
            "author": author,
            "narrator": narrator,
            "page": page 
        }).to_string();
        let (method_ptr, params_ptr) = self.write_args("search", &params).await?;
        let result_ptr = self.invoke(method_ptr, params_ptr).await?;
        let result_json = self.read_string(result_ptr).await?;
        
        // Handle error response from WASM
        if let Ok(err_obj) = serde_json::from_str::<serde_json::Value>(&result_json) {
            if let Some(err_msg) = err_obj.get("error").and_then(|v| v.as_str()) {
                return Err(TingError::PluginExecutionError(format!("WASM error: {}", err_msg)));
            }
        }
        
        serde_json::from_str(&result_json)
            .map_err(|e| TingError::PluginExecutionError(format!("Invalid search result: {}", e)))
    }
    
    async fn get_detail(&self, book_id: &str) -> Result<BookDetail> {
        let params = serde_json::json!({ "id": book_id }).to_string();
        let (method_ptr, params_ptr) = self.write_args("get_detail", &params).await?;
        let result_ptr = self.invoke(method_ptr, params_ptr).await?;
        let result_json = self.read_string(result_ptr).await?;
        
        if let Ok(err_obj) = serde_json::from_str::<serde_json::Value>(&result_json) {
            if let Some(err_msg) = err_obj.get("error").and_then(|v| v.as_str()) {
                return Err(TingError::PluginExecutionError(format!("WASM error: {}", err_msg)));
            }
        }
        
        serde_json::from_str(&result_json)
            .map_err(|e| TingError::PluginExecutionError(format!("Invalid book detail: {}", e)))
    }
    
    async fn get_chapters(&self, book_id: &str) -> Result<Vec<Chapter>> {
        let params = serde_json::json!({ "id": book_id }).to_string();
        let (method_ptr, params_ptr) = self.write_args("get_chapters", &params).await?;
        let result_ptr = self.invoke(method_ptr, params_ptr).await?;
        let result_json = self.read_string(result_ptr).await?;
        
        if let Ok(err_obj) = serde_json::from_str::<serde_json::Value>(&result_json) {
            if let Some(err_msg) = err_obj.get("error").and_then(|v| v.as_str()) {
                return Err(TingError::PluginExecutionError(format!("WASM error: {}", err_msg)));
            }
        }
        
        serde_json::from_str(&result_json)
            .map_err(|e| TingError::PluginExecutionError(format!("Invalid chapters: {}", e)))
    }
    
    async fn download_cover(&self, url: &str) -> Result<Vec<u8>> {
        let params = serde_json::json!({ "url": url }).to_string();
        let (method_ptr, params_ptr) = self.write_args("download_cover", &params).await?;
        let result_ptr = self.invoke(method_ptr, params_ptr).await?;
        let result_json = self.read_string(result_ptr).await?;
        
        let wrapper: serde_json::Value = serde_json::from_str(&result_json)
            .map_err(|e| TingError::PluginExecutionError(format!("Invalid JSON: {}", e)))?;
            
        if let Some(err_msg) = wrapper.get("error").and_then(|v| v.as_str()) {
            return Err(TingError::PluginExecutionError(format!("WASM error: {}", err_msg)));
        }
        
        if let Some(data_str) = wrapper.get("data").and_then(|v| v.as_str()) {
             use base64::Engine;
             base64::engine::general_purpose::STANDARD.decode(data_str)
                 .map_err(|e| TingError::PluginExecutionError(format!("Invalid base64 cover: {}", e)))
        } else {
             Err(TingError::PluginExecutionError("Invalid cover response".to_string()))
        }
    }
    
    async fn get_audio_url(&self, chapter_id: &str) -> Result<String> {
        let params = serde_json::json!({ "id": chapter_id }).to_string();
        let (method_ptr, params_ptr) = self.write_args("get_audio_url", &params).await?;
        let result_ptr = self.invoke(method_ptr, params_ptr).await?;
        let result_json = self.read_string(result_ptr).await?;
        
        let wrapper: serde_json::Value = serde_json::from_str(&result_json)
            .map_err(|e| TingError::PluginExecutionError(format!("Invalid JSON: {}", e)))?;
            
        if let Some(err_msg) = wrapper.get("error").and_then(|v| v.as_str()) {
            return Err(TingError::PluginExecutionError(format!("WASM error: {}", err_msg)));
        }
        
        wrapper.get("url").and_then(|v| v.as_str())
             .map(|s| s.to_string())
             .ok_or_else(|| TingError::PluginExecutionError("Invalid audio url response".to_string()))
    }
}

/// Exported functions from a WASM module
/// 
/// Contains typed references to the standard plugin interface functions.
pub(crate) struct WasmExports {
    /// Initialize function: () -> i32
    pub initialize: TypedFunc<(), i32>,
    
    /// Shutdown function: () -> i32
    pub shutdown: TypedFunc<(), i32>,
    
    /// Invoke function: (method_ptr: i32, params_ptr: i32) -> i32
    pub invoke: TypedFunc<(i32, i32), i32>,
}

impl WasmExports {
    /// Extract exported functions from a WASM instance
    /// 
    /// # Arguments
    /// * `instance` - The WASM instance
    /// * `store` - The WASM store
    /// 
    /// # Returns
    /// WasmExports with typed function references
    pub fn from_instance(instance: &Instance, store: &mut Store<PluginState>) -> Result<Self> {
        let initialize = instance
            .get_typed_func::<(), i32>(&mut *store, "initialize")
            .map_err(|e| TingError::PluginLoadError(format!("Failed to get 'initialize' function: {}", e)))?;
        
        let shutdown = instance
            .get_typed_func::<(), i32>(&mut *store, "shutdown")
            .map_err(|e| TingError::PluginLoadError(format!("Failed to get 'shutdown' function: {}", e)))?;
        
        let invoke = instance
            .get_typed_func::<(i32, i32), i32>(&mut *store, "invoke")
            .map_err(|e| TingError::PluginLoadError(format!("Failed to get 'invoke' function: {}", e)))?;
        
        Ok(Self {
            initialize,
            shutdown,
            invoke,
        })
    }
}

/// Plugin state stored in the WASM store
/// 
/// Contains the execution context and resource limiter for the plugin.
pub struct PluginState {
    /// WASI context for system interface
    pub(crate) wasi: WasiCtx,
    
    /// Resource table for managing handles
    pub(crate) table: ResourceTable,
    
    /// WASI Preview 1 adapter for compatibility
    pub(crate) adapter: WasiPreview1Adapter,
    
    /// HTTP Responses storage for simple host function
    pub(crate) http_responses: HashMap<u32, Vec<u8>>,
    
    /// Resource limiter for memory and compute
    pub(crate) limiter: StoreLimits,
}

impl PluginState {
    /// Create a new plugin state with default limits
    pub fn new() -> Self {
        let mut builder = wasmtime_wasi::preview2::WasiCtxBuilder::new();
        builder.inherit_stdio();
        
        Self {
            wasi: builder.build(),
            table: ResourceTable::new(),
            adapter: WasiPreview1Adapter::new(),
            http_responses: HashMap::new(),
            limiter: StoreLimits::default(),
        }
    }
    
    /// Create a plugin state with custom limits
    pub fn with_limits(memory_limit: usize) -> Self {
        let mut builder = wasmtime_wasi::preview2::WasiCtxBuilder::new();
        builder.inherit_stdio();

        Self {
            wasi: builder.build(),
            table: ResourceTable::new(),
            adapter: WasiPreview1Adapter::new(),
            http_responses: HashMap::new(),
            limiter: StoreLimits::new(memory_limit),
        }
    }
}

impl WasiView for PluginState {
    fn table(&mut self) -> &mut ResourceTable { &mut self.table }
    fn ctx(&mut self) -> &mut WasiCtx { &mut self.wasi }
}

impl WasiPreview1View for PluginState {
    fn adapter(&self) -> &WasiPreview1Adapter { &self.adapter }
    fn adapter_mut(&mut self) -> &mut WasiPreview1Adapter { &mut self.adapter }
}

// impl WasiHttpView for PluginState {
//     fn ctx(&mut self) -> &mut WasiHttpCtx { &mut self.http }
//     fn table(&mut self) -> &mut ResourceTable { &mut self.table }
// }

impl Default for PluginState {
    fn default() -> Self {
        Self::new()
    }
}

/// Store resource limits
/// 
/// Implements ResourceLimiter to enforce memory and compute limits on WASM execution.
pub struct StoreLimits {
    /// Maximum memory in bytes
    max_memory_bytes: usize,
    
    /// Current memory usage
    current_memory_bytes: usize,
}

impl StoreLimits {
    /// Create new store limits with specified maximum memory
    pub fn new(max_memory_bytes: usize) -> Self {
        Self {
            max_memory_bytes,
            current_memory_bytes: 0,
        }
    }
    
    /// Get current memory usage
    pub fn current_memory(&self) -> usize {
        self.current_memory_bytes
    }
}

impl Default for StoreLimits {
    fn default() -> Self {
        Self::new(512 * 1024 * 1024) // 512 MB default
    }
}

impl ResourceLimiter for StoreLimits {
    fn memory_growing(&mut self, current: usize, desired: usize, _maximum: Option<usize>) -> std::result::Result<bool, anyhow::Error> {
        let delta = desired.saturating_sub(current);
        let new_total = self.current_memory_bytes.saturating_add(delta);
        
        if new_total <= self.max_memory_bytes {
            self.current_memory_bytes = new_total;
            Ok(true)
        } else {
            tracing::warn!(
                current = current,
                desired = desired,
                limit = self.max_memory_bytes,
                "Memory limit exceeded"
            );
            Ok(false)
        }
    }
    
    fn table_growing(&mut self, _current: u32, _desired: u32, _maximum: Option<u32>) -> std::result::Result<bool, anyhow::Error> {
        // Allow table growth (could add limits here if needed)
        Ok(true)
    }
}

