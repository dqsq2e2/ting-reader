//! JavaScript plugin subsystem

pub mod bindings;
pub mod init_code;
pub mod npm;
pub mod plugin;
pub mod runtime;
pub mod wrapper;

pub use bindings::{
    create_js_runtime_with_bindings, JsPluginEventBus, JsPluginLogger, JsScraperPlugin,
};
pub use plugin::{JavaScriptPluginExecutor, JavaScriptPluginLoader};
pub use runtime::{JsError, JsRuntimeWrapper};
pub use wrapper::JavaScriptPluginWrapper;
