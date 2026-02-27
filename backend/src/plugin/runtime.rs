//! WASM runtime implementation
//!
//! This module provides the WebAssembly runtime for loading and executing WASM plugins.
//! It uses wasmtime as the WASM engine and provides sandboxed execution with resource limits.

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};
use wasmtime::*;
use wasmtime_wasi::preview2::{WasiCtx, WasiView, ResourceTable};
use wasmtime_wasi::preview2::preview1::{WasiPreview1View, WasiPreview1Adapter};
use crate::core::error::{Result, TingError};
use crate::plugin::sandbox::{Sandbox, ResourceLimits, Permission};
use crate::plugin::types::{PluginId, PluginMetadata, Plugin, PluginContext};
use crate::plugin::scraper::{ScraperPlugin, SearchResult, BookDetail, Chapter};

/// WASM runtime for loading and executing WASM plugins
/// 
/// Manages the wasmtime engine and provides methods for loading WASM modules,
/// creating instances, and executing WASM functions with sandboxing.
pub struct WasmRuntime {
    /// Wasmtime engine instance
    engine: Engine,
    
    /// Active sandboxes for each plugin
    sandboxes: Arc<RwLock<HashMap<PluginId, Sandbox>>>,
}

impl WasmRuntime {
    /// Create a new WASM runtime with default configuration
    pub fn new() -> Result<Self> {
        let mut config = Config::new();
        
        // Enable WASI support for system interface
        config.wasm_backtrace_details(WasmBacktraceDetails::Enable);
        config.wasm_multi_memory(true);
        config.async_support(true);
        
        // Enable component model for WASI HTTP if needed in future
        // config.wasm_component_model(true);
        
        // Create engine with configuration
        let engine = Engine::new(&config)
            .map_err(|e| TingError::PluginExecutionError(format!("Failed to create WASM engine: {}", e)))?;
        
        Ok(Self {
            engine,
            sandboxes: Arc::new(RwLock::new(HashMap::new())),
        })
    }
    
    /// Load a WASM module from bytes
    /// 
    /// # Arguments
    /// * `wasm_bytes` - The WASM binary data
    /// 
    /// # Returns
    /// A compiled WASM module ready for instantiation
    pub async fn load_module(&self, wasm_bytes: &[u8]) -> Result<Module> {
        Module::from_binary(&self.engine, wasm_bytes)
            .map_err(|e| TingError::PluginLoadError(format!("Failed to load WASM module: {}", e)))
    }
    
    /// Load a WASM module from a file
    /// 
    /// # Arguments
    /// * `path` - Path to the WASM file
    /// 
    /// # Returns
    /// A compiled WASM module ready for instantiation
    pub async fn load_module_from_file(&self, path: &Path) -> Result<Module> {
        Module::from_file(&self.engine, path)
            .map_err(|e| TingError::PluginLoadError(format!("Failed to load WASM module from file: {}", e)))
    }
    
