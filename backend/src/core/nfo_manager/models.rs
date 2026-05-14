use serde::{Deserialize, Serialize};

/// Book metadata stored in NFO files
///
/// This structure contains all detailed metadata for a book,
/// which is stored in the book.nfo file.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename = "audiobook")]
pub struct BookMetadata {
    /// Book title
    pub title: String,
    
    /// Author name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    
    /// Narrator name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub narrator: Option<String>,

    /// Subtitle (added for compatibility)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subtitle: Option<String>,
    
    /// Book introduction/description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub intro: Option<String>,
    
    /// Source platform identifier (e.g., "ximalaya")
    pub source: String,
    
    /// Source platform's book ID
    pub source_id: String,
    
    /// Cover image URL
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cover_url: Option<String>,
    
    /// Tags/categories
    #[serde(default)]
    pub tags: Tags,

    /// Genre
    #[serde(default)]
    pub genre: Tags, // Use Tags struct for list of genres, mapped to <genre>
    
    /// Total number of chapters
    pub chapter_count: u32,
    
    /// Total duration in seconds
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_duration: Option<u64>,
    
    /// Creation timestamp (Unix timestamp)
    pub created_at: i64,
    
    /// Last update timestamp (Unix timestamp)
    pub updated_at: i64,
}

/// Tags wrapper for XML serialization
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct Tags {
    #[serde(rename = "tag", default)]
    pub items: Vec<String>,
}

impl BookMetadata {
    /// Create a new BookMetadata instance
    pub fn new(
        title: String,
        source: String,
        source_id: String,
        chapter_count: u32,
    ) -> Self {
        let now = chrono::Utc::now().timestamp();
        Self {
            title,
            author: None,
            narrator: None,
            subtitle: None,
            intro: None,
            source,
            source_id,
            cover_url: None,
            tags: Tags::default(),
            genre: Tags::default(),
            chapter_count,
            total_duration: None,
            created_at: now,
            updated_at: now,
        }
    }

    /// Update the updated_at timestamp to current time
    pub fn touch(&mut self) {
        self.updated_at = chrono::Utc::now().timestamp();
    }
}

/// Chapter metadata stored in NFO files
///
/// This structure contains all detailed metadata for a chapter,
/// which is stored in chapter_XXX.nfo files.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename = "chapter")]
pub struct ChapterMetadata {
    /// Chapter title
    pub title: String,
    
    /// Chapter index (1-based)
    pub index: u32,
    
    /// Duration in seconds
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration: Option<u64>,
    
    /// Source URL for downloading
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_url: Option<String>,
    
    /// Local file path (relative to book directory)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_path: Option<String>,
    
    /// Whether the chapter is free (not requiring payment)
    pub is_free: bool,
    
    /// Creation timestamp (Unix timestamp)
    pub created_at: i64,
}

impl ChapterMetadata {
    /// Create a new ChapterMetadata instance
    pub fn new(title: String, index: u32) -> Self {
        Self {
            title,
            index,
            duration: None,
            source_url: None,
            file_path: None,
            is_free: true,
            created_at: chrono::Utc::now().timestamp(),
        }
    }
}
