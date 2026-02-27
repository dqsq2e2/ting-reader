//! NFO Metadata Manager
//!
//! Manages NFO metadata files for books and chapters.
//! NFO files store detailed metadata in XML format following Kodi/Jellyfin standards.
//!
//! File organization:
//! ```
//! data/
//! ├── books/
//! │   ├── {book_id}/
//! │   │   ├── book.nfo          # Book metadata
//! │   │   ├── cover.jpg         # Cover image
//! │   │   ├── chapter_001.nfo   # Chapter 1 metadata
//! │   │   ├── chapter_001.m4a   # Chapter 1 audio
//! │   │   ├── chapter_002.nfo
//! │   │   ├── chapter_002.m4a
//! │   │   └── ...
//! ```

use crate::core::error::{Result, TingError};
use quick_xml::de::from_str;
use quick_xml::se::to_string;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

/// NFO Manager for managing metadata files
#[derive(Debug, Clone)]
pub struct NfoManager {
    /// Base directory for NFO files (e.g., "data/books")
    base_dir: PathBuf,
}

impl NfoManager {
    /// Create a new NFO Manager
    ///
    /// # Arguments
    /// * `base_dir` - Base directory for storing NFO files
    ///
    /// # Example
    /// ```
    /// use std::path::PathBuf;
    /// use ting_reader_rust::core::nfo_manager::NfoManager;
    ///
    /// let manager = NfoManager::new(PathBuf::from("data/books"));
    /// ```
    pub fn new(base_dir: PathBuf) -> Self {
        Self { base_dir }
    }

    /// Get the book directory path for a given book ID
    ///
    /// # Arguments
    /// * `book_id` - The book ID
    ///
    /// # Returns
    /// Path to the book directory (e.g., "data/books/123")
    pub fn get_book_dir(&self, book_id: i64) -> PathBuf {
        self.base_dir.join(book_id.to_string())
    }

    /// Get the book NFO file path
    ///
    /// # Arguments
    /// * `book_id` - The book ID
    ///
    /// # Returns
    /// Path to the book NFO file (e.g., "data/books/123/book.nfo")
    pub fn get_book_nfo_path(&self, book_id: i64) -> PathBuf {
        self.get_book_dir(book_id).join("book.nfo")
    }

    /// Get the chapter NFO file path
    ///
    /// # Arguments
    /// * `book_id` - The book ID
    /// * `chapter_index` - The chapter index (1-based)
    ///
    /// # Returns
    /// Path to the chapter NFO file (e.g., "data/books/123/chapter_001.nfo")
    pub fn get_chapter_nfo_path(&self, book_id: i64, chapter_index: u32) -> PathBuf {
        self.get_book_dir(book_id)
            .join(format!("chapter_{:03}.nfo", chapter_index))
    }

    /// Ensure the book directory exists
    ///
    /// Creates the directory if it doesn't exist.
    ///
    /// # Arguments
    /// * `book_id` - The book ID
    ///
    /// # Returns
    /// Result indicating success or failure
    pub fn ensure_book_dir(&self, book_id: i64) -> Result<PathBuf> {
        let book_dir = self.get_book_dir(book_id);
        
        if !book_dir.exists() {
            std::fs::create_dir_all(&book_dir).map_err(|e| {
                TingError::IoError(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!(
                        "Failed to create book directory {}: {}",
                        book_dir.display(),
                        e
                    ),
                ))
            })?;
        }
        
