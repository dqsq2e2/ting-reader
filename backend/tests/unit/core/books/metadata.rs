use super::*;

fn metadata_chapter(id: u32, title: &str, duration: f64) -> AudiobookshelfChapter {
    AudiobookshelfChapter {
        id,
        start: 0.0,
        end: duration,
        title: title.to_string(),
    }
}

fn db_chapter(id: &str, path: &str, title: &str, duration: i32) -> Chapter {
    Chapter {
        id: id.to_string(),
        book_id: "book".to_string(),
        title: Some(title.to_string()),
        path: path.to_string(),
        duration: Some(duration),
        chapter_index: Some(1),
        is_extra: 0,
        hash: None,
        manual_corrected: 0,
        start_time: None,
        end_time: None,
        created_at: String::new(),
    }
}

#[test]
fn aligns_interleaved_extra_chapters_to_file_order() {
    let chapters = vec![
        metadata_chapter(0, "山河稷-第0001章-凶案（一）", 465.0),
        metadata_chapter(1, "山河稷-第0471章-元慕鱼番外（1）", 285.0),
        metadata_chapter(2, "山河稷-第0002章-凶案（二）", 519.0),
    ];
    let file_stems = vec![
        "山河稷-第0001章-凶案（一）".to_string(),
        "山河稷-第0002章-凶案（二）".to_string(),
        "山河稷-第0471章-元慕鱼番外（1）".to_string(),
    ];

    let (aligned, matched) = align_chapters_to_file_stems(chapters, &file_stems);

    assert!(matched);
    assert_eq!(aligned[1].title, file_stems[1]);
    assert_eq!(aligned[1].end - aligned[1].start, 519.0);
    assert_eq!(aligned[2].end - aligned[2].start, 285.0);
}

#[test]
fn keeps_original_order_when_titles_do_not_all_match() {
    let chapters = vec![
        metadata_chapter(0, "Chapter 1", 10.0),
        metadata_chapter(1, "Chapter 2", 20.0),
    ];
    let file_stems = vec!["Chapter 1".to_string(), "02".to_string()];

    let (aligned, matched) = align_chapters_to_file_stems(chapters, &file_stems);

    assert!(!matched);
    assert_eq!(aligned[0].title, "Chapter 1");
    assert_eq!(aligned[1].title, "Chapter 2");
}

#[test]
fn writes_offsets_in_natural_file_order_not_extra_display_order() {
    let chapters = vec![
        db_chapter("extra", "/book/0471-extra.mp3", "extra 1", 285),
        db_chapter("main-2", "/book/0002.mp3", "main 2", 519),
        db_chapter("main-1", "/book/0001.mp3", "main 1", 465),
    ];

    let metadata = build_audiobookshelf_chapters(chapters);

    assert_eq!(metadata[0].title, "main 1");
    assert_eq!(metadata[1].title, "main 2");
    assert_eq!(metadata[1].start, 465.0);
    assert_eq!(metadata[1].end, 984.0);
    assert_eq!(metadata[2].title, "extra 1");
}

#[test]
fn chinese_chapter_sidecar_offsets_follow_the_same_order_as_scanning() {
    let chapters = vec![
        db_chapter("c470", "/book/第四百七十章 示例.mp3", "470", 10),
        db_chapter("c10", "/book/第十章 示例.mp3", "10", 20),
        db_chapter("c2", "/book/第二章 示例.mp3", "2", 30),
    ];
    let metadata = build_audiobookshelf_chapters(chapters);
    assert_eq!(
        metadata
            .iter()
            .map(|chapter| chapter.title.as_str())
            .collect::<Vec<_>>(),
        ["2", "10", "470"]
    );
    assert_eq!(metadata[1].start, 30.0);
    assert_eq!(metadata[2].start, 50.0);
}

struct MetadataFixture {
    directory: tempfile::TempDir,
    chapters: ChapterRepository,
    series: SeriesRepository,
    libraries: LibraryRepository,
    nfo: NfoManager,
    books: crate::db::repository::BookRepository,
    book: Book,
    library: Library,
}

