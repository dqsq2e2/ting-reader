//! JavaScript plugin subsystem

pub mod bindings;
pub mod init_code;
pub mod npm;
pub mod plugin;
pub mod runtime;
pub mod wrapper;

pub use bindings::{JsScraperPlugin, JsPluginLogger, JsPluginEventBus, create_js_runtime_with_bindings};
pub use plugin::{JavaScriptPluginLoader, JavaScriptPluginExecutor};
pub use runtime::{JsRuntimeWrapper, JsError};
pub use wrapper::JavaScriptPluginWrapper;
