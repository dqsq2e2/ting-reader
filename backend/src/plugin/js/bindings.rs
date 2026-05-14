//! JavaScript Plugin Bindings
//!
//! This module provides the bridge between Rust and JavaScript for plugin functionality.
//! It implements:
//! - ScraperPlugin trait bindings for JavaScript plugins
//! - Rust function exports (logging, config, events) for JavaScript to call
//! - Data type conversion between Rust and JavaScript
//! - Async function support (Promise ↔ Future)

use anyhow::{Context, Result};
use serde::Serialize;
use serde_json::Value;
use tracing::{debug, error, info, warn};

use super::plugin::JavaScriptPluginExecutor;
use super::super::scraper::{BookDetail, Chapter, SearchResult};
use super::super::types::{PluginContext, PluginEventBus, PluginLogger, PluginMetadata, PluginType};

/// JavaScript Scraper Plugin Adapter
///
/// This adapter wraps a JavaScriptPluginExecutor and implements the ScraperPlugin trait,
/// allowing JavaScript plugins to be used as scraper plugins.
///
/// Note: This struct is NOT Send + Sync because it contains a JavaScriptPluginExecutor
/// which wraps a Deno JsRuntime (V8 isolates are single-threaded).
pub struct JsScraperPlugin {
    executor: JavaScriptPluginExecutor,
    metadata: PluginMetadata,
}

impl JsScraperPlugin {
    /// Create a new JavaScript scraper plugin adapter
    pub fn new(executor: JavaScriptPluginExecutor) -> Self {
        let metadata = executor.metadata().clone();
        Self { executor, metadata }
    }
}

// Note: We cannot implement Plugin trait directly because it requires Send + Sync
// Instead, we provide similar methods that can be called in a single-threaded context

impl JsScraperPlugin {
    /// Get plugin metadata (similar to Plugin::metadata)
    pub fn metadata(&self) -> &PluginMetadata {
        &self.metadata
    }

    /// Initialize the plugin (similar to Plugin::initialize)
    pub async fn initialize(&mut self, context: &PluginContext) -> Result<()> {
        self.executor
            .initialize(context.config.clone(), context.data_dir.clone())
            .await
    }

    /// Shutdown the plugin (similar to Plugin::shutdown)
    pub fn shutdown(&mut self) -> Result<()> {
        self.executor.shutdown()
    }

    /// Get the plugin type (similar to Plugin::plugin_type)
    pub fn plugin_type(&self) -> PluginType {
        self.metadata.plugin_type
    }
}

// Implement ScraperPlugin methods (but not the trait itself due to Send + Sync requirement)
impl JsScraperPlugin {
    /// Search for books by keyword
    pub async fn search(&mut self, query: &str, page: u32) -> Result<SearchResult> {
        debug!("JavaScript plugin search: query={}, page={}", query, page);

        #[derive(Serialize)]
        struct SearchArgs {
            query: String,
            page: u32,
        }

        let args = SearchArgs {
            query: query.to_string(),
            page,
        };

        self.executor
            .call_function("search", args)
            .await
            .context("Failed to call JavaScript search function")
    }

    /// Get detailed information about a book
    pub async fn get_detail(&mut self, book_id: &str) -> Result<BookDetail> {
        debug!("JavaScript plugin get_detail: book_id={}", book_id);

        #[derive(Serialize)]
        struct DetailArgs {
            book_id: String,
        }

        let args = DetailArgs {
            book_id: book_id.to_string(),
        };

        self.executor
            .call_function("getDetail", args)
            .await
            .context("Failed to call JavaScript getDetail function")
    }

    /// Get the list of chapters for a book
    pub async fn get_chapters(&mut self, book_id: &str) -> Result<Vec<Chapter>> {
        debug!("JavaScript plugin get_chapters: book_id={}", book_id);

        #[derive(Serialize)]
        struct ChaptersArgs {
            book_id: String,
        }

        let args = ChaptersArgs {
            book_id: book_id.to_string(),
        };

        self.executor
            .call_function("getChapters", args)
            .await
            .context("Failed to call JavaScript getChapters function")
    }

