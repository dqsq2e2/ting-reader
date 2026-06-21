use crate::core::error::{Result, TingError};
use crate::db::models::NotificationWebhook;
use crate::db::repository::NotificationWebhookRepository;
use lazy_static::lazy_static;
use regex::{Captures, Regex};
use reqwest::header::{HeaderMap, HeaderName, HeaderValue, CONTENT_TYPE};
use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

const DEFAULT_BODY_TEMPLATE: &str = "{{json:payload}}";
const MAX_RESPONSE_BODY_CHARS: usize = 4096;

lazy_static! {
    static ref TEMPLATE_VARIABLE_RE: Regex =
        Regex::new(r"\{\{\s*(?:(json)\s*:)?\s*([A-Za-z0-9_.]+)\s*\}\}")
            .expect("valid webhook template regex");
}

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

    pub fn test_payload() -> Self {
        Self::new(
            "webhook.test",
            "听悦测试通知",
            "如果你看到这条消息，说明 Webhook 配置正常。",
            serde_json::json!({
                "username": "admin",
                "book_title": "示例有声书",
                "chapter_title": "第一章"
            }),
        )
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct WebhookDeliveryResult {
    pub success: bool,
    pub status: u16,
    pub response_body: String,
    pub rendered_body: String,
    pub error: Option<String>,
}

pub fn default_body_template() -> String {
    DEFAULT_BODY_TEMPLATE.to_string()
}

pub fn parse_headers(raw: &str) -> Result<HashMap<String, String>> {
    serde_json::from_str(raw).map_err(|error| {
        TingError::DeserializationError(format!("Invalid webhook headers: {error}"))
    })
}

pub fn validate_headers(headers: &HashMap<String, String>) -> Result<()> {
    for (name, value) in headers {
        HeaderName::from_bytes(name.as_bytes()).map_err(|_| {
            TingError::ValidationError(format!("Invalid webhook header name: {name}"))
        })?;
        HeaderValue::from_str(value).map_err(|_| {
            TingError::ValidationError(format!("Invalid value for webhook header: {name}"))
        })?;
    }
    Ok(())
}

pub fn render_template(template: &str, payload: &NotificationEventPayload) -> Result<String> {
    let mut render_error = None;
    let rendered = TEMPLATE_VARIABLE_RE.replace_all(template, |captures: &Captures<'_>| {
        let json_encoded = captures.get(1).is_some();
        let variable = captures
            .get(2)
            .map(|value| value.as_str())
            .unwrap_or_default();

        match resolve_template_variable(variable, payload) {
            Some(value) => {
                if json_encoded {
                    serde_json::to_string(&value).unwrap_or_else(|error| {
                        render_error = Some(format!(
                            "Failed to encode template variable {variable}: {error}"
                        ));
                        String::new()
                    })
                } else {
                    raw_template_value(&value)
                }
            }
            None => {
                render_error = Some(format!("Unknown webhook template variable: {variable}"));
                String::new()
            }
        }
    });

    if let Some(error) = render_error {
        return Err(TingError::ValidationError(error));
    }

    Ok(rendered.into_owned())
}

