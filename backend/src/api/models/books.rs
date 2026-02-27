use serde::{Deserialize, Serialize};
use crate::plugin::scraper::{BookItem, BookDetail};
use crate::db::models::Book;
use super::common::deserialize_tags_or_string;

// Search and Scraper API models

/// Query parameters for book search
#[derive(Debug, Deserialize)]
pub struct SearchQuery {
    /// Search keyword
    pub q: String,
    /// Optional source plugin to search from
    pub source: Option<String>,
    /// Page number (1-indexed, default: 1)
    #[serde(default = "default_page")]
    pub page: u32,
    /// Page size (default: 20)
    #[serde(default = "default_page_size")]
    pub page_size: u32,
}

fn default_page() -> u32 {
    1
}

fn default_page_size() -> u32 {
    20
}

/// Response for book search
#[derive(Debug, Serialize)]
pub struct SearchResponse {
    /// List of book items
    pub items: Vec<BookItem>,
    /// Total number of results
    pub total: u32,
    /// Current page number
    pub page: u32,
    /// Number of items per page
    pub page_size: u32,
}

/// Response for scraper sources list
#[derive(Debug, Serialize)]
pub struct ScraperSourcesResponse {
    /// List of available scraper sources
    pub sources: Vec<ScraperSourceInfo>,
}

/// Request body for scraper search
#[derive(Debug, Deserialize)]
pub struct ScraperSearchRequest {
    /// Search query
    pub query: String,
    /// Optional source plugin ID
    pub source: Option<String>,
    /// Page number (1-indexed)
    pub page: Option<u32>,
    /// Page size
    pub page_size: Option<u32>,
}

/// Information about a scraper source
#[derive(Debug, Serialize)]
pub struct ScraperSourceInfo {
    /// Plugin ID
    pub id: String,
    /// Plugin name
    pub name: String,
    /// Plugin description
    pub description: Option<String>,
    /// Plugin version
    pub version: String,
    /// Whether the plugin is enabled
    pub enabled: bool,
}

/// Response for book detail from scraper
#[derive(Debug, Serialize)]
pub struct ScraperDetailResponse {
    /// Book detail information
    #[serde(flatten)]
    pub detail: BookDetail,
}

/// Request body for applying scrape result to a book
#[derive(Debug, Deserialize)]
pub struct ScrapeBookRequest {
    /// Source plugin ID
    pub source: Option<String>,
    /// External ID in the source system
    pub external_id: Option<String>,
}

// Book API models

/// Request body for creating a new book
#[derive(Debug, Clone, Deserialize)]
pub struct CreateBookRequest {
    pub library_id: String,
    pub title: Option<String>,
    pub author: Option<String>,
    pub narrator: Option<String>,
    pub cover_url: Option<String>,
    pub theme_color: Option<String>,
    pub description: Option<String>,
    #[serde(default)]
    pub skip_intro: i32,
    #[serde(default)]
    pub skip_outro: i32,
    pub path: String,
    pub hash: String,
    #[serde(default, deserialize_with = "deserialize_tags_or_string")]
    pub tags: Option<String>,
    // V6
    pub chapter_regex: Option<String>,
}

/// Request body for updating a book
#[derive(Debug, Deserialize)]
pub struct UpdateBookRequest {
    pub library_id: Option<String>,
    pub title: Option<String>,
    pub author: Option<String>,
    pub narrator: Option<String>,
    pub cover_url: Option<String>,
    pub theme_color: Option<String>,
    pub description: Option<String>,
    pub skip_intro: Option<i32>,
    pub skip_outro: Option<i32>,
    pub path: Option<String>,
    pub hash: Option<String>,
    #[serde(default, deserialize_with = "deserialize_tags_or_string")]
    pub tags: Option<String>,
    // V6
    pub chapter_regex: Option<String>,
}

/// Response for book operations
#[derive(Debug, Serialize)]
pub struct BookResponse {
    pub id: String,
    pub library_id: String,
    pub title: Option<String>,
    pub author: Option<String>,
    pub narrator: Option<String>,
    pub cover_url: Option<String>,
    pub theme_color: Option<String>,
    pub description: Option<String>,
    pub skip_intro: i32,
    pub skip_outro: i32,
    pub path: String,
    pub hash: String,
    pub tags: Option<String>,
    pub created_at: String,
    pub library_type: Option<String>,
    pub is_favorite: bool,
    pub manual_corrected: bool,
    pub match_pattern: Option<String>,
    pub chapter_regex: Option<String>,
}

impl From<Book> for BookResponse {
    fn from(book: Book) -> Self {
        Self {
            id: book.id,
            library_id: book.library_id,
            title: book.title,
            author: book.author,
            narrator: book.narrator,
            cover_url: book.cover_url,
            theme_color: book.theme_color,
            description: book.description,
            skip_intro: book.skip_intro,
            skip_outro: book.skip_outro,
            path: book.path,
            hash: book.hash,
            tags: book.tags,
            created_at: book.created_at,
            library_type: None, // To be filled by handler
            is_favorite: false, // To be filled by handler
            manual_corrected: book.manual_corrected != 0,
            match_pattern: book.match_pattern,
            chapter_regex: book.chapter_regex,
        }
    }
}

