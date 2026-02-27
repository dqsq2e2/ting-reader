//! Configuration management

use serde::Deserialize;
use std::path::{Path, PathBuf};
use thiserror::Error;
use config::{Config as ConfigBuilder, ConfigError as BuilderError, Environment, File};
use clap::Parser;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("Invalid server configuration: {0}")]
    InvalidServer(String),
    
    #[error("Invalid database configuration: {0}")]
    InvalidDatabase(String),
    
    #[error("Invalid plugin configuration: {0}")]
    InvalidPlugin(String),
    
    #[error("Invalid task queue configuration: {0}")]
    InvalidTaskQueue(String),
    
    #[error("Invalid logging configuration: {0}")]
    InvalidLogging(String),
    
    #[error("Invalid security configuration: {0}")]
    InvalidSecurity(String),
    
    #[error("Invalid storage configuration: {0}")]
    InvalidStorage(String),
    
    #[error("Failed to load configuration: {0}")]
    LoadError(String),
    
    #[error("Configuration file not found: {0}")]
    FileNotFound(String),
}

impl From<BuilderError> for ConfigError {
    fn from(err: BuilderError) -> Self {
        ConfigError::LoadError(err.to_string())
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub server: ServerConfig,
    pub database: DatabaseConfig,
    pub plugins: PluginConfig,
    pub task_queue: TaskQueueConfig,
    pub logging: LoggingConfig,
    pub security: SecurityConfig,
    pub storage: StorageConfig,
}

impl Config {
    /// Load configuration with precedence: CLI args > Environment variables > Config file > Defaults
    pub fn load() -> Result<Self, ConfigError> {
        // Parse command-line arguments
        let cli_args = CliArgs::parse();
        
        // Build configuration with proper precedence
        let mut builder = ConfigBuilder::builder();
        
        // 1. Start with defaults (lowest priority)
        builder = builder
            .set_default("server.host", "127.0.0.1")?
            .set_default("server.port", 3000)?
            .set_default("server.max_connections", 100)?
            .set_default("server.request_timeout", 30)?
            .set_default("database.path", "./data/ting-reader.db")?
            .set_default("database.connection_pool_size", 10)?
            .set_default("database.busy_timeout", 5000)?
            .set_default("plugins.plugin_dir", "./plugins")?
            .set_default("plugins.enable_hot_reload", true)?
            .set_default("plugins.max_memory_per_plugin", 536870912)? // 512 MB
            .set_default("plugins.max_execution_time", 300)?
            .set_default("task_queue.max_concurrent_tasks", 10)?
            .set_default("task_queue.default_retry_count", 3)?
            .set_default("task_queue.task_timeout", 600)?
            .set_default("logging.level", "info")?
            .set_default("logging.format", "json")?
            .set_default("logging.output", "stdout")?
            .set_default("logging.max_file_size", 10485760)? // 10 MB
            .set_default("logging.max_backups", 5)?
            .set_default("security.enable_auth", false)?
            .set_default("security.api_key", "")?
            .set_default("security.jwt_secret", "change-this-secret-in-production")?
            .set_default("security.allowed_origins", vec!["*"])?
            .set_default("security.rate_limit_requests", 100)?
            .set_default("security.rate_limit_window", 60)?
            .set_default("security.enable_hsts", false)?
            .set_default("security.hsts_max_age", 31536000)?
            .set_default("storage.data_dir", "./data")?
            .set_default("storage.temp_dir", "./temp")?
            .set_default("storage.local_storage_root", "./storage")?
            .set_default("storage.max_disk_usage", 10737418240u64)?; // 10 GB
        
        // 2. Load from config file if specified (medium priority)
        if let Some(config_path) = &cli_args.config {
            if !config_path.exists() {
                return Err(ConfigError::FileNotFound(
                    config_path.display().to_string()
                ));
            }
            builder = builder.add_source(File::from(config_path.as_path()));
        }
        
        // 3. Override with environment variables (higher priority)
        // Environment variables should be prefixed with TING_ and use __ for nesting
        // Example: TING_SERVER__PORT=8080
        builder = builder.add_source(
            Environment::with_prefix("TING")
                .separator("__")
                .try_parsing(true)
        );
        
        // 4. Override with CLI arguments (highest priority)
        if let Some(host) = &cli_args.host {
            builder = builder.set_override("server.host", host.clone())?;
        }
        if let Some(port) = cli_args.port {
            builder = builder.set_override("server.port", port)?;
        }
        if let Some(db_path) = &cli_args.database {
            builder = builder.set_override("database.path", db_path.display().to_string())?;
        }
        if let Some(plugin_dir) = &cli_args.plugin_dir {
            builder = builder.set_override("plugins.plugin_dir", plugin_dir.display().to_string())?;
        }
        if let Some(log_level) = &cli_args.log_level {
            builder = builder.set_override("logging.level", log_level.clone())?;
        }
        
        // Build and deserialize configuration
        let config: Config = builder.build()?.try_deserialize()?;
        
        // Validate configuration
        config.validate()?;
        
        Ok(config)
    }
    
