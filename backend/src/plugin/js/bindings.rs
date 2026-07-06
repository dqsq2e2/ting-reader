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

use super::super::scraper::{Chapter, SearchResult};
use super::super::types::{
    PluginContext, PluginEventBus, PluginLogger, PluginMetadata, PluginType,
};
use super::npm::NpmDependency;
use super::plugin::JavaScriptPluginExecutor;
use crate::plugin::{PluginHostGatewayHandle, PluginHostUser};

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
            return Err(anyhow::anyhow!(
                "Invalid response format from downloadCover: missing 'data' field"
            ));
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

#[derive(Clone)]
struct JsHostGatewayState {
    plugin_id: String,
    host_gateway: Option<PluginHostGatewayHandle>,
}

#[derive(Clone, Default)]
pub struct JsHostInvocationContext {
    pub user: Option<PluginHostUser>,
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
    plugin_id: String,
    config: Value,
    sandbox: Option<&crate::plugin::wasm::sandbox::Sandbox>,
    host_gateway: Option<PluginHostGatewayHandle>,
    plugin_dir: std::path::PathBuf,
    npm_dependencies: Vec<NpmDependency>,
) -> Result<deno_core::JsRuntime> {
    use deno_core::{op2, Extension, JsRuntime, OpState, RuntimeOptions};
    use std::cell::RefCell;
    use std::collections::HashSet;
    use std::path::PathBuf;
    use std::rc::Rc;
    use std::sync::OnceLock;

    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();

    #[derive(Clone)]
    struct JsFetchPermissions {
        allowed_domains: Vec<String>,
    }

    #[derive(Clone)]
    struct JsNpmModuleState {
        plugin_dir: PathBuf,
        allowed_packages: HashSet<String>,
    }

    fn get_client() -> &'static reqwest::Client {
        CLIENT.get_or_init(|| {
            reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(180))
                .connect_timeout(std::time::Duration::from_secs(10))
                .danger_accept_invalid_certs(true)
                .no_proxy()
                .build()
                .expect("Failed to build global reqwest client")
        })
    }

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

    #[op2(async)]
    #[string]
    pub async fn op_fetch(
        state: Rc<RefCell<OpState>>,
        #[string] url: String,
        #[serde] options: Option<Value>,
    ) -> Result<String, anyhow::Error> {
        tracing::info!("op_fetch: starting request {}", url);

        let allowed_domains = {
            let state = state.borrow();
            state
                .try_borrow::<JsFetchPermissions>()
                .map(|permissions| permissions.allowed_domains.clone())
                .unwrap_or_default()
        };

        if !is_network_allowed(&allowed_domains, &url) {
            return Err(anyhow::anyhow!("Network access denied: {}", url));
        }

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
            if let Some(timeout_ms) = opts.get("timeout_ms").and_then(|v| v.as_u64()) {
                let timeout_ms = timeout_ms.clamp(1_000, 600_000);
                builder = builder.timeout(std::time::Duration::from_millis(timeout_ms));
            }
        }

        tracing::info!("op_fetch: sending request...");
        match builder.send().await {
            Ok(resp) => {
                let status = resp.status();
                tracing::info!("op_fetch: received response status {}", status);
                match resp.text().await {
                    Ok(text) => {
                        tracing::info!(
                            "op_fetch: request to {} completed, body length: {}",
                            url,
                            text.len()
                        );
                        Ok(text)
                    }
                    Err(e) => {
                        tracing::error!(
                            url = %url,
                            error = %e,
                            message_key = "plugin.fetch.body_read_failed",
                            message_params = %serde_json::json!({ "error": e.to_string() }),
                            "Plugin fetch body read failed"
                        );
                        Err(e.into())
                    }
                }
            }
            Err(e) => {
                tracing::error!(
                    url = %url,
                    error = %e,
                    message_key = "plugin.fetch.request_failed",
                    message_params = %serde_json::json!({ "error": e.to_string() }),
                    "Plugin fetch request failed"
                );
                Err(e.into())
            }
        }
    }

    #[op2(async)]
    #[serde]
    pub async fn op_host_invoke(
        state: Rc<RefCell<OpState>>,
        #[string] method: String,
        #[serde] params: serde_json::Value,
    ) -> Result<serde_json::Value, anyhow::Error> {
        let (plugin_id, host_gateway, user) = {
            let state = state.borrow();
            let host_state = state.try_borrow::<JsHostGatewayState>().cloned();
            let invocation_context = state
                .try_borrow::<JsHostInvocationContext>()
                .cloned()
                .unwrap_or_default();

            match host_state {
                Some(host_state) => (
                    host_state.plugin_id,
                    host_state.host_gateway.and_then(|handle| handle.get()),
                    invocation_context.user,
                ),
                None => (String::new(), None, invocation_context.user),
            }
        };

        let gateway = host_gateway.ok_or_else(|| {
            anyhow::anyhow!("Ting.host.invoke is not configured for this plugin runtime")
        })?;
        let user = user.ok_or_else(|| {
            anyhow::anyhow!("Ting.host.invoke requires an authenticated user context")
        })?;

        gateway
            .invoke_plugin(&plugin_id, &user, &method, params)
            .await
            .map_err(|error| anyhow::anyhow!(error.to_string()))
    }

    #[op2]
    #[serde]
    pub fn op_require_module(
        state: Rc<RefCell<OpState>>,
        #[string] request: String,
        #[string] parent_path: String,
    ) -> Result<serde_json::Value, anyhow::Error> {
        let npm_state = {
            let state = state.borrow();
            state
                .try_borrow::<JsNpmModuleState>()
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("npm module state is not configured"))?
        };

        let module_path = resolve_js_module(
            &npm_state.plugin_dir,
            &npm_state.allowed_packages,
            &request,
            &parent_path,
        )?;
        let canonical = std::fs::canonicalize(&module_path)?;
        ensure_path_inside(&npm_state.plugin_dir, &canonical)?;

        let code = if canonical
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| extension.eq_ignore_ascii_case("json"))
        {
            let json_text = std::fs::read_to_string(&canonical)?;
            let parsed: serde_json::Value = serde_json::from_str(&json_text)?;
            format!(
                "module.exports = {};",
                serde_json::to_string(&parsed).unwrap_or_else(|_| "null".to_string())
            )
        } else {
            std::fs::read_to_string(&canonical)?
        };

        let dirname = canonical
            .parent()
            .map(|path| path.to_string_lossy().to_string())
            .unwrap_or_default();
        let filename = canonical.to_string_lossy().to_string();

        Ok(serde_json::json!({
            "id": filename,
            "filename": filename,
            "dirname": dirname,
            "code": code,
        }))
    }

    #[allow(deprecated)]
    let ext = Extension {
        name: "ting_fetch",
        ops: std::borrow::Cow::Owned(vec![
            op_fetch::decl(),
            op_host_invoke::decl(),
            op_require_module::decl(),
        ]),
        ..Default::default()
    };

    let mut runtime = JsRuntime::new(RuntimeOptions {
        extensions: vec![ext],
        ..Default::default()
    });
    runtime.op_state().borrow_mut().put(JsFetchPermissions {
        allowed_domains: allowed_domains.clone(),
    });
    runtime.op_state().borrow_mut().put(JsHostGatewayState {
        plugin_id: plugin_id.clone(),
        host_gateway,
    });
    runtime
        .op_state()
        .borrow_mut()
        .put(JsHostInvocationContext::default());
    runtime.op_state().borrow_mut().put(JsNpmModuleState {
        plugin_dir: plugin_dir.clone(),
        allowed_packages: npm_dependencies
            .iter()
            .map(|dependency| dependency.name.clone())
            .collect(),
    });

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

