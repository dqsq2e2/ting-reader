use super::*;
use crate::core::app::error::PluginErrorKind;
use crate::plugin::manager::{FailedPlugin, PluginConfig, PluginEntry};
use crate::plugin::types::{Plugin, PluginMetadata};
use std::sync::Arc;

#[test]
fn source_detail_shape_is_opt_in_and_requires_complete_unique_chapters() {
    let mut capability: PluginCapability = serde_json::from_value(serde_json::json!({
        "id":"metadata","kind":"metadata_provider","operations":["search","detail","resolve_audio"],
        "search_fields":[],"result_fields":[]
    }))
    .unwrap();
    let metadata = serde_json::json!({
        "id":"one","source_url":null,"title":"Sample","author":null,"narrator":null,
        "cover_url":null,"intro":null,"subtitle":null,"publisher":null,"language":null,
        "genre":null,"published_year":null,"published_date":null,"isbn":null,"asin":null,
        "explicit":null,"abridged":null,"tags":[],"duration":null,"score":null,
        "chapter_title_template":null,"chapter_titles":[]
    });
    let request = serde_json::json!({"id":"one","url":null});
    assert!(validate_capability_output(&capability, "detail", &request, &metadata).is_ok());
    let mut detail = serde_json::json!({
        "metadata":metadata,"chapters":[{"id":"chapter-1","title":"One","index":0,"duration":null,"source_url":null}]
    });
    assert!(validate_capability_output(&capability, "detail", &request, &detail).is_err());
    if let PluginCapability::MetadataProvider(cap) = &mut capability {
        cap.book_source = true;
    }
    assert!(validate_capability_output(&capability, "detail", &request, &metadata).is_err());
    assert!(validate_capability_output(&capability, "detail", &request, &detail).is_ok());
    detail["chapters"] = serde_json::json!([detail["chapters"][0], detail["chapters"][0]]);
    assert!(validate_capability_output(&capability, "detail", &request, &detail).is_err());
    detail["chapters"] = serde_json::json!([]);
    assert!(validate_capability_output(&capability, "detail", &request, &detail).is_err());
}

#[tokio::test]
async fn resource_scope_rejections_preserve_permission_and_cancellation_codes() {
    use crate::plugin::host_api::resources::ResourceScope;
    use ting_plugin_contract::protocol::PluginErrorCode;
    let dir = tempfile::tempdir().unwrap();
    let manager = PluginManager::new(PluginConfig {
        plugin_dir: dir.path().into(),
        enable_hot_reload: false,
        max_memory_per_plugin: 32 * 1024 * 1024,
        max_execution_time: Duration::from_secs(1),
    })
    .unwrap();
    let metadata = PluginMetadata::new(
        "scoped".into(),
        "Scoped".into(),
        "2.0.0".into(),
        "Test".into(),
        "Test".into(),
        "plugin.js".into(),
    );
    let id = metadata.instance_id();
    let instance = Arc::new(FailedPlugin::new(
        metadata.clone(),
        Arc::new(TingError::plugin_load(
            PluginErrorKind::Loading,
            "test.fixture",
            "unused".into(),
        )),
    ));
    manager
        .registry
        .write()
        .await
        .insert(id.clone(), PluginEntry::new(metadata, instance));
    let stale = Arc::new(ResourceScope::new(
        id.clone(),
        uuid::Uuid::new_v4(),
        None,
        dir.path().into(),
        Default::default(),
    ));
    let cancelled = manager
        .resource_scope(
            &id,
            &Default::default(),
            dir.path().into(),
            Default::default(),
        )
        .await
        .unwrap();
    cancelled.cancel();
    for (scope, expected) in [
        (stale, PluginErrorCode::PermissionDenied),
        (cancelled, PluginErrorCode::Cancelled),
    ] {
        let error = manager
            .invoke_capability(
                &id,
                "test",
                "test",
                serde_json::json!({}),
                &PluginInvocationContext {
                    user: None,
                    resources: Some(scope),
                },
            )
            .await
            .unwrap_err();
        assert_eq!(error.plugin_error_code(), expected);
        assert!(!error.is_retryable());
        assert!(std::error::Error::source(&error).is_some());
    }
}

