use crate::db::repository::NotificationWebhookRepository;
use serde::Serialize;
use serde_json::Value;
use std::sync::Arc;
use std::time::Duration;

#[derive(Debug, Clone, Serialize)]
pub struct NotificationEventPayload {
    pub event: String,
    pub title: String,
    pub message: String,
    pub data: Value,
    pub occurred_at: String,
}

impl NotificationEventPayload {
    pub fn new(
        event: impl Into<String>,
        title: impl Into<String>,
        message: impl Into<String>,
        data: Value,
    ) -> Self {
        Self {
            event: event.into(),
            title: title.into(),
            message: message.into(),
            data,
            occurred_at: chrono::Utc::now().to_rfc3339(),
        }
    }
}

pub fn dispatch_notification_event(
    repo: Arc<NotificationWebhookRepository>,
    payload: NotificationEventPayload,
) {
    tokio::spawn(async move {
        if let Err(error) = dispatch_notification_event_inner(repo, payload).await {
            tracing::warn!(
                target: "audit::notification",
                error = %error,
                "发送 webhook 通知失败"
            );
        }
    });
}

async fn dispatch_notification_event_inner(
    repo: Arc<NotificationWebhookRepository>,
    payload: NotificationEventPayload,
) -> crate::core::error::Result<()> {
    let webhooks = repo.find_enabled_for_event(&payload.event).await?;
    if webhooks.is_empty() {
        return Ok(());
    }

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(8))
        .build()
        .map_err(|error| crate::core::error::TingError::ExternalError(error.to_string()))?;

    for webhook in webhooks {
        let mut request = client
            .post(&webhook.url)
            .header("Content-Type", "application/json")
            .header("X-Ting-Event", &payload.event)
            .json(&payload);

        if let Some(secret) = webhook.secret.as_deref().filter(|value| !value.is_empty()) {
            request = request.header("X-Ting-Webhook-Secret", secret);
        }

        match request.send().await {
            Ok(response) if response.status().is_success() => {
                tracing::info!(
                    target: "audit::notification",
                    webhook_id = %webhook.id,
                    webhook_name = %webhook.name,
                    event = %payload.event,
                    status = %response.status(),
                    "Webhook 通知已发送"
                );
            }
            Ok(response) => {
                tracing::warn!(
                    target: "audit::notification",
                    webhook_id = %webhook.id,
                    webhook_name = %webhook.name,
                    event = %payload.event,
                    status = %response.status(),
                    "Webhook 通知返回非成功状态"
                );
            }
            Err(error) => {
                tracing::warn!(
                    target: "audit::notification",
                    webhook_id = %webhook.id,
                    webhook_name = %webhook.name,
                    event = %payload.event,
                    error = %error,
                    "Webhook 通知请求失败"
                );
            }
        }
    }

    Ok(())
}