fn resolve_js_module(
    plugin_dir: &std::path::Path,
    allowed_packages: &std::collections::HashSet<String>,
    request: &str,
    parent_path: &str,
) -> Result<std::path::PathBuf, anyhow::Error> {
    let request = request.trim();
    if request.is_empty() {
        return Err(anyhow::anyhow!("require() needs a module name"));
    }

    if is_relative_module_request(request) {
        let base_dir = if parent_path.trim().is_empty() {
            plugin_dir.to_path_buf()
        } else {
            let parent = std::path::PathBuf::from(parent_path);
            parent
                .parent()
                .map(std::path::Path::to_path_buf)
                .unwrap_or_else(|| plugin_dir.to_path_buf())
        };
        return resolve_module_path(&base_dir.join(request));
    }

    if request.starts_with('/') || request.contains('\\') {
        return Err(anyhow::anyhow!(
            "require() only accepts declared package names or relative paths"
        ));
    }

    let package_name = npm_package_name(request)?;
    if !allowed_packages.contains(&package_name) {
        return Err(anyhow::anyhow!(
            "npm package '{}' is not declared in npm_dependencies",
            package_name
        ));
    }

    let package_root = plugin_dir.join("node_modules").join(&package_name);
    let remainder = request
        .strip_prefix(&package_name)
        .unwrap_or_default()
        .trim_start_matches('/');
    if remainder.is_empty() {
        resolve_module_path(&package_root)
    } else {
        resolve_module_path(&package_root.join(remainder))
    }
}

