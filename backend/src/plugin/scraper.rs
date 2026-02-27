//! Scraper plugin interface
//!
//! This module defines the interface for scraper plugins that fetch book metadata
//! and resources from external platforms.
//!
//! Scraper plugins must implement the `ScraperPlugin` trait in addition to the base
//! `Plugin` trait. They provide functionality for:
//! - Searching for books by keyword
//! - Retrieving detailed book information
//! - Getting chapter lists
//! - Downloading cover images
//! - Getting audio download URLs

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use crate::core::error::Result;
use super::Plugin;

/// Scraper plugin trait
///
/// All scraper plugins must implement this trait to provide book metadata
/// fetching functionality from external sources.
#[async_trait]
pub trait ScraperPlugin: Plugin {
    /// Search for books by keyword
    ///
    /// # Arguments
    /// * `query` - Search keyword
    /// * `page` - Page number (1-indexed)
    ///
    /// # Returns
    /// Search results containing a list of books and pagination info
    ///
    /// # Errors
    /// Returns an error if the search fails or the network request fails
    async fn search(&self, query: &str, page: u32) -> Result<SearchResult>;

    /// Get detailed information about a book
    ///
    /// # Arguments
    /// * `book_id` - Unique identifier of the book on the source platform
    ///
    /// # Returns
    /// Detailed book information including metadata and chapter count
    ///
    /// # Errors
    /// Returns an error if the book is not found or the request fails
    async fn get_detail(&self, book_id: &str) -> Result<BookDetail>;

    /// Get the list of chapters for a book
    ///
    /// # Arguments
    /// * `book_id` - Unique identifier of the book on the source platform
    ///
    /// # Returns
    /// List of chapters with their metadata
    ///
    /// # Errors
    /// Returns an error if the book is not found or the request fails
    async fn get_chapters(&self, book_id: &str) -> Result<Vec<Chapter>>;

    /// Download a cover image
    ///
    /// # Arguments
    /// * `cover_url` - URL of the cover image
    ///
    /// # Returns
    /// Raw image data as bytes
    ///
    /// # Errors
    /// Returns an error if the download fails or the URL is invalid
    async fn download_cover(&self, cover_url: &str) -> Result<Vec<u8>>;

    /// Get the audio download URL for a chapter
    ///
    /// # Arguments
    /// * `chapter_id` - Unique identifier of the chapter on the source platform
    ///
    /// # Returns
    /// Direct download URL for the audio file
    ///
    /// # Errors
    /// Returns an error if the chapter is not found or the request fails
    async fn get_audio_url(&self, chapter_id: &str) -> Result<String>;
}

/// Search result containing a list of books and pagination information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    /// List of book items in the search results
    pub items: Vec<BookItem>,
    
    /// Total number of results available
    pub total: u32,
    
    /// Current page number (1-indexed)
    pub page: u32,
    
    /// Number of items per page
    pub page_size: u32,
}

/// Book item in search results
///
/// Contains basic information about a book, typically shown in search results
/// or book lists.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BookItem {
    /// Unique identifier on the source platform
    pub id: String,
    
    /// Book title
    pub title: String,
    
    /// Author name
    pub author: String,
    
    /// Cover image URL (optional)
    #[serde(default)]
    pub cover_url: Option<String>,
    
    /// Brief introduction or description (optional)
    #[serde(default)]
    pub intro: Option<String>,
    
    /// Narrator name (optional, for audiobooks)
    #[serde(default)]
    pub narrator: Option<String>,
    
    /// Tags or categories
    #[serde(default)]
    pub tags: Vec<String>,
    
    /// Total number of chapters (optional)
    #[serde(default)]
    pub chapter_count: Option<u32>,
    
    /// Total duration in seconds (optional)
    #[serde(default)]
    pub duration: Option<u64>,
}

/// Detailed book information
///
/// Contains comprehensive metadata about a book, including all information
/// needed to display a book detail page.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BookDetail {
    /// Unique identifier on the source platform
    pub id: String,
    
    /// Book title
    pub title: String,
    
    /// Author name
    pub author: String,
    
    /// Narrator name (optional, for audiobooks)
    #[serde(default)]
    pub narrator: Option<String>,
    
    /// Cover image URL (optional)
    #[serde(default)]
    pub cover_url: Option<String>,
    
    /// Full introduction or description
    pub intro: String,
    
    /// Tags or categories
    #[serde(default)]
    pub tags: Vec<String>,
    
    /// Total number of chapters
    pub chapter_count: u32,
    
    /// Total duration in seconds (optional)
    #[serde(default)]
    pub duration: Option<u64>,
}

/// Chapter information
///
/// Represents a single chapter or episode in a book.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Chapter {
    /// Unique identifier on the source platform
    pub id: String,
    
    /// Chapter title
    pub title: String,
    
    /// Chapter index (0-indexed)
    pub index: u32,
    
    /// Duration in seconds (optional)
    #[serde(default)]
    pub duration: Option<u64>,
    
    /// Whether the chapter is free to access
    #[serde(default = "default_true")]
    pub is_free: bool,
}

/// Default value for is_free field (true)
fn default_true() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_search_result_serialization() {
        let result = SearchResult {
            items: vec![
                BookItem {
                    id: "123".to_string(),
                    title: "Test Book".to_string(),
                    author: "Test Author".to_string(),
                    cover_url: Some("https://example.com/cover.jpg".to_string()),
                    intro: Some("Test intro".to_string()),
                },
            ],
            total: 100,
            page: 1,
            page_size: 20,
        };

        let json = serde_json::to_string(&result).unwrap();
        let deserialized: SearchResult = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.items.len(), 1);
        assert_eq!(deserialized.total, 100);
        assert_eq!(deserialized.page, 1);
    }

    #[test]
    fn test_book_detail_serialization() {
        let detail = BookDetail {
            id: "456".to_string(),
            title: "Test Book".to_string(),
            author: "Test Author".to_string(),
            narrator: Some("Test Narrator".to_string()),
            cover_url: Some("https://example.com/cover.jpg".to_string()),
            intro: "Full introduction".to_string(),
            tags: vec!["fiction".to_string(), "sci-fi".to_string()],
            chapter_count: 50,
            duration: Some(36000),
        };

        let json = serde_json::to_string(&detail).unwrap();
        let deserialized: BookDetail = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.id, "456");
        assert_eq!(deserialized.chapter_count, 50);
        assert_eq!(deserialized.tags.len(), 2);
    }

    #[test]
    fn test_chapter_serialization() {
        let chapter = Chapter {
            id: "789".to_string(),
            title: "Chapter 1".to_string(),
            index: 0,
            duration: Some(1800),
            is_free: true,
        };

        let json = serde_json::to_string(&chapter).unwrap();
        let deserialized: Chapter = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.id, "789");
        assert_eq!(deserialized.index, 0);
        assert!(deserialized.is_free);
    }

    #[test]
    fn test_chapter_default_is_free() {
        let json = r#"{"id":"789","title":"Chapter 1","index":0}"#;
        let chapter: Chapter = serde_json::from_str(json).unwrap();
        
        assert!(chapter.is_free);
    }
}
