use std::thread;
use std::time::Duration;
use tokio::sync::{mpsc, oneshot};
use serde_json::Value;
use tracing::{info, error};

use crate::core::error::{Result, TingError};
use super::super::types::{Plugin, PluginMetadata, PluginType, PluginContext};
use super::plugin::JavaScriptPluginLoader;

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
        
        // Create error channel to detect early failures
        let (error_tx, mut error_rx) = oneshot::channel::<String>();
        
        let plugin_id_clone = plugin_id.clone();
        let plugin_id_clone2 = plugin_id.clone(); // For panic handler
        let _metadata_clone = metadata.clone();
        
        // Spawn dedicated thread for this plugin
        let thread_result = thread::Builder::new()
            .name(format!("js-plugin-{}", plugin_id))
            .spawn(move || {
                info!("Starting JS worker thread for {}", plugin_id_clone);
                
                // Wrap entire thread logic to catch panics
                let thread_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    // Create single-threaded Tokio runtime
                    let rt = match tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .build() 
                    {
                        Ok(rt) => rt,
                        Err(e) => {
                            let err_msg = format!("Failed to create Tokio runtime: {}", e);
                            error!("{} for {}", err_msg, plugin_id_clone);
                            let _ = error_tx.send(err_msg);
                            return;
                        }
                    };
                    
                    // Run the local task set
                    let local = tokio::task::LocalSet::new();
                    
                    local.block_on(&rt, async move {
                        // Create loader
                        let loader = match JavaScriptPluginLoader::new(plugin_dir) {
                            Ok(l) => l,
                            Err(e) => {
                                let err_msg = format!("Failed to initialize JS loader: {}", e);
                                error!("{} for {}", err_msg, plugin_id_clone);
                                let _ = error_tx.send(err_msg);
                                return;
                            }
                        };
                        
                        // Create executor
                        let mut executor = match loader.create_executor() {
                            Ok(e) => e,
                            Err(e) => {
                                let err_msg = format!("Failed to create JS executor: {}", e);
                                error!("{} for {}", err_msg, plugin_id_clone);
                                let _ = error_tx.send(err_msg);
                                return;
                            }
                        };
                        
                        // Load the module with timeout
                        match tokio::time::timeout(
                            Duration::from_secs(30),
                            executor.load_module()
                        ).await {
                            Ok(Ok(())) => {
                                info!("JS executor ready for {}", plugin_id_clone);
                                // Signal success by dropping error_tx without sending
                                drop(error_tx);
                            }
                            Ok(Err(e)) => {
                                let err_msg = format!("Failed to load JS module: {}", e);
                                error!("{} for {}", err_msg, plugin_id_clone);
                                let _ = error_tx.send(err_msg);
                                return;
                            }
                            Err(_) => {
                                let err_msg = "Module load timeout after 30s".to_string();
                                error!("{} for {}", err_msg, plugin_id_clone);
                                let _ = error_tx.send(err_msg);
                                return;
                            }
                        }
                        
                        // Message loop
                        while let Some(cmd) = rx.recv().await {
                            match cmd {
                                JsCommand::Initialize { context, resp } => {
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
                                    break; 
                                }
                                JsCommand::CallFunction { name, args, resp } => {
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
                        
                        info!("JS worker thread for {} exiting normally", plugin_id_clone);
                    });
                }));
                
                // Handle panic
                if let Err(panic_err) = thread_result {
                    let panic_msg = if let Some(s) = panic_err.downcast_ref::<&str>() {
                        s.to_string()
                    } else if let Some(s) = panic_err.downcast_ref::<String>() {
                        s.clone()
                    } else {
                        "Unknown panic".to_string()
                    };
                    error!("JS worker thread panicked for {}: {}", plugin_id_clone2, panic_msg);
                }
            });
        
        // Check if thread spawn failed
        thread_result.map_err(|e| TingError::PluginLoadError(format!("Failed to spawn thread: {}", e)))?;
        
        // Wait for early initialization errors (increased timeout for slow machines)
        // This gives slow machines enough time to detect initialization failures
        // before we assume success
        info!("Waiting 3s to check for early initialization errors for {}", plugin_id);
        let init_check = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(3000));  // Increased from 200ms to 3s
            error_rx.try_recv()
        });
        
        // Block on the check (this is acceptable since it's only 3 seconds)
        match init_check.join() {
            Ok(recv_result) => {
                match recv_result {
                    Ok(err_msg) => {
                        // Early error detected
                        error!("JS plugin {} failed early initialization check: {}", plugin_id, err_msg);
                        return Err(TingError::PluginLoadError(format!("Plugin initialization failed: {}", err_msg)));
                    }
                    Err(_) => {
                        // Channel closed without error = success
                        info!("JS plugin {} passed early initialization check (3s)", plugin_id);
                        info!("Note: Plugin is still initializing in background, full initialization may take up to 30s");
                    }
                }
            }
            Err(_) => {
                // Join failed
                error!("JS plugin {} init check thread panicked", plugin_id);
                return Err(TingError::PluginLoadError("Init check thread panicked".to_string()));
            }
        }
            
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
