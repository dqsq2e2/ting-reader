//! Plugin type definitions
//!
//! This module defines the core plugin interfaces and data structures for the plugin system.
//! All plugins must implement the base Plugin trait and provide metadata.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use crate::core::error::Result;

/// Unique identifier for a plugin instance
pub type PluginId = String;

/// Base plugin trait that all plugins must implement
/// 
/// This trait defines the lifecycle methods and basic information
/// that every plugin must provide.
#[async_trait::async_trait]
pub trait Plugin: Send + Sync {
    /// Get plugin metadata
    fn metadata(&self) -> &PluginMetadata;
    
    /// Initialize the plugin with the given context
    /// 
    /// This method is called once when the plugin is loaded.
    /// Plugins should perform any necessary setup here.
    async fn initialize(&self, context: &PluginContext) -> Result<()>;
    
    /// Shutdown the plugin and cleanup resources
    /// 
    /// This method is called before the plugin is unloaded.
    /// Plugins should release all resources here.
    async fn shutdown(&self) -> Result<()>;
    
    /// Perform garbage collection or resource cleanup
    /// 
    /// This method is called periodically or when memory pressure is detected.
    async fn garbage_collect(&self) -> Result<()> {
        Ok(())
    }

    /// Get the plugin type
    fn plugin_type(&self) -> PluginType;
    
    /// Get a reference to self as Any for downcasting
    fn as_any(&self) -> &dyn std::any::Any;
}

/// Plugin type enumeration
/// 
/// Defines the three types of plugins supported by the system:
/// - Scraper: Fetches book metadata from external sources
/// - Format: Handles audio file decryption and transcoding
/// - Utility: Provides auxiliary functionality and enhancements
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PluginType {
    /// Scraper plugin for fetching book metadata
    Scraper,
    /// Format plugin for audio file processing
    Format,
    /// Utility plugin for auxiliary functionality
    Utility,
}

impl std::fmt::Display for PluginType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PluginType::Scraper => write!(f, "scraper"),
            PluginType::Format => write!(f, "format"),
            PluginType::Utility => write!(f, "utility"),
        }
    }
}

/// Plugin metadata
/// 
/// Contains all the information about a plugin including its identity,
/// version, dependencies, and permissions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginMetadata {
    /// Plugin name (unique identifier)
    pub name: String,
    
    /// Plugin version (semantic versioning)
    pub version: String,
    
    /// Plugin type
    pub plugin_type: PluginType,
    
    /// Plugin author
    pub author: String,
    
    /// Plugin description
    pub description: String,
    
    /// Plugin license (e.g., "MIT", "Apache-2.0")
    #[serde(default)]
    pub license: Option<String>,
    
    /// Plugin homepage URL
    #[serde(default)]
    pub homepage: Option<String>,
    
    /// Entry point file (e.g., "plugin.wasm" or "libplugin.so")
    pub entry_point: String,
    
    /// Plugin dependencies (other plugins)
    #[serde(default)]
    pub dependencies: Vec<PluginDependency>,
    
    /// npm dependencies (for JavaScript plugins)
    #[serde(default)]
    pub npm_dependencies: Vec<super::npm_manager::NpmDependency>,
    
    /// Required permissions
    #[serde(default)]
    pub permissions: Vec<super::sandbox::Permission>,
    
    /// Configuration schema (JSON Schema)
    #[serde(default)]
    pub config_schema: Option<serde_json::Value>,
    
    /// Minimum core system version required
    #[serde(default)]
    pub min_core_version: Option<String>,

    /// Supported file extensions (for Format plugins)
    #[serde(default)]
    pub supported_extensions: Option<Vec<String>>,
}

/// Plan for decrypting a file stream
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecryptionPlan {
    /// Segments of the file to process
    pub segments: Vec<DecryptionSegment>,
    /// Total size of the output stream (if known)
    pub total_size: Option<u64>,
}

/// A segment of the decryption plan
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum DecryptionSegment {
    /// Plain text segment (direct copy)
    #[serde(rename = "plain")]
    Plain { 
        /// Start offset in the source file
        offset: u64, 
        /// Length of the segment (-1 or 0 for "until end")
        length: i64 
    },
    
    /// Encrypted segment (needs decryption)
    #[serde(rename = "encrypted")]
    Encrypted { 
        /// Start offset in the source file
        offset: u64, 
        /// Length of the segment
        length: i64,
        /// Parameters for decryption (passed to decrypt_chunk)
        params: serde_json::Value 
    },
}