pub fn validate_template(template: &str) -> Result<String> {
    let template = template.trim();
    if template.is_empty() {
        return Err(TingError::ValidationError(
            "Webhook body template cannot be empty".to_string(),
        ));
    }

    render_template(template, &NotificationEventPayload::test_payload())?;
    Ok(template.to_string())
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
) -> Result<()> {
    let webhooks = repo.find_enabled_for_event(&payload.event).await?;
    if webhooks.is_empty() {
        return Ok(());
    }

    let client = build_client()?;

    for webhook in webhooks {
        match deliver_webhook_with_client(&client, &webhook, &payload).await {
            Ok(result) if result.success => {
                tracing::info!(
                    target: "audit::notification",
                    webhook_id = %webhook.id,
                    webhook_name = %webhook.name,
                    event = %payload.event,
                    status = result.status,
                    "Webhook 通知已发送"
                );
            }
            Ok(result) => {
                tracing::warn!(
                    target: "audit::notification",
                    webhook_id = %webhook.id,
                    webhook_name = %webhook.name,
                    event = %payload.event,
                    status = result.status,
                    error = result.error.as_deref().unwrap_or("Unknown webhook error"),
                    "Webhook 通知返回失败"
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

pub async fn deliver_webhook(
    webhook: &NotificationWebhook,
    payload: &NotificationEventPayload,
) -> Result<WebhookDeliveryResult> {
    let client = build_client()?;
    deliver_webhook_with_client(&client, webhook, payload).await
}

async fn deliver_webhook_with_client(
    client: &reqwest::Client,
    webhook: &NotificationWebhook,
    payload: &NotificationEventPayload,
) -> Result<WebhookDeliveryResult> {
    let rendered_body = render_template(&webhook.body_template, payload)?;
    let configured_headers = parse_headers(&webhook.headers)?;
    validate_headers(&configured_headers)?;

    let mut headers = HeaderMap::new();
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    headers.insert(
        HeaderName::from_static("x-ting-event"),
        HeaderValue::from_str(&payload.event).map_err(|_| {
            TingError::ValidationError("Webhook event cannot be used as a header".to_string())
        })?,
    );

    if let Some(secret) = webhook.secret.as_deref().filter(|value| !value.is_empty()) {
        headers.insert(
            HeaderName::from_static("x-ting-webhook-secret"),
            HeaderValue::from_str(secret).map_err(|_| {
                TingError::ValidationError("Webhook secret contains invalid characters".to_string())
            })?,
        );
    }

    for (name, value_template) in configured_headers {
        let name = HeaderName::from_bytes(name.as_bytes()).map_err(|_| {
            TingError::ValidationError(format!("Invalid webhook header name: {name}"))
        })?;
        let rendered_value = render_template(&value_template, payload)?;
        let value = HeaderValue::from_str(&rendered_value).map_err(|_| {
            TingError::ValidationError(format!("Invalid value for webhook header: {name}"))
        })?;
        headers.insert(name, value);
    }

    let response = client
        .post(&webhook.url)
        .headers(headers)
        .body(rendered_body.clone())
        .send()
        .await
        .map_err(|error| TingError::NetworkError(error.to_string()))?;

    let status = response.status();
    let response_body = response
        .text()
        .await
        .map_err(|error| TingError::NetworkError(error.to_string()))?;
    let response_body = truncate_chars(&response_body, MAX_RESPONSE_BODY_CHARS);

    let business_error = detect_business_error(&response_body);
    let success = status.is_success() && business_error.is_none();
    let error = if !status.is_success() {
        Some(format!("HTTP {}", status.as_u16()))
    } else {
        business_error
    };

    Ok(WebhookDeliveryResult {
        success,
        status: status.as_u16(),
        response_body,
        rendered_body,
        error,
    })
}

fn build_client() -> Result<reqwest::Client> {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(8))
        .build()
        .map_err(|error| TingError::ExternalError(error.to_string()))
}

fn resolve_template_variable(variable: &str, payload: &NotificationEventPayload) -> Option<Value> {
    if variable == "notification" {
        return Some(Value::String(format!(
            "{}\n{}",
            payload.title, payload.message
        )));
    }

    let payload_value = serde_json::to_value(payload).ok()?;
    if variable == "payload" {
        return Some(payload_value);
    }

    let mut current = &payload_value;
    for segment in variable.split('.') {
        current = current.get(segment)?;
    }
    Some(current.clone())
}

fn raw_template_value(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::String(value) => value.clone(),
        other => serde_json::to_string(other).unwrap_or_default(),
    }
}

fn detect_business_error(response_body: &str) -> Option<String> {
    let value = serde_json::from_str::<Value>(response_body).ok()?;
    let errcode = value.get("errcode")?.as_i64()?;
    if errcode == 0 {
        return None;
    }

    let message = value
        .get("errmsg")
        .and_then(Value::as_str)
        .unwrap_or("Webhook service returned an error");
    Some(format!("{message} (errcode: {errcode})"))
}

fn truncate_chars(value: &str, max_chars: usize) -> String {
    let mut chars = value.chars();
    let truncated = chars.by_ref().take(max_chars).collect::<String>();
    if chars.next().is_some() {
        format!("{truncated}...")
    } else {
        truncated
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_raw_and_json_variables() {
        let payload = NotificationEventPayload::new(
            "book.created",
            "A \"quoted\" title",
            "line one\nline two",
            serde_json::json!({"book": {"title": "Example"}}),
        );
        let template = r#"{"title":{{json:title}},"message":{{json:message}},"book":{{json:data.book.title}}}"#;

        let rendered = render_template(template, &payload).unwrap();
        let json: Value = serde_json::from_str(&rendered).unwrap();

        assert_eq!(json["title"], "A \"quoted\" title");
        assert_eq!(json["message"], "line one\nline two");
        assert_eq!(json["book"], "Example");
    }

    #[test]
    fn renders_complete_payload_as_json() {
        let payload = NotificationEventPayload::test_payload();
        let rendered = render_template(DEFAULT_BODY_TEMPLATE, &payload).unwrap();
        let json: Value = serde_json::from_str(&rendered).unwrap();

        assert_eq!(json["event"], "webhook.test");
        assert_eq!(json["data"]["username"], "admin");
    }

    #[test]
    fn rejects_unknown_template_variables() {
        let error = render_template(
            "{{json:missing}}",
            &NotificationEventPayload::test_payload(),
        )
        .unwrap_err();

        assert!(error
            .to_string()
            .contains("Unknown webhook template variable"));
    }

    #[test]
    fn detects_wecom_business_errors() {
        assert_eq!(
            detect_business_error(r#"{"errcode":0,"errmsg":"ok"}"#),
            None
        );
        assert!(
            detect_business_error(r#"{"errcode":40058,"errmsg":"invalid parameter"}"#)
                .unwrap()
                .contains("40058")
        );
    }
}