/// Response for list of books
#[derive(Debug, Serialize)]
pub struct BooksListResponse {
    pub books: Vec<BookResponse>,
    pub total: usize,
}

// Chapter Management API models

/// Response for chapter operations
#[derive(Debug, Serialize)]
pub struct ChapterResponse {
    pub id: String,
    pub book_id: String,
    pub title: Option<String>,
    pub path: String,
    pub duration: Option<i32>,
    pub chapter_index: Option<i32>,
    pub is_extra: i32,
    pub created_at: String,
    pub progress_position: Option<f64>,
    pub progress_updated_at: Option<String>,
}

impl From<crate::db::models::Chapter> for ChapterResponse {
    fn from(chapter: crate::db::models::Chapter) -> Self {
        Self {
            id: chapter.id,
            book_id: chapter.book_id,
            title: chapter.title,
            path: chapter.path,
            duration: chapter.duration,
            chapter_index: chapter.chapter_index,
            is_extra: chapter.is_extra,
            created_at: chapter.created_at,
            progress_position: None,
            progress_updated_at: None,
        }
    }
}

/// Response for list of chapters
#[derive(Debug, Serialize)]
pub struct ChaptersListResponse {
    pub chapters: Vec<ChapterResponse>,
    pub total: usize,
}

/// Request body for updating a chapter
#[derive(Debug, Deserialize)]
pub struct UpdateChapterRequest {
    pub title: Option<String>,
    pub path: Option<String>,
    pub duration: Option<i32>,
    pub chapter_index: Option<i32>,
    pub is_extra: Option<i32>,
}

// Tags API models

/// Response for tags list
#[derive(Debug, Serialize)]
pub struct TagsResponse {
    pub tags: Vec<String>,
    pub total: usize,
}

// Statistics API models

/// Response for statistics
#[derive(Debug, Serialize)]
pub struct StatsResponse {
    pub total_books: usize,
    pub total_chapters: usize,
    pub total_duration: i64,
    pub last_scan_time: Option<String>,
}

// Merge System API models

/// Response for merge suggestion
#[derive(Debug, Serialize)]
pub struct MergeSuggestionResponse {
    pub id: String,
    pub source_book_id: String,
    pub target_book_id: String,
    pub source_title: String,
    pub target_title: String,
    pub source_author: Option<String>,
    pub target_author: Option<String>,
    pub score: f64,
    pub reason: String,
    pub status: String,
    pub created_at: String,
}

/// Request body for merging books
#[derive(Debug, Deserialize)]
pub struct MergeBooksRequest {
    pub source_book_id: String,
    pub target_book_id: String,
}

/// Request body for ignoring a merge suggestion
#[derive(Debug, Deserialize)]
pub struct IgnoreMergeSuggestionRequest {
    pub suggestion_id: String,
}

/// Request body for updating book correction status
#[derive(Debug, Deserialize)]
pub struct UpdateBookCorrectionRequest {
    pub manual_corrected: bool,
    #[serde(default)]
    pub match_pattern: Option<String>,
    // V6 Migration
    #[serde(default)]
    pub chapter_regex: Option<String>,
}

// Chapter Batch Update models

#[derive(Debug, Deserialize)]
pub struct BatchUpdateChaptersRequest {
    pub updates: Vec<BatchUpdateChapterItem>,
}

#[derive(Debug, Deserialize)]
pub struct BatchUpdateChapterItem {
    pub id: String,
    pub title: Option<String>,
    pub chapter_index: Option<i32>,
    pub is_extra: Option<i32>,
}

// Scrape Diff models

#[derive(Debug, Deserialize)]
pub struct ScrapeDiffRequest {
    pub query: String,
}

#[derive(Debug, Serialize)]
pub struct ScrapeDiffResponse {
    pub current: ScrapeMetadata,
    pub scraped: ScrapeMetadata,
    pub chapter_changes: Vec<ChapterChangeResponse>,
}

#[derive(Debug, Serialize)]
pub struct ScrapeMetadata {
    pub title: String,
    pub author: String,
    pub narrator: String,
    pub description: String,
    pub cover_url: Option<String>,
    pub tags: Option<Vec<String>>,
}

#[derive(Debug, Serialize)]
pub struct ChapterChangeResponse {
    pub index: i32,
    pub current_title: Option<String>,
    pub scraped_title: Option<String>,
    pub status: String, // "match", "update", "missing", "new"
}

#[derive(Debug, Deserialize)]
pub struct ScrapeApplyRequest {
    pub metadata: BookDetail,
    pub apply_metadata: bool,
    pub apply_chapters: Option<Vec<i32>>,
}

#[derive(Debug, Deserialize)]
pub struct MoveChaptersRequest {
    pub target_book_id: String,
    pub chapter_ids: Vec<String>,
}
