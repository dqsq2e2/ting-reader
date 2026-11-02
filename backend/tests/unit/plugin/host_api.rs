use super::*;
use serde_json::json;

#[test]
fn plugin_host_permission_requires_matching_manifest_permission() {
    assert_eq!(
        PluginHostGateway::required_permission("runtime.sleep"),
        Some(PluginHostPermission::Runtime)
    );
    assert!(PluginHostGateway::authorize("demo", &[], "runtime.sleep").is_ok());
    assert_eq!(
        PluginHostGateway::required_permission("books.list"),
        Some(PluginHostPermission::BooksRead)
    );
    assert_eq!(
        PluginHostGateway::required_permission("chapters.get"),
        Some(PluginHostPermission::ChaptersRead)
    );
    assert_eq!(
        PluginHostGateway::required_permission("libraries.list"),
        Some(PluginHostPermission::LibrariesRead)
    );
    assert_eq!(
        PluginHostGateway::required_permission("progress.recent"),
        Some(PluginHostPermission::ProgressRead)
    );
    assert_eq!(
        PluginHostGateway::required_permission("media.get_url"),
        Some(PluginHostPermission::MediaReadUrl)
    );
    assert_eq!(
        PluginHostGateway::required_permission("media.get_signed_url"),
        Some(PluginHostPermission::MediaReadUrl)
    );
    assert_eq!(
        PluginHostGateway::required_permission("plugin_routes.sign"),
        Some(PluginHostPermission::PluginRouteSign)
    );
    assert_eq!(
        PluginHostGateway::required_permission("metadata.write"),
        Some(PluginHostPermission::MetadataWrite)
    );
    assert_eq!(
        PluginHostGateway::required_permission("tasks.create"),
        Some(PluginHostPermission::TaskCreate)
    );
    assert_eq!(
        PluginHostGateway::required_permission("cache.get"),
        Some(PluginHostPermission::CacheRead)
    );
    assert_eq!(
        PluginHostGateway::required_permission("cache.set"),
        Some(PluginHostPermission::CacheWrite)
    );
    assert_eq!(
        PluginHostGateway::required_permission("unknown.method"),
        None
    );

    assert!(PluginHostGateway::has_permission(
        &[Permission::BooksRead],
        PluginHostPermission::BooksRead
    ));
    assert!(PluginHostGateway::has_permission(
        &[Permission::LibrariesRead],
        PluginHostPermission::LibrariesRead
    ));
    assert!(!PluginHostGateway::has_permission(
        &[Permission::BooksRead],
        PluginHostPermission::LibrariesRead
    ));
    assert!(!PluginHostGateway::has_permission(
        &[Permission::BooksRead],
        PluginHostPermission::ChaptersRead
    ));
    assert!(PluginHostGateway::has_permission(
        &[Permission::ChaptersRead],
        PluginHostPermission::ChaptersRead
    ));
    assert!(PluginHostGateway::has_permission(
        &[Permission::ProgressRead],
        PluginHostPermission::ProgressRead
    ));
    assert!(!PluginHostGateway::has_permission(
        &[Permission::BooksRead],
        PluginHostPermission::ProgressRead
    ));
    assert!(PluginHostGateway::has_permission(
        &[Permission::MediaReadUrl],
        PluginHostPermission::MediaReadUrl
    ));
    assert!(PluginHostGateway::has_permission(
        &[Permission::PluginRouteSign],
        PluginHostPermission::PluginRouteSign
    ));
    assert!(!PluginHostGateway::has_permission(
        &[Permission::BooksRead],
        PluginHostPermission::MediaReadUrl
    ));
    assert!(PluginHostGateway::has_permission(
        &[Permission::MetadataWrite],
        PluginHostPermission::MetadataWrite
    ));
    assert!(!PluginHostGateway::has_permission(
        &[Permission::BooksWrite],
        PluginHostPermission::MetadataWrite
    ));
    assert!(PluginHostGateway::has_permission(
        &[Permission::TaskCreate],
        PluginHostPermission::TaskCreate
    ));
    assert!(!PluginHostGateway::has_permission(
        &[Permission::BooksWrite],
        PluginHostPermission::TaskCreate
    ));
    assert!(PluginHostGateway::has_permission(
        &[Permission::CacheRead],
        PluginHostPermission::CacheRead
    ));
    assert!(PluginHostGateway::has_permission(
        &[Permission::CacheWrite],
        PluginHostPermission::CacheRead
    ));
    assert!(PluginHostGateway::has_permission(
        &[Permission::CacheWrite],
        PluginHostPermission::CacheWrite
    ));
    assert!(!PluginHostGateway::has_permission(
        &[Permission::CacheRead],
        PluginHostPermission::CacheWrite
    ));
}

