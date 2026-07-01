use super::AppState;
use crate::core::error::{Result, TingError};
use crate::core::logging::LogEntry;
use axum::{
    extract::{Query, State},
    response::IntoResponse,
    Json,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path as StdPath;

#[derive(Debug, Deserialize)]
pub struct LogsQuery {
    pub level: Option<String>,
    pub module: Option<String>,
    #[serde(default = "default_page")]
    pub page: usize,
    #[serde(default = "default_page_size")]
    pub page_size: usize,
}

fn default_page() -> usize {
    1
}
fn default_page_size() -> usize {
    50
}

#[derive(Debug, Serialize)]
pub struct LogsResponse {
    pub logs: Vec<LogEntry>,
    pub total: usize,
    pub page: usize,
    pub page_size: usize,
}

fn parse_message_params(fields: &serde_json::Value) -> Option<serde_json::Value> {
    let value = fields.get("message_params")?;
    if value.is_object() {
        return Some(value.clone());
    }
    value
        .as_str()
        .and_then(|text| serde_json::from_str::<serde_json::Value>(text).ok())
        .filter(|parsed| parsed.is_object())
}

fn parse_log_file(path: &StdPath, logs: &mut Vec<LogEntry>) {
    let file = match File::open(path) {
        Ok(f) => f,
        Err(_) => return,
    };
    let reader = BufReader::new(file);

    for line in reader.lines().flatten() {
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&line) {
            let timestamp = json
                .get("timestamp")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let level = json
                .get("level")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let module = json
                .get("target")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            let (message, raw_message, message_key, message_params) =
                if let Some(fields) = json.get("fields") {
                    let raw_message = fields.get("message").and_then(|v| v.as_str()).unwrap_or("");
                    let message_key = fields
                        .get("message_key")
                        .and_then(|v| v.as_str())
                        .map(ToOwned::to_owned);
                    let message_params = parse_message_params(fields);
                    (
                        raw_message.to_string(),
                        Some(raw_message.to_string()).filter(|value| !value.is_empty()),
                        message_key,
                        message_params,
                    )
                } else {
                    (String::new(), None, None, None)
                };
            let fields = json.get("fields").and_then(|value| {
                value.as_object().and_then(|fields| {
                    let mut map = fields.clone();
                    map.remove("message");
                    map.remove("message_key");
                    map.remove("message_params");
                    if map.is_empty() {
                        None
                    } else {
                        Some(serde_json::Value::Object(map))
                    }
                })
            });

            logs.push(LogEntry {
                timestamp,
                level,
                module,
                message,
                raw_message,
                message_key,
                message_params,
                fields,
                task_id: None,
                task_status: None,
                task_type: None,
            });
        }
    }
}

fn read_api_logs(data_dir: &StdPath) -> Vec<LogEntry> {
    let mut logs = Vec::new();
    let api_log_dir = data_dir.join("logs");

    for i in (1..=3).rev() {
        let path = api_log_dir.join(format!("system.json.{}", i));
        if path.exists() {
            parse_log_file(&path, &mut logs);
        }
    }

    let current_path = api_log_dir.join("system.json");
    if current_path.exists() {
        parse_log_file(&current_path, &mut logs);
    }

    logs
}