fn is_relative_module_request(request: &str) -> bool {
    request == "." || request == ".." || request.starts_with("./") || request.starts_with("../")
}

fn npm_package_name(request: &str) -> Result<String, anyhow::Error> {
    let mut parts = request.split('/');
    let first = parts.next().unwrap_or_default();
    if first.is_empty() || first == "." || first == ".." {
        return Err(anyhow::anyhow!("Invalid npm package name '{}'", request));
    }
    if first.starts_with('@') {
        let second = parts
            .next()
            .ok_or_else(|| anyhow::anyhow!("Invalid scoped npm package '{}'", request))?;
        if second.is_empty() || second == "." || second == ".." {
            return Err(anyhow::anyhow!("Invalid scoped npm package '{}'", request));
        }
        Ok(format!("{}/{}", first, second))
    } else {
        Ok(first.to_string())
    }
}

fn resolve_module_path(base: &std::path::Path) -> Result<std::path::PathBuf, anyhow::Error> {
    if base.is_file() {
        return Ok(base.to_path_buf());
    }

    for extension in ["js", "json"] {
        let candidate = base.with_extension(extension);
        if candidate.is_file() {
            return Ok(candidate);
        }
    }

    if base.is_dir() {
        let package_json = base.join("package.json");
        if package_json.is_file() {
            let package: serde_json::Value =
                serde_json::from_slice(&std::fs::read(&package_json)?)?;
            if let Some(main) = package.get("main").and_then(serde_json::Value::as_str) {
                if !main.trim().is_empty() {
                    if let Ok(path) = resolve_module_path(&base.join(main)) {
                        return Ok(path);
                    }
                }
            }
        }

        for index in ["index.js", "index.json"] {
            let candidate = base.join(index);
            if candidate.is_file() {
                return Ok(candidate);
            }
        }
    }

    Err(anyhow::anyhow!(
        "Cannot resolve JavaScript module '{}'",
        base.display()
    ))
}

fn ensure_path_inside(root: &std::path::Path, path: &std::path::Path) -> Result<(), anyhow::Error> {
    let canonical_root = std::fs::canonicalize(root)?;
    if path.starts_with(&canonical_root) {
        Ok(())
    } else {
        Err(anyhow::anyhow!(
            "Resolved JavaScript module escapes plugin directory"
        ))
    }
}

