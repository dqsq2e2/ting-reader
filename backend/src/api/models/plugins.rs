use crate::plugin::types::{LocalizedText, PluginCapability, ScraperCapabilities};
use serde::{Deserialize, Serialize};

// Plugin Management API models

/// Response for plugin list
#[derive(Debug, Serialize)]
pub struct PluginsListResponse {
    /// List of plugins
    pub plugins: Vec<PluginInfoResponse>,
    /// Total number of plugins
    pub total: usize,
}

/// Plugin information response
#[derive(Debug, Serialize)]
pub struct PluginInfoResponse {
    /// Plugin ID
    pub id: String,
    /// Plugin name
    pub name: String,
    /// Plugin version
    pub version: String,
    /// Plugin type (scraper, format, utility)
    pub plugin_type: String,
    /// Plugin runtime (wasm, javascript, native)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub runtime: Option<String>,
    /// Plugin author
    pub author: Option<String>,
    /// Plugin description
    pub description: Option<String>,
    /// Localized plugin descriptions keyed by locale, e.g. zh/en/ja
    pub description_i18n: LocalizedText,
    /// Whether the plugin is enabled
    pub is_enabled: bool,
    /// Plugin state (loading, loaded, active, unloading, unloaded, failed)
    pub state: String,
    /// Plugin load error, available when state is failed
    pub error: Option<String>,
    /// Plugin statistics
    pub stats: Option<PluginStatsResponse>,
    /// Configuration schema (JSON Schema format)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config_schema: Option<serde_json::Value>,
    /// Plugin permissions
    #[serde(skip_serializing_if = "Option::is_none")]
    pub permissions: Option<Vec<String>>,
    /// Plugin license
    #[serde(skip_serializing_if = "Option::is_none")]
    pub license: Option<String>,
    /// Plugin repository
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repo: Option<String>,
    /// Minimum Ting Reader core version required
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_core_version: Option<String>,
    /// Minimum Flutter client version required for client-facing plugins
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_flutter_version: Option<String>,
    /// Scraper capability declaration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scraper: Option<ScraperCapabilities>,
    /// Generic manifest v2 capability declarations
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub capabilities: Vec<PluginCapability>,
}

/// Plugin statistics response
#[derive(Debug, Serialize)]
pub struct PluginStatsResponse {
    /// Total number of calls
    pub total_calls: u64,
    /// Number of successful calls
    pub successful_calls: u64,
    /// Number of failed calls
    pub failed_calls: u64,
    /// Average execution time in milliseconds
    pub avg_execution_time_ms: f64,
}

/// Response for plugin detail
#[derive(Debug, Serialize)]
pub struct PluginDetailResponse {
    /// Plugin ID
    pub id: String,
    /// Plugin name
    pub name: String,
    /// Plugin version
    pub version: String,
    /// Plugin type (scraper, format, utility)
    pub plugin_type: String,
    /// Plugin runtime (wasm, javascript, native)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub runtime: Option<String>,
    /// Plugin author
    pub author: Option<String>,
    /// Plugin description
    pub description: Option<String>,
    /// Localized plugin descriptions keyed by locale, e.g. zh/en/ja
    pub description_i18n: LocalizedText,
    /// Plugin license
    pub license: Option<String>,
    /// Plugin repository
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repo: Option<String>,
    /// Minimum Ting Reader core version required
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_core_version: Option<String>,
    /// Minimum Flutter client version required for client-facing plugins
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_flutter_version: Option<String>,
    /// Whether the plugin is enabled
    pub is_enabled: bool,
    /// Plugin state
    pub state: String,
    /// Plugin load error, available when state is failed
    pub error: Option<String>,
    /// Plugin entry point
    pub entry_point: String,
    /// Plugin dependencies
    pub dependencies: Vec<PluginDependencyResponse>,
    /// Plugin permissions
    pub permissions: Vec<String>,
    /// Supported file extensions (format plugins only)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub supported_extensions: Option<Vec<String>>,
    /// Configuration schema (JSON Schema format)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config_schema: Option<serde_json::Value>,
    /// Scraper capability declaration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scraper: Option<ScraperCapabilities>,
    /// Generic manifest v2 capability declarations
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub capabilities: Vec<PluginCapability>,
    /// Plugin statistics
    pub stats: Option<PluginStatsResponse>,
}

/// Plugin dependency response
#[derive(Debug, Serialize)]
pub struct PluginDependencyResponse {
    /// Dependency plugin name
    pub plugin_name: String,
    /// Version requirement
    pub version_requirement: String,
}

/// Request body for installing a plugin
#[derive(Debug, Deserialize)]
pub struct InstallPluginRequest {
    /// Path to the plugin directory or file
    pub path: String,
}

/// Request body for installing a plugin from the store
#[derive(Debug, Deserialize)]
pub struct InstallStorePluginRequest {
    /// ID of the plugin to install
    pub plugin_id: String,
    /// Whether the user has accepted the risk warning for unsigned/untrusted packages.
    #[serde(default)]
    pub accept_unverified: bool,
}

/// Response for plugin installation
#[derive(Debug, Serialize)]
pub struct InstallPluginResponse {
    /// Installed plugin ID
    pub plugin_id: String,
    /// Success message
    pub message: String,
}