impl MetadataFixture {
    async fn new() -> Self {
        use crate::db::repository::Repository;
        let directory = tempfile::tempdir().unwrap();
        let db = std::sync::Arc::new(crate::db::DatabaseManager::new_in_memory().unwrap());
        let libraries = LibraryRepository::new(db.clone());
        let books = crate::db::repository::BookRepository::new(db.clone());
        let library = Library {
            id: "library".into(),
            name: "library".into(),
            library_type: "local".into(),
            url: directory.path().to_string_lossy().into_owned(),
            root_path: "/".into(),
            username: None,
            password: None,
            last_scanned_at: None,
            created_at: chrono::Utc::now().to_rfc3339(),
            scraper_config: Some(
                serde_json::to_string(&ScraperConfig {
                    nfo_writing_enabled: true,
                    metadata_writing_enabled: true,
                    ..Default::default()
                })
                .unwrap(),
            ),
        };
        libraries.create(&library).await.unwrap();
        let book = Book {
            id: "book".into(),
            library_id: library.id.clone(),
            title: Some("Book 2".into()),
            author: Some("Author".into()),
            narrator: Some("Narrator".into()),
            cover_url: None,
            theme_color: None,
            description: Some("Description".into()),
            skip_intro: 0,
            skip_outro: 0,
            path: directory.path().to_string_lossy().into_owned(),
            hash: "book-hash".into(),
            tags: Some(" tag , , second ".into()),
            genre: Some(" fantasy , ".into()),
            year: None,
            created_at: chrono::Utc::now().to_rfc3339(),
            manual_corrected: 0,
            match_pattern: None,
            chapter_regex: None,
        };
        books.create(&book).await.unwrap();
        Self {
            chapters: ChapterRepository::new(db.clone()),
            series: SeriesRepository::new(db),
            nfo: NfoManager::new(directory.path().into()),
            directory,
            libraries,
            books,
            book,
            library,
        }
    }

    fn service(&self) -> BookMetadataService<'_> {
        BookMetadataService {
            chapters: &self.chapters,
            series: &self.series,
            libraries: &self.libraries,
            nfo: &self.nfo,
            remote: None,
        }
    }
}

#[tokio::test]
async fn edits_preserve_extended_fields_and_unselected_flags_and_use_one_nfo_mapping() {
    use crate::db::repository::Repository;
    let fixture = MetadataFixture::new().await;
    let dir = fixture.directory.path();
    let mut original = AudiobookshelfMetadata {
        subtitle: Some("Original subtitle".into()),
        published_year: Some("1999".into()),
        explicit: true,
        abridged: true,
        ..Default::default()
    };
    original
        .extra
        .insert("providerId".into(), "opaque-id".into());
    write_metadata_json(dir, &original).unwrap();
    for mut chapter in [
        db_chapter("second", "02.mp3", "Second", 12),
        db_chapter("first", "01.mp3", "First", 10),
    ] {
        chapter.created_at = chrono::Utc::now().to_rfc3339();
        fixture.chapters.create(&chapter).await.unwrap();
    }
    let patch = MetadataPatch {
        publisher: Some("New publisher".into()),
        explicit: Some(false),
        duration: Some(900),
        ..Default::default()
    };
    fixture
        .service()
        .sync(&fixture.book, MetadataChange::Book, &patch)
        .await
        .unwrap();
    let metadata = read_metadata_json(dir).unwrap().unwrap();
    assert_eq!(metadata.subtitle, original.subtitle);
    assert_eq!(metadata.published_year, original.published_year);
    assert_eq!(metadata.extra["providerId"], "opaque-id");
    assert!(!metadata.explicit);
    assert!(metadata.abridged);
    assert_eq!(metadata.chapters[0].title, "First");
    assert_eq!(metadata.chapters[1].start, 10.0);
    assert_eq!(metadata.tags, ["tag", "second"]);
    assert_eq!(metadata.genres, ["fantasy"]);
    let nfo = fixture.nfo.read_book_nfo(&dir.join("book.nfo")).unwrap();
    assert_eq!(nfo.subtitle, original.subtitle);
    assert_eq!(nfo.tags.items, metadata.tags);
    assert_eq!(nfo.genre.items, metadata.genres);
    assert_eq!(nfo.chapter_count, 2);
    assert_eq!(nfo.total_duration, Some(900));
}