impl PluginMetadata {
    /// Create a new plugin metadata with required fields
    pub fn new(
        name: String,
        version: String,
        plugin_type: PluginType,
        author: String,
        description: String,
        entry_point: String,
    ) -> Self {
        Self {
            name,
            version,
            plugin_type,
            author,
            description,
            license: None,
            homepage: None,
            entry_point,
            dependencies: Vec::new(),
            npm_dependencies: Vec::new(),
            permissions: Vec::new(),
            config_schema: None,
            min_core_version: None,
            supported_extensions: None,
        }
    }
    
    /// Add a dependency to this plugin
    pub fn with_dependency(mut self, dependency: PluginDependency) -> Self {
        self.dependencies.push(dependency);
        self
    }
    
    /// Add an npm dependency to this plugin
    pub fn with_npm_dependency(mut self, dependency: super::npm_manager::NpmDependency) -> Self {
        self.npm_dependencies.push(dependency);
        self
    }
    
    /// Add a permission to this plugin
    pub fn with_permission(mut self, permission: super::sandbox::Permission) -> Self {
        self.permissions.push(permission);
        self
    }
    
    /// Set the configuration schema
    pub fn with_config_schema(mut self, schema: serde_json::Value) -> Self {
        self.config_schema = Some(schema);
        self
    }
    
    /// Set supported extensions
    pub fn with_supported_extensions(mut self, extensions: Vec<String>) -> Self {
        self.supported_extensions = Some(extensions);
        self
    }

    /// Get the unique plugin ID (name@version)
    pub fn id(&self) -> PluginId {
        format!("{}@{}", self.name, self.version)
    }
}

/// Plugin dependency specification
/// 
/// Specifies a dependency on another plugin with version requirements.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginDependency {
    /// Name of the required plugin
    pub plugin_name: String,
    
    /// Version requirement (e.g., "^1.0.0", ">=2.0.0")
    pub version_requirement: String,
}

impl PluginDependency {
    /// Create a new plugin dependency
    pub fn new(plugin_name: String, version_requirement: String) -> Self {
        Self {
            plugin_name,
            version_requirement,
        }
    }
}

/// Plugin runtime context
/// 
/// Provides plugins with access to system resources and configuration.
/// This is passed to plugins during initialization.
#[derive(Clone)]
pub struct PluginContext {
    /// Plugin-specific configuration (JSON)
    pub config: serde_json::Value,
    
    /// Plugin data directory (for storing plugin-specific data)
    pub data_dir: PathBuf,
    
    /// Logger instance for the plugin
    pub logger: Arc<dyn PluginLogger>,
    
    /// Event bus for publishing and subscribing to events
    pub event_bus: Arc<dyn PluginEventBus>,
}

impl PluginContext {
    /// Create a new plugin context
    pub fn new(
        config: serde_json::Value,
        data_dir: PathBuf,
        logger: Arc<dyn PluginLogger>,
        event_bus: Arc<dyn PluginEventBus>,
    ) -> Self {
        Self {
            config,
            data_dir,
            logger,
            event_bus,
        }
    }
}

/// Plugin logger trait
/// 
/// Provides logging functionality to plugins.
pub trait PluginLogger: Send + Sync {
    /// Log a debug message
    fn debug(&self, message: &str);
    
    /// Log an info message
    fn info(&self, message: &str);
    
    /// Log a warning message
    fn warn(&self, message: &str);
    
    /// Log an error message
    fn error(&self, message: &str);
}

/// Plugin event bus trait
/// 
/// Allows plugins to publish and subscribe to events.
pub trait PluginEventBus: Send + Sync {
    /// Publish an event
    fn publish(&self, event_type: &str, data: serde_json::Value) -> Result<()>;
    
    /// Subscribe to an event type
    fn subscribe(&self, event_type: &str, handler: Box<dyn Fn(serde_json::Value) + Send + Sync>) -> Result<String>;
    
    /// Unsubscribe from an event
    fn unsubscribe(&self, subscription_id: &str) -> Result<()>;
}

