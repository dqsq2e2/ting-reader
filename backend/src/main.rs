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

    info!("配置加载成功");
    info!("正在启动 Ting Reader 后端 v{}", env!("CARGO_PKG_VERSION"));
    info!(
        host = %config.server.host,
        port = config.server.port,
        "服务器配置"
    );
    info!(
        path = ?config.database.path,
        "数据库配置"
    );
    info!(
        plugin_dir = ?config.plugins.plugin_dir,
        enable_hot_reload = config.plugins.enable_hot_reload,
        "插件配置"
    );

    // Ensure required directories exist
    info!("正在确保必需的目录存在...");
    let required_dirs = vec![
        &config.storage.data_dir,
        &config.storage.temp_dir,
        &config.storage.local_storage_root,
        &config.plugins.plugin_dir,
    ];

    for dir in required_dirs {
        if !dir.exists() {
            info!("Creating directory: {:?}", dir);
            std::fs::create_dir_all(dir)
                .map_err(|e| anyhow::anyhow!("Failed to create directory {:?}: {}", dir, e))?;
        }
    }
    info!("所有必需的目录已就绪");

    // Initialize database
    info!("正在初始化数据库...");
    let db = std::sync::Arc::new(db::DatabaseManager::new(
        &config.database.path,
        config.database.connection_pool_size as u32,
        std::time::Duration::from_millis(config.database.busy_timeout),
    )?);
    info!("正在运行数据库迁移...");
    db.migrate()?;
    info!("数据库初始化成功");

    // Ensure default admin user exists
    ensure_admin_user(db.clone()).await?;

    // Initialize plugin system
    let plugin_config = plugin::PluginConfig {
        plugin_dir: config.plugins.plugin_dir.clone(),
        enable_hot_reload: config.plugins.enable_hot_reload,
        max_memory_per_plugin: config.plugins.max_memory_per_plugin,
        max_execution_time: std::time::Duration::from_secs(config.plugins.max_execution_time),
    };
    let plugin_manager = std::sync::Arc::new(plugin::PluginManager::new(plugin_config)?);
    plugin_manager
        .discover_plugins(&config.plugins.plugin_dir)
        .await?;

    // Initialize API server
    info!("正在初始化 HTTP 服务器...");
    let server_url = format!("http://{}:{}", config.server.host, config.server.port);
    let server = api::ApiServer::new(config, db, plugin_manager)?;

    info!("听书后端初始化成功");
    info!(url = %server_url, "服务器已就绪 - 开始处理请求");

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
        info!("No users found, creating default admin user...");
        let password_hash = hash_password("admin123")?;
        let admin_user = User {
            id: Uuid::new_v4().to_string(),
            username: "admin".to_string(),
            password_hash,
            role: "admin".to_string(),
            created_at: chrono::Utc::now().to_rfc3339(),
        };
        user_repo.create(&admin_user).await?;
        info!("Default admin user created: username='admin', password='admin123'");
    }

    Ok(())
}