#[tokio::test]
async fn chapter_and_series_changes_keep_sidecar_owned_book_fields() {
    use crate::db::repository::Repository;
    let fixture = MetadataFixture::new().await;
    let original = AudiobookshelfMetadata {
        title: Some("Sidecar title".into()),
        subtitle: Some("Sidecar subtitle".into()),
        ..Default::default()
    };
    write_metadata_json(fixture.directory.path(), &original).unwrap();
    let mut chapter = db_chapter("chapter", "01.mp3", "Renamed chapter", 20);
    chapter.created_at = chrono::Utc::now().to_rfc3339();
    fixture.chapters.create(&chapter).await.unwrap();
    fixture
        .service()
        .sync(&fixture.book, MetadataChange::Chapters, &Default::default())
        .await
        .unwrap();
    let changed = read_metadata_json(fixture.directory.path())
        .unwrap()
        .unwrap();
    assert_eq!(changed.title, original.title);
    assert_eq!(changed.subtitle, original.subtitle);
    assert_eq!(changed.chapters[0].title, "Renamed chapter");
    fixture
        .service()
        .import_series(&fixture.book, &["Series #4".into()])
        .await
        .unwrap();
    fixture
        .service()
        .sync(&fixture.book, MetadataChange::Series, &Default::default())
        .await
        .unwrap();
    let changed = read_metadata_json(fixture.directory.path())
        .unwrap()
        .unwrap();
    assert_eq!(changed.title, original.title);
    assert_eq!(changed.chapters[0].title, "Renamed chapter");
    assert_eq!(changed.series, ["Series #4"]);
}

#[tokio::test]
async fn corrupt_sidecars_survive_edits_without_defaults_replacing_them() {
    let fixture = MetadataFixture::new().await;
    let path = fixture.directory.path().join("metadata.json");
    let bytes = b"{ malformed JSON with user-owned content";
    std::fs::write(&path, bytes).unwrap();
    assert!(
        fixture
            .service()
            .sync(&fixture.book, MetadataChange::Book, &Default::default(),)
            .await
            .is_err()
    );
    assert_eq!(std::fs::read(path).unwrap(), bytes);
    assert!(!fixture.directory.path().join("book.nfo").exists());
}

#[tokio::test]
async fn nfo_only_edits_preserve_subtitle_source_and_creation_time() {
    let fixture = MetadataFixture::new().await;
    let mut original = BookMetadata::new(
        "Original".into(),
        "custom-source".into(),
        "original-id".into(),
        0,
    );
    original.subtitle = Some("User subtitle".into());
    original.created_at = 123;
    fixture
        .nfo
        .write_book_nfo_to_dir(fixture.directory.path(), &original)
        .unwrap();
    fixture
        .service()
        .sync(&fixture.book, MetadataChange::Book, &Default::default())
        .await
        .unwrap();
    let nfo = fixture
        .nfo
        .read_book_nfo(&fixture.directory.path().join("book.nfo"))
        .unwrap();
    assert_eq!(nfo.subtitle, original.subtitle);
    assert_eq!(nfo.source, original.source);
    assert_eq!(nfo.source_id, original.source_id);
    assert_eq!(nfo.created_at, original.created_at);
    let json = read_metadata_json(fixture.directory.path())
        .unwrap()
        .unwrap();
    assert_eq!(json.subtitle, original.subtitle);
}

#[tokio::test]
async fn corrupt_nfo_is_not_replaced_and_does_not_partially_rewrite_json() {
    let fixture = MetadataFixture::new().await;
    let dir = fixture.directory.path();
    let original = AudiobookshelfMetadata {
        title: Some("Original".into()),
        ..Default::default()
    };
    write_metadata_json(dir, &original).unwrap();
    let json_before = std::fs::read(dir.join("metadata.json")).unwrap();
    let nfo_before = b"<audiobook><title>incomplete XML";
    std::fs::write(dir.join("book.nfo"), nfo_before).unwrap();
    assert!(
        fixture
            .service()
            .sync(&fixture.book, MetadataChange::Book, &Default::default())
            .await
            .is_err()
    );
    assert_eq!(
        std::fs::read(dir.join("metadata.json")).unwrap(),
        json_before
    );
    assert_eq!(std::fs::read(dir.join("book.nfo")).unwrap(), nfo_before);
}

#[test]
fn partial_json_and_nfo_share_normalized_metadata_fields() {
    let json: AudiobookshelfMetadata = serde_json::from_value(serde_json::json!({
        "title": "Book", "subtitle": "Subtitle", "authors": ["Author"],
        "narrators": ["Narrator"], "tags": ["tag"], "genres": ["Genre"],
        "coverUrl": "cover.jpg",
    }))
    .unwrap();
    let fields = BookMetadataFields::from(json);
    let mut nfo = BookMetadata::new("Book".into(), "source".into(), "id".into(), 0);
    nfo.subtitle = Some("Subtitle".into());
    nfo.author = Some("Author".into());
    nfo.narrator = Some("Narrator".into());
    nfo.tags.items = vec!["tag".into()];
    nfo.genre.items = vec!["Genre".into()];
    nfo.cover_url = Some("cover.jpg".into());
    let nfo_fields = BookMetadataFields::from(nfo);
    assert_eq!(fields.title, nfo_fields.title);
    assert_eq!(fields.subtitle, nfo_fields.subtitle);
    assert_eq!(fields.author, nfo_fields.author);
    assert_eq!(fields.narrator, nfo_fields.narrator);
    assert_eq!(fields.tags, nfo_fields.tags);
    assert_eq!(fields.genre, nfo_fields.genre);
    assert_eq!(fields.cover_url, nfo_fields.cover_url);
}

