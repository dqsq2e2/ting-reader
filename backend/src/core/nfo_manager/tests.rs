use super::*;
use std::path::{Path, PathBuf};

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
