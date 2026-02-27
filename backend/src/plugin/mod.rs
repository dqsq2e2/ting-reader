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
pub mod installer;
pub mod js_bindings;
pub mod js_plugin;
pub mod js_wrapper;
pub mod js_runtime;
pub mod manager;
pub mod logger;
pub mod events;
pub mod native;
pub mod native_plugin;
pub mod npm_manager;
pub mod registry;
pub mod runtime;
pub mod sandbox;
pub mod scraper;
pub mod types;
pub mod utility;

pub use config::{PluginConfigManager, ConfigChangeEvent};
pub use format::{FormatPlugin, TranscodeOptions, AudioFormat, AudioMetadata, ProgressCallback};
pub use installer::{PluginInstaller, PluginPackage};
pub use js_bindings::{JsScraperPlugin, JsPluginLogger, JsPluginEventBus, create_js_runtime_with_bindings};
pub use js_plugin::{JavaScriptPluginLoader, JavaScriptPluginExecutor};
pub use js_wrapper::JavaScriptPluginWrapper;
pub use js_runtime::{JsRuntimeWrapper, JsError};
pub use manager::{PluginManager, PluginConfig, PluginInfo};
pub use native::NativeLoader;
pub use native_plugin::NativePlugin;
pub use npm_manager::{NpmManager, NpmDependency, PackageJson};
pub use registry::{PluginRegistry, PluginEntry};
pub use runtime::{WasmRuntime, WasmPlugin};
pub use sandbox::{Sandbox, Permission, ResourceLimits, FileAccess};
pub use scraper::{ScraperPlugin, SearchResult, BookItem, BookDetail, Chapter};
pub use types::{Plugin, PluginType, PluginMetadata, PluginId, PluginState, PluginStats};
pub use utility::{
    UtilityPlugin, Capability, Endpoint, HttpMethod, Request, Response,
    EndpointHandler, EventType, Event, EventSource,
};