#[test]
fn empty_priority_uses_the_same_defaults_as_new_library_config() {
    let config = ScraperConfig::default();
    let mut empty = config.clone();
    empty.metadata_priority.clear();
    assert_eq!(
        empty.metadata_sources().collect::<Vec<_>>(),
        config.metadata_sources().collect::<Vec<_>>()
    );
    assert_eq!(empty.metadata_rank("filename"), 0);
    assert_eq!(empty.metadata_rank("local_metadata"), 1);
    assert_eq!(empty.metadata_rank("audio_metadata"), 2);
    empty.metadata_priority = vec!["scraper".into(), "audio_metadata".into()];
    assert_eq!(empty.metadata_rank("scraper"), 1);
    assert_eq!(empty.metadata_rank("audio_metadata"), 2);
    assert_eq!(empty.metadata_rank("local_metadata"), usize::MAX - 1);
}

#[test]
fn title_priority_selects_available_sources_and_rejects_generic_audio_tags() {
    for (priority, expected) in [
        (
            vec!["filename", "local_metadata", "audio_metadata", "scraper"],
            ("目录标题", "filename"),
        ),
        (
            vec!["local_metadata", "filename", "audio_metadata", "scraper"],
            ("元数据标题", "local_metadata"),
        ),
        (
            vec!["audio_metadata", "local_metadata", "filename", "scraper"],
            ("音频标题", "audio_metadata"),
        ),
        (
            vec!["scraper", "local_metadata", "audio_metadata", "filename"],
            ("刮削标题", "scraper"),
        ),
    ] {
        let config = ScraperConfig {
            metadata_priority: priority.into_iter().map(str::to_owned).collect(),
            ..Default::default()
        };
        assert_eq!(
            config.select_title(
                "目录标题",
                Some("元数据标题"),
                Some("音频标题"),
                Some("刮削标题")
            ),
            expected
        );
    }
    let config = ScraperConfig {
        metadata_priority: vec![
            "audio_metadata".into(),
            "local_metadata".into(),
            "scraper".into(),
            "filename".into(),
        ],
        ..Default::default()
    };
    assert_eq!(
        config.select_title("目录标题", Some("元数据标题"), Some("Track 01"), None),
        ("元数据标题", "local_metadata")
    );
    assert_eq!(
        config.select_title("目录标题", Some(" "), None, Some("刮削标题")),
        ("刮削标题", "scraper")
    );
    assert_eq!(
        config.select_title("目录标题", None, None, None),
        ("目录标题", "filename")
    );
}

struct RemoteMetadataResources {
    server: tokio::task::JoinHandle<()>,
    cache: PathBuf,
}

impl Drop for RemoteMetadataResources {
    fn drop(&mut self) {
        self.server.abort();
        let root = std::env::current_dir().unwrap().join("temp");
        if self.cache.parent() == Some(root.as_path()) {
            let _ = std::fs::remove_dir_all(&self.cache);
        }
    }
}