/// Plugin state enumeration
/// 
/// Tracks the current state of a plugin in its lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PluginState {
    /// Plugin has been discovered but not yet loaded
    Discovered,
    
    /// Plugin is currently being loaded
    Loading,
    
    /// Plugin has been loaded but not initialized
    Loaded,
    
    /// Plugin is being initialized
    Initializing,
    
    /// Plugin is active and ready to use
    Active,
    
    /// Plugin is currently executing
    Executing,
    
    /// Plugin is being unloaded
    Unloading,
    
    /// Plugin has been unloaded
    Unloaded,
    
    /// Plugin failed to load or initialize
    Failed,
}

/// Event triggered when a plugin's state changes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginStateEvent {
    /// ID of the plugin
    pub plugin_id: PluginId,
    /// Name of the plugin
    pub plugin_name: String,
    /// Previous state (None if new)
    pub old_state: Option<PluginState>,
    /// New state
    pub new_state: PluginState,
    /// Timestamp of the event
    pub timestamp: i64,
}

/// Plugin statistics
/// 
/// Tracks performance and usage metrics for a plugin.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginStats {
    /// Total number of calls to this plugin
    pub total_calls: u64,
    
    /// Number of successful calls
    pub successful_calls: u64,
    
    /// Number of failed calls
    pub failed_calls: u64,
    
    /// Minimum execution time in milliseconds
    pub min_execution_time_ms: Option<u64>,
    
    /// Maximum execution time in milliseconds
    pub max_execution_time_ms: Option<u64>,
    
    /// Average execution time in milliseconds
    pub avg_execution_time_ms: Option<f64>,
    
    /// P95 execution time in milliseconds
    pub p95_execution_time_ms: Option<u64>,
    
    /// Current memory usage in bytes
    pub memory_usage_bytes: Option<u64>,
    
    /// Peak memory usage in bytes
    pub peak_memory_bytes: Option<u64>,
    
    /// Timestamp of last call (Unix timestamp)
    pub last_call_timestamp: Option<i64>,
    
    /// Execution time history for P95 calculation (circular buffer, last 1000 calls)
    #[serde(skip)]
    execution_times: std::collections::VecDeque<u64>,
    
    /// Error type distribution (error type -> count)
    pub error_distribution: std::collections::HashMap<String, u64>,
}

impl PluginStats {
    /// Maximum number of execution times to keep for P95 calculation
    const MAX_EXECUTION_TIMES: usize = 1000;
    
    /// Create new empty statistics
    pub fn new() -> Self {
        Self {
            total_calls: 0,
            successful_calls: 0,
            failed_calls: 0,
            min_execution_time_ms: None,
            max_execution_time_ms: None,
            avg_execution_time_ms: None,
            p95_execution_time_ms: None,
            memory_usage_bytes: None,
            peak_memory_bytes: None,
            last_call_timestamp: None,
            execution_times: std::collections::VecDeque::with_capacity(Self::MAX_EXECUTION_TIMES),
            error_distribution: std::collections::HashMap::new(),
        }
    }
    
    /// Record a successful call with execution time
    pub fn record_success(&mut self, execution_time_ms: u64) {
        self.total_calls += 1;
        self.successful_calls += 1;
        self.update_execution_time(execution_time_ms);
        self.last_call_timestamp = Some(chrono::Utc::now().timestamp());
    }
    
    /// Record a failed call with error type
    pub fn record_failure(&mut self, error_type: Option<&str>) {
        self.total_calls += 1;
        self.failed_calls += 1;
        self.last_call_timestamp = Some(chrono::Utc::now().timestamp());
        
        // Track error type distribution
        if let Some(err_type) = error_type {
            *self.error_distribution.entry(err_type.to_string()).or_insert(0) += 1;
        } else {
            *self.error_distribution.entry("Unknown".to_string()).or_insert(0) += 1;
        }
    }
    
    /// Update memory usage statistics
    pub fn update_memory_usage(&mut self, bytes: u64) {
        self.memory_usage_bytes = Some(bytes);
        
        // Update peak memory
        self.peak_memory_bytes = Some(
            self.peak_memory_bytes
                .map(|peak| peak.max(bytes))
                .unwrap_or(bytes)
        );
    }
    
