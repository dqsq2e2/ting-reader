//! Ting Reader Backend - Rust Implementation
//!
//! A high-performance audiobook management backend with plugin system support.

use ting_reader::{api, core, db, plugin};

use anyhow::Result;
use tracing::info;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize V8 Platform before creating any JsRuntime
    // This must be called once at program startup to avoid race conditions
    // when multiple threads create JsRuntime instances concurrently
    deno_core::JsRuntime::init_platform(None);

    // Load configuration (handles CLI args, env vars, and config file)
    let config = match core::config::Config::load() {
        Ok(cfg) => cfg,
        Err(e) => {
            // Print error to stderr since logging isn't initialized yet
            eprintln!("Failed to load configuration: {}", e);
            return Err(e.into());
        }
    };

    // Initialize logging system based on configuration
    let _logger = match core::Logger::init(&config.logging, &config.storage.data_dir) {
        Ok(logger) => logger,
        Err(e) => {
            eprintln!("Failed to initialize logging: {}", e);
            return Err(e);
        }
    };

    info!(message_key = "system.config.loaded", "Configuration loaded");
    info!(
        message_key = "system.backend.starting",
        message_params = %serde_json::json!({ "version": env!("CARGO_PKG_VERSION") }),
        version = env!("CARGO_PKG_VERSION"),
        "Starting Ting Reader backend"
    );
    info!(
        message_key = "system.server.config",
        host = %config.server.host,
        port = config.server.port,
        "Server configuration"
    );
    info!(
        message_key = "system.database.config",
        path = ?config.database.path,
        "Database configuration"
    );
    info!(
        message_key = "system.plugin.config",
        plugin_dir = ?config.plugins.plugin_dir,
        preinstalled_dir = ?config.plugins.preinstalled_dir,
        enable_hot_reload = config.plugins.enable_hot_reload,
        "Plugin configuration"
    );

    // Ensure required directories exist
    info!(
        message_key = "system.directories.ensuring",
        "Ensuring required directories exist"
    );
    let required_dirs = vec![
        &config.storage.data_dir,
        &config.storage.temp_dir,
        &config.storage.local_storage_root,
        &config.plugins.plugin_dir,
    ];

    for dir in required_dirs {
        if !dir.exists() {
            info!(
                message_key = "system.directory.creating",
                message_params = %serde_json::json!({ "path": dir.display().to_string() }),
                path = %dir.display(),
                "Creating directory"
            );
            std::fs::create_dir_all(dir)
                .map_err(|e| anyhow::anyhow!("Failed to create directory {:?}: {}", dir, e))?;
        }
    }
    info!(
        message_key = "system.directories.ready",
        "All required directories are ready"
    );

    // Initialize database
    info!(
        message_key = "system.database.initializing",
        "Initializing database"
    );
    let db = std::sync::Arc::new(db::DatabaseManager::new(
        &config.database.path,
        config.database.connection_pool_size as u32,
        std::time::Duration::from_millis(config.database.busy_timeout),
    )?);
    info!(
        message_key = "system.database.migrating",
        "Running database migrations"
    );
    db.migrate()?;
    info!(
        message_key = "system.database.initialized",
        "Database initialized"
    );

    // Ensure default admin user exists
    ensure_admin_user(db.clone()).await?;

    // Derive the shared encryption key before plugin discovery so plugin
    // configuration can be loaded before plugin initialization.
    let encryption_key =
        core::master_key::MasterKeyManager::derive_master_key(&config.database.path)
            .map_err(|e| anyhow::anyhow!("Failed to derive master key: {}", e))?;

    info!(
        message_key = "system.master_key.derived",
        "Master key derived from machine features and database path"
    );

    let plugin_config_manager = std::sync::Arc::new(
        plugin::PluginConfigManager::new(config.plugins.plugin_dir.join("configs"), encryption_key)
            .map_err(|e| anyhow::anyhow!("Failed to create plugin config manager: {}", e))?,
    );

    // Initialize plugin system
    let plugin_config = plugin::PluginConfig {
        plugin_dir: config.plugins.plugin_dir.clone(),
        enable_hot_reload: config.plugins.enable_hot_reload,
        max_memory_per_plugin: config.plugins.max_memory_per_plugin,
        max_execution_time: std::time::Duration::from_secs(config.plugins.max_execution_time),
    };
    let plugin_manager = std::sync::Arc::new(plugin::PluginManager::new(plugin_config)?);
    plugin_manager.set_config_manager(plugin_config_manager.clone());
    plugin_manager
        .install_preinstalled_packages(&config.plugins.preinstalled_dir)
        .await?;
    plugin_manager
        .discover_plugins(&config.plugins.plugin_dir)
        .await?;

    // Initialize API server
    info!(
        message_key = "system.http.initializing",
        "Initializing HTTP server"
    );
    let server_url = format!("http://{}:{}", config.server.host, config.server.port);
    let server = api::ApiServer::new(
        config,
        db,
        plugin_manager,
        plugin_config_manager,
        encryption_key,
    )?;

    info!(
        message_key = "system.backend.initialized",
        "Ting Reader backend initialized"
    );
    info!(
        message_key = "system.server.ready",
        message_params = %serde_json::json!({ "url": server_url }),
        url = %server_url,
        "Server ready"
    );

    // Start serving (this will block until shutdown signal)
    server.serve().await?;

    Ok(())
}

async fn ensure_admin_user(db: std::sync::Arc<db::DatabaseManager>) -> Result<()> {
    use ting_reader::auth::hash_password;
    use ting_reader::db::models::User;
    use ting_reader::db::repository::{Repository, UserRepository};
    use uuid::Uuid;

    let user_repo = UserRepository::new(db);
    let count = user_repo.count().await?;

    if count == 0 {
        info!(
            message_key = "system.default_admin.creating",
            "No users found, creating default admin user"
        );
        let password_hash = hash_password("admin123")?;
        let admin_user = User {
            id: Uuid::new_v4().to_string(),
            username: "admin".to_string(),
            password_hash,
            role: "admin".to_string(),
            created_at: chrono::Utc::now().to_rfc3339(),
        };
        user_repo.create(&admin_user).await?;
        info!(
            message_key = "system.default_admin.created",
            message_params = %serde_json::json!({ "username": "admin" }),
            username = "admin",
            "Default admin user created"
        );
    }

    Ok(())
}
