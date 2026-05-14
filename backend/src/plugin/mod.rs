//! Plugin system module
//!
//! This module provides the plugin system implementation including:
//! - Plugin manager for loading/unloading plugins
//! - Plugin registry for tracking installed plugins
//! - WASM runtime for executing WebAssembly plugins
//! - Native loader for loading native dynamic libraries
//! - Security sandbox for isolating plugin execution
//! - Plugin interfaces (Scraper, Format, Utility)
//! - npm dependency manager for JavaScript plugins

pub mod config;
pub mod format;
pub mod fs_utils;
pub mod installer;
pub mod js;
pub mod manager;
pub mod logger;
pub mod events;
pub mod native;
// npm moved into js/
pub mod registry;
pub mod wasm;
pub mod scraper;
pub mod store;
pub mod types;
pub mod utility;

pub use config::{PluginConfigManager, ConfigChangeEvent};
pub use format::{FormatPlugin, TranscodeOptions, AudioFormat, AudioMetadata, ProgressCallback};
pub use installer::{PluginInstaller, PluginPackage};
pub use js::{JsScraperPlugin, JsPluginLogger, JsPluginEventBus, create_js_runtime_with_bindings, JavaScriptPluginLoader, JavaScriptPluginExecutor, JavaScriptPluginWrapper, JsRuntimeWrapper, JsError};
pub use manager::{PluginManager, PluginConfig, PluginInfo};
pub use native::{NativeLoader, NativePlugin};
pub use js::npm::{NpmManager, NpmDependency, PackageJson, NpmSecurityConfig, VulnerabilitySeverity, NpmAuditResult};
pub use registry::{PluginRegistry, PluginEntry};
pub use wasm::{WasmRuntime, WasmPlugin, Sandbox, Permission, ResourceLimits, FileAccess};
pub use scraper::{ScraperPlugin, SearchResult, BookItem, BookDetail, Chapter};
pub use store::{StorePlugin, StoreDownload};
pub use types::{Plugin, PluginType, PluginMetadata, PluginId, PluginState, PluginStats};
pub use utility::{
    UtilityPlugin, Capability, Endpoint, HttpMethod, Request, Response,
    EndpointHandler, EventType, Event, EventSource,
};
