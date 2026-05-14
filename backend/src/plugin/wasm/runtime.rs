//! WASM runtime implementation
//!
//! This module provides the WebAssembly runtime for loading and executing WASM plugins.
//! It uses wasmtime as the WASM engine and provides sandboxed execution with resource limits.

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, RwLock};
use wasmtime::*;
use wasmtime_wasi::preview2::ResourceTable;
use wasmtime_wasi::preview2::preview1::WasiPreview1Adapter;
use crate::core::error::{Result, TingError};
use super::sandbox::{Sandbox, Permission, ResourceLimits};
use super::plugin::{WasmPlugin, WasmPluginInner, WasmExports, PluginState, StoreLimits};
use crate::plugin::types::{PluginId, PluginMetadata};

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
        
        // Register custom HTTP host functions
        super::host_functions::add_host_functions(&mut linker)
            .map_err(|e| TingError::PluginExecutionError(format!("Failed to register host functions: {}", e)))?;

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