    /// Load configuration from a specific file path
    pub fn from_file(path: &Path) -> Result<Self, ConfigError> {
        if !path.exists() {
            return Err(ConfigError::FileNotFound(path.display().to_string()));
        }
        
        let config: Config = ConfigBuilder::builder()
            .add_source(File::from(path))
            .build()?
            .try_deserialize()?;
        
        config.validate()?;
        Ok(config)
    }
    
    /// Load configuration from environment variables only
    pub fn from_env() -> Result<Self, ConfigError> {
        let mut builder = ConfigBuilder::builder();
        
        // Set defaults first
        builder = builder
            .set_default("server.host", "127.0.0.1")?
            .set_default("server.port", 3000)?
            .set_default("server.max_connections", 100)?
            .set_default("server.request_timeout", 30)?
            .set_default("database.path", "./data/ting-reader.db")?
            .set_default("database.connection_pool_size", 10)?
            .set_default("database.busy_timeout", 5000)?
            .set_default("plugins.plugin_dir", "./plugins")?
            .set_default("plugins.enable_hot_reload", true)?
            .set_default("plugins.max_memory_per_plugin", 536870912)?
            .set_default("plugins.max_execution_time", 300)?
            .set_default("task_queue.max_concurrent_tasks", 10)?
            .set_default("task_queue.default_retry_count", 3)?
            .set_default("task_queue.task_timeout", 600)?
            .set_default("logging.level", "info")?
            .set_default("logging.format", "json")?
            .set_default("logging.output", "stdout")?
            .set_default("logging.max_file_size", 10485760)?
            .set_default("logging.max_backups", 5)?
            .set_default("security.enable_auth", false)?
            .set_default("security.api_key", "")?
            .set_default("security.jwt_secret", "change-this-secret-in-production")?
            .set_default("security.allowed_origins", vec!["*"])?
            .set_default("security.rate_limit_requests", 100)?
            .set_default("security.rate_limit_window", 60)?
            .set_default("security.enable_hsts", false)?
            .set_default("security.hsts_max_age", 31536000)?
            .set_default("storage.data_dir", "./data")?
            .set_default("storage.temp_dir", "./temp")?
            .set_default("storage.max_disk_usage", 10737418240u64)?;
        
        // Override with environment variables
        let config: Config = builder
            .add_source(
                Environment::with_prefix("TING")
                    .separator("__")
                    .try_parsing(true)
            )
            .build()?
            .try_deserialize()?;
        
        config.validate()?;
        Ok(config)
    }
    
    /// Merge this configuration with another, with the other taking precedence
    pub fn merge(self, other: Config) -> Self {
        other
    }
    
    /// Validate all configuration parameters
    pub fn validate(&self) -> Result<(), ConfigError> {
        self.server.validate()?;
        self.database.validate()?;
        self.plugins.validate()?;
        self.task_queue.validate()?;
        self.logging.validate()?;
        self.security.validate()?;
        self.storage.validate()?;
        Ok(())
    }
}

/// Command-line arguments for configuration override
#[derive(Debug, Parser)]
#[command(name = "ting-reader")]
#[command(about = "Ting Reader Backend Server", long_about = None)]
pub struct CliArgs {
    /// Path to configuration file (TOML format)
    #[arg(short, long, value_name = "FILE")]
    pub config: Option<PathBuf>,
    
    /// Server host address
    #[arg(long, value_name = "HOST")]
    pub host: Option<String>,
    
    /// Server port
    #[arg(short, long, value_name = "PORT")]
    pub port: Option<u16>,
    
    /// Database file path
    #[arg(short, long, value_name = "PATH")]
    pub database: Option<PathBuf>,
    
    /// Plugin directory path
    #[arg(long, value_name = "DIR")]
    pub plugin_dir: Option<PathBuf>,
    
    /// Log level (debug, info, warn, error)
    #[arg(short, long, value_name = "LEVEL")]
    pub log_level: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub max_connections: usize,
    pub request_timeout: u64, // seconds
}

impl ServerConfig {
    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.host.is_empty() {
            return Err(ConfigError::InvalidServer("host cannot be empty".to_string()));
        }
        
        if self.port == 0 {
            return Err(ConfigError::InvalidServer("port must be greater than 0".to_string()));
        }
        
        if self.max_connections == 0 {
            return Err(ConfigError::InvalidServer("max_connections must be greater than 0".to_string()));
        }
        
        if self.request_timeout == 0 {
            return Err(ConfigError::InvalidServer("request_timeout must be greater than 0".to_string()));
        }
        
        Ok(())
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct DatabaseConfig {
    pub path: PathBuf,
    pub connection_pool_size: usize,
    pub busy_timeout: u64, // milliseconds
}

impl DatabaseConfig {
    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.path.as_os_str().is_empty() {
            return Err(ConfigError::InvalidDatabase("path cannot be empty".to_string()));
        }
        
        if self.connection_pool_size == 0 {
            return Err(ConfigError::InvalidDatabase("connection_pool_size must be greater than 0".to_string()));
        }
        
