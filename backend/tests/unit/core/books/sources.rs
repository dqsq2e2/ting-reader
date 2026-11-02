use super::*;
use crate::db::manager::DatabaseManager;
use crate::db::repository::Repository;
use serde_json::json;
use std::sync::Arc;

fn detail() -> SourceBookDetail {
    let metadata = json!({
        "id":"book-1","source_url":null,"title":"Sample book","author":"Author",
        "narrator":null,"cover_url":null,"intro":"Description","subtitle":null,
        "publisher":null,"language":null,"genre":null,"published_year":null,
        "published_date":null,"isbn":null,"asin":null,"explicit":null,"abridged":null,
        "tags":["sample","audio"],"duration":null,"score":null,"chapter_title_template":null,"chapter_titles":[]
    });
    serde_json::from_value(json!({
        "metadata":metadata,
        "chapters":[{"id":"chapter-1","title":"First","index":0,"duration":null,"source_url":null}]
    }))
    .unwrap()
}

#[tokio::test]
async fn import_is_atomic_idempotent_and_grants_only_requested_book() {
    let db = Arc::new(DatabaseManager::new_in_memory().unwrap());
    db.execute(|conn| {
        conn.execute("INSERT INTO users (id, username, password_hash, role) VALUES ('one', 'one', '', 'user'), ('two', 'two', '', 'user')", [])?;
        Ok(())
    }).await.unwrap();
    let books = BookRepository::new(db.clone());
    let first = books
        .import_source_book("source", "Source", "one", None, detail())
        .await
        .unwrap();
    assert!(!first.already_added);
    assert!(
        books
            .check_access(&first.book_id, "one", false)
            .await
            .unwrap()
    );
    assert!(
        !books
            .check_access(&first.book_id, "two", false)
            .await
            .unwrap()
    );
    let second = books
        .import_source_book(
            "source",
            "Renamed source",
            "two",
            Some("rgba(12, 34, 56, 0.1)".into()),
            detail(),
        )
        .await
        .unwrap();
    assert!(second.already_added);
    assert_eq!(first.book_id, second.book_id);
    assert!(
        books
            .check_access(&first.book_id, "two", false)
            .await
            .unwrap()
    );
    let book = books.find_by_id(&first.book_id).await.unwrap().unwrap();
    assert_eq!(book.path, "book-1");
    assert_eq!(book.theme_color.as_deref(), Some("rgba(12, 34, 56, 0.1)"));
    assert_eq!(book.tags.as_deref(), Some("sample,audio"));
    let chapters = crate::db::repository::ChapterRepository::new(db.clone())
        .find_by_book(&book.id)
        .await
        .unwrap();
    assert_eq!(chapters.len(), 1);
    assert_eq!(chapters[0].path, "chapter-1");
    assert_eq!(
        books
            .source_books_for_user("source", "one", false)
            .await
            .unwrap()["book-1"],
        book.id
    );
    let count = db
        .execute(|conn| {
            Ok(conn.query_row(
                "SELECT count(*) FROM libraries WHERE type = 'source'",
                [],
                |r| r.get::<_, u64>(0),
            )?)
        })
        .await
        .unwrap();
    assert_eq!(count, 1);
}

#[tokio::test]
async fn invalid_or_failed_import_leaves_no_partial_records() {
    let db = Arc::new(DatabaseManager::new_in_memory().unwrap());
    let books = BookRepository::new(db.clone());
    assert!(
        books
            .import_source_book("source", "Source", "missing-user", None, detail())
            .await
            .is_err()
    );
    let count = db
        .execute(|conn| {
            Ok(conn.query_row("SELECT count(*) FROM books", [], |r| r.get::<_, u64>(0))?)
        })
        .await
        .unwrap();
    assert_eq!(count, 0);
    let count = db
        .execute(|conn| {
            Ok(conn.query_row(
                "SELECT count(*) FROM libraries WHERE type = 'source'",
                [],
                |r| r.get::<_, u64>(0),
            )?)
        })
        .await
        .unwrap();
    assert_eq!(count, 0);
    let mut invalid = detail();
    invalid.chapters.push(invalid.chapters[0].clone());
    assert!(
        books
            .import_source_book("source", "Source", "one", None, invalid)
            .await
            .is_err()
    );
}

#[tokio::test]
async fn concurrent_imports_create_one_book_and_one_chapter() {
    let dir = tempfile::tempdir().unwrap();
    let db = Arc::new(
        DatabaseManager::new(
            &dir.path().join("source.sqlite"),
            4,
            std::time::Duration::from_secs(5),
        )
        .unwrap(),
    );
    db.execute(|conn| {
        conn.execute("INSERT INTO users (id, username, password_hash, role) VALUES ('one', 'one', '', 'user')", [])?;
        Ok(())
    }).await.unwrap();
    let books = Arc::new(BookRepository::new(db.clone()));
    let jobs = (0..8).map(|_| {
        let books = books.clone();
        tokio::spawn(async move {
            books
                .import_source_book("source", "Source", "one", None, detail())
                .await
                .unwrap()
        })
    });
    let results = futures::future::join_all(jobs)
        .await
        .into_iter()
        .map(|result| result.unwrap())
        .collect::<Vec<_>>();
    assert_eq!(
        results
            .iter()
            .filter(|result| !result.already_added)
            .count(),
        1
    );
    assert!(
        results
            .iter()
            .all(|result| result.book_id == results[0].book_id)
    );
    let counts = db.execute(|conn| {
        Ok(conn.query_row("SELECT (SELECT count(*) FROM books), (SELECT count(*) FROM chapters), (SELECT count(*) FROM user_book_access)", [], |row| {
            Ok((row.get::<_, u64>(0)?, row.get::<_, u64>(1)?, row.get::<_, u64>(2)?))
        })?)
    }).await.unwrap();
    assert_eq!(counts, (1, 1, 1));
}