#[test]
fn plugin_host_authorize_rejects_unknown_or_missing_permissions() {
    assert!(PluginHostGateway::authorize("demo", &[Permission::BooksRead], "books.list").is_ok());
    assert!(
        PluginHostGateway::authorize("demo", &[Permission::ChaptersRead], "chapters.get").is_ok()
    );
    assert!(
        PluginHostGateway::authorize("demo", &[Permission::BooksWrite], "books.update").is_ok()
    );
    assert!(
        PluginHostGateway::authorize("demo", &[Permission::BooksRead], "books.update").is_err()
    );
    assert!(PluginHostGateway::required_permission("database.update").is_none());

    let unknown = PluginHostGateway::authorize("demo", &[Permission::BooksRead], "unknown.method")
        .unwrap_err();
    assert!(matches!(unknown, TingError::InvalidRequest(_)));

    let missing =
        PluginHostGateway::authorize("demo", &[Permission::BooksRead], "cache.set").unwrap_err();
    assert!(matches!(missing, TingError::PermissionDenied(_)));
}

#[test]
fn plugin_host_param_helpers_parse_optional_values() {
    let params = json!({
        "book_id": " book-1 ",
        "limit": 25,
        "download": true,
        "empty": " "
    });

    assert_eq!(string_param(&params, "book_id"), Some("book-1".to_string()));
    assert_eq!(string_param(&params, "empty"), None);
    assert_eq!(usize_param(&params, "limit"), Some(25));
    assert_eq!(bool_param(&params, "download"), Some(true));
    assert!(required_string_param(&params, "missing").is_err());
}

#[test]
fn plugin_task_priority_defaults_to_normal() {
    assert_eq!(plugin_task_priority(&json!({})), Priority::Normal);
    assert_eq!(
        plugin_task_priority(&json!({ "priority": "high" })),
        Priority::High
    );
    assert_eq!(
        plugin_task_priority(&json!({ "priority": "low" })),
        Priority::Low
    );
}

#[test]
fn bookmark_methods_require_their_own_read_or_write_permission() {
    for method in ["bookmarks.books", "bookmarks.list", "bookmarks.get"] {
        assert_eq!(
            PluginHostGateway::required_permission(method),
            Some(PluginHostPermission::BookmarksRead)
        );
        assert!(PluginHostGateway::authorize("test", &[Permission::BookmarksRead], method).is_ok());
        assert!(
            PluginHostGateway::authorize("test", &[Permission::BookmarksWrite], method).is_ok()
        );
        assert!(PluginHostGateway::authorize("test", &[], method).is_err());
        for permission in [
            Permission::BooksRead,
            Permission::ProgressRead,
            Permission::UserSettingsWrite,
        ] {
            assert!(PluginHostGateway::authorize("test", &[permission], method).is_err());
        }
    }
    for method in ["bookmarks.create", "bookmarks.update", "bookmarks.delete"] {
        assert_eq!(
            PluginHostGateway::required_permission(method),
            Some(PluginHostPermission::BookmarksWrite)
        );
        assert!(
            PluginHostGateway::authorize("test", &[Permission::BookmarksWrite], method).is_ok()
        );
        assert!(
            PluginHostGateway::authorize("test", &[Permission::BookmarksRead], method).is_err()
        );
        assert!(PluginHostGateway::authorize("test", &[], method).is_err());
    }
}

