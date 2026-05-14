//! Event bus implementation for publish-subscribe pattern
//!
//! The event bus enables plugins and system components to communicate through events.

// It supports:
// - Asynchronous event publishing and handling
// - Multiple subscribers per event type
// - Event filtering and history
// - Isolated error handling (one handler failure doesn't affect others)

#[cfg(test)]
mod tests;

use crate::core::error::{Result, TingError};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

/// Event bus for publish-subscribe pattern
pub struct EventBus {
    subscribers: Arc<RwLock<HashMap<EventType, Vec<Subscriber>>>>,
    event_log: Arc<RwLock<Vec<Event>>>,
    max_history: usize,
}

/// Unique identifier for an event
pub type EventId = String;

/// Unique identifier for a subscription
pub type SubscriptionId = String;

/// Unique identifier for a plugin
pub type PluginId = String;

/// Unique identifier for a task
pub type TaskId = String;

/// Event structure containing all event information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    pub id: EventId,
    pub event_type: EventType,
    pub timestamp: DateTime<Utc>,
    pub source: EventSource,
    pub data: Value,
}

/// Types of events that can be published
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EventType {
    // System events
    SystemStarted,
    SystemShutdown,

    // Plugin events
    PluginLoaded(PluginId),
    PluginUnloaded(PluginId),
    PluginError(PluginId),

    // Task events
    TaskSubmitted(TaskId),
    TaskCompleted(TaskId),
    TaskFailed(TaskId),

    // Data events
    BookAdded(i64),
    BookUpdated(i64),
    BookDeleted(i64),

    // Custom events
    Custom(String),
}

/// Source of an event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EventSource {
    System,
    Plugin(PluginId),
    User(String),
}

/// Event handler function type
pub type EventHandler = Arc<
    dyn Fn(Event) -> Pin<Box<dyn Future<Output = Result<()>> + Send>> + Send + Sync,
>;

/// Subscriber information
#[derive(Clone)]
struct Subscriber {
    id: SubscriptionId,
    handler: EventHandler,
}

/// Filter for querying event history
#[derive(Debug, Clone)]
pub struct EventFilter {
    pub event_types: Option<Vec<EventType>>,
    pub sources: Option<Vec<EventSource>>,
    pub since: Option<DateTime<Utc>>,
    pub until: Option<DateTime<Utc>>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

/// Event statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventStatistics {
    pub total_events: usize,
    pub events_by_type: HashMap<String, usize>,
    pub events_by_source: HashMap<String, usize>,
    pub error_count: usize,
    pub error_rate: f64,
}

impl EventBus {
    /// Create a new event bus with default history size
    pub fn new() -> Self {
        Self::with_history_size(1000)
    }

    /// Create a new event bus with specified history size
    pub fn with_history_size(max_history: usize) -> Self {
        Self {
            subscribers: Arc::new(RwLock::new(HashMap::new())),
            event_log: Arc::new(RwLock::new(Vec::new())),
            max_history,
        }
    }

    /// Subscribe to an event type with a handler
    ///
    /// Returns a subscription ID that can be used to unsubscribe
    pub async fn subscribe(
        &self,
        event_type: EventType,
        handler: EventHandler,
    ) -> SubscriptionId {
        let subscription_id = Uuid::new_v4().to_string();
        let subscriber = Subscriber {
            id: subscription_id.clone(),
            handler,
        };

        let mut subscribers = self.subscribers.write().await;
        subscribers
            .entry(event_type)
            .or_insert_with(Vec::new)
            .push(subscriber);

        subscription_id
    }

    /// Unsubscribe from events using a subscription ID
    pub async fn unsubscribe(&self, subscription_id: &str) -> Result<()> {
        let mut subscribers = self.subscribers.write().await;

        for (_, subs) in subscribers.iter_mut() {
            if let Some(pos) = subs.iter().position(|s| s.id == subscription_id) {
                subs.remove(pos);
                return Ok(());
            }
        }

        Err(TingError::NotFound(format!(
            "Subscription not found: {}",
            subscription_id
        )))
    }

    /// Publish an event to all subscribers
    ///
    /// Handlers are called asynchronously and errors are isolated
    /// (one handler failure doesn't affect others)
    pub async fn publish(&self, event: Event) -> Result<()> {
        // Add to event log
        {
            let mut log = self.event_log.write().await;
            log.push(event.clone());

            // Trim history if needed
            if log.len() > self.max_history {
                let excess = log.len() - self.max_history;
                log.drain(0..excess);
            }
        }

        // Get subscribers for this event type
        let subscribers = {
            let subs = self.subscribers.read().await;
            subs.get(&event.event_type).map(|v| v.clone())
        };

        if let Some(subscribers) = subscribers {
            // Call all handlers asynchronously
            let mut handles = Vec::new();

            for subscriber in subscribers {
                let event_clone = event.clone();
                let handler = subscriber.handler.clone();

                let handle = tokio::spawn(async move {
                    if let Err(e) = handler(event_clone).await {
                        tracing::error!(
                            "Event handler failed for subscription {}: {}",
                            subscriber.id,
                            e
                        );
                    }
                });

                handles.push(handle);
            }

            // Wait for all handlers to complete
            for handle in handles {
                let _ = handle.await;
            }
        }

        Ok(())
    }

