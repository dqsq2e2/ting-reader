pub mod books;
pub mod users;
pub mod libraries;
pub mod plugins;
pub mod system;
pub mod media;
pub mod tools;

pub use books::*;
pub use users::*;
pub use libraries::*;
pub use plugins::*;
pub use system::*;
pub use media::*;
pub use tools::*;

use crate::core::services::{BookService, ScraperService};
use crate::core::task_queue::TaskQueue;
use crate::db::repository::{
    BookRepository, UserRepository, ProgressRepository, 
    FavoriteRepository, UserSettingsRepository, LibraryRepository, ChapterRepository
};
use std::sync::Arc;
use crate::core::merge_service::MergeService;
use crate::core::StorageService;
use crate::core::audio_streamer::AudioStreamer;
use crate::cache::CacheManager;
use crate::plugin::manager::PluginManager;
use crate::plugin::config::PluginConfigManager;
use crate::core::config::Config;
use crate::core::nfo_manager::NfoManager;

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
    pub book_service: Arc<BookService>,
    pub scraper_service: Arc<ScraperService>,
    pub plugin_manager: Arc<PluginManager>,
    pub config_manager: Arc<PluginConfigManager>,
    pub task_queue: Arc<TaskQueue>,
    pub config: Arc<tokio::sync::RwLock<Config>>,
    pub jwt_secret: Arc<String>,
    pub cache_manager: Arc<CacheManager>,
    pub encryption_key: Arc<[u8; 32]>,
    pub storage_service: Arc<StorageService>,
    pub preload_cache: Arc<tokio::sync::RwLock<std::collections::HashMap<String, bytes::Bytes>>>,
    pub audio_streamer: Arc<AudioStreamer>,
    pub merge_service: Arc<MergeService>,
    pub nfo_manager: Arc<NfoManager>,
}