struct BookmarkFixture {
    gateway: PluginHostGateway,
    db: Arc<crate::db::manager::DatabaseManager>,
    _temp: tempfile::TempDir,
}

impl BookmarkFixture {
    async fn new() -> Self {
        use crate::db::repository::*;
        let db = Arc::new(crate::db::manager::DatabaseManager::new_in_memory().unwrap());
        db.execute(|conn| {
            conn.execute_batch(
                "INSERT INTO users(id,username,password_hash,role) VALUES
                    ('u1','one','hash','user'),('u2','two','hash','user'),('admin','admin','hash','admin');
                 INSERT INTO libraries(id,name,type,url) VALUES
                    ('library','Shared','local',''),('private-library','Private','local','');
                 INSERT INTO books(id,library_id,title,path,hash) VALUES
                    ('book','library','Book','/book','hash'),
                    ('private-book','private-library','Private book','/private','private-hash');
                 INSERT INTO chapters(id,book_id,title,path,duration) VALUES
                    ('chapter','book','Chapter','/book/1.mp3',660),
                    ('private-chapter','private-book','Private chapter','/private/1.mp3',500);
                 INSERT INTO user_library_access(user_id,library_id) VALUES
                    ('u1','library'),('u2','library');",
            )?;
            Ok(())
        })
        .await
        .unwrap();
        let temp = tempfile::tempdir().unwrap();
        let mut config = Config::from_env().unwrap();
        config.plugins.plugin_dir = temp.path().join("plugins");
        config.plugins.preinstalled_dir = temp.path().join("preinstalled");
        config.plugins.enable_hot_reload = false;
        let gateway = PluginHostGateway::new(PluginHostGatewayDependencies {
            book_repo: Arc::new(BookRepository::new(db.clone())),
            library_repo: Arc::new(LibraryRepository::new(db.clone())),
            chapter_repo: Arc::new(ChapterRepository::new(db.clone())),
            progress_repo: Arc::new(ProgressRepository::new(db.clone())),
            playlist_repo: Arc::new(PlaylistRepository::new(db.clone())),
            favorite_repo: Arc::new(FavoriteRepository::new(db.clone())),
            settings_repo: Arc::new(UserSettingsRepository::new(db.clone())),
            task_queue: Arc::new(TaskQueue::new(
                config.task_queue.clone(),
                db.clone(),
                temp.path().join("temp"),
            )),
            plugin_manager: Arc::new(
                PluginManager::new(crate::plugin::manager::PluginConfig {
                    plugin_dir: config.plugins.plugin_dir.clone(),
                    enable_hot_reload: false,
                    max_memory_per_plugin: config.plugins.max_memory_per_plugin,
                    max_execution_time: std::time::Duration::from_secs(
                        config.plugins.max_execution_time,
                    ),
                })
                .unwrap(),
            ),
            plugin_cache: Arc::new(PluginCache::new(temp.path().join("cache")).unwrap()),
            plugin_storage: Arc::new(PluginCache::new(temp.path().join("storage")).unwrap()),
            config_manager: Arc::new(
                crate::plugin::PluginConfigManager::new(temp.path().join("config"), [0; 32])
                    .unwrap(),
            ),
            plugin_route_revocations: Arc::new(
                crate::core::security::signing::PluginRouteRevocations::new(
                    temp.path().join("revocations.json"),
                )
                .unwrap(),
            ),
            encryption_key: Arc::new([0; 32]),
            config,
        });
        Self {
            gateway,
            db,
            _temp: temp,
        }
    }

    async fn call(&self, user_id: &str, method: &str, params: Value) -> Result<Value> {
        let user = PluginHostUser {
            id: user_id.into(),
            username: user_id.into(),
            role: if user_id == "admin" { "admin" } else { "user" }.into(),
        };
        self.gateway
            .invoke_with_permissions(
                "bookmark-test",
                &[Permission::BookmarksWrite],
                Some(&user),
                method,
                params,
                None,
            )
            .await
    }
}

