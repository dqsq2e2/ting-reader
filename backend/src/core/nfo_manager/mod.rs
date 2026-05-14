//! NFO Metadata Manager
//!
//! Manages NFO metadata files for books and chapters.
//! NFO files store detailed metadata in XML format following Kodi/Jellyfin standards.
//!
//! File organization:
//! ```text
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

mod models;
#[cfg(test)]
mod tests;

pub use models::{BookMetadata, Tags, ChapterMetadata};

use crate::core::error::{Result, TingError};
use quick_xml::de::from_str;
use quick_xml::se::to_string;
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
    /// use ting_reader::core::nfo_manager::NfoManager;
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
    /// use ting_reader::core::nfo_manager::{NfoManager, BookMetadata};
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
    /// use ting_reader::core::nfo_manager::NfoManager;
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
    /// use ting_reader::core::nfo_manager::{NfoManager, ChapterMetadata};
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
    /// use ting_reader::core::nfo_manager::NfoManager;
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