        if self.busy_timeout == 0 {
            return Err(ConfigError::InvalidDatabase("busy_timeout must be greater than 0".to_string()));
        }
        
        Ok(())
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct PluginConfig {
    pub plugin_dir: PathBuf,
    pub enable_hot_reload: bool,
    pub max_memory_per_plugin: usize, // bytes
    pub max_execution_time: u64, // seconds
}

impl PluginConfig {
    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.plugin_dir.as_os_str().is_empty() {
            return Err(ConfigError::InvalidPlugin("plugin_dir cannot be empty".to_string()));
        }
        
        if self.max_memory_per_plugin == 0 {
            return Err(ConfigError::InvalidPlugin("max_memory_per_plugin must be greater than 0".to_string()));
        }
        
        if self.max_execution_time == 0 {
            return Err(ConfigError::InvalidPlugin("max_execution_time must be greater than 0".to_string()));
        }
        
        Ok(())
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct TaskQueueConfig {
    pub max_concurrent_tasks: usize,
    pub default_retry_count: u32,
    pub task_timeout: u64, // seconds
}

impl TaskQueueConfig {
    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.max_concurrent_tasks == 0 {
            return Err(ConfigError::InvalidTaskQueue("max_concurrent_tasks must be greater than 0".to_string()));
        }
        
        if self.task_timeout == 0 {
            return Err(ConfigError::InvalidTaskQueue("task_timeout must be greater than 0".to_string()));
        }
        
        Ok(())
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct LoggingConfig {
    pub level: String,
    pub format: String,
    pub output: String,
    pub log_file: Option<PathBuf>,
    pub max_file_size: usize, // bytes
    pub max_backups: usize,
}

impl LoggingConfig {
    pub fn validate(&self) -> Result<(), ConfigError> {
        let valid_levels = ["debug", "info", "warn", "error"];
        if !valid_levels.contains(&self.level.as_str()) {
            return Err(ConfigError::InvalidLogging(
                format!("level must be one of: {:?}", valid_levels)
            ));
        }
        
        let valid_formats = ["json", "text"];
        if !valid_formats.contains(&self.format.as_str()) {
            return Err(ConfigError::InvalidLogging(
                format!("format must be one of: {:?}", valid_formats)
            ));
        }
        
        let valid_outputs = ["stdout", "file"];
        if !valid_outputs.contains(&self.output.as_str()) {
            return Err(ConfigError::InvalidLogging(
                format!("output must be one of: {:?}", valid_outputs)
            ));
        }
        
        if self.output == "file" && self.log_file.is_none() {
            return Err(ConfigError::InvalidLogging(
                "log_file must be specified when output is 'file'".to_string()
            ));
        }
        
        if self.max_file_size == 0 {
            return Err(ConfigError::InvalidLogging("max_file_size must be greater than 0".to_string()));
        }
        
        if self.max_backups == 0 {
            return Err(ConfigError::InvalidLogging("max_backups must be greater than 0".to_string()));
        }
        
        Ok(())
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct SecurityConfig {
    pub enable_auth: bool,
    pub api_key: String,
    pub jwt_secret: String,
    pub allowed_origins: Vec<String>,
    pub rate_limit_requests: usize,
    pub rate_limit_window: u64, // seconds
    pub enable_hsts: bool,
    pub hsts_max_age: u64, // seconds
}

impl SecurityConfig {
    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.enable_auth && self.api_key.is_empty() {
            return Err(ConfigError::InvalidSecurity(
                "api_key must be provided when enable_auth is true".to_string()
            ));
        }
        
        if self.allowed_origins.is_empty() {
            return Err(ConfigError::InvalidSecurity("allowed_origins cannot be empty".to_string()));
        }
        
        if self.rate_limit_requests == 0 {
            return Err(ConfigError::InvalidSecurity("rate_limit_requests must be greater than 0".to_string()));
        }
        
        if self.rate_limit_window == 0 {
            return Err(ConfigError::InvalidSecurity("rate_limit_window must be greater than 0".to_string()));
        }
        
        if self.enable_hsts && self.hsts_max_age == 0 {
            return Err(ConfigError::InvalidSecurity("hsts_max_age must be greater than 0 when enable_hsts is true".to_string()));
        }
        
        Ok(())
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct StorageConfig {
    pub data_dir: PathBuf,
    pub temp_dir: PathBuf,
    pub max_disk_usage: u64, // bytes
    pub local_storage_root: PathBuf, // Root directory for local libraries
}

impl StorageConfig {
    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.data_dir.as_os_str().is_empty() {
            return Err(ConfigError::InvalidStorage("data_dir cannot be empty".to_string()));
        }
        
        if self.temp_dir.as_os_str().is_empty() {
            return Err(ConfigError::InvalidStorage("temp_dir cannot be empty".to_string()));
        }
        
        if self.max_disk_usage == 0 {
            return Err(ConfigError::InvalidStorage("max_disk_usage must be greater than 0".to_string()));
        }
        
        Ok(())
    }
}