#[tokio::test]
async fn zero_permission_js_uses_host_limits_and_marks_failed_after_timeout() {
    let dir = tempfile::tempdir().unwrap();
    let manifest = serde_json::json!({
        "id": "bounded-js", "name": "Bounded JS", "version": "2.0.0",
        "min_core_version": "2.0.0", "author": "Test",
        "description": {"en": "Test"}, "runtime": "javascript",
        "entry_point": "plugin.js", "dependencies": [], "permissions": [],
        "capabilities": [
            {"kind": "plugin_store", "id": "test.store", "operations": ["list_plugins"]}
        ],
    });
    std::fs::write(
        dir.path().join("plugin.yml"),
        serde_yaml::to_string(&manifest).unwrap(),
    )
    .unwrap();
    std::fs::write(
        dir.path().join("plugin.js"),
        "export function list_plugins() { while (true) {} }",
    )
    .unwrap();
    let manager = PluginManager::new(PluginConfig {
        plugin_dir: dir.path().into(),
        enable_hot_reload: false,
        max_memory_per_plugin: 32 * 1024 * 1024,
        max_execution_time: Duration::from_millis(100),
    })
    .unwrap();
    let metadata = crate::plugin::types::metadata::read_plugin_metadata(dir.path()).unwrap();
    let id = metadata.instance_id();
    let instance = manager
        .load_plugin_instance(dir.path(), &metadata)
        .await
        .unwrap();
    let mut entry = PluginEntry::new(metadata, instance);
    entry.set_state(PluginState::Active);
    manager.registry.write().await.insert(id.clone(), entry);
    let scope = manager
        .resource_scope(
            &id,
            &Default::default(),
            dir.path().into(),
            Default::default(),
        )
        .await
        .unwrap();
    let started = Instant::now();
    let error = manager
        .invoke_plugin(
            &id,
            "list_plugins",
            serde_json::json!({}),
            &PluginInvocationContext {
                user: None,
                resources: Some(scope.clone()),
            },
        )
        .await
        .unwrap_err();
    assert!(
        matches!(error.root_error(), TingError::Timeout(_)),
        "{error}"
    );
    assert!(started.elapsed() < Duration::from_secs(3));
    assert!(scope.cancellation_token().is_cancelled());
    let registry = manager.registry.read().await;
    let entry = registry.get(&id).unwrap();
    assert_eq!(entry.state, PluginState::Failed);
    let retained = entry.load_error.as_ref().unwrap();
    assert!(retained.to_string().contains("deadline"));
    assert_eq!(retained.error_type(), "Timeout");
    let info = crate::plugin::manager::PluginInfo::from_entry(entry);
    let snapshot = info.error.unwrap();
    assert_eq!(snapshot.error, "Timeout");
    assert_eq!(snapshot.details.unwrap()["reason"], "elapsed");
    assert!(!entry.instance.is_available());
}

#[test]
fn metadata_search_requires_the_formal_request_and_result() {
    let capability: PluginCapability = serde_json::from_value(serde_json::json!({
        "kind": "metadata_provider", "id": "metadata.search", "operations": ["search"],
        "auto_scrape": false, "search_fields": [{
            "key": "title", "label": {"en": "Title"}, "required": false, "type": "text"
        }], "result_fields": [{"key": "title", "label": {"en": "Title"}}]
    }))
    .unwrap();
    let request = serde_json::json!({
        "title": "Book", "author": null, "narrator": null,
        "page": 1, "page_size": 20, "filters": {},
        "chapter_candidates": [], "context": null
    });
    assert!(validate_capability_input(&capability, "search", &request).is_ok());
    assert!(
        validate_capability_input(
            &capability,
            "search",
            &serde_json::json!({
                "query": "Book", "page": 1, "page_size": 20, "filters": {}
            })
        )
        .is_err()
    );
    let mut output = serde_json::to_value(ting_plugin_contract::scraper::SearchPage {
        items: vec![ting_plugin_contract::scraper::ScraperResult {
            id: None,
            source_url: None,
            title: "Book".into(),
            author: None,
            narrator: None,
            cover_url: None,
            intro: None,
            subtitle: None,
            publisher: None,
            language: None,
            genre: None,
            published_year: None,
            published_date: None,
            isbn: None,
            asin: None,
            explicit: None,
            abridged: None,
            tags: Vec::new(),
            duration: None,
            score: None,
            chapter_title_template: None,
            chapter_titles: Vec::new(),
        }],
        page: 1,
        page_size: 20,
        total: None,
        has_more: None,
    })
    .unwrap();
    assert!(validate_capability_output(&capability, "search", &request, &output).is_ok());
    output["items"][0]["artist"] = serde_json::json!("old alias");
    assert!(validate_capability_output(&capability, "search", &request, &output).is_err());
}

#[test]
fn format_call_validates_declared_operation_request_and_response() {
    let capability: PluginCapability = serde_json::from_value(serde_json::json!({
        "kind": "format_handler",
        "id": "special.audio",
        "extensions": ["special"],
        "operations": ["probe", "get_metadata_read_size", "extract_metadata"]
    }))
    .unwrap();
    let input = serde_json::json!({
        "input": "host:opaque",
        "extension_hint": "special",
        "mime_hint": null,
        "prefix_bytes": 4096
    });
    assert!(validate_capability_input(&capability, "probe", &input).is_ok());
    let more = serde_json::json!({"kind": "need_more", "total_bytes": 8192});
    assert!(validate_capability_output(&capability, "probe", &input, &more).is_ok());
    let repeated = serde_json::json!({"kind": "need_more", "total_bytes": 4096});
    assert!(validate_capability_output(&capability, "probe", &input, &repeated).is_err());
    assert!(validate_capability_input(
        &capability,
        "probe",
        &serde_json::json!({"input": "host:opaque", "prefix_bytes": 4096, "file_path": "C:/secret"})
    )
    .is_err());
    assert!(validate_capability_input(&capability, "open_decrypt", &input).is_err());
}

