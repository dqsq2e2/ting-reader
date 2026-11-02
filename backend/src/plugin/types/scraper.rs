//! Search-facing plugin DTOs. Wire DTOs are defined in ting-plugin-contract.

use super::{LocalizedText, ScraperSearchField};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

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
    pub book_source: bool,
    /// Whether the plugin can be used by automatic library scraping
    pub auto_scrape: bool,
    /// Whether this source aggregates candidates from other scraper sources
    pub aggregate_auto_scrape: bool,
    /// Search fields declared by the plugin
    pub search_fields: Vec<ScraperSearchField>,
    /// Metadata fields this plugin can return
    pub result_fields: Vec<String>,
    /// Localized labels for result fields declared by the plugin
    pub result_field_labels: BTreeMap<String, LocalizedText>,
}

/// Search result containing a list of books and pagination information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    /// List of book items in the search results
    pub items: Vec<BookItem>,

    /// Total number of results available
    pub total: Option<u64>,

    /// Whether the source knows another page exists.
    pub has_more: Option<bool>,

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
#[serde(deny_unknown_fields)]
pub struct BookItem {
    /// Unique identifier on the source platform
    pub id: Option<String>,

    /// Source URL when the source has no stable platform identifier.
    #[serde(default)]
    pub source_url: Option<String>,

    /// Book title
    #[serde(default)]
    pub title: String,

    /// Author name
    #[serde(default)]
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

    /// Subtitle (optional)
    #[serde(default)]
    pub subtitle: Option<String>,

    /// Published Year (optional)
    #[serde(default)]
    pub published_year: Option<String>,

    /// Published Date (optional)
    #[serde(default)]
    pub published_date: Option<String>,

    /// Publisher (optional)
    #[serde(default)]
    pub publisher: Option<String>,

    /// ISBN (optional)
    #[serde(default)]
    pub isbn: Option<String>,

    /// ASIN (optional)
    #[serde(default)]
    pub asin: Option<String>,

    /// Language (optional)
    #[serde(default)]
    pub language: Option<String>,

    /// Genre
    #[serde(default)]
    pub genre: Option<String>,

    /// Explicit content
    #[serde(default)]
    pub explicit: Option<bool>,

    /// Abridged version
    #[serde(default)]
    pub abridged: Option<bool>,

    /// Tags or categories
    #[serde(default)]
    pub tags: Vec<String>,

    /// Total duration in seconds (optional)
    #[serde(default)]
    pub duration: Option<u64>,

    /// Optional chapter title template selected by metadata provider.
    #[serde(default)]
    pub chapter_title_template: Option<String>,

    /// Optional cleaned chapter titles in the same order as scanned files.
    #[serde(default)]
    pub chapter_titles: Vec<String>,

    #[serde(default)]
    pub score: Option<f64>,
}

/// Detailed book information
///
/// Contains comprehensive metadata about a book, including all information
/// needed to display a book detail page.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct BookDetail {
    /// Unique identifier on the source platform
    pub id: Option<String>,

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

    /// Subtitle (optional)
    #[serde(default)]
    pub subtitle: Option<String>,

    /// Published Year (optional)
    #[serde(default)]
    pub published_year: Option<String>,

    /// Published Date (optional)
    #[serde(default)]
    pub published_date: Option<String>,

    /// Publisher (optional)
    #[serde(default)]
    pub publisher: Option<String>,

    /// ISBN (optional)
    #[serde(default)]
    pub isbn: Option<String>,

    /// ASIN (optional)
    #[serde(default)]
    pub asin: Option<String>,

    /// Language (optional)
    #[serde(default)]
    pub language: Option<String>,

    /// Explicit content
    #[serde(default)]
    pub explicit: bool,

    /// Abridged version
    #[serde(default)]
    pub abridged: bool,

    /// Full introduction or description
    pub intro: String,

    /// Tags or categories
    #[serde(default)]
    pub tags: Vec<String>,

    /// Genre
    #[serde(default)]
    pub genre: Option<String>,

    /// Total number of chapters
    pub chapter_count: u32,

    /// Total duration in seconds (optional)
    #[serde(default)]
    pub duration: Option<u64>,

    /// Optional chapter title template selected by metadata provider.
    #[serde(default)]
    pub chapter_title_template: Option<String>,

    /// Optional cleaned chapter titles in the same order as scanned files.
    #[serde(default)]
    pub chapter_titles: Vec<String>,
}

impl From<&BookItem> for BookDetail {
    fn from(item: &BookItem) -> Self {
        Self {
            id: item.id.clone(),
            title: item.title.clone(),
            author: item.author.clone(),
            narrator: item.narrator.clone(),
            cover_url: item.cover_url.clone(),
            intro: item.intro.clone().unwrap_or_default(),
            tags: item.tags.clone(),
            chapter_count: 0,
            duration: item.duration,
            subtitle: item.subtitle.clone(),
            published_year: item.published_year.clone(),
            published_date: item.published_date.clone(),
            publisher: item.publisher.clone(),
            isbn: item.isbn.clone(),
            asin: item.asin.clone(),
            language: item.language.clone(),
            explicit: item.explicit.unwrap_or(false),
            abridged: item.abridged.unwrap_or(false),
            genre: item.genre.clone(),
            chapter_title_template: item.chapter_title_template.clone(),
            chapter_titles: item.chapter_titles.clone(),
        }
    }
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