    /// Update execution time statistics
    fn update_execution_time(&mut self, time_ms: u64) {
        // Update min
        self.min_execution_time_ms = Some(
            self.min_execution_time_ms
                .map(|min| min.min(time_ms))
                .unwrap_or(time_ms)
        );
        
        // Update max
        self.max_execution_time_ms = Some(
            self.max_execution_time_ms
                .map(|max| max.max(time_ms))
                .unwrap_or(time_ms)
        );
        
        // Update average
        let current_avg = self.avg_execution_time_ms.unwrap_or(0.0);
        let count = self.successful_calls as f64;
        self.avg_execution_time_ms = Some(
            (current_avg * (count - 1.0) + time_ms as f64) / count
        );
        
        // Add to execution times history for P95 calculation
        if self.execution_times.len() >= Self::MAX_EXECUTION_TIMES {
            self.execution_times.pop_front();
        }
        self.execution_times.push_back(time_ms);
        
        // Calculate P95
        self.calculate_p95();
    }
    
    /// Calculate P95 execution time from the execution times history
    fn calculate_p95(&mut self) {
        if self.execution_times.is_empty() {
            self.p95_execution_time_ms = None;
            return;
        }
        
        // Sort execution times to find P95
        let mut sorted_times: Vec<u64> = self.execution_times.iter().copied().collect();
        sorted_times.sort_unstable();
        
        // Calculate P95 index (95th percentile)
        let p95_index = ((sorted_times.len() as f64 * 0.95).ceil() as usize).saturating_sub(1);
        self.p95_execution_time_ms = Some(sorted_times[p95_index]);
    }
    
    /// Calculate success rate as a percentage
    pub fn success_rate(&self) -> f64 {
        if self.total_calls == 0 {
            0.0
        } else {
            (self.successful_calls as f64 / self.total_calls as f64) * 100.0
        }
    }
    
    /// Get error type distribution as a sorted vector
    pub fn error_distribution_sorted(&self) -> Vec<(String, u64)> {
        let mut distribution: Vec<(String, u64)> = self.error_distribution
            .iter()
            .map(|(k, v)| (k.clone(), *v))
            .collect();
        distribution.sort_by(|a, b| b.1.cmp(&a.1)); // Sort by count descending
        distribution
    }
}

impl Default for PluginStats {
    fn default() -> Self {
        Self::new()
    }
}

/// Performance thresholds for alerting
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceThresholds {
    /// Maximum acceptable average execution time in milliseconds
    pub max_avg_execution_time_ms: Option<u64>,
    
    /// Maximum acceptable P95 execution time in milliseconds
    pub max_p95_execution_time_ms: Option<u64>,
    
    /// Maximum acceptable memory usage in bytes
    pub max_memory_bytes: Option<u64>,
    
    /// Minimum acceptable success rate (0.0 - 100.0)
    pub min_success_rate: Option<f64>,
    
    /// Maximum acceptable error rate (0.0 - 100.0)
    pub max_error_rate: Option<f64>,
}

impl PerformanceThresholds {
    /// Create new thresholds with no limits
    pub fn new() -> Self {
        Self {
            max_avg_execution_time_ms: None,
            max_p95_execution_time_ms: None,
            max_memory_bytes: None,
            min_success_rate: None,
            max_error_rate: None,
        }
    }
    
    /// Create default thresholds with reasonable limits
    pub fn default_limits() -> Self {
        Self {
            max_avg_execution_time_ms: Some(1000), // 1 second
            max_p95_execution_time_ms: Some(5000), // 5 seconds
            max_memory_bytes: Some(512 * 1024 * 1024), // 512 MB
            min_success_rate: Some(95.0), // 95%
            max_error_rate: Some(5.0), // 5%
        }
    }
}

impl Default for PerformanceThresholds {
    fn default() -> Self {
        Self::new()
    }
}

/// Performance alert indicating a threshold violation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceAlert {
    /// Type of threshold that was exceeded
    pub alert_type: AlertType,
    
    /// Current value that triggered the alert
    pub current_value: f64,
    
    /// Threshold value that was exceeded
    pub threshold_value: f64,
    
    /// Human-readable message
    pub message: String,
}

/// Types of performance alerts
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum AlertType {
    /// Average execution time exceeded
    AvgExecutionTime,
    
    /// P95 execution time exceeded
    P95ExecutionTime,
    
    /// Memory usage exceeded
    MemoryUsage,
    
    /// Success rate below threshold
    LowSuccessRate,
    
    /// Error rate above threshold
    HighErrorRate,
}