/// Handler for GET /api/v1/system/logs - Get system logs
pub async fn get_system_logs(
    State(state): State<AppState>,
    user: crate::auth::middleware::AuthUser,
    Query(query): Query<LogsQuery>,
) -> Result<impl IntoResponse> {
    if user.role != "admin" {
        return Err(TingError::PermissionDenied(
            "Admin access required".to_string(),
        ));
    }

    let config = state.config.read().await;
    let data_dir = config.storage.data_dir.clone();
    drop(config);

    let level_filter = query.level.clone();
    let module_filter = query.module.clone();

    // Get all tasks
    let tasks = state.task_queue.list_tasks().await.unwrap_or_default();

    let filtered_logs = tokio::task::spawn_blocking(move || {
        let all_logs = read_api_logs(&data_dir);

        let mut filtered: Vec<LogEntry> = all_logs
            .into_iter()
            .filter(|log| {
                // Ignore duplicate text logs for tasks so we only have one record per task
                if log.module == "audit::scan" || log.module == "audit::metadata" {
                    // Skip these text logs, we will use Task records instead
                    return false;
                }

                let level_match = match &level_filter {
                    Some(l) if !l.is_empty() => log.level.eq_ignore_ascii_case(l),
                    _ => true,
                };

                let module_match = match &module_filter {
                    Some(m) if !m.is_empty() => {
                        if m.eq_ignore_ascii_case("audit") {
                            log.module.starts_with("audit::")
                                || log.level.eq_ignore_ascii_case("error")
                        } else if m.eq_ignore_ascii_case("all") {
                            true
                        } else {
                            log.module.to_lowercase().starts_with(&m.to_lowercase())
                        }
                    }
                    _ => {
                        log.module.starts_with("audit::") || log.level.eq_ignore_ascii_case("error")
                    } // 默认只返回 audit 相关的和错误
                };

                level_match && module_match
            })
            .collect();

        // Convert tasks to LogEntry and add them
        for task in tasks {
            let module = match task.task_type.as_str() {
                "scan" | "library_scan" | "scrape" => "audit::scan",
                "write_metadata" => "audit::metadata",
                _ => "audit::task",
            };

            let level = if task.status == "failed" {
                "ERROR"
            } else {
                "INFO"
            };

            let level_match = match &level_filter {
                Some(l) if !l.is_empty() => level.eq_ignore_ascii_case(l),
                _ => true,
            };

            let module_match = match &module_filter {
                Some(m) if !m.is_empty() => {
                    if m.eq_ignore_ascii_case("audit") {
                        module.starts_with("audit::") || level.eq_ignore_ascii_case("error")
                    } else if m.eq_ignore_ascii_case("all") {
                        true
                    } else {
                        module.to_lowercase().starts_with(&m.to_lowercase())
                    }
                }
                _ => module.starts_with("audit::") || level.eq_ignore_ascii_case("error"),
            };

            if level_match && module_match {
                let (message, raw_message, message_key, message_params) =
                    if let Some(key) = task.message_key {
                        let message_params = task
                            .message_params
                            .and_then(|params| serde_json::from_str(&params).ok());
                        (
                            task.message.clone().unwrap_or_default(),
                            task.message.filter(|value| !value.is_empty()),
                            Some(key),
                            message_params,
                        )
                    } else if let Some(msg) = task.message {
                        if !msg.is_empty() {
                            (msg.clone(), Some(msg), None, None)
                        } else if let Some(payload) = task.payload {
                            let params = serde_json::json!({ "payload": payload });
                            (
                                String::new(),
                                None,
                                Some("task.execute_with_payload".to_string()),
                                Some(params),
                            )
                        } else {
                            (String::new(), None, Some("task.execute".to_string()), None)
                        }
                    } else if let Some(payload) = task.payload {
                        let params = serde_json::json!({ "payload": payload });
                        (
                            String::new(),
                            None,
                            Some("task.execute_with_payload".to_string()),
                            Some(params),
                        )
                    } else {
                        (String::new(), None, Some("task.execute".to_string()), None)
                    };

                let timestamp = if task.status == "running" {
                    // Update running tasks to "now" so they appear at the top, or use updated_at
                    Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
                } else {
                    task.updated_at
                };

                filtered.push(LogEntry {
                    timestamp,
                    level: level.to_string(),
                    module: module.to_string(),
                    message,
                    raw_message,
                    message_key,
                    message_params,
                    fields: Some(serde_json::json!({
                        "task_id": task.id.clone(),
                        "task_status": task.status.clone(),
                        "task_type": task.task_type.clone(),
                    })),
                    task_id: Some(task.id),
                    task_status: Some(task.status),
                    task_type: Some(task.task_type),
                });
            }
        }

        // Sort by timestamp descending
        filtered.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

        filtered
    })
    .await
    .map_err(|e| TingError::ExternalError(e.to_string()))?;

    let total = filtered_logs.len();

    let start = (query.page.saturating_sub(1)) * query.page_size;
    let end = std::cmp::min(start + query.page_size, total);

    let page_logs = if start < total {
        filtered_logs[start..end].to_vec()
    } else {
        Vec::new()
    };

    Ok(Json(LogsResponse {
        logs: page_logs,
        total,
        page: query.page,
        page_size: query.page_size,
    }))
}