#[tokio::test]
async fn background_html_parsing_needs_permission_but_not_a_user_principal() {
    let fixture = BookmarkFixture::new().await;
    let gateway = &fixture.gateway;
    assert!(
        gateway
            .invoke_with_permissions(
                "source",
                &[],
                None,
                "html.parse",
                json!({"html":"<h1>Sample</h1>"}),
                None
            )
            .await
            .is_err()
    );
    let parsed = gateway
        .invoke_with_permissions(
            "source",
            &[Permission::HtmlParse],
            None,
            "html.parse",
            json!({"html":"<h1>Sample</h1>"}),
            None,
        )
        .await
        .unwrap();
    let id = parsed["document_id"].as_str().unwrap();
    let selection = gateway
        .invoke_with_permissions(
            "source",
            &[Permission::HtmlParse],
            None,
            "html.select",
            json!({"document_id":id,"selector":"h1"}),
            None,
        )
        .await
        .unwrap();
    assert_eq!(selection["matches"][0]["text"], "Sample");
    assert!(
        gateway
            .invoke_with_permissions(
                "another",
                &[Permission::HtmlParse],
                None,
                "html.select",
                json!({"document_id":id,"selector":"h1"}),
                None
            )
            .await
            .is_err()
    );
    gateway
        .invoke_with_permissions(
            "source",
            &[Permission::HtmlParse],
            None,
            "html.close",
            json!({"document_id":id}),
            None,
        )
        .await
        .unwrap();
    assert!(gateway.html_documents.read().unwrap().is_empty());
    assert!(
        gateway
            .invoke_with_permissions(
                "source",
                &[Permission::BooksRead],
                None,
                "books.list",
                json!({}),
                None
            )
            .await
            .is_err()
    );
}

#[tokio::test]
#[ignore = "requires TING_TEST_BOOK_SOURCE and a reachable public source website"]
async fn live_book_source_uses_real_sandbox_import_and_ranged_media_proxy() {
    use crate::core::books::ScraperService;
    use crate::db::repository::Repository;
    use ting_plugin_contract::scraper::SourceRef;
    use tokio::io::AsyncReadExt;
    let plugin_dir = std::env::var_os("TING_TEST_BOOK_SOURCE")
        .map(std::path::PathBuf::from)
        .expect("Set TING_TEST_BOOK_SOURCE to the source plugin directory");
    let query = std::env::var("TING_TEST_BOOK_SOURCE_QUERY")
        .expect("Set TING_TEST_BOOK_SOURCE_QUERY to a public test query");
    let fixture = BookmarkFixture::new().await;
    let gateway = Arc::new(fixture.gateway);
    let manager = gateway.plugin_manager.clone();
    manager.set_host_gateway(&gateway);
    let id = manager.load_plugin(&plugin_dir).await.unwrap();
    let service = ScraperService::new(manager.clone());
    let sources = service.get_sources().await;
    assert!(
        sources
            .iter()
            .any(|source| source.id == id && source.enabled && source.book_source)
    );
    let page = service
        .search(&query, None, None, Some(&id), 1, 20)
        .await
        .unwrap();
    let item = page
        .items
        .first()
        .expect("Source must return a public test book");
    let added = service
        .add_source_book(
            &gateway.book_repo,
            "u1",
            false,
            &id,
            SourceRef {
                id: item.id.clone(),
                url: item.source_url.clone(),
            },
        )
        .await
        .unwrap();
    let book = gateway
        .book_repo
        .find_by_id(&added.book_id)
        .await
        .unwrap()
        .unwrap();
    let library = gateway
        .library_repo
        .find_by_id(&book.library_id)
        .await
        .unwrap()
        .unwrap();
    let chapters = gateway.chapter_repo.find_by_book(&book.id).await.unwrap();
    assert!(!chapters.is_empty());
    let audio = service
        .resolve_source_audio(&library, &chapters[0].path)
        .await
        .unwrap();
    let (reader, _) = audio
        .reader(&crate::core::StorageService::new(), Some((0, 4096)))
        .await
        .unwrap();
    let mut bytes = Vec::new();
    reader.take(4096).read_to_end(&mut bytes).await.unwrap();
    assert!(!bytes.is_empty());
    assert!(bytes.len() <= 4096);
    assert!(
        gateway.html_documents.read().unwrap().is_empty(),
        "All source HTML handles must be closed"
    );
    println!(
        "Live sandbox source: {} search results, {} imported chapters, {} media bytes",
        page.items.len(),
        chapters.len(),
        bytes.len()
    );
    manager.unload_plugin(&id).await.unwrap();
}