impl PluginStats {
    /// Check if any performance thresholds are exceeded
    /// Returns a list of alerts for any violated thresholds
    pub fn check_thresholds(&self, thresholds: &PerformanceThresholds) -> Vec<PerformanceAlert> {
        let mut alerts = Vec::new();
        
        // Check average execution time
        if let (Some(avg), Some(max_avg)) = (self.avg_execution_time_ms, thresholds.max_avg_execution_time_ms) {
            if avg > max_avg as f64 {
                alerts.push(PerformanceAlert {
                    alert_type: AlertType::AvgExecutionTime,
                    current_value: avg,
                    threshold_value: max_avg as f64,
                    message: format!(
                        "Average execution time ({:.2}ms) exceeds threshold ({}ms)",
                        avg, max_avg
                    ),
                });
            }
        }
        
        // Check P95 execution time
        if let (Some(p95), Some(max_p95)) = (self.p95_execution_time_ms, thresholds.max_p95_execution_time_ms) {
            if p95 > max_p95 {
                alerts.push(PerformanceAlert {
                    alert_type: AlertType::P95ExecutionTime,
                    current_value: p95 as f64,
                    threshold_value: max_p95 as f64,
                    message: format!(
                        "P95 execution time ({}ms) exceeds threshold ({}ms)",
                        p95, max_p95
                    ),
                });
            }
        }
        
        // Check memory usage
        if let (Some(memory), Some(max_memory)) = (self.memory_usage_bytes, thresholds.max_memory_bytes) {
            if memory > max_memory {
                alerts.push(PerformanceAlert {
                    alert_type: AlertType::MemoryUsage,
                    current_value: memory as f64,
                    threshold_value: max_memory as f64,
                    message: format!(
                        "Memory usage ({} bytes) exceeds threshold ({} bytes)",
                        memory, max_memory
                    ),
                });
            }
        }
        
        // Check success rate
        if let Some(min_success) = thresholds.min_success_rate {
            let success_rate = self.success_rate();
            if success_rate < min_success {
                alerts.push(PerformanceAlert {
                    alert_type: AlertType::LowSuccessRate,
                    current_value: success_rate,
                    threshold_value: min_success,
                    message: format!(
                        "Success rate ({:.2}%) is below threshold ({:.2}%)",
                        success_rate, min_success
                    ),
                });
            }
        }
        
        // Check error rate
        if let Some(max_error) = thresholds.max_error_rate {
            let error_rate = if self.total_calls == 0 {
                0.0
            } else {
                (self.failed_calls as f64 / self.total_calls as f64) * 100.0
            };
            
            if error_rate > max_error {
                alerts.push(PerformanceAlert {
                    alert_type: AlertType::HighErrorRate,
                    current_value: error_rate,
                    threshold_value: max_error,
                    message: format!(
                        "Error rate ({:.2}%) exceeds threshold ({:.2}%)",
                        error_rate, max_error
                    ),
                });
            }
        }
        
        alerts
    }
    
    /// Export statistics to JSON string
    pub fn export_json(&self) -> crate::core::error::Result<String> {
        serde_json::to_string_pretty(self)
            .map_err(|e| crate::core::error::TingError::SerializationError(e.to_string()))
    }
    
    /// Export statistics to CSV format
    /// Returns a CSV string with headers and one data row
    pub fn export_csv(&self) -> String {
        let mut csv = String::new();
        
        // Headers
        csv.push_str("total_calls,successful_calls,failed_calls,");
        csv.push_str("min_execution_time_ms,max_execution_time_ms,avg_execution_time_ms,p95_execution_time_ms,");
        csv.push_str("memory_usage_bytes,peak_memory_bytes,");
        csv.push_str("success_rate,last_call_timestamp\n");
        
        // Data
        csv.push_str(&format!("{},{},{},", 
            self.total_calls, 
            self.successful_calls, 
            self.failed_calls
        ));
        
        csv.push_str(&format!("{},{},{},{},",
            self.min_execution_time_ms.map(|v| v.to_string()).unwrap_or_default(),
            self.max_execution_time_ms.map(|v| v.to_string()).unwrap_or_default(),
            self.avg_execution_time_ms.map(|v| format!("{:.2}", v)).unwrap_or_default(),
            self.p95_execution_time_ms.map(|v| v.to_string()).unwrap_or_default(),
        ));
        
        csv.push_str(&format!("{},{},",
            self.memory_usage_bytes.map(|v| v.to_string()).unwrap_or_default(),
            self.peak_memory_bytes.map(|v| v.to_string()).unwrap_or_default(),
        ));
        
        csv.push_str(&format!("{:.2},{}\n",
            self.success_rate(),
            self.last_call_timestamp.map(|v| v.to_string()).unwrap_or_default(),
        ));
        
        csv
    }
    
