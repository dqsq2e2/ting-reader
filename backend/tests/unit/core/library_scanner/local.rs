use super::{collapse_local_scan_roots, local_group_is_changed, mark_changed_local_directory};
use crate::core::library_scanner::shared::CoalescedRangeDirs;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

#[tokio::test]
async fn parent_series_people_reach_saved_volumes_and_numeric_chapter_indexes() {
    use crate::core::library_scanner::{LocalLibraryScanContext, ScanMode, test_support};
    let directory = tempfile::tempdir().unwrap();
    let scanner = test_support::scanner(directory.path());
    let root = directory.path().join("library");
    let parent = root.join("示例书名 - 示例作者 ／ 示例演播");
    let chapters = [
        "10",
        "2",
        "1",
        "第四百七十章 示例",
        "第十章 示例",
        "第三章 示例",
    ];
    for volume in ["示例书名第一部", "示例书名第二部", "示例书名第三部"] {
        for (range, files) in [("1-50", &chapters[..3]), ("51-100任意文字", &chapters[3..])] {
            let path = parent.join(volume).join(range);
            std::fs::create_dir_all(&path).unwrap();
            for name in files {
                std::fs::write(
                    path.join(format!("{name}.strm")),
                    "https://example.test/audio",
                )
                .unwrap();
            }
        }
    }
    let library = test_support::library(&scanner, &root).await;
    let mut config = crate::db::models::ScraperConfig {
        cloud_mode: true,
        extract_audio_cover: false,
        metadata_priority: vec!["local_metadata".into()],
        ..Default::default()
    };
    // Changing the setting must invalidate scan caches without changing files.
    // Disabling it later must preserve existing relationships.
    for (enabled, mode, expected_series) in [
        (false, ScanMode::Full, 0),
        (false, ScanMode::Incremental, 0),
        (true, ScanMode::Incremental, 1),
        (false, ScanMode::Full, 1),
    ] {
        config.auto_generate_series = enabled;
        let result = scanner
            .scan_local_library(LocalLibraryScanContext {
                library_id: &library.id,
                path: &root,
                task_id: None,
                last_scanned: None,
                scraper_config: &config,
                mode,
                scan_paths: None,
            })
            .await
            .unwrap();
        assert_eq!(result.failed_count, 0, "{:?}", result.errors);
        let books = scanner
            .book_repo
            .find_by_library(&library.id)
            .await
            .unwrap();
        assert_eq!(books.len(), 3);
        for book in books {
            assert!(book.title.as_deref().unwrap().starts_with("示例书名第"));
            assert_eq!(book.author.as_deref(), Some("示例作者"));
            assert_eq!(book.narrator.as_deref(), Some("示例演播"));
            let chapters = scanner.chapter_repo.find_by_book(&book.id).await.unwrap();
            let names: Vec<_> = chapters
                .iter()
                .map(|chapter| {
                    Path::new(&chapter.path)
                        .file_stem()
                        .unwrap()
                        .to_str()
                        .unwrap()
                })
                .collect();
            assert_eq!(
                names,
                [
                    "1",
                    "2",
                    "10",
                    "第三章 示例",
                    "第十章 示例",
                    "第四百七十章 示例"
                ]
            );
            let series = scanner
                .series_repo
                .find_series_by_book(&book.id)
                .await
                .unwrap();
            assert_eq!(series.len(), expected_series);
            if expected_series == 0 {
                continue;
            }
            assert_eq!(series[0].title, "示例书名");
            let members = scanner
                .series_repo
                .find_books_by_series(&series[0].id)
                .await
                .unwrap();
            assert_eq!(members.len(), 3);
            assert_eq!(
                members.iter().map(|(_, order)| *order).collect::<Vec<_>>(),
                [1, 2, 3]
            );
        }
    }
}

