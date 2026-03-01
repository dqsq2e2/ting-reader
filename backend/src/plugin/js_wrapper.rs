use std::thread;
use tokio::sync::{mpsc, oneshot};
use serde_json::Value;
use tracing::{info, error};

use crate::core::error::{Result, TingError};
use super::types::{Plugin, PluginMetadata, PluginType, PluginContext};
use super::js_plugin::JavaScriptPluginLoader;

/// Command sent to the JS worker thread
enum JsCommand {
    Initialize {
        context: PluginContext,
        resp: oneshot::Sender<Result<()>>,
    },
    Shutdown {
        resp: oneshot::Sender<Result<()>>,
    },
    CallFunction {
        name: String,
        args: Value,
        resp: oneshot::Sender<Result<Value>>,
    },
    GarbageCollect {
        resp: oneshot::Sender<Result<()>>,
    },
}

/// Wrapper for JavaScript plugins to make them Send + Sync
///
/// This struct spawns a dedicated thread for the JS runtime and communicates
/// with it via channels. This bridges the gap between the multi-threaded
/// PluginManager and the single-threaded Deno runtime.
pub struct JavaScriptPluginWrapper {
    metadata: PluginMetadata,
    tx: mpsc::Sender<JsCommand>,
    _plugin_id: String,
}

impl JavaScriptPluginWrapper {
    /// Create a new JavaScript plugin wrapper
    pub fn new(
        loader: JavaScriptPluginLoader,
    ) -> Result<Self> {
        let metadata = loader.metadata().clone();
        let plugin_id = format!("{}@{}", metadata.name, metadata.version);
        let plugin_dir = loader.plugin_dir().to_path_buf();
        
        // Create channel for communication
        let (tx, mut rx) = mpsc::channel::<JsCommand>(32);
        
        let plugin_id_clone = plugin_id.clone();
        let _metadata_clone = metadata.clone();
        
        // Spawn dedicated thread for this plugin
        thread::Builder::new()
            .name(format!("js-plugin-{}", plugin_id))
            .spawn(move || {
                info!("Starting JS worker thread for {}", plugin_id_clone);
                
                // Create single-threaded Tokio runtime
                let rt = match tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build() 
                {
                    Ok(rt) => rt,
                    Err(e) => {
                        error!("Failed to create Tokio runtime for {}: {}", plugin_id_clone, e);
                        return;
                    }
                };
                
                // Run the local task set
                let local = tokio::task::LocalSet::new();
                
                local.block_on(&rt, async move {
                    // Create executor inside the thread
                    // We need to recreate the loader logic or pass the loader? 
                    // Loader is not Send? check js_plugin.rs.
                    // JavaScriptPluginLoader contains PathBuf and PluginMetadata, which are Send.
                    // So we can pass the loader or just reconstruct the executor.
                    // But create_executor is a method on Loader.
                    // Let's assume we can just use the dir and metadata.
                    
                    // Actually, let's use the code from create_executor directly or instantiate it.
                    // But `JavaScriptPluginLoader` is not passed here, we passed `plugin_dir` and `metadata`.
                    // We can reconstruct a loader or just modify `js_plugin.rs` to allow creating executor from dir+metadata.
                    // Or just use `JavaScriptPluginLoader::new(plugin_dir)` again inside the thread.
                    
                    let loader = match JavaScriptPluginLoader::new(plugin_dir) {
                        Ok(l) => l,
                        Err(e) => {
                            error!("Failed to initialize JS loader for {}: {}", plugin_id_clone, e);
                            return;
                        }
                    };
                    
                    let mut executor = match loader.create_executor() {
                        Ok(e) => e,
                        Err(e) => {
                            error!("Failed to create JS executor for {}: {}", plugin_id_clone, e);
                            return;
                        }
                    };
                    
                    // Load the module
                    if let Err(e) = executor.load_module().await {
                         error!("Failed to load JS module for {}: {}", plugin_id_clone, e);
                         return;
                    }
                    
                    info!("JS executor ready for {}", plugin_id_clone);
                    
                    // Message loop
                    while let Some(cmd) = rx.recv().await {
                        match cmd {
                            JsCommand::Initialize { context, resp } => {
                                // Convert context to Value and data_dir
                                let config = context.config.clone();
                                let data_dir = context.data_dir.clone();
                                
                                let result = executor.initialize(config, data_dir).await
                                    .map_err(|e| TingError::PluginExecutionError(e.to_string()));
                                    
                                let _ = resp.send(result);
                            }
                            JsCommand::Shutdown { resp } => {
                                let result = executor.shutdown()
                                    .map_err(|e| TingError::PluginExecutionError(e.to_string()));
                                    
                                let _ = resp.send(result);
                                // We continue the loop to allow graceful exit or potential restart?
                                // Usually shutdown means we are done.
                                break; 
                            }
                            JsCommand::CallFunction { name, args, resp } => {
                                // We need to define generic return type. 
                                // call_function returns Result<R>.
                                // We expect R to be Value.
                                let result = executor.call_function::<Value, Value>(&name, args).await
                                    .map_err(|e| TingError::PluginExecutionError(e.to_string()));
                                    
                                let _ = resp.send(result);
                            }
                            JsCommand::GarbageCollect { resp } => {
                                let result = executor.garbage_collect()
                                    .map_err(|e| TingError::PluginExecutionError(e.to_string()));
                                    
                                let _ = resp.send(result);
                            }
                        }
                    }
                    
                    info!("JS worker thread for {} exiting", plugin_id_clone);
                });
            })
            .map_err(|e| TingError::PluginLoadError(format!("Failed to spawn thread: {}", e)))?;
            
        Ok(Self {
            metadata,
            tx,
            _plugin_id: plugin_id,
        })
    }
}