    /// Get event history with optional filtering and pagination
    pub async fn get_history(&self, filter: EventFilter) -> Vec<Event> {
        let log = self.event_log.read().await;
        let mut events: Vec<Event> = log.clone();

        // Apply filters
        if let Some(event_types) = &filter.event_types {
            events.retain(|e| event_types.contains(&e.event_type));
        }

        if let Some(sources) = &filter.sources {
            events.retain(|e| {
                sources.iter().any(|s| match (s, &e.source) {
                    (EventSource::System, EventSource::System) => true,
                    (EventSource::Plugin(id1), EventSource::Plugin(id2)) => id1 == id2,
                    (EventSource::User(id1), EventSource::User(id2)) => id1 == id2,
                    _ => false,
                })
            });
        }

        if let Some(since) = filter.since {
            events.retain(|e| e.timestamp >= since);
        }

        if let Some(until) = filter.until {
            events.retain(|e| e.timestamp <= until);
        }

        // Apply offset
        if let Some(offset) = filter.offset {
            if offset < events.len() {
                events = events[offset..].to_vec();
            } else {
                events.clear();
            }
        }

        // Apply limit
        if let Some(limit) = filter.limit {
            events.truncate(limit);
        }

        events
    }

    /// Get event statistics
    pub async fn get_statistics(&self, filter: Option<EventFilter>) -> EventStatistics {
        let events = if let Some(filter) = filter {
            self.get_history(filter).await
        } else {
            let log = self.event_log.read().await;
            log.clone()
        };

        let total_events = events.len();
        let mut events_by_type: HashMap<String, usize> = HashMap::new();
        let mut events_by_source: HashMap<String, usize> = HashMap::new();
        let mut error_count = 0;

        for event in &events {
            // Count by type
            let type_key = format!("{:?}", event.event_type);
            *events_by_type.entry(type_key).or_insert(0) += 1;

            // Count by source
            let source_key = match &event.source {
                EventSource::System => "System".to_string(),
                EventSource::Plugin(id) => format!("Plugin({})", id),
                EventSource::User(id) => format!("User({})", id),
            };
            *events_by_source.entry(source_key).or_insert(0) += 1;

            // Count errors
            if matches!(
                event.event_type,
                EventType::PluginError(_) | EventType::TaskFailed(_)
            ) {
                error_count += 1;
            }
        }

        let error_rate = if total_events > 0 {
            error_count as f64 / total_events as f64
        } else {
            0.0
        };

        EventStatistics {
            total_events,
            events_by_type,
            events_by_source,
            error_count,
            error_rate,
        }
    }

    /// Export event log to JSON
    pub async fn export_events(&self, filter: Option<EventFilter>) -> Result<String> {
        let events = if let Some(filter) = filter {
            self.get_history(filter).await
        } else {
            let log = self.event_log.read().await;
            log.clone()
        };

        serde_json::to_string_pretty(&events)
            .map_err(|e| TingError::SerializationError(format!("Failed to export events: {}", e)))
    }

    /// Query events with advanced filtering
    pub async fn query_events(
        &self,
        event_types: Option<Vec<EventType>>,
        sources: Option<Vec<EventSource>>,
        time_range: Option<(DateTime<Utc>, DateTime<Utc>)>,
        page: usize,
        page_size: usize,
    ) -> (Vec<Event>, usize) {
        let mut filter = EventFilter::new();

        if let Some(types) = event_types {
            filter = filter.with_event_types(types);
        }

        if let Some(srcs) = sources {
            filter = filter.with_sources(srcs);
        }

        if let Some((since, until)) = time_range {
            filter = filter.with_time_range(since, until);
        }

        // Get total count before pagination
        let all_events = self.get_history(filter.clone()).await;
        let total_count = all_events.len();

        // Apply pagination
        let offset = page * page_size;
        filter = filter.with_offset(offset).with_limit(page_size);

        let events = self.get_history(filter).await;

        (events, total_count)
    }

    /// Clear event history
    pub async fn clear_history(&self) {
        let mut log = self.event_log.write().await;
        log.clear();
    }

    /// Get the number of subscribers for an event type
    pub async fn subscriber_count(&self, event_type: &EventType) -> usize {
        let subscribers = self.subscribers.read().await;
        subscribers.get(event_type).map(|s| s.len()).unwrap_or(0)
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}

impl Event {
    /// Create a new event
    pub fn new(event_type: EventType, source: EventSource, data: Value) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            event_type,
            timestamp: Utc::now(),
            source,
            data,
        }
    }

    /// Create a system event
    pub fn system(event_type: EventType, data: Value) -> Self {
        Self::new(event_type, EventSource::System, data)
    }

    /// Create a plugin event
    pub fn plugin(event_type: EventType, plugin_id: PluginId, data: Value) -> Self {
        Self::new(event_type, EventSource::Plugin(plugin_id), data)
    }

    /// Create a user event
    pub fn user(event_type: EventType, user_id: String, data: Value) -> Self {
        Self::new(event_type, EventSource::User(user_id), data)
    }
}

impl EventFilter {
    /// Create a new empty filter
    pub fn new() -> Self {
        Self {
            event_types: None,
            sources: None,
            since: None,
            until: None,
            limit: None,
            offset: None,
        }
    }

    /// Filter by event types
    pub fn with_event_types(mut self, event_types: Vec<EventType>) -> Self {
        self.event_types = Some(event_types);
        self
    }

    /// Filter by sources
    pub fn with_sources(mut self, sources: Vec<EventSource>) -> Self {
        self.sources = Some(sources);
        self
    }

    /// Filter by time range
    pub fn with_time_range(mut self, since: DateTime<Utc>, until: DateTime<Utc>) -> Self {
        self.since = Some(since);
        self.until = Some(until);
        self
    }

    /// Limit number of results
    pub fn with_limit(mut self, limit: usize) -> Self {
        self.limit = Some(limit);
        self
    }

    /// Set offset for pagination
    pub fn with_offset(mut self, offset: usize) -> Self {
        self.offset = Some(offset);
        self
    }
}

impl Default for EventFilter {
    fn default() -> Self {
        Self::new()
    }
}

