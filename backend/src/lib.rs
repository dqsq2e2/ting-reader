//! Ting Reader Backend Library
//!
//! This library provides the core functionality for the Ting Reader backend,
//! including plugin system, database management, and REST API services.

pub mod api;
pub mod auth;
pub mod cache;
pub mod core;
pub mod db;
pub mod plugin;

// Re-export commonly used types
pub use api::ApiServer;
pub use crate::core::{Config, EventBus, TaskQueue};
pub use db::DatabaseManager;
pub use plugin::{PluginManager, PluginRegistry};

/// Library version
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Result type alias for the library
pub type Result<T> = anyhow::Result<T>;
