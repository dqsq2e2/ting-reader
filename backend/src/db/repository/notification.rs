use crate::core::error::{Result, TingError};
use crate::db::manager::DatabaseManager;
use crate::db::models::NotificationWebhook;
use rusqlite::{OptionalExtension, Row};
use std::sync::Arc;

fn map_notification_webhook_row(row: &Row<'_>) -> rusqlite::Result<NotificationWebhook> {
    Ok(NotificationWebhook {
        id: row.get(0)?,
        name: row.get(1)?,
        url: row.get(2)?,
        enabled: row.get(3)?,
        events: row.get(4)?,
        secret: row.get(5)?,
        created_at: row.get(6)?,
        updated_at: row.get(7)?,
    })
}

/// Repository for webhook notification configurations.
pub struct NotificationWebhookRepository {
    db: Arc<DatabaseManager>,
}

impl NotificationWebhookRepository {
    pub fn new(db: Arc<DatabaseManager>) -> Self {
        Self { db }
    }

    pub async fn find_all(&self) -> Result<Vec<NotificationWebhook>> {
        self.db
            .execute(|conn| {
                let mut stmt = conn
                    .prepare(
                        "SELECT id, name, url, enabled, events, secret, created_at, updated_at \
                         FROM notification_webhooks \
                         ORDER BY created_at DESC",
                    )
                    .map_err(TingError::DatabaseError)?;

                let rows = stmt
                    .query_map([], map_notification_webhook_row)
                    .map_err(TingError::DatabaseError)?
                    .collect::<std::result::Result<Vec<_>, _>>()
                    .map_err(TingError::DatabaseError)?;

                Ok(rows)
            })
            .await
    }

    pub async fn find_by_id(&self, id: &str) -> Result<Option<NotificationWebhook>> {
        let id = id.to_string();
        self.db
            .execute(move |conn| {
                conn.query_row(
                    "SELECT id, name, url, enabled, events, secret, created_at, updated_at \
                     FROM notification_webhooks WHERE id = ?",
                    [&id],
                    map_notification_webhook_row,
                )
                .optional()
                .map_err(TingError::DatabaseError)
            })
            .await
    }

    pub async fn find_enabled_for_event(&self, event: &str) -> Result<Vec<NotificationWebhook>> {
        let event = event.to_string();
        self.db
            .execute(move |conn| {
                let mut stmt = conn
                    .prepare(
                        "SELECT id, name, url, enabled, events, secret, created_at, updated_at \
                         FROM notification_webhooks WHERE enabled = 1",
                    )
                    .map_err(TingError::DatabaseError)?;

                let webhooks = stmt
                    .query_map([], map_notification_webhook_row)
                    .map_err(TingError::DatabaseError)?
                    .collect::<std::result::Result<Vec<_>, _>>()
                    .map_err(TingError::DatabaseError)?;

                Ok(webhooks
                    .into_iter()
                    .filter(|webhook| {
                        serde_json::from_str::<Vec<String>>(&webhook.events)
                            .map(|events| {
                                events
                                    .iter()
                                    .any(|configured| configured == "*" || configured == &event)
                            })
                            .unwrap_or(false)
                    })
                    .collect())
            })
            .await
    }

    pub async fn create(&self, webhook: &NotificationWebhook) -> Result<()> {
        let webhook = webhook.clone();
        self.db
            .execute(move |conn| {
                conn.execute(
                    "INSERT INTO notification_webhooks \
                     (id, name, url, enabled, events, secret, created_at, updated_at) \
                     VALUES (?, ?, ?, ?, ?, ?, STRFTIME('%Y-%m-%dT%H:%M:%fZ', 'now'), STRFTIME('%Y-%m-%dT%H:%M:%fZ', 'now'))",
                    rusqlite::params![
                        &webhook.id,
                        &webhook.name,
                        &webhook.url,
                        webhook.enabled,
                        &webhook.events,
                        &webhook.secret,
                    ],
                )
                .map_err(TingError::DatabaseError)?;
                Ok(())
            })
            .await
    }

    pub async fn update(&self, webhook: &NotificationWebhook) -> Result<()> {
        let webhook = webhook.clone();
        self.db
            .execute(move |conn| {
                conn.execute(
                    "UPDATE notification_webhooks \
                     SET name = ?, url = ?, enabled = ?, events = ?, secret = ?, updated_at = STRFTIME('%Y-%m-%dT%H:%M:%fZ', 'now') \
                     WHERE id = ?",
                    rusqlite::params![
                        &webhook.name,
                        &webhook.url,
                        webhook.enabled,
                        &webhook.events,
                        &webhook.secret,
                        &webhook.id,
                    ],
                )
                .map_err(TingError::DatabaseError)?;
                Ok(())
            })
            .await
    }

    pub async fn delete(&self, id: &str) -> Result<()> {
        let id = id.to_string();
        self.db
            .execute(move |conn| {
                conn.execute("DELETE FROM notification_webhooks WHERE id = ?", [&id])
                    .map_err(TingError::DatabaseError)?;
                Ok(())
            })
            .await
    }
}
