//! Plugin type definitions
//!
//! Defines core plugin interfaces and data structures for the plugin system.

pub mod metadata;
pub mod stats;

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use crate::core::error::Result;

pub use stats::{PluginStats, PerformanceThresholds, PerformanceAlert, AlertType, PerformanceComparison};

/// Unique identifier for a plugin instance
pub type PluginId = String;

/// Base plugin trait that all plugins must implement
#[async_trait::async_trait]
pub trait Plugin: Send + Sync {
    fn metadata(&self) -> &PluginMetadata;
    async fn initialize(&self, context: &PluginContext) -> Result<()>;
    async fn shutdown(&self) -> Result<()>;
    async fn garbage_collect(&self) -> Result<()> { Ok(()) }
    fn plugin_type(&self) -> PluginType;
    fn as_any(&self) -> &dyn std::any::Any;
}

/// Plugin type enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PluginType {
    Scraper,
    Format,
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
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginMetadata {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
    pub version: String,
    pub plugin_type: PluginType,
    pub author: String,
    pub description: String,
    #[serde(default)]
    pub description_en: Option<String>,
    #[serde(default)]
    pub license: Option<String>,
    #[serde(default)]
    pub homepage: Option<String>,
    pub entry_point: String,
    /// Runtime type: "wasm", "javascript", or "native" (auto-detected if absent)
    #[serde(default)]
    pub runtime: Option<String>,
    #[serde(default)]
    pub dependencies: Vec<PluginDependency>,
    #[serde(default)]
    pub npm_dependencies: Vec<super::js::npm::NpmDependency>,
    #[serde(default)]
    pub permissions: Vec<super::wasm::sandbox::Permission>,
    #[serde(default)]
    pub config_schema: Option<serde_json::Value>,
    #[serde(default)]
    pub min_core_version: Option<String>,
    #[serde(default)]
    pub supported_extensions: Option<Vec<String>>,
}

/// Plan for decrypting a file stream
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecryptionPlan {
    pub segments: Vec<DecryptionSegment>,
    pub total_size: Option<u64>,
}

/// A segment of the decryption plan
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum DecryptionSegment {
    #[serde(rename = "plain")]
    Plain { offset: u64, length: i64 },
    #[serde(rename = "encrypted")]
    Encrypted { offset: u64, length: i64, params: serde_json::Value },
}

impl PluginMetadata {
    pub fn new(
        id: String, name: String, version: String, plugin_type: PluginType,
        author: String, description: String, entry_point: String,
    ) -> Self {
        Self {
            id, name, version, plugin_type, author, description,
            description_en: None, license: None, homepage: None,
            entry_point, runtime: None,
            dependencies: Vec::new(), npm_dependencies: Vec::new(),
            permissions: Vec::new(), config_schema: None,
            min_core_version: None, supported_extensions: None,
        }
    }

    pub fn with_runtime(mut self, runtime: String) -> Self {
        self.runtime = Some(runtime);
        self
    }

    pub fn with_dependency(mut self, dependency: PluginDependency) -> Self {
        self.dependencies.push(dependency);
        self
    }

    pub fn with_npm_dependency(mut self, dependency: super::js::npm::NpmDependency) -> Self {
        self.npm_dependencies.push(dependency);
        self
    }

    pub fn with_permission(mut self, permission: super::wasm::sandbox::Permission) -> Self {
        self.permissions.push(permission);
        self
    }

    pub fn with_config_schema(mut self, schema: serde_json::Value) -> Self {
        self.config_schema = Some(schema);
        self
    }

    pub fn with_supported_extensions(mut self, extensions: Vec<String>) -> Self {
        self.supported_extensions = Some(extensions);
        self
    }

    pub fn instance_id(&self) -> PluginId {
        format!("{}@{}", self.id, self.version)
    }
}

/// Plugin dependency specification
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(from = "PluginDependencyDef")]
pub struct PluginDependency {
    pub plugin_name: String,
    pub version_requirement: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
enum PluginDependencyDef {
    Simple(String),
    Detailed { plugin_name: String, version_requirement: String },
}

impl From<PluginDependencyDef> for PluginDependency {
    fn from(def: PluginDependencyDef) -> Self {
        match def {
            PluginDependencyDef::Simple(name) => PluginDependency { plugin_name: name, version_requirement: "*".to_string() },
            PluginDependencyDef::Detailed { plugin_name, version_requirement } => PluginDependency { plugin_name, version_requirement },
        }
    }
}

impl PluginDependency {
    pub fn new(plugin_name: String, version_requirement: String) -> Self {
        Self { plugin_name, version_requirement }
    }
}

/// Plugin runtime context
#[derive(Clone)]
pub struct PluginContext {
    pub config: serde_json::Value,
    pub data_dir: PathBuf,
    pub logger: Arc<dyn PluginLogger>,
    pub event_bus: Arc<dyn PluginEventBus>,
}

impl PluginContext {
    pub fn new(
        config: serde_json::Value, data_dir: PathBuf,
        logger: Arc<dyn PluginLogger>, event_bus: Arc<dyn PluginEventBus>,
    ) -> Self {
        Self { config, data_dir, logger, event_bus }
    }
}

/// Plugin logger trait
pub trait PluginLogger: Send + Sync {
    fn debug(&self, message: &str);
    fn info(&self, message: &str);
    fn warn(&self, message: &str);
    fn error(&self, message: &str);
}

/// Plugin event bus trait
pub trait PluginEventBus: Send + Sync {
    fn publish(&self, event_type: &str, data: serde_json::Value) -> Result<()>;
    fn subscribe(&self, event_type: &str, handler: Box<dyn Fn(serde_json::Value) + Send + Sync>) -> Result<String>;
    fn unsubscribe(&self, subscription_id: &str) -> Result<()>;
}

/// Plugin state enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PluginState {
    Discovered,
    Loading,
    Loaded,
    Initializing,
    Active,
    Executing,
    Unloading,
    Unloaded,
    Failed,
}

/// Event triggered when a plugin's state changes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginStateEvent {
    pub plugin_id: PluginId,
    pub plugin_name: String,
    pub old_state: Option<PluginState>,
    pub new_state: PluginState,
    pub timestamp: i64,
}