#[test]
fn series_generation_defaults_on_for_legacy_configs_and_preserves_explicit_false() {
    use crate::db::models::ScraperConfig;
    assert!(ScraperConfig::default().auto_generate_series);
    for json in ["{}", r#"{"cloud_mode":true,"default_sources":[]}"#] {
        let config: ScraperConfig = serde_json::from_str(json).unwrap();
        assert!(config.auto_generate_series);
    }
    let config: ScraperConfig = serde_json::from_str(r#"{"auto_generate_series":false}"#).unwrap();
    assert!(!config.auto_generate_series);
    let saved = serde_json::to_value(&config).unwrap();
    assert_eq!(saved["auto_generate_series"], false);
}

#[tokio::test]
async fn local_metadata_json_series_import_ignores_directory_series_setting() {
    use crate::core::library_scanner::{LocalLibraryScanContext, ScanMode, test_support};
    let directory = tempfile::tempdir().unwrap();
    let scanner = test_support::scanner(directory.path());
    let root = directory.path().join("library");
    let book_dir = root.join("独立书籍");
    std::fs::create_dir_all(&book_dir).unwrap();
    std::fs::write(book_dir.join("001.strm"), "https://example.test/audio").unwrap();
    std::fs::write(
        book_dir.join("metadata.json"),
        r#"{"title":"元数据书名","series":["元数据系列 #2"],"chapters":[{"id":0,"start":0,"end":10,"title":"元数据章节"}]}"#,
    )
    .unwrap();
    let library = test_support::library(&scanner, &root).await;
    let mut config = crate::db::models::ScraperConfig {
        cloud_mode: true,
        extract_audio_cover: false,
        metadata_priority: vec!["local_metadata".into()],
        ..Default::default()
    };
    for (enabled, priority, expected_book, expected_chapter) in [
        (false, vec!["filename", "local_metadata"], "独立书籍", "1"),
        (
            true,
            vec!["local_metadata", "filename"],
            "元数据书名",
            "元数据章节",
        ),
        (false, vec!["filename", "local_metadata"], "独立书籍", "1"),
    ] {
        config.auto_generate_series = enabled;
        config.metadata_priority = priority.into_iter().map(str::to_owned).collect();
        let result = scanner
            .scan_local_library(LocalLibraryScanContext {
                library_id: &library.id,
                path: &root,
                task_id: None,
                last_scanned: None,
                scraper_config: &config,
                mode: ScanMode::Incremental,
                scan_paths: None,
            })
            .await
            .unwrap();
        assert_eq!(result.failed_count, 0, "{:?}", result.errors);
        let books = scanner
            .book_repo
            .find_by_library(&library.id)
            .await
            .unwrap();
        assert_eq!(books.len(), 1);
        assert_eq!(books[0].title.as_deref(), Some(expected_book));
        let chapters = scanner
            .chapter_repo
            .find_by_book(&books[0].id)
            .await
            .unwrap();
        assert_eq!(chapters.len(), 1);
        assert_eq!(chapters[0].title.as_deref(), Some(expected_chapter));
        assert_eq!(chapters[0].duration, Some(10));
        let series = scanner
            .series_repo
            .find_series_by_book(&books[0].id)
            .await
            .unwrap();
        assert_eq!(series.len(), 1);
        if let Some(series) = series.first() {
            assert_eq!(series.title, "元数据系列");
            let members = scanner
                .series_repo
                .find_books_by_series(&series.id)
                .await
                .unwrap();
            assert_eq!(members[0].1, 2);
        }
    }
}

#[tokio::test]
async fn full_scan_saves_clean_directory_titles_and_all_completed_range_chapters() {
    use crate::core::library_scanner::{LocalLibraryScanContext, ScanMode, test_support};
    let directory = tempfile::tempdir().unwrap();
    let scanner = test_support::scanner(directory.path());
    let root = directory.path().join("books");
    for name in ["书名-作者／演播", "另一本 ｜ 简介", "单名示例-示例演播"] {
        for (range, chapter) in [
            ("1-50", "001"),
            ("51-100", "051"),
            ("101-150完结撒花", "101"),
        ] {
            let path = root.join(name).join(range);
            std::fs::create_dir_all(&path).unwrap();
            std::fs::write(
                path.join(format!("{chapter}.strm")),
                "https://example.test/audio.mp3",
            )
            .unwrap();
        }
    }
    let library = test_support::library(&scanner, &root).await;
    let config = crate::db::models::ScraperConfig {
        metadata_priority: vec!["local_metadata".into()],
        extract_audio_cover: false,
        cloud_mode: true,
        ..Default::default()
    };
    for mode in [ScanMode::Full, ScanMode::Full] {
        let result = scanner
            .scan_local_library(LocalLibraryScanContext {
                library_id: &library.id,
                path: &root,
                task_id: None,
                last_scanned: None,
                scraper_config: &config,
                mode,
                scan_paths: None,
            })
            .await
            .unwrap();
        assert_eq!(result.failed_count, 0, "{:?}", result.errors);
        let mut books = scanner
            .book_repo
            .find_by_library(&library.id)
            .await
            .unwrap();
        books.sort_by(|a, b| a.title.cmp(&b.title));
        assert_eq!(books.len(), 3);
        assert_eq!(
            books
                .iter()
                .map(|book| book.title.as_deref().unwrap())
                .collect::<Vec<_>>(),
            vec!["书名", "单名示例", "另一本"]
        );
        for book in books {
            if book.title.as_deref() == Some("书名") {
                assert_eq!(book.author.as_deref(), Some("作者"));
                assert_eq!(book.narrator.as_deref(), Some("演播"));
            } else if book.title.as_deref() == Some("单名示例") {
                assert_eq!(book.author.as_deref(), Some("示例演播"));
                assert!(book.narrator.is_none());
            } else {
                assert!(book.author.is_none());
                assert!(book.narrator.is_none());
            }
            let chapters = scanner.chapter_repo.find_by_book(&book.id).await.unwrap();
            assert_eq!(chapters.len(), 3);
            assert!(
                chapters
                    .iter()
                    .any(|chapter| chapter.path.contains("101-150完结撒花"))
            );
        }
    }
}

#[tokio::test]
async fn directory_title_cleanup_preserves_explicit_metadata_and_group_override() {
    use crate::core::library_scanner::test_support;
    let directory = tempfile::tempdir().unwrap();
    let scanner = test_support::scanner(directory.path());
    let book_dir = directory.path().join("目录书名-目录作者／目录演播");
    std::fs::create_dir_all(&book_dir).unwrap();
    std::fs::write(
        book_dir.join("metadata.json"),
        r#"{"title":"真实书名 - 副标题","authors":["真实作者"],"narrators":["真实演播"]}"#,
    )
    .unwrap();
    let mut config = crate::db::models::ScraperConfig {
        metadata_priority: vec!["local_metadata".into(), "filename".into()],
        extract_audio_cover: false,
        ..Default::default()
    };
    let (metadata, _) = scanner
        .extract_final_metadata(&book_dir, &[], &config, Some("合并书名-目录作者／目录演播"))
        .await;
    assert_eq!(metadata.title.as_deref(), Some("真实书名 - 副标题"));
    assert_eq!(metadata.author.as_deref(), Some("真实作者"));
    assert_eq!(metadata.narrator.as_deref(), Some("真实演播"));
    config.metadata_priority.reverse();
    let (metadata, _) = scanner
        .extract_final_metadata(&book_dir, &[], &config, Some("合并书名-目录作者／目录演播"))
        .await;
    assert_eq!(metadata.title.as_deref(), Some("合并书名"));
    assert_eq!(metadata.author.as_deref(), Some("真实作者"));
    assert_eq!(metadata.narrator.as_deref(), Some("真实演播"));
}

#[tokio::test]
async fn directory_contributors_are_available_without_scraping_or_sidecars() {
    use crate::core::library_scanner::test_support;
    let directory = tempfile::tempdir().unwrap();
    let scanner = test_support::scanner(directory.path());
    let config = crate::db::models::ScraperConfig {
        metadata_priority: vec!["local_metadata".into()],
        extract_audio_cover: false,
        ..Default::default()
    };
    for (name, author, narrator) in [
        ("示例书名-示例作者", Some("示例作者"), None),
        ("示例书名-示例演播", Some("示例演播"), None),
        (
            "示例书名-示例作者／示例演播",
            Some("示例作者"),
            Some("示例演播"),
        ),
        (
            "示例书名-示例作者-示例演播",
            Some("示例作者"),
            Some("示例演播"),
        ),
        ("示例书名-／示例演播", None, Some("示例演播")),
    ] {
        let path = directory.path().join(name);
        std::fs::create_dir_all(&path).unwrap();
        let (metadata, _) = scanner
            .extract_final_metadata(&path, &[], &config, None)
            .await;
        assert_eq!(metadata.title.as_deref(), Some("示例书名"));
        assert_eq!(metadata.author.as_deref(), author);
        assert_eq!(metadata.narrator.as_deref(), narrator);
    }
}
#[test]
fn collapses_overlapping_scan_roots() {
    let roots = vec![
        PathBuf::from("library/book/chapters"),
        PathBuf::from("library/book"),
        PathBuf::from("library/other"),
        PathBuf::from("library/book"),
    ];

    assert_eq!(
        collapse_local_scan_roots(roots),
        vec![
            PathBuf::from("library/book"),
            PathBuf::from("library/other")
        ]
    );
}

#[test]
fn changed_directory_does_not_wake_siblings() {
    let mut changed_dirs = HashSet::new();
    let mut changed_ancestors = HashSet::new();
    mark_changed_local_directory(
        Some(Path::new("library/book-a")),
        &mut changed_dirs,
        &mut changed_ancestors,
    );

    assert!(local_group_is_changed(
        Path::new("library/book-a"),
        None,
        &changed_dirs,
        &changed_ancestors
    ));
    assert!(!local_group_is_changed(
        Path::new("library/book-b"),
        None,
        &changed_dirs,
        &changed_ancestors
    ));
}

#[test]
fn changed_range_child_wakes_only_its_coalesced_book() {
    let mut changed_dirs = HashSet::new();
    let mut changed_ancestors = HashSet::new();
    mark_changed_local_directory(
        Some(Path::new("library/book/001-100")),
        &mut changed_dirs,
        &mut changed_ancestors,
    );
    let range_dirs = CoalescedRangeDirs {
        child_dirs: vec![
            PathBuf::from("library/book/001-100"),
            PathBuf::from("library/book/101-200"),
        ],
        title_override: None,
    };

    assert!(local_group_is_changed(
        Path::new("library/book"),
        Some(&range_dirs),
        &changed_dirs,
        &changed_ancestors
    ));
    assert!(!local_group_is_changed(
        Path::new("library/other-book"),
        None,
        &changed_dirs,
        &changed_ancestors
    ));
}

fn mp4_atom(kind: &[u8; 4], payload: &[u8]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(8 + payload.len());
    bytes.extend_from_slice(&((payload.len() + 8) as u32).to_be_bytes());
    bytes.extend_from_slice(kind);
    bytes.extend_from_slice(payload);
    bytes
}

fn mp4_container(kind: &[u8; 4], children: &[Vec<u8>]) -> Vec<u8> {
    mp4_atom(kind, &children.concat())
}

fn mp4_text_tag(name: &[u8; 4], value: &str) -> Vec<u8> {
    let mut payload = 1_u32.to_be_bytes().to_vec();
    payload.extend_from_slice(&[0, 0, 0, 0]);
    payload.extend_from_slice(value.as_bytes());
    mp4_atom(name, &mp4_atom(b"data", &payload))
}

fn m4b_fixture(chapters: Option<&[(u64, &str)]>) -> Vec<u8> {
    let ilst_items = vec![
        mp4_text_tag(b"\xa9nam", "Embedded Book"),
        mp4_text_tag(b"\xa9alb", "Embedded Book"),
        mp4_text_tag(b"\xa9ART", "Narrator Name"),
        mp4_text_tag(b"aART", "Author Name"),
        mp4_text_tag(b"\xa9wrt", "Narrator Name"),
    ];
    let mut hdlr_payload = vec![0_u8; 8];
    hdlr_payload.extend_from_slice(b"mdir");
    hdlr_payload.extend_from_slice(&[0; 13]);
    let meta = mp4_container(
        b"meta",
        &[
            mp4_atom(b"hdlr", &hdlr_payload),
            mp4_container(b"ilst", &ilst_items),
        ],
    );

    let mut udta = Vec::new();
    if let Some(chapters) = chapters {
        let mut payload = vec![1, 0, 0, 0, 0, 0, 0, 0, chapters.len() as u8];
        for (start_ms, title) in chapters {
            payload.extend_from_slice(&(start_ms * 10_000).to_be_bytes());
            payload.push(title.len() as u8);
            payload.extend_from_slice(title.as_bytes());
        }
        udta.push(mp4_atom(b"chpl", &payload));
    }
    udta.push(meta);

    let mut mvhd_payload = vec![0_u8; 4];
    mvhd_payload.extend_from_slice(&0_u32.to_be_bytes());
    mvhd_payload.extend_from_slice(&0_u32.to_be_bytes());
    mvhd_payload.extend_from_slice(&1000_u32.to_be_bytes());
    mvhd_payload.extend_from_slice(&180_000_u32.to_be_bytes());
    mvhd_payload.resize(100, 0);

    let mut file = mp4_atom(b"ftyp", b"M4B \0\0\0\0M4B mp42");
    file.extend(mp4_atom(b"mdat", &[0_u8; 32]));
    file.extend(mp4_container(
        b"moov",
        &[
            mp4_atom(b"mvhd", &mvhd_payload),
            mp4_container(b"udta", &udta),
        ],
    ));
    file
}

async fn scan_local(
    scanner: &crate::core::library_scanner::LibraryScanner,
    library_id: &str,
    root: &Path,
    config: &crate::db::models::ScraperConfig,
    mode: crate::core::library_scanner::ScanMode,
    last_scanned: Option<chrono::DateTime<chrono::Utc>>,
) -> crate::core::library_scanner::ScanResult {
    scanner
        .scan_local_library(crate::core::library_scanner::LocalLibraryScanContext {
            library_id,
            path: root,
            task_id: None,
            last_scanned,
            scraper_config: config,
            mode,
            scan_paths: None,
        })
        .await
        .unwrap()
}

#[tokio::test]
async fn single_file_m4b_chapters_are_created_and_kept_incremental() {
    use crate::core::library_scanner::{ScanMode, test_support};
    use crate::db::repository::Repository;

    let directory = tempfile::tempdir().unwrap();
    let scanner = test_support::scanner(directory.path());
    let root = directory.path().join("library");
    let book_dir = root.join("Embedded Book");
    std::fs::create_dir_all(&book_dir).unwrap();
    let audio = book_dir.join("Embedded Book.m4b");
    std::fs::write(
        &audio,
        m4b_fixture(Some(&[
            (0, "Opening Credits"),
            (60_000, "Chapter 1"),
            (120_000, "Chapter 2"),
        ])),
    )
    .unwrap();

    let library = test_support::library(&scanner, &root).await;
    let config = crate::db::models::ScraperConfig::default();

    let result = scan_local(&scanner, &library.id, &root, &config, ScanMode::Full, None).await;
    assert_eq!(result.failed_count, 0, "{:?}", result.errors);
    let books = scanner
        .book_repo
        .find_by_library(&library.id)
        .await
        .unwrap();
    assert_eq!(books.len(), 1);
    let book = &books[0];
    assert_eq!(book.title.as_deref(), Some("Embedded Book"));
    assert_eq!(book.author.as_deref(), Some("Author Name"));
    assert_eq!(book.narrator.as_deref(), Some("Narrator Name"));

    let chapters = scanner.chapter_repo.find_by_book(&book.id).await.unwrap();
    let summary: Vec<(String, i32, Option<i64>, Option<i64>)> = chapters
        .iter()
        .map(|chapter| {
            (
                chapter.title.clone().unwrap_or_default(),
                chapter.duration.unwrap_or_default(),
                chapter.start_time,
                chapter.end_time,
            )
        })
        .collect();
    assert_eq!(
        summary,
        vec![
            ("Opening Credits".into(), 60, Some(0), Some(60_000)),
            ("Chapter 1".into(), 60, Some(60_000), Some(120_000)),
            ("Chapter 2".into(), 60, Some(120_000), Some(180_000)),
        ]
    );
    assert!(
        chapters
            .iter()
            .all(|chapter| chapter.path == audio.to_string_lossy())
    );

    // An unchanged incremental scan keeps the split without re-probing.
    let scanned_at = chrono::Utc::now() + chrono::Duration::seconds(5);
    let result = scan_local(
        &scanner,
        &library.id,
        &root,
        &config,
        ScanMode::Incremental,
        Some(scanned_at),
    )
    .await;
    assert_eq!(result.failed_count, 0, "{:?}", result.errors);
    assert_eq!(
        scanner
            .chapter_repo
            .find_by_book(&book.id)
            .await
            .unwrap()
            .len(),
        3
    );

    // A whole-file chapter from an older version is replaced on the next scan.
    scanner.chapter_repo.delete_by_book(&book.id).await.unwrap();
    scanner
        .chapter_repo
        .create(&crate::db::models::Chapter {
            id: "legacy-chapter".into(),
            book_id: book.id.clone(),
            title: Some("Embedded Book".into()),
            path: audio.to_string_lossy().to_string(),
            duration: Some(180),
            chapter_index: Some(1),
            is_extra: 0,
            hash: Some("legacy-hash".into()),
            manual_corrected: 0,
            start_time: None,
            end_time: None,
            created_at: chrono::Utc::now().to_rfc3339(),
        })
        .await
        .unwrap();

    let result = scan_local(
        &scanner,
        &library.id,
        &root,
        &config,
        ScanMode::Incremental,
        Some(scanned_at),
    )
    .await;
    assert_eq!(result.failed_count, 0, "{:?}", result.errors);
    let chapters = scanner.chapter_repo.find_by_book(&book.id).await.unwrap();
    assert_eq!(chapters.len(), 3);
    assert!(chapters.iter().all(|chapter| chapter.start_time.is_some()));
    assert!(
        chapters
            .iter()
            .all(|chapter| chapter.id != "legacy-chapter")
    );
}

#[tokio::test]
async fn single_file_m4b_without_chapters_keeps_one_whole_file_chapter() {
    use crate::core::library_scanner::{ScanMode, test_support};

    let directory = tempfile::tempdir().unwrap();
    let scanner = test_support::scanner(directory.path());
    let root = directory.path().join("library");
    let book_dir = root.join("Plain Book");
    std::fs::create_dir_all(&book_dir).unwrap();
    std::fs::write(book_dir.join("Plain Book.m4b"), m4b_fixture(None)).unwrap();

    let library = test_support::library(&scanner, &root).await;
    let config = crate::db::models::ScraperConfig::default();
    scan_local(&scanner, &library.id, &root, &config, ScanMode::Full, None).await;

    let books = scanner
        .book_repo
        .find_by_library(&library.id)
        .await
        .unwrap();
    assert_eq!(books.len(), 1);
    let chapters = scanner
        .chapter_repo
        .find_by_book(&books[0].id)
        .await
        .unwrap();
    assert_eq!(chapters.len(), 1);
    assert_eq!(chapters[0].start_time, Some(0));
    assert_eq!(chapters[0].end_time, Some(180_000));
}