fn bookmark_create_params() -> Value {
    json!({"book_id": "book", "chapter_id": "chapter", "position": 12.5, "note": "Original"})
}

#[tokio::test]
async fn bookmark_host_round_trips_user_bookmarks_and_metadata() {
    let fixture = BookmarkFixture::new().await;
    let bookmark = fixture
        .call("u1", "bookmarks.create", bookmark_create_params())
        .await
        .unwrap();
    let id = bookmark["id"].as_str().unwrap();
    assert_eq!(bookmark["position"], 12.5);
    assert_eq!(bookmark["chapter_duration"], 660.0);
    assert_eq!(bookmark["chapter_title"], "Chapter");
    let fetched = fixture
        .call("u1", "bookmarks.get", json!({"bookmark_id": id}))
        .await
        .unwrap();
    assert_eq!(fetched, bookmark);
    let books = fixture
        .call("u1", "bookmarks.books", json!({}))
        .await
        .unwrap();
    assert_eq!(books["total"], 1);
    assert_eq!(books["items"][0]["chapter_count"], 1);
    let list = fixture
        .call("u1", "bookmarks.list", json!({"book_id": "book"}))
        .await
        .unwrap();
    assert_eq!(list["total"], 1);
    assert_eq!(list["items"][0], bookmark);
    let updated = fixture
        .call(
            "u1",
            "bookmarks.update",
            json!({"id": id, "note": "Updated"}),
        )
        .await
        .unwrap();
    assert_eq!(updated["note"], "Updated");
    assert_eq!(updated["position"], 12.5);
    let removed = fixture
        .call("u1", "bookmarks.delete", json!({"id": id}))
        .await
        .unwrap();
    assert_eq!(removed, json!({"ok": true, "id": id}));
    assert!(matches!(
        fixture.call("u1", "bookmarks.get", json!({"id": id})).await,
        Err(TingError::NotFound(_))
    ));
}

#[tokio::test]
async fn bookmark_host_requires_an_authenticated_principal_for_every_method() {
    let fixture = BookmarkFixture::new().await;
    for method in [
        "bookmarks.books",
        "bookmarks.list",
        "bookmarks.get",
        "bookmarks.create",
        "bookmarks.update",
        "bookmarks.delete",
    ] {
        let result = fixture
            .gateway
            .invoke_with_permissions(
                "bookmark-test",
                &[Permission::BookmarksWrite],
                None,
                method,
                json!({"_context": {"user": {"id": "u1", "role": "admin"}}}),
                None,
            )
            .await;
        assert!(
            matches!(result, Err(TingError::PermissionDenied(_))),
            "{method}"
        );
    }
}

#[tokio::test]
async fn bookmark_host_read_permission_cannot_mutate() {
    let fixture = BookmarkFixture::new().await;
    let user = PluginHostUser {
        id: "u1".into(),
        username: "one".into(),
        role: "user".into(),
    };
    let result = fixture
        .gateway
        .invoke_with_permissions(
            "bookmark-test",
            &[Permission::BookmarksRead],
            Some(&user),
            "bookmarks.create",
            bookmark_create_params(),
            None,
        )
        .await;
    assert!(matches!(result, Err(TingError::PermissionDenied(_))));
    let books = fixture
        .call("u1", "bookmarks.books", json!({}))
        .await
        .unwrap();
    assert_eq!(books["total"], 0);
}

