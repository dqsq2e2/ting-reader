use super::AppState;
use crate::api::require_admin;
use crate::core::error::{Result, TingError};
use crate::db::models::NotificationWebhook;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

const SUPPORTED_EVENTS: &[(&str, &str, &str)] = &[
    ("user.login", "用户登录", "用户成功登录系统"),
    ("playback.play", "播放", "用户开始播放作品或章节"),
    ("library.created", "新增媒体库", "管理员创建媒体库"),
    ("library.deleted", "删除媒体库", "管理员删除媒体库"),
    ("book.created", "作品入库", "作品被创建或入库"),
    ("book.deleted", "删除作品", "作品被删除"),
    ("library.scan_completed", "扫描完成", "媒体库扫描任务完成"),
];

#[derive(Debug, Serialize)]
pub struct NotificationEventOption {
    pub id: String,
    pub label: String,
    pub description: String,
}

#[derive(Debug, Serialize)]
pub struct NotificationWebhookResponse {
    pub id: String,
    pub name: String,
    pub url: String,
    pub enabled: bool,
    pub events: Vec<String>,
    pub secret: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Deserialize)]
pub struct NotificationWebhookRequest {
    pub name: String,
    pub url: String,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    #[serde(default)]
    pub events: Vec<String>,
    pub secret: Option<String>,
}

fn default_enabled() -> bool {
    true
}

impl From<NotificationWebhook> for NotificationWebhookResponse {
    fn from(webhook: NotificationWebhook) -> Self {
        Self {
            id: webhook.id,
            name: webhook.name,
            url: webhook.url,
            enabled: webhook.enabled == 1,
            events: serde_json::from_str(&webhook.events).unwrap_or_default(),
            secret: webhook.secret,
            created_at: webhook.created_at,
            updated_at: webhook.updated_at,
        }
    }
}

pub async fn list_notification_events(
    State(_state): State<AppState>,
    user: crate::auth::middleware::AuthUser,
) -> Result<impl IntoResponse> {
    require_admin(&user)?;

    Ok(Json(
        SUPPORTED_EVENTS
            .iter()
            .map(|(id, label, description)| NotificationEventOption {
                id: id.to_string(),
                label: label.to_string(),
                description: description.to_string(),
            })
            .collect::<Vec<_>>(),
    ))
}

pub async fn list_notification_webhooks(
    State(state): State<AppState>,
    user: crate::auth::middleware::AuthUser,
) -> Result<impl IntoResponse> {
    require_admin(&user)?;

    let webhooks = state
        .notification_repo
        .find_all()
        .await?
        .into_iter()
        .map(NotificationWebhookResponse::from)
        .collect::<Vec<_>>();

    Ok(Json(webhooks))
}

pub async fn create_notification_webhook(
    State(state): State<AppState>,
    user: crate::auth::middleware::AuthUser,
    Json(req): Json<NotificationWebhookRequest>,
) -> Result<impl IntoResponse> {
    require_admin(&user)?;

    let now = chrono::Utc::now().to_rfc3339();
    let webhook = NotificationWebhook {
        id: Uuid::new_v4().to_string(),
        name: normalize_name(&req.name)?,
        url: normalize_url(&req.url)?,
        enabled: req.enabled as i32,
        events: normalize_events(req.events)?,
        secret: normalize_secret(req.secret),
        created_at: now.clone(),
        updated_at: now,
    };

    state.notification_repo.create(&webhook).await?;

    tracing::info!(
        target: "audit::notification",
        webhook_id = %webhook.id,
        webhook_name = %webhook.name,
        actor = %user.username,
        "管理员创建了 webhook 通知配置"
    );

    Ok((
        StatusCode::CREATED,
        Json(NotificationWebhookResponse::from(webhook)),
    ))
}

pub async fn update_notification_webhook(
    State(state): State<AppState>,
    Path(id): Path<String>,
    user: crate::auth::middleware::AuthUser,
    Json(req): Json<NotificationWebhookRequest>,
) -> Result<impl IntoResponse> {
    require_admin(&user)?;

    let mut webhook = state
        .notification_repo
        .find_by_id(&id)
        .await?
        .ok_or_else(|| TingError::NotFound(format!("Webhook {} not found", id)))?;

    webhook.name = normalize_name(&req.name)?;
    webhook.url = normalize_url(&req.url)?;
    webhook.enabled = req.enabled as i32;
    webhook.events = normalize_events(req.events)?;
    webhook.secret = normalize_secret(req.secret);
    webhook.updated_at = chrono::Utc::now().to_rfc3339();

    state.notification_repo.update(&webhook).await?;
    let response = state
        .notification_repo
        .find_by_id(&id)
        .await?
        .unwrap_or(webhook);

    tracing::info!(
        target: "audit::notification",
        webhook_id = %id,
        actor = %user.username,
        "管理员更新了 webhook 通知配置"
    );

    Ok(Json(NotificationWebhookResponse::from(response)))
}

pub async fn delete_notification_webhook(
    State(state): State<AppState>,
    Path(id): Path<String>,
    user: crate::auth::middleware::AuthUser,
) -> Result<impl IntoResponse> {
    require_admin(&user)?;

    let webhook = state
        .notification_repo
        .find_by_id(&id)
        .await?
        .ok_or_else(|| TingError::NotFound(format!("Webhook {} not found", id)))?;

    state.notification_repo.delete(&id).await?;

    tracing::info!(
        target: "audit::notification",
        webhook_id = %id,
        webhook_name = %webhook.name,
        actor = %user.username,
        "管理员删除了 webhook 通知配置"
    );

    Ok(StatusCode::NO_CONTENT)
}

fn normalize_name(name: &str) -> Result<String> {
    let name = name.trim();
    if name.is_empty() {
        return Err(TingError::ValidationError(
            "Webhook name cannot be empty".to_string(),
        ));
    }
    Ok(name.to_string())
}

fn normalize_url(raw_url: &str) -> Result<String> {
    let url = raw_url.trim();
    let parsed = url::Url::parse(url)
        .map_err(|_| TingError::ValidationError("Webhook URL is invalid".to_string()))?;
    if parsed.scheme() != "http" && parsed.scheme() != "https" {
        return Err(TingError::ValidationError(
            "Webhook URL must start with http:// or https://".to_string(),
        ));
    }
    Ok(url.to_string())
}

fn normalize_events(events: Vec<String>) -> Result<String> {
    let supported = SUPPORTED_EVENTS
        .iter()
        .map(|(id, _, _)| *id)
        .collect::<std::collections::HashSet<_>>();

    let mut normalized = events
        .into_iter()
        .map(|event| event.trim().to_string())
        .filter(|event| !event.is_empty())
        .collect::<Vec<_>>();
    normalized.sort();
    normalized.dedup();

    if normalized.is_empty() {
        return Err(TingError::ValidationError(
            "At least one event must be selected".to_string(),
        ));
    }

    if let Some(unknown) = normalized
        .iter()
        .find(|event| !supported.contains(event.as_str()))
    {
        return Err(TingError::ValidationError(format!(
            "Unsupported webhook event: {}",
            unknown
        )));
    }

    serde_json::to_string(&normalized)
        .map_err(|error| TingError::SerializationError(error.to_string()))
}

fn normalize_secret(secret: Option<String>) -> Option<String> {
    secret
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}
