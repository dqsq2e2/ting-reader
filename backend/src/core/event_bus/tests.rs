use super::*;
use serde_json::json;
use std::sync::atomic::{AtomicUsize, Ordering};

#[tokio::test]
async fn test_subscribe_and_publish() {
    let bus = EventBus::new();
    let counter = Arc::new(AtomicUsize::new(0));
    let counter_clone = counter.clone();

    let handler: EventHandler = Arc::new(move |_event| {
        let counter = counter_clone.clone();
        Box::pin(async move {
            counter.fetch_add(1, Ordering::SeqCst);
            Ok(())
        })
    });

    bus.subscribe(EventType::SystemStarted, handler).await;

    let event = Event::system(EventType::SystemStarted, json!({}));
    bus.publish(event).await.unwrap();

    // Give handlers time to execute
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    assert_eq!(counter.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn test_multiple_subscribers() {
    let bus = EventBus::new();
    let counter = Arc::new(AtomicUsize::new(0));

    for _ in 0..3 {
        let counter_clone = counter.clone();
        let handler: EventHandler = Arc::new(move |_event| {
            let counter = counter_clone.clone();
            Box::pin(async move {
                counter.fetch_add(1, Ordering::SeqCst);
                Ok(())
            })
        });
        bus.subscribe(EventType::SystemStarted, handler).await;
    }

    let event = Event::system(EventType::SystemStarted, json!({}));
    bus.publish(event).await.unwrap();

    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    assert_eq!(counter.load(Ordering::SeqCst), 3);
}

#[tokio::test]
async fn test_unsubscribe() {
    let bus = EventBus::new();
    let counter = Arc::new(AtomicUsize::new(0));
    let counter_clone = counter.clone();

    let handler: EventHandler = Arc::new(move |_event| {
        let counter = counter_clone.clone();
        Box::pin(async move {
            counter.fetch_add(1, Ordering::SeqCst);
            Ok(())
        })
    });

    let sub_id = bus.subscribe(EventType::SystemStarted, handler).await;

    // Publish before unsubscribe
    let event = Event::system(EventType::SystemStarted, json!({}));
    bus.publish(event).await.unwrap();
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    assert_eq!(counter.load(Ordering::SeqCst), 1);

    // Unsubscribe
    bus.unsubscribe(&sub_id).await.unwrap();

    // Publish after unsubscribe
    let event = Event::system(EventType::SystemStarted, json!({}));
    bus.publish(event).await.unwrap();
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // Counter should still be 1
    assert_eq!(counter.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn test_event_history() {
    let bus = EventBus::with_history_size(10);

    for i in 0..5 {
        let event = Event::system(EventType::SystemStarted, json!({ "index": i }));
        bus.publish(event).await.unwrap();
    }

    let history = bus.get_history(EventFilter::new()).await;
    assert_eq!(history.len(), 5);
}

#[tokio::test]
async fn test_event_history_limit() {
    let bus = EventBus::with_history_size(3);

    for i in 0..5 {
        let event = Event::system(EventType::SystemStarted, json!({ "index": i }));
        bus.publish(event).await.unwrap();
    }

    let history = bus.get_history(EventFilter::new()).await;
    // Should only keep last 3 events
    assert_eq!(history.len(), 3);
}

#[tokio::test]
async fn test_handler_isolation() {
    let bus = EventBus::new();
    let counter = Arc::new(AtomicUsize::new(0));

    // First handler that fails
    let handler1: EventHandler = Arc::new(move |_event| {
        Box::pin(async move {
            Err(TingError::EventError("Handler 1 failed".to_string()))
        })
    });

    // Second handler that succeeds
    let counter_clone = counter.clone();
    let handler2: EventHandler = Arc::new(move |_event| {
        let counter = counter_clone.clone();
        Box::pin(async move {
            counter.fetch_add(1, Ordering::SeqCst);
            Ok(())
        })
    });

    bus.subscribe(EventType::SystemStarted, handler1).await;
    bus.subscribe(EventType::SystemStarted, handler2).await;

    let event = Event::system(EventType::SystemStarted, json!({}));
    bus.publish(event).await.unwrap();

    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // Second handler should still execute despite first handler failing
    assert_eq!(counter.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn test_subscriber_count() {
    let bus = EventBus::new();

    assert_eq!(bus.subscriber_count(&EventType::SystemStarted).await, 0);

    let handler: EventHandler = Arc::new(move |_event| Box::pin(async move { Ok(()) }));

    bus.subscribe(EventType::SystemStarted, handler.clone())
        .await;
    assert_eq!(bus.subscriber_count(&EventType::SystemStarted).await, 1);

    bus.subscribe(EventType::SystemStarted, handler.clone())
        .await;
    assert_eq!(bus.subscriber_count(&EventType::SystemStarted).await, 2);
}

#[tokio::test]
async fn test_event_filter_by_source() {
    let bus = EventBus::new();

    // Publish events from different sources
    let event1 = Event::system(EventType::SystemStarted, json!({}));
    let event2 = Event::plugin(
        EventType::PluginLoaded("plugin1".to_string()),
        "plugin1".to_string(),
        json!({}),
    );
    let event3 = Event::user(EventType::Custom("test".to_string()), "user1".to_string(), json!({}));

    bus.publish(event1).await.unwrap();
    bus.publish(event2).await.unwrap();
    bus.publish(event3).await.unwrap();

    // Filter by system source
    let filter = EventFilter::new().with_sources(vec![EventSource::System]);
    let history = bus.get_history(filter).await;
    assert_eq!(history.len(), 1);
    assert!(matches!(history[0].source, EventSource::System));

    // Filter by plugin source
    let filter = EventFilter::new().with_sources(vec![EventSource::Plugin("plugin1".to_string())]);
    let history = bus.get_history(filter).await;
    assert_eq!(history.len(), 1);
    assert!(matches!(history[0].source, EventSource::Plugin(_)));
}

#[tokio::test]
async fn test_event_pagination() {
    let bus = EventBus::new();

    // Publish 10 events
    for i in 0..10 {
        let event = Event::system(EventType::SystemStarted, json!({ "index": i }));
        bus.publish(event).await.unwrap();
    }

    // Get first page (5 items)
    let (events, total) = bus.query_events(None, None, None, 0, 5).await;
    assert_eq!(events.len(), 5);
    assert_eq!(total, 10);

    // Get second page (5 items)
    let (events, total) = bus.query_events(None, None, None, 1, 5).await;
    assert_eq!(events.len(), 5);
    assert_eq!(total, 10);

    // Get third page (should be empty)
    let (events, total) = bus.query_events(None, None, None, 2, 5).await;
    assert_eq!(events.len(), 0);
    assert_eq!(total, 10);
}

#[tokio::test]
async fn test_event_statistics() {
    let bus = EventBus::new();

    // Publish various events
    bus.publish(Event::system(EventType::SystemStarted, json!({})))
        .await
        .unwrap();
    bus.publish(Event::system(EventType::SystemStarted, json!({})))
        .await
        .unwrap();
    bus.publish(Event::plugin(
        EventType::PluginLoaded("plugin1".to_string()),
        "plugin1".to_string(),
        json!({}),
    ))
    .await
    .unwrap();
    bus.publish(Event::plugin(
        EventType::PluginError("plugin1".to_string()),
        "plugin1".to_string(),
        json!({}),
    ))
    .await
    .unwrap();
    bus.publish(Event::system(
        EventType::TaskFailed("task1".to_string()),
        json!({}),
    ))
    .await
    .unwrap();

    let stats = bus.get_statistics(None).await;

    assert_eq!(stats.total_events, 5);
    assert_eq!(stats.error_count, 2); // PluginError + TaskFailed
    assert_eq!(stats.error_rate, 0.4); // 2/5 = 0.4

    // Check events by type
    assert!(stats.events_by_type.contains_key("SystemStarted"));
    assert_eq!(stats.events_by_type.get("SystemStarted"), Some(&2));

    // Check events by source
    assert!(stats.events_by_source.contains_key("System"));
    assert!(stats.events_by_source.contains_key("Plugin(plugin1)"));
}

#[tokio::test]
async fn test_event_export() {
    let bus = EventBus::new();

    // Publish some events
    bus.publish(Event::system(EventType::SystemStarted, json!({ "test": "data" })))
        .await
        .unwrap();
    bus.publish(Event::plugin(
        EventType::PluginLoaded("plugin1".to_string()),
        "plugin1".to_string(),
        json!({ "version": "1.0.0" }),
    ))
    .await
    .unwrap();

    // Export events
    let json_export = bus.export_events(None).await.unwrap();

    // Verify it's valid JSON
    let parsed: Vec<Event> = serde_json::from_str(&json_export).unwrap();
    assert_eq!(parsed.len(), 2);
}

#[tokio::test]
async fn test_event_query_with_time_range() {
    let bus = EventBus::new();

    let now = Utc::now();
    let past = now - chrono::Duration::hours(1);
    let future = now + chrono::Duration::hours(1);

    // Publish events
    bus.publish(Event::system(EventType::SystemStarted, json!({})))
        .await
        .unwrap();

    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

    bus.publish(Event::system(EventType::SystemShutdown, json!({})))
        .await
        .unwrap();

    // Query with time range
    let (events, total) = bus
        .query_events(None, None, Some((past, future)), 0, 10)
        .await;

    assert_eq!(events.len(), 2);
    assert_eq!(total, 2);

    // Query with narrow time range (should get fewer events)
    let narrow_past = now + chrono::Duration::milliseconds(5);
    let (events, total) = bus
        .query_events(None, None, Some((narrow_past, future)), 0, 10)
        .await;

    assert_eq!(events.len(), 1);
    assert_eq!(total, 1);
}

#[tokio::test]
async fn test_event_filter_with_offset() {
    let bus = EventBus::new();

    // Publish 10 events
    for i in 0..10 {
        let event = Event::system(EventType::SystemStarted, json!({ "index": i }));
        bus.publish(event).await.unwrap();
    }

    // Get events with offset
    let filter = EventFilter::new().with_offset(5).with_limit(3);
    let events = bus.get_history(filter).await;

    assert_eq!(events.len(), 3);
    // Verify we got the right events (indices 5, 6, 7)
    assert_eq!(events[0].data["index"], 5);
    assert_eq!(events[1].data["index"], 6);
    assert_eq!(events[2].data["index"], 7);
}

#[tokio::test]
async fn test_statistics_with_filter() {
    let bus = EventBus::new();

    // Publish events
    bus.publish(Event::system(EventType::SystemStarted, json!({})))
        .await
        .unwrap();
    bus.publish(Event::plugin(
        EventType::PluginLoaded("plugin1".to_string()),
        "plugin1".to_string(),
        json!({}),
    ))
    .await
    .unwrap();
    bus.publish(Event::plugin(
        EventType::PluginError("plugin1".to_string()),
        "plugin1".to_string(),
        json!({}),
    ))
    .await
    .unwrap();

    // Get statistics filtered by plugin events only
    let filter = EventFilter::new().with_sources(vec![EventSource::Plugin("plugin1".to_string())]);
    let stats = bus.get_statistics(Some(filter)).await;

    assert_eq!(stats.total_events, 2);
    assert_eq!(stats.error_count, 1);
    assert_eq!(stats.error_rate, 0.5);
}
