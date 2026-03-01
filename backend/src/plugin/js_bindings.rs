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

use super::js_plugin::JavaScriptPluginExecutor;
use super::scraper::{BookDetail, Chapter, SearchResult};
use super::types::{PluginContext, PluginEventBus, PluginLogger, PluginMetadata, PluginType};

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
    sandbox: Option<&super::sandbox::Sandbox>,
) -> Result<deno_core::JsRuntime> {
    use deno_core::{JsRuntime, RuntimeOptions, op2, Extension, Op};

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
    tracing::info!("op_fetch: starting request to {}", url);
    
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
    
    tracing::info!("op_fetch: sending request...");
    match builder.send().await {
        Ok(resp) => {
            let status = resp.status();
            tracing::info!("op_fetch: got response status {}", status);
            
            match resp.text().await {
                Ok(text) => {
                    tracing::info!("op_fetch: request to {} completed, body len: {}", url, text.len());
                    Ok(text)
                },
                Err(e) => {
                    tracing::error!("op_fetch: failed to read body from {}: {}", url, e);
                    Err(e.into())
                }
            }
        },
        Err(e) => {
            tracing::error!("op_fetch: request failed to {}: {}", url, e);
            Err(e.into())
        }
    }
}

    let ext = Extension {
        name: "ting_fetch",
        ops: std::borrow::Cow::Borrowed(&[op_fetch::DECL]),
        ..Default::default()
    };

    // Create runtime with default options and fetch extension
    let mut runtime = JsRuntime::new(RuntimeOptions {
        extensions: vec![ext],
        ..Default::default()
    });

    // Get allowed paths and domains from sandbox
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

    // Store config and plugin name in a global variable
    let init_code = format!(
        r#"
        "use strict";

        // Polyfill Headers
        globalThis.Headers = class Headers {{
            constructor(init) {{
                this.map = new Map();
                if (init) {{
                    if (init instanceof Headers) {{
                        init.forEach((value, key) => this.append(key, value));
                    }} else if (Array.isArray(init)) {{
                        init.forEach(([key, value]) => this.append(key, value));
                    }} else {{
                        Object.keys(init).forEach(key => this.append(key, init[key]));
                    }}
                }}
            }}
            append(name, value) {{
                name = name.toLowerCase();
                value = String(value);
                if (this.map.has(name)) {{
                    this.map.get(name).push(value);
                }} else {{
                    this.map.set(name, [value]);
                }}
            }}
            delete(name) {{ this.map.delete(name.toLowerCase()); }}
            get(name) {{
                const values = this.map.get(name.toLowerCase());
                return values ? values[0] : null;
            }}
            has(name) {{ return this.map.has(name.toLowerCase()); }}
            set(name, value) {{ this.map.set(name.toLowerCase(), [String(value)]); }}
            forEach(callback, thisArg) {{
                for (const [name, values] of this.map) {{
                    // Headers.forEach yields values joined by comma
                    callback.call(thisArg, values.join(', '), name, this);
                }}
            }}
        }};

        // Polyfill URL (Minimal)
        globalThis.URL = class URL {{
            constructor(url, base) {{
                if (base) {{
                    if (base.endsWith('/')) base = base.slice(0, -1);
                    if (!url.startsWith('/')) url = '/' + url;
                    url = base + url;
                }}
                this.href = url;
                const match = url.match(/^(https?:)\/\/([^/?#]+)(.*)$/);
                if (match) {{
                    this.protocol = match[1];
                    this.hostname = match[2];
                    this.pathname = match[3] || '/';
                    this.search = '';
                    if (this.pathname.includes('?')) {{
                        const parts = this.pathname.split('?');
                        this.pathname = parts[0];
                        this.search = '?' + parts[1];
                    }}
                }} else {{
                    this.hostname = '';
                    this.protocol = '';
                    this.pathname = '';
                    this.search = '';
                }}
            }}
            toString() {{ return this.href; }}
        }};

        // Ting Plugin API for JavaScript
        globalThis.Ting = {{
            pluginName: "{}",
            config: {},
            
            // Sandbox information
            sandbox: {{
                allowedPaths: {},
                allowedDomains: {},
            }},
            
            // Logging functions (will be implemented via console.log for now)
            log: {{
                debug: (message) => console.log(`[DEBUG] [{}] ${{message}}`),
                info: (message) => console.log(`[INFO] [{}] ${{message}}`),
                warn: (message) => console.warn(`[WARN] [{}] ${{message}}`),
                error: (message) => console.error(`[ERROR] [{}] ${{message}}`),
            }},
            
            // Configuration access
            getConfig: (key) => {{
                const config = {};
                return config[key] || null;
            }},
            
            // Event bus (placeholder for now)
            events: {{
                publish: (eventType, data) => {{
                    console.log(`[EVENT] [{}] Publishing: ${{eventType}}`);
                    return true;
                }},
                subscribe: (eventType, handler) => {{
                    console.log(`[EVENT] [{}] Subscribing to: ${{eventType}}`);
                    return `sub_{}_${{eventType}}`;
                }},
            }},
        }};
        
        // Override fetch to enforce network access control
        globalThis.fetch = async function(url, options) {{
            const urlStr = typeof url === 'string' ? url : url.toString();
            Ting.log.info('fetch: ' + urlStr);
            
            // Check if URL is allowed
            const allowedDomains = Ting.sandbox.allowedDomains;
            if (allowedDomains.length > 0) {{
                const domain = extractDomain(urlStr);
                const isAllowed = allowedDomains.some(pattern => domainMatches(domain, pattern));
                
                if (!isAllowed) {{
                    throw new Error(`Network access denied: ${{urlStr}}`);
                }}
            }}
            
            // Use Rust backend via Deno op
            try {{
                Ting.log.info('calling op_fetch for ' + urlStr);
                const responseText = await Deno.core.ops.op_fetch(urlStr, options);
                Ting.log.info('op_fetch returned for ' + urlStr);
                return {{
                    ok: true,
                    status: 200,
                    statusText: "OK",
                    text: async () => responseText,
                    json: async () => JSON.parse(responseText),
                    headers: new Headers(),
                }};
            }} catch (e) {{
                Ting.log.error('op_fetch failed: ' + e);
                throw e;
            }}
        }};
        
        // Helper function to extract domain from URL
        function extractDomain(url) {{
            // Use regex instead of URL object to avoid dependency issues if URL is not perfect
            const matches = url.match(/^https?:\/\/([^/?#]+)(?:[/?#]|$)/i);
            return matches ? matches[1] : '';
        }}
        
        // Helper function to check if domain matches pattern (supports wildcards)
        function domainMatches(domain, pattern) {{
            if (pattern.startsWith('*.')) {{
                const base = pattern.substring(2);
                return domain.endsWith(base) || domain === base;
            }} else {{
                return domain === pattern;
            }}
        }}

        // Helper for invoking functions from Rust without recompiling scripts
        globalThis._ting_invoke = async function(funcName, args) {{
            try {{
                globalThis._ting_status = 'pending';
                globalThis._ting_result = undefined;
                globalThis._ting_error = undefined;
                
                const func = globalThis[funcName];
                if (typeof func !== 'function') {{
                    throw new Error(`Function ${{funcName}} not found`);
                }}
                
                const result = await func(args);
                globalThis._ting_result = JSON.stringify(result);
                globalThis._ting_status = 'success';
            }} catch (e) {{
                globalThis._ting_error = e.toString();
                globalThis._ting_status = 'error';
            }}
        }};
        "#,
        plugin_name,
        serde_json::to_string(&config).unwrap_or_else(|_| "{}".to_string()),
        serde_json::to_string(&allowed_paths).unwrap_or_else(|_| "[]".to_string()),
        serde_json::to_string(&allowed_domains).unwrap_or_else(|_| "[]".to_string()),
        plugin_name,
        plugin_name,
        plugin_name,
        plugin_name,
        serde_json::to_string(&config).unwrap_or_else(|_| "{}".to_string()),
        plugin_name,
        plugin_name,
        plugin_name,
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
        
        // These should not panic
        logger.debug("Debug message");
        logger.info("Info message");
        logger.warn("Warning message");
        logger.error("Error message");
    }

    #[test]
    fn test_js_plugin_event_bus() {
        let event_bus = JsPluginEventBus::new("test-plugin".to_string());
        
        // Test publish
        let result = event_bus.publish("test_event", serde_json::json!({"key": "value"}));
        assert!(result.is_ok());
        
        // Test subscribe
        let handler = Box::new(|_data: Value| {});
        let result = event_bus.subscribe("test_event", handler);
        assert!(result.is_ok());
        
        // Test unsubscribe
        let sub_id = result.unwrap();
        let result = event_bus.unsubscribe(&sub_id);
        assert!(result.is_ok());
    }

    #[test]
    fn test_create_js_runtime_with_bindings() {
        let config = serde_json::json!({
            "api_key": "test_key",
            "cache_enabled": true
        });
        
        let result = create_js_runtime_with_bindings("test-plugin".to_string(), config, None);
        assert!(result.is_ok());
        
        let mut runtime = result.unwrap();
        
        // Test that Ting global is available
        let test_code = r#"
            if (typeof Ting === 'undefined') {
                throw new Error('Ting global not found');
            }
            if (typeof Ting.log === 'undefined') {
                throw new Error('Ting.log not found');
            }
            if (typeof Ting.getConfig === 'undefined') {
                throw new Error('Ting.getConfig not found');
            }
            if (typeof Ting.events === 'undefined') {
                throw new Error('Ting.events not found');
            }
            'ok'
        "#;
        
        let result = runtime.execute_script("<test>", test_code.to_string().into());
        assert!(result.is_ok());
    }

    #[test]
    fn test_js_runtime_logging() {
        let config = serde_json::json!({});
        let mut runtime = create_js_runtime_with_bindings("test-plugin".to_string(), config, None).unwrap();
        
        // Test logging functions
        let test_code = r#"
            Ting.log.debug('Debug message');
            Ting.log.info('Info message');
            Ting.log.warn('Warning message');
            Ting.log.error('Error message');
            'ok'
        "#;
        
        let result = runtime.execute_script("<test_logging>", test_code.to_string().into());
        assert!(result.is_ok());
    }

    #[test]
    fn test_js_runtime_config_access() {
        let config = serde_json::json!({
            "api_key": "test_key_123",
            "cache_enabled": true,
            "timeout": 30
        });
        
        let mut runtime = create_js_runtime_with_bindings("test-plugin".to_string(), config, None).unwrap();
        
        // Test config access
        let test_code = r#"
            const apiKey = Ting.getConfig('api_key');
            const cacheEnabled = Ting.getConfig('cache_enabled');
            const timeout = Ting.getConfig('timeout');
            const missing = Ting.getConfig('missing_key');
            
            JSON.stringify({
                apiKey,
                cacheEnabled,
                timeout,
                missing
            })
        "#;
        
        let result = runtime.execute_script("<test_config>", test_code.to_string().into());
        assert!(result.is_ok());
        
        // Parse the result
        let scope = &mut runtime.handle_scope();
        let local_value = deno_core::v8::Local::new(scope, result.unwrap());
        let result_str = local_value
            .to_string(scope)
            .unwrap()
            .to_rust_string_lossy(scope);
        
        let parsed: Value = serde_json::from_str(&result_str).unwrap();
        assert_eq!(parsed["apiKey"], "test_key_123");
        assert_eq!(parsed["cacheEnabled"], true);
        assert_eq!(parsed["timeout"], 30);
        assert_eq!(parsed["missing"], Value::Null);
    }

    #[test]
    fn test_js_runtime_event_publishing() {
        let config = serde_json::json!({});
        let mut runtime = create_js_runtime_with_bindings("test-plugin".to_string(), config, None).unwrap();
        
        // Test event publishing
        let test_code = r#"
            Ting.events.publish('test_event', { key: 'value', count: 42 });
            'ok'
        "#;
        
        let result = runtime.execute_script("<test_events>", test_code.to_string().into());
        assert!(result.is_ok());
    }
}

    #[test]
    fn test_js_runtime_sandbox_network_whitelist() {
        use super::sandbox::{Permission, ResourceLimits, Sandbox};
        
        let config = serde_json::json!({});
        
        // Create sandbox with network access to example.com only
        let permissions = vec![
            Permission::NetworkAccess("*.example.com".to_string()),
        ];
        let sandbox = Sandbox::new(permissions, ResourceLimits::default());
        
        let mut runtime = create_js_runtime_with_bindings(
            "test-plugin".to_string(),
            config,
            Some(&sandbox)
        ).unwrap();
        
        // Test that sandbox info is available
        let test_code = r#"
            const allowedDomains = Ting.sandbox.allowedDomains;
            JSON.stringify({ allowedDomains })
        "#;
        
        let result = runtime.execute_script("<test_sandbox>", test_code.to_string().into());
        assert!(result.is_ok());
        
        let scope = &mut runtime.handle_scope();
        let local_value = deno_core::v8::Local::new(scope, result.unwrap());
        let result_str = local_value
            .to_string(scope)
            .unwrap()
            .to_rust_string_lossy(scope);
        
        let parsed: Value = serde_json::from_str(&result_str).unwrap();
        assert_eq!(parsed["allowedDomains"], serde_json::json!(["*.example.com"]));
    }

    #[test]
    fn test_js_runtime_sandbox_file_paths() {
        use super::sandbox::{Permission, ResourceLimits, Sandbox};
        use std::path::PathBuf;
        
        let config = serde_json::json!({});
        
        // Create sandbox with file access
        let permissions = vec![
            Permission::FileRead(PathBuf::from("./data/cache")),
            Permission::FileWrite(PathBuf::from("./data/output")),
        ];
        let sandbox = Sandbox::new(permissions, ResourceLimits::default());
        
        let mut runtime = create_js_runtime_with_bindings(
            "test-plugin".to_string(),
            config,
            Some(&sandbox)
        ).unwrap();
        
        // Test that sandbox info is available
        let test_code = r#"
            const allowedPaths = Ting.sandbox.allowedPaths;
            JSON.stringify({ allowedPaths })
        "#;
        
        let result = runtime.execute_script("<test_sandbox>", test_code.to_string().into());
        assert!(result.is_ok());
        
        let scope = &mut runtime.handle_scope();
        let local_value = deno_core::v8::Local::new(scope, result.unwrap());
        let result_str = local_value
            .to_string(scope)
            .unwrap()
            .to_rust_string_lossy(scope);
        
        let parsed: Value = serde_json::from_str(&result_str).unwrap();
        let paths = parsed["allowedPaths"].as_array().unwrap();
        assert_eq!(paths.len(), 2);
    }
