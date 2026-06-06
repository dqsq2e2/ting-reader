//! Core business logic module
//!
//! This module provides the core application layer including:
//! - Business logic services
//! - Task queue and scheduling
//! - Event bus for pub/sub messaging
//! - Configuration management
//! - Structured logging system
//! - Error handling and type system
//! - Text cleaning and normalization
//! - Audio streaming and metadata reading

#[path = "books/color.rs"]
pub mod color;
#[path = "app/config.rs"]
pub mod config;
#[path = "security/crypto.rs"]
pub mod crypto;
#[path = "security/decryption_cache.rs"]
pub mod decryption_cache;
#[path = "app/error.rs"]
pub mod error;
#[path = "library_scanner/watcher.rs"]
pub mod library_watcher;
#[path = "app/logging.rs"]
pub mod logging;
#[path = "security/master_key.rs"]
pub mod master_key;
#[path = "books/merge_service.rs"]
pub mod merge_service;
#[path = "books/metadata_writer.rs"]
pub mod metadata_writer;
#[path = "storage/service.rs"]
pub mod storage;
#[path = "books/text_cleaner.rs"]
pub mod text_cleaner;
#[path = "storage/webdav_client.rs"]
pub mod webdav_client;

#[path = "common/lru_cache.rs"]
pub mod lru_cache;
#[path = "common/utils.rs"]
pub mod utils;

pub mod audio_streamer;
pub mod event_bus;
pub mod library_scanner;
pub mod nfo_manager;
pub mod services;
pub mod task_queue;

pub use audio_streamer::{AudioFormat, AudioMetadata, AudioStreamer, StreamerConfig};
pub use config::Config;
pub use decryption_cache::{CacheStats, DecryptionCacheConfig, DecryptionCacheService};
pub use error::{ErrorContext, ErrorResponse, Result, TingError};
pub use event_bus::{Event, EventBus, EventType};
pub use library_scanner::{LibraryScanner, ScanResult};
pub use logging::Logger;
pub use lru_cache::LruCache;
pub use merge_service::MergeService;
pub use nfo_manager::{BookMetadata, ChapterMetadata, NfoManager};
pub use services::{BookService, ScraperService};
pub use storage::StorageService;
pub use task_queue::{Task, TaskQueue, TaskStatus};
pub use text_cleaner::{CleanerConfig, CleaningResult, CleaningRule, TextCleaner};
pub use utils::release_memory;
pub use webdav_client::WebDavClient;