#[tokio::test]
async fn bookmark_host_never_exposes_or_mutates_other_users_even_to_admins() {
    let fixture = BookmarkFixture::new().await;
    let original = fixture
        .call("u1", "bookmarks.create", bookmark_create_params())
        .await
        .unwrap();
    let id = original["id"].as_str().unwrap();
    for caller in ["u2", "admin"] {
        for (method, params) in [
            ("bookmarks.get", json!({"id": id})),
            ("bookmarks.update", json!({"id": id, "note": "Stolen"})),
            ("bookmarks.delete", json!({"id": id})),
        ] {
            assert!(matches!(
                fixture.call(caller, method, params).await,
                Err(TingError::NotFound(_))
            ));
        }
        let books = fixture
            .call(caller, "bookmarks.books", json!({}))
            .await
            .unwrap();
        assert_eq!(books["total"], 0);
        let list = fixture
            .call(caller, "bookmarks.list", json!({"book_id": "book"}))
            .await
            .unwrap();
        assert_eq!(list["total"], 0);
    }
    assert_eq!(
        fixture
            .call("u1", "bookmarks.get", json!({"id": id}))
            .await
            .unwrap(),
        original
    );
}

#[tokio::test]
async fn bookmark_host_honors_revoked_book_access_without_mutating_data() {
    let fixture = BookmarkFixture::new().await;
    let bookmark = fixture
        .call("u1", "bookmarks.create", bookmark_create_params())
        .await
        .unwrap();
    let id = bookmark["id"].as_str().unwrap();
    fixture
        .db
        .execute(|conn| {
            conn.execute("DELETE FROM user_library_access WHERE user_id='u1'", [])?;
            Ok(())
        })
        .await
        .unwrap();
    for (method, params) in [
        ("bookmarks.list", json!({"book_id": "book"})),
        ("bookmarks.get", json!({"id": id})),
        ("bookmarks.update", json!({"id": id, "note": "Denied"})),
        ("bookmarks.delete", json!({"id": id})),
        ("bookmarks.create", bookmark_create_params()),
    ] {
        assert!(
            matches!(
                fixture.call("u1", method, params).await,
                Err(TingError::PermissionDenied(_))
            ),
            "{method}"
        );
    }
    let books = fixture
        .call("u1", "bookmarks.books", json!({}))
        .await
        .unwrap();
    assert_eq!(books["total"], 0);
    let stored = crate::db::repository::reading::ReadingRepository::new(fixture.db.clone())
        .bookmark("u1", id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(stored.note, "Original");
}

#[tokio::test]
async fn bookmark_host_rejects_forged_user_fields_and_unknown_patch_fields() {
    let fixture = BookmarkFixture::new().await;
    let bookmark = fixture
        .call("u1", "bookmarks.create", bookmark_create_params())
        .await
        .unwrap();
    let id = bookmark["id"].as_str().unwrap();
    for (method, params) in [
        ("bookmarks.books", json!({})),
        ("bookmarks.list", json!({"book_id": "book"})),
        ("bookmarks.get", json!({"id": id})),
        ("bookmarks.create", bookmark_create_params()),
        ("bookmarks.update", json!({"id": id, "note": "Updated"})),
        ("bookmarks.delete", json!({"id": id})),
    ] {
        for field in ["user_id", "role", "_context"] {
            let mut forged = params.clone();
            forged[field] = json!("admin");
            assert!(
                matches!(
                    fixture.call("u1", method, forged).await,
                    Err(TingError::InvalidRequest(_))
                ),
                "{method}: {field}"
            );
        }
    }
    for field in ["book_id", "chapter_id", "position"] {
        let mut params = json!({"id": id, "note": "Updated"});
        params[field] = json!("Changed");
        assert!(matches!(
            fixture.call("u1", "bookmarks.update", params).await,
            Err(TingError::InvalidRequest(_))
        ));
    }
    let params = json!({"id": id, "bookmark_id": id});
    assert!(matches!(
        fixture.call("u1", "bookmarks.get", params).await,
        Err(TingError::InvalidRequest(_))
    ));
}

#[tokio::test]
async fn bookmark_host_validates_positions_notes_and_chapter_ownership() {
    use crate::core::books::{BookmarkService, CreateBookmark};
    let fixture = BookmarkFixture::new().await;
    for (field, value) in [
        ("position", json!(-1)),
        ("position", json!("12")),
        ("position", Value::Null),
        ("note", json!("x".repeat(2001))),
        ("chapter_id", json!("private-chapter")),
        ("chapter_id", json!("missing")),
        ("book_id", json!("missing")),
        ("book_id", json!("private-book")),
    ] {
        let mut params = bookmark_create_params();
        params[field] = value;
        assert!(
            fixture
                .call("u1", "bookmarks.create", params)
                .await
                .is_err()
        );
    }
    let service = BookmarkService::new(
        &fixture.gateway.book_repo,
        &fixture.gateway.chapter_repo,
        "u1",
        false,
    );
    for position in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert!(matches!(
            service
                .create(CreateBookmark {
                    book_id: "book".into(),
                    chapter_id: "chapter".into(),
                    position,
                    note: String::new(),
                })
                .await,
            Err(TingError::InvalidRequest(_))
        ));
    }
    let note = "\u{4e2d}".repeat(2000);
    let bookmark = fixture
        .call(
            "u1",
            "bookmarks.create",
            json!({
                "book_id": "book", "chapter_id": "chapter", "position": 0, "note": note,
            }),
        )
        .await
        .unwrap();
    let id = bookmark["id"].as_str().unwrap();
    assert!(matches!(
        fixture
            .call(
                "u1",
                "bookmarks.update",
                json!({
                    "id": id, "note": "\u{4e2d}".repeat(2001),
                })
            )
            .await,
        Err(TingError::InvalidRequest(_))
    ));
    assert_eq!(
        fixture
            .call("u1", "bookmarks.get", json!({"id": id}))
            .await
            .unwrap()["note"],
        note
    );
}