async fn load_test_source(
    manager: &Arc<crate::plugin::manager::PluginManager>,
    dir: &std::path::Path,
    version: &str,
    admin_only: bool,
    expires_at: Option<&str>,
) -> String {
    std::fs::create_dir_all(dir).unwrap();
    let manifest = json!({
        "id":"sample-source","name":"Sample source","version":version,"min_core_version":"2.0.0",
        "author":"Test","description":{"en":"Test"},"runtime":"javascript","entry_point":"plugin.js",
        "dependencies":[],"permissions":[{"type":"network_access","domain":"127.0.0.1"}],
        "admin_only":admin_only,
        "capabilities":[{
            "id":"metadata","kind":"metadata_provider","book_source":true,
            "operations":["search","detail","resolve_audio"],
            "search_fields":[{"key":"title","label":{"en":"Title"},"required":true,"type":"text"}],
            "result_fields":[{"key":"title","label":{"en":"Title"}}]
        }]
    });
    std::fs::write(
        dir.join("plugin.yml"),
        serde_yaml::to_string(&manifest).unwrap(),
    )
    .unwrap();
    let audio = json!({"type":"proxy","url":"http://127.0.0.1/chapter.mp3","headers":{"Referer":"https://source.example/"},"expires_at":expires_at,"seekable":true,"mime":"audio/mpeg"});
    std::fs::write(dir.join("plugin.js"), format!(
        "export function search(request) {{return {{ok:true,data:{{items:[],page:request.page,page_size:20,total:0,has_more:false}}}};}}
         export function detail() {{return {{ok:true,data:{}}};}}
         export function resolve_audio() {{return {{ok:true,data:{audio}}};}}",
        serde_json::to_string(&detail()).unwrap(),
    )).unwrap();
    manager.load_plugin(dir).await.unwrap()
}

#[tokio::test]
async fn sandbox_source_enforces_opt_in_access_identity_and_expiry_across_upgrades() {
    use crate::plugin::manager::{PluginConfig, PluginManager};
    let dir = tempfile::tempdir().unwrap();
    let manager = Arc::new(
        PluginManager::new(PluginConfig {
            plugin_dir: dir.path().into(),
            enable_hot_reload: false,
            max_memory_per_plugin: 64 * 1024 * 1024,
            max_execution_time: std::time::Duration::from_secs(5),
        })
        .unwrap(),
    );
    let first = load_test_source(&manager, &dir.path().join("v1"), "1.0.0", true, None).await;
    let service = ScraperService::new(manager.clone());
    let db = Arc::new(DatabaseManager::new_in_memory().unwrap());
    db.execute(|conn| {
        conn.execute("INSERT INTO users (id, username, password_hash, role) VALUES ('one', 'one', '', 'user')", [])?;
        Ok(())
    }).await.unwrap();
    let books = BookRepository::new(db.clone());
    let reference = SourceRef {
        id: Some("book-1".into()),
        url: None,
    };
    assert!(
        service
            .add_source_book(&books, "one", false, &first, reference.clone())
            .await
            .is_err()
    );
    assert!(
        service
            .add_source_book(
                &books,
                "one",
                true,
                &first,
                SourceRef {
                    id: Some("wrong-id".into()),
                    url: None
                }
            )
            .await
            .is_err()
    );
    let added = service
        .add_source_book(&books, "one", true, &first, reference.clone())
        .await
        .unwrap();
    let library = crate::db::repository::LibraryRepository::new(db)
        .find_by_id(&source_record_id(&["library", "sample-source"]))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(library.library_type, "source");
    let source = service
        .resolve_source_audio(&library, "chapter-1")
        .await
        .unwrap();
    assert_eq!(source.allowed_domains, ["127.0.0.1"]);
    assert_eq!(
        source.descriptor.headers["Referer"],
        "https://source.example/"
    );
    manager.unload_plugin(&first).await.unwrap();
    assert!(
        service
            .resolve_source_audio(&library, "chapter-1")
            .await
            .is_err()
    );
    let second = load_test_source(
        &manager,
        &dir.path().join("v2"),
        "1.1.0",
        false,
        Some("2000-01-01T00:00:00Z"),
    )
    .await;
    let duplicate = service
        .add_source_book(&books, "one", false, &second, reference)
        .await
        .unwrap();
    assert!(duplicate.already_added);
    assert_eq!(duplicate.book_id, added.book_id);
    assert!(
        service
            .resolve_source_audio(&library, "chapter-1")
            .await
            .is_err()
    );
    manager.unload_plugin(&second).await.unwrap();
}

#[test]
fn reference_hashing_is_unambiguous_and_scoped_by_plugin() {
    assert_ne!(
        source_record_id(&["ab", "c"]),
        source_record_id(&["a", "bc"])
    );
    assert_ne!(
        source_record_id(&["book", "one", "1"]),
        source_record_id(&["book", "two", "1"])
    );
}