#[tokio::test]
async fn record_plugin_call_updates_listed_stats() {
    let temp_dir = tempfile::tempdir().unwrap();
    let manager = PluginManager::new(PluginConfig {
        plugin_dir: temp_dir.path().join("plugins"),
        enable_hot_reload: false,
        max_memory_per_plugin: 128 * 1024 * 1024,
        max_execution_time: Duration::from_secs(30),
    })
    .unwrap();

    let metadata = PluginMetadata::new(
        "stats-plugin".to_string(),
        "Stats Plugin".to_string(),
        "1.0.0".to_string(),
        "Ting Reader".to_string(),
        "Stats plugin".to_string(),
        "plugin.js".to_string(),
    );
    let plugin_id = metadata.instance_id();
    let instance = Arc::new(FailedPlugin::new(
        metadata.clone(),
        Arc::new(TingError::plugin_load(
            PluginErrorKind::Loading,
            "test.fixture",
            "unused".into(),
        )),
    )) as Arc<dyn Plugin>;

    manager
        .registry
        .write()
        .await
        .insert(plugin_id.clone(), PluginEntry::new(metadata, instance));

    manager
        .record_plugin_call(
            &plugin_id,
            "search",
            Duration::from_millis(25),
            None,
            PluginLogLevel::Info,
        )
        .await;
    manager
        .record_plugin_call(
            &plugin_id,
            "search",
            Duration::from_millis(5),
            Some(&TingError::plugin_execution(
                PluginErrorKind::Runtime,
                "record_plugin_call_updates_listed_stats::plugin_execution",
                "boom".to_string(),
            )),
            PluginLogLevel::Info,
        )
        .await;

    let plugins = manager.list_plugins().await;
    let stats_plugin = plugins
        .iter()
        .find(|plugin| plugin.id == plugin_id)
        .expect("stats plugin should be listed");

    assert_eq!(stats_plugin.total_calls, 2);
    assert_eq!(stats_plugin.successful_calls, 1);
    assert_eq!(stats_plugin.failed_calls, 1);

    for index in 0..20 {
        manager
            .record_plugin_call(
                &plugin_id,
                "search",
                Duration::from_millis(1),
                Some(
                    &TingError::plugin_execution(
                        PluginErrorKind::Runtime,
                        "search",
                        format!("Different failure #{index}"),
                    )
                    .with_context(format!("Request #{index}")),
                ),
                PluginLogLevel::Info,
            )
            .await;
    }
    let registry = manager.registry.read().await;
    let stats = &registry.get(&plugin_id).unwrap().stats;
    assert_eq!(
        stats.error_distribution_sorted(),
        vec![("PluginExecutionError".into(), 21)]
    );
}

#[tokio::test]
async fn invoke_plugin_records_failure_for_unsupported_runtime() {
    let temp_dir = tempfile::tempdir().unwrap();
    let manager = PluginManager::new(PluginConfig {
        plugin_dir: temp_dir.path().join("plugins"),
        enable_hot_reload: false,
        max_memory_per_plugin: 128 * 1024 * 1024,
        max_execution_time: Duration::from_secs(30),
    })
    .unwrap();

    let metadata = PluginMetadata::new(
        "unsupported-plugin".to_string(),
        "Unsupported Plugin".to_string(),
        "1.0.0".to_string(),
        "Ting Reader".to_string(),
        "Unsupported plugin".to_string(),
        "plugin.js".to_string(),
    );
    let plugin_id = metadata.instance_id();
    let instance = Arc::new(FailedPlugin::new(
        metadata.clone(),
        Arc::new(TingError::plugin_load(
            PluginErrorKind::Loading,
            "test.fixture",
            "unused".into(),
        )),
    )) as Arc<dyn Plugin>;

    manager
        .registry
        .write()
        .await
        .insert(plugin_id.clone(), PluginEntry::new(metadata, instance));

    let result = manager
        .invoke_plugin(
            &plugin_id,
            "anything",
            serde_json::json!({}),
            &PluginInvocationContext::default(),
        )
        .await;

    assert!(result.is_err());

    let plugins = manager.list_plugins().await;
    let plugin = plugins
        .iter()
        .find(|plugin| plugin.id == plugin_id)
        .expect("plugin should be listed");

    assert_eq!(plugin.total_calls, 1);
    assert_eq!(plugin.failed_calls, 1);
}