#[derive(Debug, Deserialize)]
pub struct ExportLogsQuery {
    pub level: Option<String>,
}

/// Handler for GET /api/v1/system/logs/export - Export system logs
pub async fn export_system_logs(
    State(state): State<AppState>,
    user: crate::auth::middleware::AuthUser,
    Query(query): Query<ExportLogsQuery>,
) -> Result<impl IntoResponse> {
    if user.role != "admin" {
        return Err(TingError::PermissionDenied(
            "Admin access required".to_string(),
        ));
    }

    let config = state.config.read().await;
    let data_dir = config.storage.data_dir.clone();
    drop(config);

    let level_filter = query.level.clone();

    let filtered_logs = tokio::task::spawn_blocking(move || {
        let all_logs = read_api_logs(&data_dir);

        all_logs
            .into_iter()
            .filter(|log| match &level_filter {
                Some(l) if !l.is_empty() => log.level.eq_ignore_ascii_case(l),
                _ => true,
            })
            .collect::<Vec<_>>()
    })
    .await
    .map_err(|e| TingError::ExternalError(e.to_string()))?;

    let mut output = String::new();
    for log in filtered_logs {
        let fields = log
            .fields
            .as_ref()
            .map(|value| format!(" {}", value))
            .unwrap_or_default();
        output.push_str(&format!(
            "[{}] [{}] [{}] {}{}\n",
            log.timestamp, log.level, log.module, log.message, fields
        ));
    }

    let timestamp = Utc::now().format("%Y%m%d_%H%M%S").to_string();
    let filename = if query
        .level
        .as_deref()
        .is_some_and(|level| level.eq_ignore_ascii_case("error"))
    {
        format!("error_logs_{}.txt", timestamp)
    } else {
        format!("system_logs_{}.txt", timestamp)
    };

    let headers = [
        (
            axum::http::header::CONTENT_TYPE,
            "text/plain; charset=utf-8".to_string(),
        ),
        (
            axum::http::header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{}\"", filename),
        ),
    ];

    Ok((headers, output).into_response())
}

#[derive(Debug, Serialize)]
pub struct ClearSystemLogsResponse {
    pub message: String,
}

/// Handler for DELETE /api/v1/system/logs - Clear system logs
pub async fn clear_system_logs(
    State(state): State<AppState>,
    user: crate::auth::middleware::AuthUser,
) -> Result<impl IntoResponse> {
    if user.role != "admin" {
        return Err(TingError::PermissionDenied(
            "Admin access required".to_string(),
        ));
    }

    let config = state.config.read().await;
    let data_dir = config.storage.data_dir.clone();
    drop(config);

    tokio::task::spawn_blocking(move || {
        let api_log_dir = data_dir.join("logs");

        // Remove rolled files
        for i in 1..=3 {
            let path = api_log_dir.join(format!("system.json.{}", i));
            if path.exists() {
                let _ = std::fs::remove_file(path);
            }
        }

        // Empty the main log file by truncating it
        let current_path = api_log_dir.join("system.json");
        if current_path.exists() {
            if let Ok(file) = std::fs::OpenOptions::new()
                .write(true)
                .truncate(true)
                .open(&current_path)
            {
                // Optionally write an initial log line to say logs were cleared
                let _ = file;
            }
        }
    })
    .await
    .map_err(|e| TingError::ExternalError(e.to_string()))?;

    Ok(Json(ClearSystemLogsResponse {
        message: "System logs cleared successfully".to_string(),
    }))
}