        Ok(book_dir)
    }

    /// Get the base directory
    pub fn base_dir(&self) -> &Path {
        &self.base_dir
    }

    /// Write book NFO file to a specific directory
    ///
    /// Serializes BookMetadata to XML and writes it to the book.nfo file in the specified directory.
    ///
    /// # Arguments
    /// * `dir` - The directory to write the NFO file to
    /// * `metadata` - The book metadata to write
    ///
    /// # Returns
    /// Path to the written NFO file
    pub fn write_book_nfo_to_dir(&self, dir: &Path, metadata: &BookMetadata) -> Result<PathBuf> {
        if !dir.exists() {
            std::fs::create_dir_all(dir).map_err(|e| {
                TingError::IoError(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("Failed to create directory {}: {}", dir.display(), e),
                ))
            })?;
        }

        let nfo_path = dir.join("book.nfo");

        // Serialize to XML
        let xml = to_string(metadata).map_err(|e| {
            TingError::SerializationError(format!("Failed to serialize book metadata: {}", e))
        })?;

        // Add XML declaration
        let xml_with_declaration = format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n{}", xml);

        // Write to file
        fs::write(&nfo_path, xml_with_declaration).map_err(|e| {
            TingError::IoError(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Failed to write book NFO file {}: {}", nfo_path.display(), e),
            ))
        })?;

        Ok(nfo_path)
    }

    /// Write book NFO file
    ///
    /// Serializes BookMetadata to XML and writes it to the book.nfo file.
    ///
    /// # Arguments
    /// * `book_id` - The book ID
    /// * `metadata` - The book metadata to write
    ///
    /// # Returns
    /// Path to the written NFO file
    ///
    /// # Example
    /// ```no_run
    /// use std::path::PathBuf;
    /// use ting_reader_rust::core::nfo_manager::{NfoManager, BookMetadata};
    ///
    /// let manager = NfoManager::new(PathBuf::from("data/books"));
    /// let metadata = BookMetadata::new(
    ///     "三体".to_string(),
    ///     "ximalaya".to_string(),
    ///     "12345678".to_string(),
    ///     42,
    /// );
    /// let nfo_path = manager.write_book_nfo(123, &metadata).unwrap();
    /// ```
    pub fn write_book_nfo(&self, book_id: i64, metadata: &BookMetadata) -> Result<PathBuf> {
        // Ensure book directory exists
        self.ensure_book_dir(book_id)?;

        let nfo_path = self.get_book_nfo_path(book_id);

        // Serialize to XML
        let xml = to_string(metadata).map_err(|e| {
            TingError::SerializationError(format!("Failed to serialize book metadata: {}", e))
        })?;

        // Add XML declaration
        let xml_with_declaration = format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n{}", xml);

        // Write to file
        fs::write(&nfo_path, xml_with_declaration).map_err(|e| {
            TingError::IoError(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Failed to write book NFO file {}: {}", nfo_path.display(), e),
            ))
        })?;

        Ok(nfo_path)
    }

    /// Read book NFO file
    ///
    /// Parses XML from book.nfo file and deserializes it to BookMetadata.
    ///
    /// # Arguments
    /// * `nfo_path` - Path to the book NFO file
    ///
    /// # Returns
    /// Deserialized BookMetadata
    ///
    /// # Example
    /// ```no_run
    /// use std::path::PathBuf;
    /// use ting_reader_rust::core::nfo_manager::NfoManager;
    ///
    /// let manager = NfoManager::new(PathBuf::from("data/books"));
    /// let nfo_path = PathBuf::from("data/books/123/book.nfo");
    /// let metadata = manager.read_book_nfo(&nfo_path).unwrap();
    /// ```
    pub fn read_book_nfo(&self, nfo_path: &Path) -> Result<BookMetadata> {
        // Read file content
        let xml = fs::read_to_string(nfo_path).map_err(|e| {
            TingError::IoError(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Failed to read book NFO file {}: {}", nfo_path.display(), e),
            ))
        })?;

        // Deserialize from XML
        from_str(&xml).map_err(|e| {
            TingError::DeserializationError(format!(
                "Failed to deserialize book metadata from {}: {}",
                nfo_path.display(),
                e
            ))
        })
    }

    /// Write chapter NFO file
    ///
    /// Serializes ChapterMetadata to XML and writes it to the chapter_XXX.nfo file.
    ///
    /// # Arguments
    /// * `book_id` - The book ID
    /// * `chapter_index` - The chapter index (1-based)
    /// * `metadata` - The chapter metadata to write
    ///
    /// # Returns
    /// Path to the written NFO file
    ///
    /// # Example
    /// ```no_run
    /// use std::path::PathBuf;
    /// use ting_reader_rust::core::nfo_manager::{NfoManager, ChapterMetadata};
    ///
    /// let manager = NfoManager::new(PathBuf::from("data/books"));
    /// let metadata = ChapterMetadata::new("第一章".to_string(), 1);
    /// let nfo_path = manager.write_chapter_nfo(123, 1, &metadata).unwrap();
    /// ```
    pub fn write_chapter_nfo(
        &self,
        book_id: i64,
        chapter_index: u32,
        metadata: &ChapterMetadata,
    ) -> Result<PathBuf> {
        // Ensure book directory exists
        self.ensure_book_dir(book_id)?;

        let nfo_path = self.get_chapter_nfo_path(book_id, chapter_index);

        // Serialize to XML
        let xml = to_string(metadata).map_err(|e| {
            TingError::SerializationError(format!("Failed to serialize chapter metadata: {}", e))
        })?;

        // Add XML declaration
        let xml_with_declaration = format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n{}", xml);

        // Write to file
        fs::write(&nfo_path, xml_with_declaration).map_err(|e| {
            TingError::IoError(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!(
                    "Failed to write chapter NFO file {}: {}",
                    nfo_path.display(),
                    e
                ),
            ))
        })?;

        Ok(nfo_path)
    }

    /// Read chapter NFO file
    ///
    /// Parses XML from chapter_XXX.nfo file and deserializes it to ChapterMetadata.
    ///
    /// # Arguments
    /// * `nfo_path` - Path to the chapter NFO file
    ///
    /// # Returns
    /// Deserialized ChapterMetadata
    ///
    /// # Example
    /// ```no_run
    /// use std::path::PathBuf;
    /// use ting_reader_rust::core::nfo_manager::NfoManager;
    ///
    /// let manager = NfoManager::new(PathBuf::from("data/books"));
    /// let nfo_path = PathBuf::from("data/books/123/chapter_001.nfo");
    /// let metadata = manager.read_chapter_nfo(&nfo_path).unwrap();
    /// ```
    pub fn read_chapter_nfo(&self, nfo_path: &Path) -> Result<ChapterMetadata> {
        // Read file content
        let xml = fs::read_to_string(nfo_path).map_err(|e| {
            TingError::IoError(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!(
                    "Failed to read chapter NFO file {}: {}",
                    nfo_path.display(),
                    e
                ),
            ))
        })?;

        // Deserialize from XML
        from_str(&xml).map_err(|e| {
            TingError::DeserializationError(format!(
                "Failed to deserialize chapter metadata from {}: {}",
                nfo_path.display(),
                e
            ))
        })
    }

    /// Delete book NFO files
    ///
    /// Deletes the book.nfo file and all chapter NFO files for a given book.
    ///
    /// # Arguments
    /// * `book_id` - The book ID
    ///
    /// # Returns
    /// Result indicating success or failure
    pub fn delete_book_nfos(&self, book_id: i64) -> Result<()> {
        let book_dir = self.get_book_dir(book_id);

        if !book_dir.exists() {
            return Ok(());
        }

        // Read directory and delete all .nfo files
        let entries = fs::read_dir(&book_dir).map_err(|e| {
            TingError::IoError(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Failed to read book directory {}: {}", book_dir.display(), e),
            ))
        })?;

        for entry in entries {
            let entry = entry.map_err(|e| {
                TingError::IoError(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("Failed to read directory entry: {}", e),
                ))
            })?;

            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("nfo") {
                fs::remove_file(&path).map_err(|e| {
                    TingError::IoError(std::io::Error::new(
                        std::io::ErrorKind::Other,
                        format!("Failed to delete NFO file {}: {}", path.display(), e),
                    ))
                })?;
            }
        }

        Ok(())
    }

    /// Validate NFO file format
    ///
    /// Checks if the NFO file exists and can be parsed.
    ///
    /// # Arguments
    /// * `nfo_path` - Path to the NFO file
    ///
    /// # Returns
    /// Result indicating whether the file is valid
    pub fn validate_nfo(&self, nfo_path: &Path) -> Result<()> {
        if !nfo_path.exists() {
            return Err(TingError::NotFound(format!(
                "NFO file not found: {}",
                nfo_path.display()
            )));
        }

        // Try to read the file
        let xml = fs::read_to_string(nfo_path).map_err(|e| {
            TingError::IoError(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Failed to read NFO file {}: {}", nfo_path.display(), e),
            ))
        })?;

        // Check if it's valid XML by trying to parse it
        // We'll try both book and chapter formats
        let is_book = from_str::<BookMetadata>(&xml).is_ok();
        let is_chapter = from_str::<ChapterMetadata>(&xml).is_ok();

        if !is_book && !is_chapter {
            return Err(TingError::ValidationError(format!(
                "NFO file {} is not a valid book or chapter metadata file",
                nfo_path.display()
            )));
        }

        Ok(())
    }
}

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
            intro: None,
            source,
            source_id,
            cover_url: None,
            tags: Tags::default(),
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_nfo_manager_creation() {
        let manager = NfoManager::new(PathBuf::from("data/books"));
        assert_eq!(manager.base_dir(), Path::new("data/books"));
    }

    #[test]
    fn test_get_book_dir() {
        let manager = NfoManager::new(PathBuf::from("data/books"));
        let book_dir = manager.get_book_dir(123);
        assert_eq!(book_dir, PathBuf::from("data/books/123"));
    }

    #[test]
    fn test_get_book_nfo_path() {
        let manager = NfoManager::new(PathBuf::from("data/books"));
        let nfo_path = manager.get_book_nfo_path(123);
        assert_eq!(nfo_path, PathBuf::from("data/books/123/book.nfo"));
    }

    #[test]
    fn test_get_chapter_nfo_path() {
        let manager = NfoManager::new(PathBuf::from("data/books"));
        
        // Test single digit
        let nfo_path = manager.get_chapter_nfo_path(123, 1);
        assert_eq!(nfo_path, PathBuf::from("data/books/123/chapter_001.nfo"));
        
        // Test double digit
        let nfo_path = manager.get_chapter_nfo_path(123, 42);
        assert_eq!(nfo_path, PathBuf::from("data/books/123/chapter_042.nfo"));
        
        // Test triple digit
        let nfo_path = manager.get_chapter_nfo_path(123, 999);
        assert_eq!(nfo_path, PathBuf::from("data/books/123/chapter_999.nfo"));
    }

    #[test]
    fn test_book_metadata_creation() {
        let metadata = BookMetadata::new(
            "三体".to_string(),
            "ximalaya".to_string(),
            "12345678".to_string(),
            42,
        );
        
        assert_eq!(metadata.title, "三体");
        assert_eq!(metadata.source, "ximalaya");
        assert_eq!(metadata.source_id, "12345678");
        assert_eq!(metadata.chapter_count, 42);
        assert!(metadata.author.is_none());
        assert!(metadata.tags.items.is_empty());
    }

    #[test]
    fn test_book_metadata_touch() {
        let mut metadata = BookMetadata::new(
            "Test Book".to_string(),
            "test".to_string(),
            "123".to_string(),
            10,
        );
        
        let original_updated_at = metadata.updated_at;
        
        // Sleep for 1 second to ensure timestamp changes
        std::thread::sleep(std::time::Duration::from_secs(1));
        
        metadata.touch();
        
        assert!(
            metadata.updated_at > original_updated_at,
            "updated_at should be greater after touch: {} > {}",
            metadata.updated_at,
            original_updated_at
        );
    }

    #[test]
    fn test_chapter_metadata_creation() {
        let metadata = ChapterMetadata::new("第一章".to_string(), 1);
        
        assert_eq!(metadata.title, "第一章");
        assert_eq!(metadata.index, 1);
        assert!(metadata.duration.is_none());
        assert!(metadata.source_url.is_none());
        assert!(metadata.file_path.is_none());
        assert!(metadata.is_free);
    }

    #[test]
    fn test_write_and_read_book_nfo() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let manager = NfoManager::new(temp_dir.path().to_path_buf());

        let mut metadata = BookMetadata::new(
            "三体".to_string(),
            "ximalaya".to_string(),
            "12345678".to_string(),
            42,
        );
        metadata.author = Some("刘慈欣".to_string());
        metadata.narrator = Some("冯雪松".to_string());
        metadata.intro = Some("文化大革命如火如荼进行的同时...".to_string());
        metadata.cover_url = Some("https://example.com/cover.jpg".to_string());
        metadata.tags.items = vec!["科幻".to_string(), "硬科幻".to_string()];
        metadata.total_duration = Some(72000);

        // Write NFO file
        let nfo_path = manager.write_book_nfo(123, &metadata).unwrap();
        assert!(nfo_path.exists());

        // Read NFO file
        let read_metadata = manager.read_book_nfo(&nfo_path).unwrap();
        assert_eq!(read_metadata, metadata);
    }

    #[test]
    fn test_write_and_read_chapter_nfo() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let manager = NfoManager::new(temp_dir.path().to_path_buf());

        let mut metadata = ChapterMetadata::new("第一章 科学边界".to_string(), 1);
        metadata.duration = Some(1800);
        metadata.source_url = Some("https://example.com/audio/chapter1.m4a".to_string());
        metadata.file_path = Some("./data/books/123/chapter_001.m4a".to_string());
        metadata.is_free = true;

        // Write NFO file
        let nfo_path = manager.write_chapter_nfo(123, 1, &metadata).unwrap();
        assert!(nfo_path.exists());

        // Read NFO file
        let read_metadata = manager.read_chapter_nfo(&nfo_path).unwrap();
        assert_eq!(read_metadata, metadata);
    }

    #[test]
    fn test_delete_book_nfos() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let manager = NfoManager::new(temp_dir.path().to_path_buf());

        // Create book and chapter NFO files
        let book_metadata = BookMetadata::new(
            "Test Book".to_string(),
            "test".to_string(),
            "123".to_string(),
            2,
        );
        manager.write_book_nfo(123, &book_metadata).unwrap();

        let chapter1 = ChapterMetadata::new("Chapter 1".to_string(), 1);
        manager.write_chapter_nfo(123, 1, &chapter1).unwrap();

        let chapter2 = ChapterMetadata::new("Chapter 2".to_string(), 2);
        manager.write_chapter_nfo(123, 2, &chapter2).unwrap();

        // Verify files exist
        assert!(manager.get_book_nfo_path(123).exists());
        assert!(manager.get_chapter_nfo_path(123, 1).exists());
        assert!(manager.get_chapter_nfo_path(123, 2).exists());

        // Delete all NFO files
        manager.delete_book_nfos(123).unwrap();

        // Verify files are deleted
        assert!(!manager.get_book_nfo_path(123).exists());
        assert!(!manager.get_chapter_nfo_path(123, 1).exists());
        assert!(!manager.get_chapter_nfo_path(123, 2).exists());
    }

    #[test]
    fn test_validate_nfo() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let manager = NfoManager::new(temp_dir.path().to_path_buf());

        // Test non-existent file
        let non_existent = temp_dir.path().join("non_existent.nfo");
        assert!(manager.validate_nfo(&non_existent).is_err());

        // Test valid book NFO
        let book_metadata = BookMetadata::new(
            "Test Book".to_string(),
            "test".to_string(),
            "123".to_string(),
            10,
        );
        let book_nfo_path = manager.write_book_nfo(123, &book_metadata).unwrap();
        assert!(manager.validate_nfo(&book_nfo_path).is_ok());

        // Test valid chapter NFO
        let chapter_metadata = ChapterMetadata::new("Chapter 1".to_string(), 1);
        let chapter_nfo_path = manager.write_chapter_nfo(123, 1, &chapter_metadata).unwrap();
        assert!(manager.validate_nfo(&chapter_nfo_path).is_ok());

        // Test invalid NFO (not valid XML)
        let invalid_nfo = temp_dir.path().join("invalid.nfo");
        std::fs::write(&invalid_nfo, "not valid xml").unwrap();
        assert!(manager.validate_nfo(&invalid_nfo).is_err());
    }

    #[test]
    fn test_xml_format() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let manager = NfoManager::new(temp_dir.path().to_path_buf());

        // Create book metadata with tags
        let mut metadata = BookMetadata::new(
            "三体".to_string(),
            "ximalaya".to_string(),
            "12345678".to_string(),
            42,
        );
        metadata.tags.items = vec!["科幻".to_string(), "硬科幻".to_string()];

        // Write NFO file
        let nfo_path = manager.write_book_nfo(123, &metadata).unwrap();

        // Read the raw XML content
        let xml_content = std::fs::read_to_string(&nfo_path).unwrap();

        // Verify XML declaration
        assert!(xml_content.starts_with("<?xml version=\"1.0\" encoding=\"UTF-8\"?>"));

        // Verify root element
        assert!(xml_content.contains("<audiobook>"));
        assert!(xml_content.contains("</audiobook>"));

        // Verify tags structure
        assert!(xml_content.contains("<tags>"));
        assert!(xml_content.contains("<tag>科幻</tag>"));
        assert!(xml_content.contains("<tag>硬科幻</tag>"));
        assert!(xml_content.contains("</tags>"));
    }

    #[test]
    fn test_read_nonexistent_file() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let manager = NfoManager::new(temp_dir.path().to_path_buf());

        // Try to read a non-existent book NFO
        let non_existent_path = temp_dir.path().join("999/book.nfo");
        let result = manager.read_book_nfo(&non_existent_path);
        assert!(result.is_err());

        // Try to read a non-existent chapter NFO
        let non_existent_chapter = temp_dir.path().join("999/chapter_001.nfo");
        let result = manager.read_chapter_nfo(&non_existent_chapter);
        assert!(result.is_err());
    }

    #[test]
    fn test_read_invalid_xml() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let manager = NfoManager::new(temp_dir.path().to_path_buf());

        // Create a file with invalid XML
        let book_dir = temp_dir.path().join("123");
        std::fs::create_dir_all(&book_dir).unwrap();
        let invalid_nfo = book_dir.join("book.nfo");
        std::fs::write(&invalid_nfo, "not valid xml at all").unwrap();

        // Try to read it
        let result = manager.read_book_nfo(&invalid_nfo);
        assert!(result.is_err());
    }

    #[test]
    fn test_read_malformed_metadata() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let manager = NfoManager::new(temp_dir.path().to_path_buf());

        // Create a file with valid XML but wrong structure
        let book_dir = temp_dir.path().join("123");
        std::fs::create_dir_all(&book_dir).unwrap();
        let malformed_nfo = book_dir.join("book.nfo");
        std::fs::write(
            &malformed_nfo,
            r#"<?xml version="1.0" encoding="UTF-8"?>
<wrongroot>
    <title>Test</title>
</wrongroot>"#,
        )
        .unwrap();

        // Try to read it
        let result = manager.read_book_nfo(&malformed_nfo);
        assert!(result.is_err());
    }

    #[test]
    fn test_utf8_encoding() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let manager = NfoManager::new(temp_dir.path().to_path_buf());

        // Create book metadata with various Chinese characters
        let mut metadata = BookMetadata::new(
            "三体：地球往事".to_string(),
            "ximalaya".to_string(),
            "12345678".to_string(),
            42,
        );
        metadata.author = Some("刘慈欣".to_string());
        metadata.narrator = Some("冯雪松、张磊".to_string());
        metadata.intro = Some(
            "文化大革命如火如荼进行的同时，军方探寻外星文明的绝秘计划红岸工程取得了突破性进展。".to_string(),
        );
        metadata.tags.items = vec![
            "科幻".to_string(),
            "硬科幻".to_string(),
            "雨果奖".to_string(),
        ];

        // Write NFO file
        let nfo_path = manager.write_book_nfo(123, &metadata).unwrap();

        // Read back and verify
        let read_metadata = manager.read_book_nfo(&nfo_path).unwrap();
        assert_eq!(read_metadata.title, "三体：地球往事");
        assert_eq!(read_metadata.author, Some("刘慈欣".to_string()));
        assert_eq!(read_metadata.narrator, Some("冯雪松、张磊".to_string()));
        assert!(read_metadata.intro.as_ref().unwrap().contains("红岸工程"));
        assert_eq!(read_metadata.tags.items.len(), 3);
        assert!(read_metadata.tags.items.contains(&"雨果奖".to_string()));

        // Verify the file is actually UTF-8 encoded
        let xml_content = std::fs::read_to_string(&nfo_path).unwrap();
        assert!(xml_content.contains("encoding=\"UTF-8\""));
        assert!(xml_content.contains("三体：地球往事"));
        assert!(xml_content.contains("刘慈欣"));
        assert!(xml_content.contains("红岸工程"));
    }

    #[test]
    fn test_utf8_chapter_encoding() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let manager = NfoManager::new(temp_dir.path().to_path_buf());

        // Create chapter metadata with Chinese characters
        let mut metadata = ChapterMetadata::new("第一章 科学边界".to_string(), 1);
        metadata.source_url = Some("https://example.com/音频/第一章.m4a".to_string());

        // Write NFO file
        let nfo_path = manager.write_chapter_nfo(123, 1, &metadata).unwrap();

        // Read back and verify
        let read_metadata = manager.read_chapter_nfo(&nfo_path).unwrap();
        assert_eq!(read_metadata.title, "第一章 科学边界");
        assert!(read_metadata
            .source_url
            .as_ref()
            .unwrap()
            .contains("音频"));

        // Verify the file is actually UTF-8 encoded
        let xml_content = std::fs::read_to_string(&nfo_path).unwrap();
        assert!(xml_content.contains("encoding=\"UTF-8\""));
        assert!(xml_content.contains("第一章 科学边界"));
    }

    #[test]
    fn test_delete_nonexistent_book() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let manager = NfoManager::new(temp_dir.path().to_path_buf());

        // Delete a book that doesn't exist should succeed (no-op)
        let result = manager.delete_book_nfos(999);
        assert!(result.is_ok());
    }

    #[test]
    fn test_ensure_book_dir_creates_directory() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let manager = NfoManager::new(temp_dir.path().to_path_buf());

        let book_id = 456;
        let book_dir = manager.get_book_dir(book_id);

        // Directory should not exist initially
        assert!(!book_dir.exists());

        // Ensure directory
        let result = manager.ensure_book_dir(book_id);
        assert!(result.is_ok());

        // Directory should now exist
        assert!(book_dir.exists());
        assert!(book_dir.is_dir());
    }

    #[test]
    fn test_ensure_book_dir_idempotent() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let manager = NfoManager::new(temp_dir.path().to_path_buf());

        let book_id = 789;

        // Call ensure_book_dir multiple times
        let result1 = manager.ensure_book_dir(book_id);
        assert!(result1.is_ok());

        let result2 = manager.ensure_book_dir(book_id);
        assert!(result2.is_ok());

        let result3 = manager.ensure_book_dir(book_id);
        assert!(result3.is_ok());

        // Directory should exist
        let book_dir = manager.get_book_dir(book_id);
        assert!(book_dir.exists());
    }

    #[test]
    fn test_xml_special_characters() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let manager = NfoManager::new(temp_dir.path().to_path_buf());

        // Create metadata with XML special characters
        let mut metadata = BookMetadata::new(
            "Book with <special> & \"characters\"".to_string(),
            "test".to_string(),
            "123".to_string(),
            1,
        );
        metadata.intro = Some("Description with <tags> & 'quotes' and \"more\"".to_string());

        // Write and read back
        let nfo_path = manager.write_book_nfo(123, &metadata).unwrap();
        let read_metadata = manager.read_book_nfo(&nfo_path).unwrap();

        // Verify special characters are preserved
        assert_eq!(
            read_metadata.title,
            "Book with <special> & \"characters\""
        );
        assert_eq!(
            read_metadata.intro,
            Some("Description with <tags> & 'quotes' and \"more\"".to_string())
        );
    }

    #[test]
    fn test_empty_optional_fields() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let manager = NfoManager::new(temp_dir.path().to_path_buf());

        // Create minimal metadata with no optional fields
        let metadata = BookMetadata::new(
            "Minimal Book".to_string(),
            "test".to_string(),
            "123".to_string(),
            5,
        );

        // Write and read back
        let nfo_path = manager.write_book_nfo(123, &metadata).unwrap();
        let read_metadata = manager.read_book_nfo(&nfo_path).unwrap();

        // Verify optional fields are None
        assert_eq!(read_metadata.author, None);
        assert_eq!(read_metadata.narrator, None);
        assert_eq!(read_metadata.intro, None);
        assert_eq!(read_metadata.cover_url, None);
        assert_eq!(read_metadata.total_duration, None);
        assert!(read_metadata.tags.items.is_empty());
    }
}