/// Response returned when installing an unverified plugin requires user confirmation.
#[derive(Debug, Serialize)]
pub struct UnverifiedPluginInstallResponse {
    pub requires_confirmation: bool,
    pub verification_status: String,
    pub plugin_id: String,
    pub plugin_name: String,
    pub plugin_version: String,
    pub publisher: String,
    pub warning: String,
}

/// Response for plugin reload
#[derive(Debug, Serialize)]
pub struct ReloadPluginResponse {
    /// Success message
    pub message: String,
}

/// Response for plugin uninstall
#[derive(Debug, Serialize)]
pub struct UninstallPluginResponse {
    /// Success message
    pub message: String,
}

/// Response for plugin configuration
#[derive(Debug, Serialize)]
pub struct PluginConfigResponse {
    /// Plugin ID
    pub plugin_id: String,
    /// Plugin configuration (JSON value)
    pub config: serde_json::Value,
}

/// Request body for updating plugin configuration
#[derive(Debug, Deserialize)]
pub struct UpdatePluginConfigRequest {
    /// New configuration (JSON value)
    pub config: serde_json::Value,
}

/// Response for plugin configuration update
#[derive(Debug, Serialize)]
pub struct UpdatePluginConfigResponse {
    /// Success message
    pub message: String,
}

/// Request body for invoking a declared plugin capability.
#[derive(Debug, Deserialize)]
pub struct InvokePluginCapabilityRequest {
    /// Parameters passed to the plugin capability handler.
    #[serde(default)]
    pub params: serde_json::Value,
}

/// Response body for invoking a declared plugin capability.
#[derive(Debug, Serialize)]
pub struct InvokePluginCapabilityResponse {
    /// Plugin result returned by the runtime-neutral invoke path.
    pub result: serde_json::Value,
}

/// Request body for generating a signed public plugin route URL.
#[derive(Debug, Deserialize)]
pub struct SignPluginRouteRequest {
    /// HTTP method used by the route.
    pub method: String,
    /// Declared plugin route path, e.g. /rss/main.xml.
    pub path: String,
    /// Optional TTL in seconds. Use 0 for a non-expiring signature; positive values are clamped to a safe upper bound.
    pub expires_in_seconds: Option<u64>,
    /// Whether the signed public URL should carry the current user's context.
    /// Defaults to true so external RSS/feed URLs can still use user-scoped HostGateway reads.
    pub bind_current_user: Option<bool>,
}

/// Response body for a signed public plugin route URL.
#[derive(Debug, Serialize)]
pub struct SignPluginRouteResponse {
    /// Normalized plugin route path.
    pub path: String,
    /// Unix timestamp when the signature expires. 0 means the signature does not expire.
    pub expires: i64,
    /// URL-safe signature string.
    pub signature: String,
    /// User id bound into the signature, when user-scoped public access is requested.
    pub user_id: Option<String>,
    /// Public route URL including expires/signature query params.
    pub signed_url: String,
}

/// Request body for invoking a HostGateway method on behalf of a plugin.
#[derive(Debug, Deserialize)]
pub struct InvokePluginHostRequest {
    /// Plugin instance ID, usually metadata.id@version.
    pub plugin_id: String,
    /// Stable HostGateway method name, e.g. books.list.
    pub method: String,
    /// Method parameters.
    #[serde(default)]
    pub params: serde_json::Value,
}

/// Response body for HostGateway invocation.
#[derive(Debug, Serialize)]
pub struct InvokePluginHostResponse {
    /// Method result.
    pub result: serde_json::Value,
}

/// Query params for listing registered plugin capabilities.
#[derive(Debug, Deserialize)]
pub struct ListPluginCapabilitiesQuery {
    /// Optional capability kind filter, e.g. http_route, content_processor.
    pub kind: Option<String>,
}

/// Registered capability exposed to clients.
#[derive(Debug, Serialize)]
pub struct PluginCapabilityRegistrationResponse {
    /// Plugin instance ID.
    pub plugin_id: String,
    /// Human-readable plugin name.
    pub plugin_name: String,
    /// Declared capability.
    pub capability: PluginCapability,
}

/// Query params for content processor discovery.
#[derive(Debug, Deserialize)]
pub struct FindContentProcessorsQuery {
    /// File extension, with or without leading dot.
    pub extension: String,
    /// Optional operation filter, e.g. probe, read_chunk, render_page.
    pub operation: Option<String>,
}

/// Query params for tool provider discovery.
#[derive(Debug, Deserialize)]
pub struct FindToolProvidersQuery {
    /// Optional tool name, e.g. book.search.
    pub name: Option<String>,
}

/// Query params for task handler discovery.
#[derive(Debug, Deserialize)]
pub struct FindTaskHandlersQuery {
    /// Optional task type, e.g. book.summarize.
    pub task_type: Option<String>,
}

/// Query params for event handler discovery.
#[derive(Debug, Deserialize)]
pub struct FindEventHandlersQuery {
    /// Optional event name, e.g. book.added.
    pub event: Option<String>,
}

/// Registered tool provider exposed to clients.
#[derive(Debug, Serialize)]
pub struct ToolProviderRegistrationResponse {
    /// Plugin instance ID.
    pub plugin_id: String,
    /// Human-readable plugin name.
    pub plugin_name: String,
    /// Declared tool_provider capability.
    pub capability: PluginCapability,
    /// Matched tool declaration when a name filter is provided.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool: Option<serde_json::Value>,
}
