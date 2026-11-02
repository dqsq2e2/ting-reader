//! Book services, metadata files, cover colors and text normalization.

pub mod bookmarks;
pub mod color;
pub mod merge_service;
pub mod metadata;
pub mod nfo_manager;
pub mod scraper;
pub mod service;
pub mod sources;
pub mod text_cleaner;

pub use bookmarks::{BookmarkService, CreateBookmark};
pub use merge_service::MergeService;
pub use nfo_manager::{BookMetadata, ChapterMetadata, NfoManager};
pub use scraper::ScraperService;
pub use service::BookService;
pub use text_cleaner::{CleanerConfig, CleaningResult, CleaningRule, TextCleaner};

mod scrape_results;