#[tokio::test]
async fn bookmark_host_pages_are_bounded_stable_and_user_scoped() {
    let fixture = BookmarkFixture::new().await;
    fixture.db.execute(|conn| {
        conn.execute_batch(
            "WITH RECURSIVE sequence(n) AS (SELECT 1 UNION ALL SELECT n+1 FROM sequence WHERE n<140)
             INSERT INTO bookmarks(id,user_id,book_id,chapter_id,position,note,created_at,updated_at)
             SELECT printf('b%03d',n),'u1','book','chapter',n,'','2026-10-06','2026-10-06' FROM sequence;",
        )?;
        Ok(())
    }).await.unwrap();
    let first = fixture
        .call(
            "u1",
            "bookmarks.list",
            json!({
                "book_id": "book", "page": 0, "page_size": 500,
            }),
        )
        .await
        .unwrap();
    assert_eq!(first["page"], 1);
    assert_eq!(first["page_size"], 100);
    assert_eq!(first["items"].as_array().unwrap().len(), 100);
    assert_eq!(first["total"], 140);
    let second = fixture
        .call(
            "u1",
            "bookmarks.list",
            json!({
                "book_id": "book", "page": 2, "page_size": 100,
            }),
        )
        .await
        .unwrap();
    assert_eq!(second["items"].as_array().unwrap().len(), 40);
    assert!(first["items"].as_array().unwrap().iter().all(|item| {
        second["items"]
            .as_array()
            .unwrap()
            .iter()
            .all(|other| other["id"] != item["id"])
    }));
    let bounded = fixture
        .call(
            "u1",
            "bookmarks.list",
            json!({
                "book_id": "book", "page": usize::MAX, "page_size": 0,
            }),
        )
        .await
        .unwrap();
    assert_eq!(bounded["page"], 1_000_000);
    assert_eq!(bounded["page_size"], 1);
    assert!(bounded["items"].as_array().unwrap().is_empty());
    for params in [
        json!({"book_id": "book", "page": -1}),
        json!({"book_id": "book", "page_size": "40"}),
    ] {
        assert!(matches!(
            fixture.call("u1", "bookmarks.list", params).await,
            Err(TingError::InvalidRequest(_))
        ));
    }
    let other = fixture
        .call("u2", "bookmarks.list", json!({"book_id": "book"}))
        .await
        .unwrap();
    assert_eq!(other["total"], 0);
}