    /// Compare this statistics with another, returning the differences
    pub fn compare(&self, other: &PluginStats) -> PerformanceComparison {
        PerformanceComparison {
            total_calls_diff: other.total_calls as i64 - self.total_calls as i64,
            successful_calls_diff: other.successful_calls as i64 - self.successful_calls as i64,
            failed_calls_diff: other.failed_calls as i64 - self.failed_calls as i64,
            
            avg_execution_time_diff: match (self.avg_execution_time_ms, other.avg_execution_time_ms) {
                (Some(a), Some(b)) => Some(b - a),
                _ => None,
            },
            
            p95_execution_time_diff: match (self.p95_execution_time_ms, other.p95_execution_time_ms) {
                (Some(a), Some(b)) => Some(b as i64 - a as i64),
                _ => None,
            },
            
            memory_usage_diff: match (self.memory_usage_bytes, other.memory_usage_bytes) {
                (Some(a), Some(b)) => Some(b as i64 - a as i64),
                _ => None,
            },
            
            success_rate_diff: other.success_rate() - self.success_rate(),
        }
    }
}

/// Result of comparing two PluginStats
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceComparison {
    /// Difference in total calls (positive means increase)
    pub total_calls_diff: i64,
    
    /// Difference in successful calls
    pub successful_calls_diff: i64,
    
    /// Difference in failed calls
    pub failed_calls_diff: i64,
    
    /// Difference in average execution time (ms)
    pub avg_execution_time_diff: Option<f64>,
    
    /// Difference in P95 execution time (ms)
    pub p95_execution_time_diff: Option<i64>,
    
    /// Difference in memory usage (bytes)
    pub memory_usage_diff: Option<i64>,
    
    /// Difference in success rate (percentage points)
    pub success_rate_diff: f64,
}

impl PerformanceComparison {
    /// Check if performance has improved (lower execution times, higher success rate)
    pub fn is_improvement(&self) -> bool {
        let exec_time_improved = self.avg_execution_time_diff
            .map(|diff| diff < 0.0)
            .unwrap_or(true);
        
        let p95_improved = self.p95_execution_time_diff
            .map(|diff| diff < 0)
            .unwrap_or(true);
        
        let memory_improved = self.memory_usage_diff
            .map(|diff| diff < 0)
            .unwrap_or(true);
        
        let success_improved = self.success_rate_diff > 0.0;
        
        // Consider it an improvement if most metrics improved
        let improvements = [exec_time_improved, p95_improved, memory_improved, success_improved]
            .iter()
            .filter(|&&x| x)
            .count();
        
        improvements >= 3
    }
    
    /// Get a summary message of the comparison
    pub fn summary(&self) -> String {
        let mut parts = Vec::new();
        
        if let Some(diff) = self.avg_execution_time_diff {
            let direction = if diff < 0.0 { "decreased" } else { "increased" };
            parts.push(format!("Avg execution time {} by {:.2}ms", direction, diff.abs()));
        }
        
        if let Some(diff) = self.p95_execution_time_diff {
            let direction = if diff < 0 { "decreased" } else { "increased" };
            parts.push(format!("P95 execution time {} by {}ms", direction, diff.abs()));
        }
        
        if let Some(diff) = self.memory_usage_diff {
            let direction = if diff < 0 { "decreased" } else { "increased" };
            parts.push(format!("Memory usage {} by {} bytes", direction, diff.abs()));
        }
        
        let direction = if self.success_rate_diff > 0.0 { "increased" } else { "decreased" };
        parts.push(format!("Success rate {} by {:.2}%", direction, self.success_rate_diff.abs()));
        
        parts.join(", ")
    }
}