    /// Download a cover image
    pub async fn download_cover(&mut self, cover_url: &str) -> Result<Vec<u8>> {
        debug!("JavaScript plugin download_cover: url={}", cover_url);

        #[derive(Serialize)]
        struct CoverArgs {
            cover_url: String,
        }

        let args = CoverArgs {
            cover_url: cover_url.to_string(),
        };

        // JavaScript returns { data: "base64...", content_type: "..." }
        // We need to extract the data field
        let result_obj: serde_json::Value = self
            .executor
            .call_function("downloadCover", args)
            .await
            .context("Failed to call JavaScript downloadCover function")?;
            
        let base64_data = if let Some(data) = result_obj.get("data").and_then(|v| v.as_str()) {
            data.to_string()
        } else if let Some(s) = result_obj.as_str() {
            // Fallback for legacy plugins that return string directly
            s.to_string()
        } else {
            return Err(anyhow::anyhow!("Invalid response format from downloadCover: missing 'data' field"));
        };

        // Decode base64 to bytes
        use base64::{engine::general_purpose, Engine as _};
        general_purpose::STANDARD
            .decode(&base64_data)
            .context("Failed to decode base64 cover data")
    }

    /// Get the audio download URL for a chapter
    pub async fn get_audio_url(&mut self, chapter_id: &str) -> Result<String> {
        debug!("JavaScript plugin get_audio_url: chapter_id={}", chapter_id);

        #[derive(Serialize)]
        struct AudioUrlArgs {
            chapter_id: String,
        }

        let args = AudioUrlArgs {
            chapter_id: chapter_id.to_string(),
        };

        self.executor
            .call_function("getAudioUrl", args)
            .await
            .context("Failed to call JavaScript getAudioUrl function")
    }
}

// ============================================================================
// Rust Functions Exported to JavaScript (Helper Functions)
// ============================================================================

/// Plugin logger implementation for JavaScript plugins
#[derive(Clone)]
pub struct JsPluginLogger {
    plugin_name: String,
}

impl JsPluginLogger {
    pub fn new(plugin_name: String) -> Self {
        Self { plugin_name }
    }
}

impl PluginLogger for JsPluginLogger {
    fn debug(&self, message: &str) {
        debug!(plugin = %self.plugin_name, "{}", message);
    }

    fn info(&self, message: &str) {
        info!(plugin = %self.plugin_name, "{}", message);
    }

    fn warn(&self, message: &str) {
        warn!(plugin = %self.plugin_name, "{}", message);
    }

    fn error(&self, message: &str) {
        error!(plugin = %self.plugin_name, "{}", message);
    }
}

/// Plugin event bus implementation for JavaScript plugins
#[derive(Clone)]
pub struct JsPluginEventBus {
    plugin_name: String,
}

impl JsPluginEventBus {
    pub fn new(plugin_name: String) -> Self {
        Self { plugin_name }
    }
}

impl PluginEventBus for JsPluginEventBus {
    fn publish(&self, event_type: &str, _data: Value) -> crate::core::error::Result<()> {
        info!(
            plugin = %self.plugin_name,
            event_type = %event_type,
            "Publishing event"
        );
        // TODO: Implement actual event publishing when event bus is available
        Ok(())
    }

    fn subscribe(
        &self,
        event_type: &str,
        _handler: Box<dyn Fn(Value) + Send + Sync>,
    ) -> crate::core::error::Result<String> {
        info!(
            plugin = %self.plugin_name,
            event_type = %event_type,
            "Subscribing to event"
        );
        // TODO: Implement actual event subscription when event bus is available
        Ok(format!("sub_{}_{}", self.plugin_name, event_type))
    }

    fn unsubscribe(&self, subscription_id: &str) -> crate::core::error::Result<()> {
        info!(
            plugin = %self.plugin_name,
            subscription_id = %subscription_id,
            "Unsubscribing from event"
        );
        // TODO: Implement actual event unsubscription when event bus is available
        Ok(())
    }
}

