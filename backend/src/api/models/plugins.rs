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
    /// Plugin author
    pub author: Option<String>,
    /// Plugin description
    pub description: Option<String>,
    /// Whether the plugin is enabled
    pub is_enabled: bool,
    /// Plugin state (loading, loaded, active, unloading, unloaded, failed)
    pub state: String,
    /// Plugin statistics
    pub stats: Option<PluginStatsResponse>,
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
    /// Plugin author
    pub author: Option<String>,
    /// Plugin description
    pub description: Option<String>,
    /// Plugin license
    pub license: Option<String>,
    /// Plugin homepage
    pub homepage: Option<String>,
    /// Whether the plugin is enabled
    pub is_enabled: bool,
    /// Plugin state
    pub state: String,
    /// Plugin entry point
    pub entry_point: String,
    /// Plugin dependencies
    pub dependencies: Vec<PluginDependencyResponse>,
    /// Plugin permissions
    pub permissions: Vec<String>,
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

/// Response for plugin installation
#[derive(Debug, Serialize)]
pub struct InstallPluginResponse {
    /// Installed plugin ID
    pub plugin_id: String,
    /// Success message
    pub message: String,
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