fn is_network_allowed(allowed_domains: &[String], url: &str) -> bool {
    if allowed_domains.is_empty() {
        return false;
    }

    let Ok(parsed) = url::Url::parse(url) else {
        return false;
    };
    let Some(host) = parsed.host_str() else {
        return false;
    };

    allowed_domains
        .iter()
        .any(|domain| domain_matches(host, domain))
}

fn domain_matches(host: &str, pattern: &str) -> bool {
    let host = host.to_ascii_lowercase();
    let pattern = pattern.to_ascii_lowercase();

    if pattern == "*" {
        true
    } else if let Some(base) = pattern.strip_prefix("*.") {
        host == base || host.ends_with(&format!(".{}", base))
    } else {
        host == pattern
    }
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
        let temp_dir = tempfile::tempdir().unwrap();
        let result = create_js_runtime_with_bindings(
            "test-plugin".to_string(),
            "test-plugin@1.0.0".to_string(),
            config,
            None,
            None,
            temp_dir.path().to_path_buf(),
            Vec::new(),
        );
        assert!(result.is_ok());
    }

    #[test]
    fn js_op_fetch_network_permission_denies_by_default() {
        assert!(!is_network_allowed(&[], "https://example.com"));
        assert!(is_network_allowed(
            &["example.com".to_string()],
            "https://example.com/path"
        ));
        assert!(is_network_allowed(
            &["*.example.com".to_string()],
            "https://api.example.com/path"
        ));
        assert!(is_network_allowed(
            &["*".to_string()],
            "https://plugins.example.net/path"
        ));
        assert!(!is_network_allowed(
            &["example.com".to_string()],
            "https://evil.example.net/path"
        ));
    }

    #[tokio::test]
    async fn direct_js_op_fetch_is_denied_without_network_permission() {
        let config = serde_json::json!({});
        let temp_dir = tempfile::tempdir().unwrap();
        let mut runtime = create_js_runtime_with_bindings(
            "test-plugin".to_string(),
            "test-plugin@1.0.0".to_string(),
            config,
            None,
            None,
            temp_dir.path().to_path_buf(),
            Vec::new(),
        )
        .unwrap();

        let result = runtime.execute_script(
            "<direct_op_fetch>",
            r#"
            globalThis.__directFetchStatus = "pending";
            Deno.core.ops.op_fetch("https://example.com", {})
                .then(() => { globalThis.__directFetchStatus = "success"; })
                .catch((error) => { globalThis.__directFetchStatus = String(error); });
            "#
            .to_string()
            .into(),
        );
        assert!(result.is_ok());

        runtime.run_event_loop(Default::default()).await.unwrap();

        let scope = &mut runtime.handle_scope();
        let context = scope.get_current_context();
        let global = context.global(scope);
        let key = deno_core::v8::String::new(scope, "__directFetchStatus").unwrap();
        let value = global.get(scope, key.into()).unwrap();
        let status = value.to_string(scope).unwrap().to_rust_string_lossy(scope);

        assert!(status.contains("Network access denied"));
    }

    #[tokio::test]
    async fn ting_host_invoke_rejects_when_gateway_missing() {
        let config = serde_json::json!({});
        let temp_dir = tempfile::tempdir().unwrap();
        let mut runtime = create_js_runtime_with_bindings(
            "test-plugin".to_string(),
            "test-plugin@1.0.0".to_string(),
            config,
            None,
            None,
            temp_dir.path().to_path_buf(),
            Vec::new(),
        )
        .unwrap();

        let result = runtime.execute_script(
            "<host_invoke_without_gateway>",
            r#"
            globalThis.__hostInvokeStatus = "pending";
            Ting.host.invoke("books.list", {})
                .then(() => { globalThis.__hostInvokeStatus = "success"; })
                .catch((error) => { globalThis.__hostInvokeStatus = String(error); });
            "#
            .to_string()
            .into(),
        );
        assert!(result.is_ok());

        runtime.run_event_loop(Default::default()).await.unwrap();

        let scope = &mut runtime.handle_scope();
        let context = scope.get_current_context();
        let global = context.global(scope);
        let key = deno_core::v8::String::new(scope, "__hostInvokeStatus").unwrap();
        let value = global.get(scope, key.into()).unwrap();
        let status = value.to_string(scope).unwrap().to_rust_string_lossy(scope);

        assert!(status.contains("Ting.host.invoke is not configured"));
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
            "test-plugin".to_string(),
            "test-plugin@1.0.0".to_string(),
            config,
            Some(&sandbox),
            None,
            tempfile::tempdir().unwrap().path().to_path_buf(),
            Vec::new(),
        )
        .unwrap();

        let test_code = r#"
            const allowedPaths = Ting.sandbox.allowedPaths;
            JSON.stringify({ allowedPaths })
        "#;
        let result = runtime.execute_script("<test_sandbox>", test_code.to_string().into());
        assert!(result.is_ok());
    }

    #[test]
    fn js_require_loads_declared_commonjs_package() {
        let temp_dir = tempfile::tempdir().unwrap();
        let package_dir = temp_dir.path().join("node_modules").join("demo-package");
        std::fs::create_dir_all(&package_dir).unwrap();
        std::fs::write(
            package_dir.join("package.json"),
            r#"{ "name": "demo-package", "main": "main.js" }"#,
        )
        .unwrap();
        std::fs::write(
            package_dir.join("main.js"),
            r#"const util = require("./util"); module.exports = { answer: util.answer + 1 };"#,
        )
        .unwrap();
        std::fs::write(package_dir.join("util.js"), r#"exports.answer = 41;"#).unwrap();

        let mut runtime = create_js_runtime_with_bindings(
            "test-plugin".to_string(),
            "test-plugin@1.0.0".to_string(),
            serde_json::json!({}),
            None,
            None,
            temp_dir.path().to_path_buf(),
            vec![NpmDependency::new(
                "demo-package".to_string(),
                "1.0.0".to_string(),
            )],
        )
        .unwrap();

        runtime
            .execute_script(
                "<declared_npm_require>",
                r#"
                globalThis.__npmRequireAnswer = String(require("demo-package").answer);
                "#
                .to_string()
                .into(),
            )
            .unwrap();

        let scope = &mut runtime.handle_scope();
        let context = scope.get_current_context();
        let global = context.global(scope);
        let key = deno_core::v8::String::new(scope, "__npmRequireAnswer").unwrap();
        let value = global.get(scope, key.into()).unwrap();
        let answer = value.to_string(scope).unwrap().to_rust_string_lossy(scope);

        assert_eq!(answer, "42");
    }

    #[test]
    fn js_require_rejects_undeclared_package() {
        let temp_dir = tempfile::tempdir().unwrap();
        let package_dir = temp_dir.path().join("node_modules").join("demo-package");
        std::fs::create_dir_all(&package_dir).unwrap();
        std::fs::write(package_dir.join("index.js"), r#"module.exports = {};"#).unwrap();

        let mut runtime = create_js_runtime_with_bindings(
            "test-plugin".to_string(),
            "test-plugin@1.0.0".to_string(),
            serde_json::json!({}),
            None,
            None,
            temp_dir.path().to_path_buf(),
            Vec::new(),
        )
        .unwrap();

        runtime
            .execute_script(
                "<undeclared_npm_require>",
                r#"
                try {
                    require("demo-package");
                    globalThis.__npmRequireStatus = "loaded";
                } catch (error) {
                    globalThis.__npmRequireStatus = String(error);
                }
                "#
                .to_string()
                .into(),
            )
            .unwrap();

        let scope = &mut runtime.handle_scope();
        let context = scope.get_current_context();
        let global = context.global(scope);
        let key = deno_core::v8::String::new(scope, "__npmRequireStatus").unwrap();
        let value = global.get(scope, key.into()).unwrap();
        let status = value.to_string(scope).unwrap().to_rust_string_lossy(scope);

        assert!(status.contains("not declared in npm_dependencies"));
    }
}