/// Helper to create a JavaScript runtime with plugin bindings
///
/// This function creates a Deno runtime and injects the Ting API into the global scope.
/// The Ting API provides logging, configuration access, and event bus functionality.
/// 
/// # Arguments
/// * `plugin_name` - Name of the plugin
/// * `config` - Plugin configuration
/// * `sandbox` - Optional sandbox for permission checking
pub fn create_js_runtime_with_bindings(
    plugin_name: String,
    config: Value,
    sandbox: Option<&crate::plugin::wasm::sandbox::Sandbox>,
) -> Result<deno_core::JsRuntime> {
    use deno_core::{JsRuntime, RuntimeOptions, Extension, op2};
    use std::sync::OnceLock;

    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();

    fn get_client() -> &'static reqwest::Client {
        CLIENT.get_or_init(|| {
            reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .connect_timeout(std::time::Duration::from_secs(10))
                .danger_accept_invalid_certs(true)
                .no_proxy()
                .build()
                .expect("Failed to build global reqwest client")
        })
    }

    #[op2(async)]
    #[string]
    pub async fn op_fetch(#[string] url: String, #[serde] options: Option<Value>) -> Result<String, anyhow::Error> {
        tracing::info!("op_fetch: 开始请求 {}", url);

        let client = get_client();
        let mut builder = client.get(&url);

        if let Some(opts) = options {
            if let Some(method) = opts.get("method").and_then(|m| m.as_str()) {
                match method.to_uppercase().as_str() {
                    "POST" => builder = client.post(&url),
                    "PUT" => builder = client.put(&url),
                    "DELETE" => builder = client.delete(&url),
                    _ => {}
                }
            }
            if let Some(headers) = opts.get("headers").and_then(|h| h.as_object()) {
                for (k, v) in headers {
                    if let Some(v_str) = v.as_str() {
                        builder = builder.header(k, v_str);
                    }
                }
            }
            if let Some(body) = opts.get("body").and_then(|b| b.as_str()) {
                builder = builder.body(body.to_string());
            }
        }

        tracing::info!("op_fetch: 发送请求...");
        match builder.send().await {
            Ok(resp) => {
                let status = resp.status();
                tracing::info!("op_fetch: 获得响应状态 {}", status);
                match resp.text().await {
                    Ok(text) => {
                        tracing::info!("op_fetch: 对 {} 的请求已完成，主体长度: {}", url, text.len());
                        Ok(text)
                    },
                    Err(e) => {
                        tracing::error!("op_fetch: 无法从 {} 读取主体: {}", url, e);
                        Err(e.into())
                    }
                }
            },
            Err(e) => {
                tracing::error!("op_fetch: 对 {} 的请求失败: {}", url, e);
                Err(e.into())
            }
        }
    }

    #[allow(deprecated)]
    let ext = Extension {
        name: "ting_fetch",
        ops: std::borrow::Cow::Owned(vec![op_fetch::decl()]),
        ..Default::default()
    };

    let mut runtime = JsRuntime::new(RuntimeOptions {
        extensions: vec![ext],
        ..Default::default()
    });

    let allowed_paths = sandbox
        .map(|s| {
            s.get_allowed_paths()
                .iter()
                .map(|p| p.to_string_lossy().to_string())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    let allowed_domains = sandbox
        .map(|s| s.get_allowed_domains().to_vec())
        .unwrap_or_default();

    let init_code = super::init_code::generate_init_code(
        &plugin_name,
        &config,
        &allowed_paths,
        &allowed_domains,
    );

    runtime
        .execute_script("<init_bindings>", init_code.into())
        .context("Failed to initialize JavaScript bindings")?;

    Ok(runtime)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_js_plugin_logger() {
        let logger = JsPluginLogger::new("test-plugin".to_string());
        logger.debug("Debug message");
        logger.info("Info message");
        logger.warn("Warning message");
        logger.error("Error message");
    }

    #[test]
    fn test_js_plugin_event_bus() {
        let event_bus = JsPluginEventBus::new("test-plugin".to_string());
        let result = event_bus.publish("test_event", serde_json::json!({"key": "value"}));
        assert!(result.is_ok());

        let handler = Box::new(|_data: Value| {});
        let result = event_bus.subscribe("test_event", handler);
        assert!(result.is_ok());

        let sub_id = result.unwrap();
        let result = event_bus.unsubscribe(&sub_id);
        assert!(result.is_ok());
    }

    #[test]
    fn test_create_js_runtime_with_bindings() {
        let config = serde_json::json!({"api_key": "test_key", "cache_enabled": true});
        let result = create_js_runtime_with_bindings("test-plugin".to_string(), config, None);
        assert!(result.is_ok());
    }

    #[test]
    fn test_js_runtime_sandbox_file_paths() {
        use crate::plugin::wasm::sandbox::{Permission, ResourceLimits, Sandbox};
        use std::path::PathBuf;

        let config = serde_json::json!({});
        let permissions = vec![
            Permission::FileRead(PathBuf::from("./data/cache")),
            Permission::FileWrite(PathBuf::from("./data/output")),
        ];
        let sandbox = Sandbox::new(permissions, ResourceLimits::default());

        let mut runtime = create_js_runtime_with_bindings(
            "test-plugin".to_string(), config, Some(&sandbox)
        ).unwrap();

        let test_code = r#"
            const allowedPaths = Ting.sandbox.allowedPaths;
            JSON.stringify({ allowedPaths })
        "#;
        let result = runtime.execute_script("<test_sandbox>", test_code.to_string().into());
        assert!(result.is_ok());
    }
}