    /// Instantiate a WASM module with sandboxing
    /// 
    /// # Arguments
    /// * `module` - The compiled WASM module
    /// * `metadata` - Plugin metadata containing permissions
    /// 
    /// # Returns
    /// A WasmPlugin instance ready for execution
    pub async fn instantiate(
        &self,
        module: Module,
        metadata: &PluginMetadata,
    ) -> Result<WasmPlugin> {
        // Create sandbox with permissions from metadata
        let sandbox = Sandbox::new(
            metadata.permissions.clone(),
            ResourceLimits::default(),
        );
        
        // Create WASI context with network support
        let mut wasi_builder = wasmtime_wasi::preview2::WasiCtxBuilder::new();
        wasi_builder
            .inherit_stdio()
            .inherit_network()
            .allow_ip_name_lookup(true);
            
        let wasi = wasi_builder.build();
        let table = ResourceTable::new();
        let adapter = WasiPreview1Adapter::new();
        
        // Create plugin state
        let state = PluginState {
            wasi,
            table,
            adapter,
            http_responses: HashMap::new(),
            limiter: StoreLimits::default(),
        };
        
        // Create store with resource limits
        let mut store = Store::new(&self.engine, state);
        
        // Set resource limits on the store
        store.limiter(|state| &mut state.limiter);
        
        // Create linker for imports
        let mut linker = Linker::new(&self.engine);
        
        // Add WASI support (Preview 1 adapter)
        wasmtime_wasi::preview2::preview1::add_to_linker_sync(&mut linker)
            .map_err(|e| TingError::PluginExecutionError(format!("Failed to add WASI to linker: {}", e)))?;
            
        // Add WASI HTTP support (Preview 2)
        // wasmtime_wasi_http::proxy::add_to_linker(&mut linker, |state: &mut PluginState| state)
        //    .map_err(|e| TingError::PluginExecutionError(format!("Failed to add WASI HTTP to linker: {}", e)))?;
        
        // Custom HTTP Host Function
         // ting_http_request(url_ptr, url_len) -> handle (>0) or error (<0)
         linker.func_wrap("ting_env", "http_request", |mut caller: Caller<'_, PluginState>, url_ptr: i32, url_len: i32| -> i32 {
             // 1. Read URL
             let mem = match caller.get_export("memory") {
                 Some(Extern::Memory(mem)) => mem,
                 _ => return -1,
             };
             let ctx = caller.as_context();
             let data = mem.data(&ctx);
             let url = match std::str::from_utf8(&data[url_ptr as usize..(url_ptr + url_len) as usize]) {
                 Ok(s) => s,
                 Err(_) => return -2,
             };
             
             tracing::info!("Plugin requesting URL: {}", url);
             
             // 2. Perform Request (using reqwest blocking in host)
             // We create a new client for each request for simplicity
             let client = match reqwest::blocking::Client::builder()
                 .user_agent("TingReader/1.0")
                 .timeout(Duration::from_secs(30))
                 .build() {
                     Ok(c) => c,
                     Err(_) => return -3,
                 };
                 
             let resp = match client.get(url).send() {
                 Ok(r) => r,
                 Err(_) => return -4,
             };
             
             if !resp.status().is_success() {
                 return -(resp.status().as_u16() as i32);
             }
             
             let body = match resp.bytes() {
                 Ok(b) => b.to_vec(),
                 Err(_) => return -5,
             };
             
             if let Ok(body_str) = std::str::from_utf8(&body) {
                 tracing::info!("Plugin received response (len={}): {:.200}...", body.len(), body_str);
             }
             
             // 3. Store response
             let handle = (caller.data().http_responses.len() as u32) + 1;
             caller.data_mut().http_responses.insert(handle, body);
             
             handle as i32
         }).map_err(|e| TingError::PluginExecutionError(format!("Failed to define http_request: {}", e)))?;

         // ting_http_response_size(handle) -> size
         linker.func_wrap("ting_env", "http_response_size", |caller: Caller<'_, PluginState>, handle: i32| -> i32 {
             if let Some(body) = caller.data().http_responses.get(&(handle as u32)) {
                 body.len() as i32
             } else {
                 -1
             }
         }).map_err(|e| TingError::PluginExecutionError(format!("Failed to define http_response_size: {}", e)))?;

         // ting_http_read_body(handle, ptr, len) -> bytes_read
         linker.func_wrap("ting_env", "http_read_body", |mut caller: Caller<'_, PluginState>, handle: i32, ptr: i32, len: i32| -> i32 {
             let body = if let Some(b) = caller.data().http_responses.get(&(handle as u32)) {
                 b.clone()
             } else {
                 return -1;
             };
             
             let copy_len = std::cmp::min(body.len(), len as usize);
             
             let mem = match caller.get_export("memory") {
                 Some(Extern::Memory(mem)) => mem,
                 _ => return -2,
             };
             
             if let Err(_) = mem.write(&mut caller, ptr as usize, &body[..copy_len]) {
                 return -3;
             }
             
             // Clean up after reading (one-time read)
             caller.data_mut().http_responses.remove(&(handle as u32));
             
             copy_len as i32
         }).map_err(|e| TingError::PluginExecutionError(format!("Failed to define http_read_body: {}", e)))?;
        
        // Instantiate the module
        let instance = linker
            .instantiate_async(&mut store, &module)
            .await
            .map_err(|e| TingError::PluginExecutionError(format!("Failed to instantiate WASM module: {}", e)))?;
        
        // Extract exported functions
        let exports = WasmExports::from_instance(&instance, &mut store)?;
        
        // Store sandbox for this plugin
        let plugin_id = metadata.name.clone();
        self.sandboxes.write().unwrap().insert(plugin_id.clone(), sandbox);
        
        let inner = WasmPluginInner {
            instance,
            store,
            exports,
            _module: module,
        };
        
        Ok(WasmPlugin {
            plugin_id,
            inner: Arc::new(tokio::sync::Mutex::new(inner)),
            metadata: Some(metadata.clone()),
        })
    }
    
    /// Create a sandbox with specific permissions and limits
    /// 
    /// # Arguments
    /// * `permissions` - List of permissions to grant
    /// * `limits` - Resource limits to enforce
    /// 
    /// # Returns
    /// A configured Sandbox instance
    pub fn create_sandbox(&self, permissions: Vec<Permission>, limits: ResourceLimits) -> Result<Sandbox> {
        Ok(Sandbox::new(permissions, limits))
    }
    
    /// Get the sandbox for a specific plugin
    pub fn get_sandbox(&self, plugin_id: &PluginId) -> Option<Sandbox> {
        self.sandboxes.read().unwrap().get(plugin_id).cloned()
    }
    
    /// Remove the sandbox for a plugin (called during unload)
    pub fn remove_sandbox(&self, plugin_id: &PluginId) {
        self.sandboxes.write().unwrap().remove(plugin_id);
    }
}

