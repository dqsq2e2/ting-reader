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

pub mod services;
pub mod task_queue;
pub mod event_bus;
pub mod config;
pub mod logging;
pub mod error;
pub mod text_cleaner;
pub mod audio_streamer;
pub mod decryption_cache;
pub mod nfo_manager;
pub mod crypto;
pub mod library_scanner;
pub mod color;
pub mod storage;
pub mod merge_service;
pub mod utils;

pub use services::{BookService, ScraperService, FormatService};
pub use task_queue::{TaskQueue, Task, TaskStatus};
pub use event_bus::{EventBus, Event, EventType};
pub use config::Config;
pub use logging::Logger;
pub use error::{TingError, ErrorResponse, Result, ErrorContext};
pub use text_cleaner::{TextCleaner, CleaningRule, CleaningResult, CleanerConfig};
pub use audio_streamer::{AudioStreamer, AudioFormat, AudioMetadata, StreamerConfig};
pub use decryption_cache::{DecryptionCacheService, DecryptionCacheConfig, CacheStats};
pub use nfo_manager::{NfoManager, BookMetadata, ChapterMetadata};
pub use library_scanner::{LibraryScanner, ScanResult};
pub use storage::StorageService;
pub use merge_service::MergeService;
pub use utils::release_memory;