#[tokio::test]
async fn webdav_scans_only_cache_and_edits_upload_only_when_enabled_even_in_cloud_mode() {
    use axum::extract::{Request, State};
    use std::sync::Arc;
    use tokio::sync::Mutex;
    type Uploads = Arc<Mutex<Vec<(String, Vec<u8>)>>>;
    let mut fixture = MetadataFixture::new().await;
    let uploads = Arc::new(Mutex::new(Vec::<(String, Vec<u8>)>::new()));
    let app = axum::Router::new()
        .fallback(axum::routing::put(
            |State(uploads): State<Uploads>, request: Request| async move {
                let path = request.uri().path().to_string();
                let bytes = axum::body::to_bytes(request.into_body(), 1024 * 1024)
                    .await
                    .unwrap();
                uploads.lock().await.push((path, bytes.to_vec()));
                axum::http::StatusCode::NO_CONTENT
            },
        ))
        .with_state(uploads.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    fixture.library.library_type = "webdav".into();
    fixture.library.url = format!("http://{}/dav/", listener.local_addr().unwrap());
    fixture.book.path = format!("{}book-{}", fixture.library.url, uuid::Uuid::new_v4());
    let cache = remote_metadata_dir(&fixture.book.path).unwrap();
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let _resources = RemoteMetadataResources {
        server,
        cache: cache.clone(),
    };
    let storage = StorageService::new();
    let key = [0; 32];
    let mut config = ScraperConfig {
        cloud_mode: true,
        ..Default::default()
    };
    fixture.library.scraper_config = Some(serde_json::to_string(&config).unwrap());
    fixture.libraries.update(&fixture.library).await.unwrap();
    let service = BookMetadataService {
        remote: Some(MetadataRemote {
            storage: &storage,
            key: &key,
        }),
        ..fixture.service()
    };
    service.ensure_remote_cache(&fixture.book).await.unwrap();
    assert!(cache.join("metadata.json").is_file());
    assert!(cache.join("book.nfo").is_file());
    assert!(uploads.lock().await.is_empty());

    service
        .sync(&fixture.book, MetadataChange::Book, &Default::default())
        .await
        .unwrap();
    assert!(uploads.lock().await.is_empty());
    config.webdav_metadata_writing_enabled = true;
    fixture.library.scraper_config = Some(serde_json::to_string(&config).unwrap());
    fixture.libraries.update(&fixture.library).await.unwrap();
    let service = BookMetadataService {
        remote: Some(MetadataRemote {
            storage: &storage,
            key: &key,
        }),
        ..fixture.service()
    };
    service
        .sync(
            &fixture.book,
            MetadataChange::Book,
            &MetadataPatch {
                subtitle: Some("Updated subtitle".into()),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    let uploads = uploads.lock().await;
    assert_eq!(uploads.len(), 2);
    assert!(uploads[0].0.ends_with("/metadata.json"));
    assert!(uploads[1].0.ends_with("/book.nfo"));
    let json: AudiobookshelfMetadata = serde_json::from_slice(&uploads[0].1).unwrap();
    assert_eq!(json.subtitle.as_deref(), Some("Updated subtitle"));
    assert!(String::from_utf8_lossy(&uploads[1].1).contains("Updated subtitle"));
}

#[tokio::test]
async fn disabled_local_writes_do_not_create_or_rewrite_sidecars() {
    let mut fixture = MetadataFixture::new().await;
    fixture.library.scraper_config = Some(
        serde_json::to_string(&ScraperConfig {
            nfo_writing_enabled: false,
            metadata_writing_enabled: false,
            ..Default::default()
        })
        .unwrap(),
    );
    fixture.libraries.update(&fixture.library).await.unwrap();
    for change in [
        MetadataChange::Book,
        MetadataChange::Chapters,
        MetadataChange::Series,
    ] {
        fixture
            .service()
            .sync(&fixture.book, change, &Default::default())
            .await
            .unwrap();
    }
    assert!(!fixture.directory.path().join("metadata.json").exists());
    assert!(!fixture.directory.path().join("book.nfo").exists());
}

#[tokio::test]
async fn shared_series_import_orders_naturally_and_respects_manual_ownership() {
    use crate::db::repository::Repository;
    let fixture = MetadataFixture::new().await;
    fixture
        .service()
        .import_series(&fixture.book, &["Saga".into()])
        .await
        .unwrap();
    let mut other = fixture.book.clone();
    other.id = "other".into();
    other.hash = "other-hash".into();
    other.title = Some("Book 1".into());
    fixture.books.create(&other).await.unwrap();
    fixture
        .service()
        .import_series(&other, &["Saga".into()])
        .await
        .unwrap();
    assert_eq!(
        fixture.service().series_titles(&other.id).await.unwrap(),
        ["Saga #1"]
    );
    assert_eq!(
        fixture
            .service()
            .series_titles(&fixture.book.id)
            .await
            .unwrap(),
        ["Saga #2"]
    );
    fixture
        .service()
        .import_series(&fixture.book, &["Saga #8".into()])
        .await
        .unwrap();
    assert_eq!(
        fixture
            .service()
            .series_titles(&fixture.book.id)
            .await
            .unwrap(),
        ["Saga #8"]
    );
    other.manual_corrected = 1;
    fixture
        .service()
        .import_series(&other, &["Unwanted series #9".into()])
        .await
        .unwrap();
    assert_eq!(
        fixture.service().series_titles(&other.id).await.unwrap(),
        ["Saga #1"]
    );
}
