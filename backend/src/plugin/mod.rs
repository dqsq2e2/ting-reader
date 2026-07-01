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

pub mod cache;
pub mod config;
pub mod events;
pub mod format;
pub mod fs_utils;
pub mod host_gateway;
pub mod installer;
pub mod js;
pub mod logger;
pub mod manager;
pub mod native;
// npm moved into js/
pub mod registry;
pub mod scraper;
pub mod store;
pub mod tr_package;
pub mod types;
pub mod utility;
pub mod wasm;

pub use cache::{PluginCache, PluginCacheItem};
pub use config::{ConfigChangeEvent, PluginConfigManager};
pub use format::{AudioFormat, AudioMetadata, FormatPlugin, ProgressCallback, TranscodeOptions};
pub use host_gateway::{
    plugin_host_user_from_invocation_args, PluginHostGateway, PluginHostGatewayHandle,
    PluginHostPermission, PluginHostUser,
};
pub use installer::{PluginInstaller, PluginPackage};
pub use js::npm::{
    NpmAuditResult, NpmDependency, NpmManager, NpmSecurityConfig, PackageJson,
    VulnerabilitySeverity,
};
pub use js::{
    create_js_runtime_with_bindings, JavaScriptPluginExecutor, JavaScriptPluginLoader,
    JavaScriptPluginWrapper, JsError, JsPluginEventBus, JsPluginLogger, JsRuntimeWrapper,
    JsScraperPlugin,
};
pub use manager::{PluginConfig, PluginInfo, PluginManager};
pub use native::{NativeLoader, NativePlugin};
pub use registry::{PluginEntry, PluginRegistry};
pub use scraper::{BookDetail, BookItem, Chapter, ScraperPlugin, SearchResult};
pub use store::{StoreDownload, StorePlugin};
pub use types::{Plugin, PluginId, PluginMetadata, PluginState, PluginStats, PluginType};
pub use utility::{
    Capability, Endpoint, EndpointHandler, Event, EventSource, EventType, HttpMethod, Request,
    Response, UtilityPlugin,
};
pub use wasm::{FileAccess, Permission, ResourceLimits, Sandbox, WasmPlugin, WasmRuntime};