#[async_trait::async_trait]
impl Plugin for JavaScriptPluginWrapper {
    fn metadata(&self) -> &PluginMetadata {
        &self.metadata
    }
    
    fn plugin_type(&self) -> PluginType {
        self.metadata.plugin_type
    }
    
    async fn initialize(&self, context: &PluginContext) -> Result<()> {
        let (resp_tx, resp_rx) = oneshot::channel();
        
        self.tx.send(JsCommand::Initialize {
            context: context.clone(),
            resp: resp_tx,
        }).await.map_err(|e| TingError::PluginExecutionError(format!("Failed to send init command: {}", e)))?;
        
        // Await response
        match resp_rx.await {
            Ok(res) => res,
            Err(_) => Err(TingError::PluginExecutionError("Channel closed".to_string())),
        }
    }
    
    async fn shutdown(&self) -> Result<()> {
        let (resp_tx, resp_rx) = oneshot::channel();
        
        self.tx.send(JsCommand::Shutdown {
            resp: resp_tx,
        }).await.map_err(|e| TingError::PluginExecutionError(format!("Failed to send shutdown command: {}", e)))?;
        
        match resp_rx.await {
            Ok(res) => res,
            Err(_) => Err(TingError::PluginExecutionError("Channel closed".to_string())),
        }
    }

    async fn garbage_collect(&self) -> Result<()> {
        let (resp_tx, resp_rx) = oneshot::channel();
        
        self.tx.send(JsCommand::GarbageCollect {
            resp: resp_tx,
        }).await.map_err(|e| TingError::PluginExecutionError(format!("Failed to send gc command: {}", e)))?;
        
        match resp_rx.await {
            Ok(res) => res,
            Err(_) => Err(TingError::PluginExecutionError("Channel closed".to_string())),
        }
    }
    
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

// Add method to call arbitrary functions (not part of Plugin trait but used by Manager)
impl JavaScriptPluginWrapper {
    pub async fn call_function(&self, name: &str, args: Value) -> Result<Value> {
        let (resp_tx, resp_rx) = oneshot::channel();
        
        self.tx.send(JsCommand::CallFunction {
            name: name.to_string(),
            args,
            resp: resp_tx,
        }).await.map_err(|e| TingError::PluginExecutionError(format!("Failed to send call command: {}", e)))?;
        
        match resp_rx.await {
            Ok(res) => res,
            Err(_) => Err(TingError::PluginExecutionError("Channel closed".to_string())),
        }
    }
}
