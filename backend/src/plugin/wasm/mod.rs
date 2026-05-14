//! WebAssembly plugin subsystem

pub mod host_functions;
pub mod plugin;
pub mod runtime;
pub mod sandbox;
#[cfg(test)]
mod tests;

pub use plugin::WasmPlugin;
pub use runtime::WasmRuntime;
pub use sandbox::{Sandbox, Permission, ResourceLimits, FileAccess};