impl Default for WasmRuntime {
    fn default() -> Self {
        Self::new().expect("Failed to create default WASM runtime")
    }
}

/// WASM plugin instance
/// 
/// Represents a loaded and instantiated WASM plugin with its execution context.
pub struct WasmPlugin {
    /// Plugin identifier
    plugin_id: PluginId,
    
    /// Inner state protected by mutex for concurrent access
    inner: Arc<tokio::sync::Mutex<WasmPluginInner>>,
    
    /// Plugin metadata
    metadata: Option<PluginMetadata>,
}

/// Inner state of WASM plugin
struct WasmPluginInner {
    /// WASM instance
    instance: Instance,
    
    /// WASM store (execution context)
    store: Store<PluginState>,
    
    /// Exported functions from the WASM module
    exports: WasmExports,
    
    /// Original module (for re-instantiation if needed)
    _module: Module,
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
    
    fn plugin_type(&self) -> crate::plugin::types::PluginType {
        self.metadata().plugin_type
    }
    
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[async_trait::async_trait]
impl ScraperPlugin for WasmPlugin {
    async fn search(&self, query: &str, page: u32) -> Result<SearchResult> {
        let params = serde_json::json!({ "query": query, "page": page }).to_string();
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
pub struct WasmExports {
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
    wasi: WasiCtx,
    
    /// Resource table for managing handles
    table: ResourceTable,
    
    /// WASI Preview 1 adapter for compatibility
    adapter: WasiPreview1Adapter,
    
    /// HTTP Responses storage for simple host function
    http_responses: HashMap<u32, Vec<u8>>,
    
    /// Resource limiter for memory and compute
    limiter: StoreLimits,
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

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_wasm_runtime_creation() {
        let runtime = WasmRuntime::new();
        assert!(runtime.is_ok());
    }
    
    #[tokio::test]
    async fn test_sandbox_creation() {
        let runtime = WasmRuntime::new().unwrap();
        let permissions = vec![
            Permission::FileRead(std::path::PathBuf::from("/tmp")),
        ];
        let limits = ResourceLimits::default();
        
        let sandbox = runtime.create_sandbox(permissions, limits);
        assert!(sandbox.is_ok());
    }
    
    #[test]
    fn test_store_limits() {
        let mut limits = StoreLimits::new(1024);
        
        // Should allow growth within limit
        assert!(limits.memory_growing(0, 512, None).unwrap());
        assert_eq!(limits.current_memory(), 512);
        
        // Should allow growth up to limit
        assert!(limits.memory_growing(512, 1024, None).unwrap());
        assert_eq!(limits.current_memory(), 1024);
        
        // Should deny growth beyond limit
        assert!(!limits.memory_growing(1024, 2048, None).unwrap());
        assert_eq!(limits.current_memory(), 1024);
    }
}
