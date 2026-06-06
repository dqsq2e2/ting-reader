pub mod books;
pub mod libraries;
pub mod media;
pub mod plugins;
pub mod series;
pub mod system;
pub mod tools;
pub mod users;

pub use books::*;
pub use libraries::*;
pub use media::*;
pub use plugins::*;
pub use series::*;
pub use system::*;
pub use tools::*;
pub use users::*;

use crate::api::handlers::media::stream::HlsSessionManager;
use crate::api::ws::manager::WsSessionManager;
use crate::cache::CacheManager;
use crate::core::audio_streamer::AudioStreamer;
use crate::core::config::Config;
use crate::core::library_watcher::LibraryWatcher;
use crate::core::merge_service::MergeService;
use crate::core::nfo_manager::NfoManager;
use crate::core::services::{BookService, ScraperService};
use crate::core::task_queue::TaskQueue;
use crate::core::StorageService;
use crate::db::repository::{
    BookRepository, ChapterRepository, FavoriteRepository, LibraryRepository, ProgressRepository,
    SeriesRepository, UserRepository, UserSettingsRepository,
};
use crate::plugin::config::PluginConfigManager;
use crate::plugin::manager::PluginManager;
use std::sync::Arc;

/// Shared application state for handlers
#[derive(Clone)]
pub struct AppState {
    pub book_repo: Arc<BookRepository>,
    pub user_repo: Arc<UserRepository>,
    pub progress_repo: Arc<ProgressRepository>,
    pub favorite_repo: Arc<FavoriteRepository>,
    pub settings_repo: Arc<UserSettingsRepository>,
    pub library_repo: Arc<LibraryRepository>,
    pub chapter_repo: Arc<ChapterRepository>,
    pub series_repo: Arc<SeriesRepository>,
    pub book_service: Arc<BookService>,
    pub scraper_service: Arc<ScraperService>,
    pub plugin_manager: Arc<PluginManager>,
    pub config_manager: Arc<PluginConfigManager>,
    pub task_queue: Arc<TaskQueue>,
    pub config: Arc<tokio::sync::RwLock<Config>>,
    pub jwt_secret: Arc<String>, // 保留用于向后兼容
    pub jwt_key_manager: Option<Arc<crate::auth::JwtKeyManager>>, // 新的密钥管理器
    pub cache_manager: Arc<CacheManager>,
    pub encryption_key: Arc<[u8; 32]>,
    pub storage_service: Arc<StorageService>,
    pub preload_cache: Arc<
        tokio::sync::RwLock<std::collections::HashMap<String, (bytes::Bytes, std::time::Instant)>>,
    >,
    pub audio_streamer: Arc<AudioStreamer>,
    pub merge_service: Arc<MergeService>,
    pub nfo_manager: Arc<NfoManager>,
    pub plugin_cache: Arc<crate::plugin::store::PluginCache>,
    pub active_preload_tasks:
        Arc<tokio::sync::Mutex<std::collections::HashMap<String, tokio::task::JoinHandle<()>>>>,
    pub library_watcher: Arc<LibraryWatcher>,
    pub ws_manager: Arc<WsSessionManager>,
    pub hls_session_manager: Arc<HlsSessionManager>,
}
