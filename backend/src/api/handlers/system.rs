use crate::api::models::{
    TasksQuery, TaskInfoResponse, TaskDetailResponse, CancelTaskResponse,
    HealthResponse, HealthStatus, ComponentsHealth, ComponentStatus, ComponentHealth,
    MetricsResponse, SystemMetrics, PluginMetrics, TaskQueueMetrics, DatabaseMetrics,
    ConfigResponse, ServerConfigResponse, DatabaseConfigResponse,
    PluginSystemConfigResponse, TaskQueueConfigResponse,
    LoggingConfigResponse, SecurityConfigResponse, StorageConfigResponse,
    UpdateConfigRequest, UpdateConfigResponse,
};
use crate::core::error::{Result, TingError};
use crate::db::repository::Repository;
use axum::{
    extract::{Path, Query, State},
    response::IntoResponse,
    Json,
};
use chrono::Utc;
use std::path::PathBuf;
use super::AppState;

/// Handler for GET /api/v1/tasks - List all tasks
pub async fn list_tasks(
    State(state): State<AppState>,
    Query(query): Query<TasksQuery>,
) -> Result<impl IntoResponse> {
    let (task_records, _total) = state
        .task_queue
        .list_tasks_with_filters(
            query.status,
            query.page,
            query.page_size,
            query.sort_by,
            query.sort_order,
        )
        .await?;

    let tasks: Vec<TaskInfoResponse> = task_records
        .into_iter()
        .map(|record| TaskInfoResponse {
            id: record.id,
            task_type: record.task_type,
            status: record.status,
            payload: record.payload,
            message: record.message,
            error: record.error,
            retries: record.retries,
            max_retries: record.max_retries,
            created_at: record.created_at,
            started_at: None, // TODO: Add started_at to TaskRecord
            finished_at: None, // TODO: Add finished_at to TaskRecord
        })
        .collect();

    Ok(Json(tasks))
}

/// Handler for GET /api/v1/tasks/:id - Get task details
pub async fn get_task(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse> {
    let task_record = state.task_queue.get_task(&id).await?;

    let payload = if let Some(ref payload_str) = task_record.payload {
        serde_json::from_str(payload_str).ok()
    } else {
        None
    };

    let result = if let Some(ref message_str) = task_record.message {
        serde_json::from_str(message_str).ok()
    } else {
        None
    };

    let response = TaskDetailResponse {
        id: task_record.id,
        task_type: task_record.task_type,
        status: task_record.status,
        payload,
        message: task_record.message,
        result,
        error: task_record.error,
        retries: task_record.retries,
        max_retries: task_record.max_retries,
        created_at: task_record.created_at,
        started_at: None, 
        finished_at: None,
    };

    Ok(Json(response))
}

/// Handler for POST /api/v1/tasks/:id/cancel - Cancel a task
pub async fn cancel_task(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse> {
    state.task_queue.cancel(&id).await?;

    Ok(Json(CancelTaskResponse {
        message: format!("Task {} cancelled successfully", id),
    }))
}

/// Health check endpoint
pub async fn health_check(State(state): State<AppState>) -> Result<impl IntoResponse> {
    let db_health = check_database_health(&state).await;
    let plugin_health = check_plugin_system_health(&state).await;

    let overall_status = if db_health.status == ComponentStatus::Healthy
        && plugin_health.status == ComponentStatus::Healthy
    {
        HealthStatus::Healthy
    } else {
        HealthStatus::Unhealthy
    };

    let response = HealthResponse {
        status: overall_status,
        components: ComponentsHealth {
            database: db_health,
            plugin_system: plugin_health,
        },
        timestamp: Utc::now().to_rfc3339(),
    };

    Ok(Json(response))
}

async fn check_database_health(state: &AppState) -> ComponentHealth {
    match state.book_repo.find_all().await {
        Ok(_) => {
            ComponentHealth {
                status: ComponentStatus::Healthy,
                message: Some("Database is operational".to_string()),
                details: Some(serde_json::json!({
                    "status": "connected",
                })),
            }
        }
        Err(e) => {
            ComponentHealth {
                status: ComponentStatus::Unhealthy,
                message: Some(format!("Database error: {}", e)),
                details: None,
            }
        }
    }
}

async fn check_plugin_system_health(state: &AppState) -> ComponentHealth {
    use crate::plugin::types::PluginState;

    let plugins = state.plugin_manager.list_plugins().await;
    let total_plugins = plugins.len();
    
    let active_plugins = plugins.iter()
        .filter(|p| matches!(p.state, PluginState::Active))
        .count();
    
    let failed_plugins = plugins.iter()
        .filter(|p| matches!(p.state, PluginState::Failed))
        .count();

    let status = if failed_plugins == 0 {
        ComponentStatus::Healthy
    } else {
        ComponentStatus::Unhealthy
    };

    let message = if failed_plugins > 0 {
        Some(format!("{} plugin(s) in failed state", failed_plugins))
    } else {
        Some("Plugin system is operational".to_string())
    };

    ComponentHealth {
        status,
        message,
        details: Some(serde_json::json!({
            "total_plugins": total_plugins,
            "active_plugins": active_plugins,
            "failed_plugins": failed_plugins,
        })),
    }
}

/// Metrics endpoint
pub async fn get_metrics(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
) -> Result<impl IntoResponse> {
    let accept_header = headers
        .get(axum::http::header::ACCEPT)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("application/json");

    let is_prometheus = accept_header.contains("text/plain") 
        || accept_header.contains("application/openmetrics-text");

    let system_metrics = collect_system_metrics(&state);
    let plugin_metrics = collect_plugin_metrics(&state).await;
    let task_queue_metrics = collect_task_queue_metrics(&state).await;
    let database_metrics = collect_database_metrics(&state);

    let timestamp = Utc::now().to_rfc3339();

    if is_prometheus {
        let prometheus_output = format_prometheus_metrics(
            &system_metrics,
            &plugin_metrics,
            &task_queue_metrics,
            &database_metrics,
        );

        Ok((
            [(axum::http::header::CONTENT_TYPE, "text/plain; version=0.0.4")],
            prometheus_output,
        )
            .into_response())
    } else {
        let response = MetricsResponse {
            system: system_metrics,
            plugins: plugin_metrics,
            task_queue: task_queue_metrics,
            database: database_metrics,
            timestamp,
        };

        Ok(Json(response).into_response())
    }
}

fn collect_system_metrics(_state: &AppState) -> SystemMetrics {
    SystemMetrics {
        total_requests: 0,
        avg_response_time_ms: 0.0,
        total_errors: 0,
        error_rate: 0.0,
        uptime_seconds: 0,
    }
}

async fn collect_plugin_metrics(state: &AppState) -> Vec<PluginMetrics> {
    let plugins = state.plugin_manager.list_plugins().await;

    plugins
        .into_iter()
        .map(|info| {
            let success_rate = if info.total_calls > 0 {
                info.successful_calls as f64 / info.total_calls as f64
            } else {
                0.0
            };

            PluginMetrics {
                plugin_id: info.id.clone(),
                plugin_name: info.name.clone(),
                total_calls: info.total_calls,
                successful_calls: info.successful_calls,
                failed_calls: info.failed_calls,
                success_rate,
                min_execution_time_ms: None,
                max_execution_time_ms: None,
                avg_execution_time_ms: None,
                p95_execution_time_ms: None,
                memory_usage_bytes: None,
                peak_memory_bytes: None,
                error_distribution: std::collections::HashMap::new(),
            }
        })
        .collect()
}

async fn collect_task_queue_metrics(state: &AppState) -> TaskQueueMetrics {
    let all_tasks = state.task_queue.list_tasks().await.unwrap_or_default();

    let queued_tasks = all_tasks.iter().filter(|t| t.status == "queued").count();
    let running_tasks = all_tasks.iter().filter(|t| t.status == "running").count();
    let completed_tasks = all_tasks.iter().filter(|t| t.status == "completed").count();
    let failed_tasks = all_tasks.iter().filter(|t| t.status == "failed").count();
    let cancelled_tasks = all_tasks.iter().filter(|t| t.status == "cancelled").count();
    let total_tasks = all_tasks.len();

    let failure_rate = if total_tasks > 0 {
        failed_tasks as f64 / total_tasks as f64
    } else {
        0.0
    };

    TaskQueueMetrics {
        queued_tasks,
        running_tasks,
        completed_tasks,
        failed_tasks,
        cancelled_tasks,
        total_tasks,
        avg_processing_time_ms: 0.0,
        failure_rate,
    }
}

fn collect_database_metrics(_state: &AppState) -> DatabaseMetrics {
    DatabaseMetrics {
        active_connections: 0,
        idle_connections: 0,
        total_queries: 0,
        avg_query_time_ms: 0.0,
    }
}

fn format_prometheus_metrics(
    system: &SystemMetrics,
    plugins: &[PluginMetrics],
    task_queue: &TaskQueueMetrics,
    database: &DatabaseMetrics,
) -> String {
    let mut output = String::new();

    // System metrics
    output.push_str("# HELP ting_reader_requests_total Total number of HTTP requests\n");
    output.push_str("# TYPE ting_reader_requests_total counter\n");
    output.push_str(&format!("ting_reader_requests_total {}\n", system.total_requests));
    output.push_str("\n");

    output.push_str("# HELP ting_reader_response_time_ms Average response time in milliseconds\n");
    output.push_str("# TYPE ting_reader_response_time_ms gauge\n");
    output.push_str(&format!("ting_reader_response_time_ms {}\n", system.avg_response_time_ms));
    output.push_str("\n");

    output.push_str("# HELP ting_reader_errors_total Total number of errors\n");
    output.push_str("# TYPE ting_reader_errors_total counter\n");
    output.push_str(&format!("ting_reader_errors_total {}\n", system.total_errors));
    output.push_str("\n");

    output.push_str("# HELP ting_reader_error_rate Error rate (errors / total requests)\n");
    output.push_str("# TYPE ting_reader_error_rate gauge\n");
    output.push_str(&format!("ting_reader_error_rate {}\n", system.error_rate));
    output.push_str("\n");

    output.push_str("# HELP ting_reader_uptime_seconds System uptime in seconds\n");
    output.push_str("# TYPE ting_reader_uptime_seconds counter\n");
    output.push_str(&format!("ting_reader_uptime_seconds {}\n", system.uptime_seconds));
    output.push_str("\n");

    // Plugin metrics
    output.push_str("# HELP ting_reader_plugin_calls_total Total number of plugin calls\n");
    output.push_str("# TYPE ting_reader_plugin_calls_total counter\n");
    for plugin in plugins {
        output.push_str(&format!(
            "ting_reader_plugin_calls_total{{plugin_id=\"{}\",plugin_name=\"{}\"}} {}\n",
            plugin.plugin_id, plugin.plugin_name, plugin.total_calls
        ));
    }
    output.push_str("\n");

    output.push_str("# HELP ting_reader_plugin_success_rate Plugin success rate\n");
    output.push_str("# TYPE ting_reader_plugin_success_rate gauge\n");
    for plugin in plugins {
        output.push_str(&format!(
            "ting_reader_plugin_success_rate{{plugin_id=\"{}\",plugin_name=\"{}\"}} {}\n",
            plugin.plugin_id, plugin.plugin_name, plugin.success_rate
        ));
    }
    output.push_str("\n");

    output.push_str("# HELP ting_reader_plugin_execution_time_ms Plugin execution time in milliseconds\n");
    output.push_str("# TYPE ting_reader_plugin_execution_time_ms summary\n");
    for plugin in plugins {
        if let Some(min) = plugin.min_execution_time_ms {
            output.push_str(&format!(
                "ting_reader_plugin_execution_time_ms{{plugin_id=\"{}\",plugin_name=\"{}\",quantile=\"0.0\"}} {}\n",
                plugin.plugin_id, plugin.plugin_name, min
            ));
        }
        if let Some(avg) = plugin.avg_execution_time_ms {
            output.push_str(&format!(
                "ting_reader_plugin_execution_time_ms{{plugin_id=\"{}\",plugin_name=\"{}\",quantile=\"0.5\"}} {}\n",
                plugin.plugin_id, plugin.plugin_name, avg
            ));
        }
        if let Some(p95) = plugin.p95_execution_time_ms {
            output.push_str(&format!(
                "ting_reader_plugin_execution_time_ms{{plugin_id=\"{}\",plugin_name=\"{}\",quantile=\"0.95\"}} {}\n",
                plugin.plugin_id, plugin.plugin_name, p95
            ));
        }
        if let Some(max) = plugin.max_execution_time_ms {
            output.push_str(&format!(
                "ting_reader_plugin_execution_time_ms{{plugin_id=\"{}\",plugin_name=\"{}\",quantile=\"1.0\"}} {}\n",
                plugin.plugin_id, plugin.plugin_name, max
            ));
        }
    }
    output.push_str("\n");

    output.push_str("# HELP ting_reader_plugin_memory_bytes Plugin memory usage in bytes\n");
    output.push_str("# TYPE ting_reader_plugin_memory_bytes gauge\n");
    for plugin in plugins {
        if let Some(memory) = plugin.memory_usage_bytes {
            output.push_str(&format!(
                "ting_reader_plugin_memory_bytes{{plugin_id=\"{}\",plugin_name=\"{}\"}} {}\n",
                plugin.plugin_id, plugin.plugin_name, memory
            ));
        }
    }
    output.push_str("\n");

    // Task queue metrics
    output.push_str("# HELP ting_reader_tasks_total Total number of tasks by status\n");
    output.push_str("# TYPE ting_reader_tasks_total gauge\n");
    output.push_str(&format!("ting_reader_tasks_total{{status=\"queued\"}} {}\n", task_queue.queued_tasks));
    output.push_str(&format!("ting_reader_tasks_total{{status=\"running\"}} {}\n", task_queue.running_tasks));
    output.push_str(&format!("ting_reader_tasks_total{{status=\"completed\"}} {}\n", task_queue.completed_tasks));
    output.push_str(&format!("ting_reader_tasks_total{{status=\"failed\"}} {}\n", task_queue.failed_tasks));
    output.push_str(&format!("ting_reader_tasks_total{{status=\"cancelled\"}} {}\n", task_queue.cancelled_tasks));
    output.push_str("\n");

    output.push_str("# HELP ting_reader_task_failure_rate Task failure rate\n");
    output.push_str("# TYPE ting_reader_task_failure_rate gauge\n");
    output.push_str(&format!("ting_reader_task_failure_rate {}\n", task_queue.failure_rate));
    output.push_str("\n");

    output.push_str("# HELP ting_reader_task_processing_time_ms Average task processing time in milliseconds\n");
    output.push_str("# TYPE ting_reader_task_processing_time_ms gauge\n");
    output.push_str(&format!("ting_reader_task_processing_time_ms {}\n", task_queue.avg_processing_time_ms));
    output.push_str("\n");

    // Database metrics
    output.push_str("# HELP ting_reader_db_connections Database connections\n");
    output.push_str("# TYPE ting_reader_db_connections gauge\n");
    output.push_str(&format!("ting_reader_db_connections{{state=\"active\"}} {}\n", database.active_connections));
    output.push_str(&format!("ting_reader_db_connections{{state=\"idle\"}} {}\n", database.idle_connections));
    output.push_str("\n");

    output.push_str("# HELP ting_reader_db_queries_total Total number of database queries\n");
    output.push_str("# TYPE ting_reader_db_queries_total counter\n");
    output.push_str(&format!("ting_reader_db_queries_total {}\n", database.total_queries));
    output.push_str("\n");

    output.push_str("# HELP ting_reader_db_query_time_ms Average database query time in milliseconds\n");
    output.push_str("# TYPE ting_reader_db_query_time_ms gauge\n");
    output.push_str(&format!("ting_reader_db_query_time_ms {}\n", database.avg_query_time_ms));
    output.push_str("\n");

    output
}

/// Handler for GET /api/v1/config - Get system configuration
pub async fn get_config(State(state): State<AppState>) -> Result<impl IntoResponse> {
    let config = state.config.read().await;

    let api_key_masked = if config.security.api_key.is_empty() {
        String::new()
    } else {
        "***".to_string()
    };

    let response = ConfigResponse {
        server: ServerConfigResponse {
            host: config.server.host.clone(),
            port: config.server.port,
            max_connections: config.server.max_connections,
            request_timeout: config.server.request_timeout,
        },
        database: DatabaseConfigResponse {
            path: config.database.path.display().to_string(),
            connection_pool_size: config.database.connection_pool_size,
            busy_timeout: config.database.busy_timeout,
        },
        plugins: PluginSystemConfigResponse {
            plugin_dir: config.plugins.plugin_dir.display().to_string(),
            enable_hot_reload: config.plugins.enable_hot_reload,
            max_memory_per_plugin: config.plugins.max_memory_per_plugin,
            max_execution_time: config.plugins.max_execution_time,
        },
        task_queue: TaskQueueConfigResponse {
            max_concurrent_tasks: config.task_queue.max_concurrent_tasks,
            default_retry_count: config.task_queue.default_retry_count,
            task_timeout: config.task_queue.task_timeout,
        },
        logging: LoggingConfigResponse {
            level: config.logging.level.clone(),
            format: config.logging.format.clone(),
            output: config.logging.output.clone(),
            log_file: config.logging.log_file.as_ref().map(|p| p.display().to_string()),
            max_file_size: config.logging.max_file_size,
            max_backups: config.logging.max_backups,
        },
        security: SecurityConfigResponse {
            enable_auth: config.security.enable_auth,
            api_key: Some(api_key_masked),
            allowed_origins: config.security.allowed_origins.clone(),
            rate_limit_requests: config.security.rate_limit_requests,
            rate_limit_window: config.security.rate_limit_window,
            enable_hsts: config.security.enable_hsts,
            hsts_max_age: config.security.hsts_max_age,
        },
        storage: StorageConfigResponse {
            data_dir: config.storage.data_dir.display().to_string(),
            temp_dir: config.storage.temp_dir.display().to_string(),
            local_storage_root: config.storage.local_storage_root.display().to_string(),
            max_disk_usage: config.storage.max_disk_usage,
        },
    };

    Ok(Json(response))
}

/// Handler for PUT /api/v1/config - Update system configuration
pub async fn update_config(
    State(state): State<AppState>,
    Json(req): Json<UpdateConfigRequest>,
) -> Result<impl IntoResponse> {
    let original_config = state.config.read().await.clone();
    let mut new_config = original_config.clone();

    let mut updated_fields = Vec::new();
    let mut requires_restart = Vec::new();

    if let Some(server_update) = req.server {
        if let Some(host) = server_update.host {
            new_config.server.host = host;
            updated_fields.push("server.host".to_string());
            requires_restart.push("server.host".to_string());
        }
        if let Some(port) = server_update.port {
            new_config.server.port = port;
            updated_fields.push("server.port".to_string());
            requires_restart.push("server.port".to_string());
        }
        if let Some(max_connections) = server_update.max_connections {
            new_config.server.max_connections = max_connections;
            updated_fields.push("server.max_connections".to_string());
            requires_restart.push("server.max_connections".to_string());
        }
        if let Some(request_timeout) = server_update.request_timeout {
            new_config.server.request_timeout = request_timeout;
            updated_fields.push("server.request_timeout".to_string());
            requires_restart.push("server.request_timeout".to_string());
        }
    }

    if let Some(database_update) = req.database {
        if let Some(path) = database_update.path {
            new_config.database.path = PathBuf::from(path);
            updated_fields.push("database.path".to_string());
            requires_restart.push("database.path".to_string());
        }
        if let Some(connection_pool_size) = database_update.connection_pool_size {
            new_config.database.connection_pool_size = connection_pool_size;
            updated_fields.push("database.connection_pool_size".to_string());
            requires_restart.push("database.connection_pool_size".to_string());
        }
        if let Some(busy_timeout) = database_update.busy_timeout {
            new_config.database.busy_timeout = busy_timeout;
            updated_fields.push("database.busy_timeout".to_string());
            requires_restart.push("database.busy_timeout".to_string());
        }
    }

    if let Some(plugins_update) = req.plugins {
        if let Some(plugin_dir) = plugins_update.plugin_dir {
            new_config.plugins.plugin_dir = PathBuf::from(plugin_dir);
            updated_fields.push("plugins.plugin_dir".to_string());
            requires_restart.push("plugins.plugin_dir".to_string());
        }
        if let Some(enable_hot_reload) = plugins_update.enable_hot_reload {
            new_config.plugins.enable_hot_reload = enable_hot_reload;
            updated_fields.push("plugins.enable_hot_reload".to_string());
        }
        if let Some(max_memory_per_plugin) = plugins_update.max_memory_per_plugin {
            new_config.plugins.max_memory_per_plugin = max_memory_per_plugin;
            updated_fields.push("plugins.max_memory_per_plugin".to_string());
        }
        if let Some(max_execution_time) = plugins_update.max_execution_time {
            new_config.plugins.max_execution_time = max_execution_time;
            updated_fields.push("plugins.max_execution_time".to_string());
        }
    }

    if let Some(task_queue_update) = req.task_queue {
        if let Some(max_concurrent_tasks) = task_queue_update.max_concurrent_tasks {
            new_config.task_queue.max_concurrent_tasks = max_concurrent_tasks;
            updated_fields.push("task_queue.max_concurrent_tasks".to_string());
        }
        if let Some(default_retry_count) = task_queue_update.default_retry_count {
            new_config.task_queue.default_retry_count = default_retry_count;
            updated_fields.push("task_queue.default_retry_count".to_string());
        }
        if let Some(task_timeout) = task_queue_update.task_timeout {
            new_config.task_queue.task_timeout = task_timeout;
            updated_fields.push("task_queue.task_timeout".to_string());
        }
    }

    if let Some(logging_update) = req.logging {
        if let Some(level) = logging_update.level {
            new_config.logging.level = level;
            updated_fields.push("logging.level".to_string());
        }
        if let Some(format) = logging_update.format {
            new_config.logging.format = format;
            updated_fields.push("logging.format".to_string());
            requires_restart.push("logging.format".to_string());
        }
        if let Some(output) = logging_update.output {
            new_config.logging.output = output;
            updated_fields.push("logging.output".to_string());
            requires_restart.push("logging.output".to_string());
        }
        if let Some(log_file) = logging_update.log_file {
            new_config.logging.log_file = Some(PathBuf::from(log_file));
            updated_fields.push("logging.log_file".to_string());
            requires_restart.push("logging.log_file".to_string());
        }
        if let Some(max_file_size) = logging_update.max_file_size {
            new_config.logging.max_file_size = max_file_size;
            updated_fields.push("logging.max_file_size".to_string());
            requires_restart.push("logging.max_file_size".to_string());
        }
        if let Some(max_backups) = logging_update.max_backups {
            new_config.logging.max_backups = max_backups;
            updated_fields.push("logging.max_backups".to_string());
            requires_restart.push("logging.max_backups".to_string());
        }
    }

    if let Some(security_update) = req.security {
        if let Some(enable_auth) = security_update.enable_auth {
            new_config.security.enable_auth = enable_auth;
            updated_fields.push("security.enable_auth".to_string());
            requires_restart.push("security.enable_auth".to_string());
        }
        if let Some(api_key) = security_update.api_key {
            new_config.security.api_key = api_key;
            updated_fields.push("security.api_key".to_string());
            requires_restart.push("security.api_key".to_string());
        }
        if let Some(allowed_origins) = security_update.allowed_origins {
            new_config.security.allowed_origins = allowed_origins;
            updated_fields.push("security.allowed_origins".to_string());
            requires_restart.push("security.allowed_origins".to_string());
        }
        if let Some(rate_limit_requests) = security_update.rate_limit_requests {
            new_config.security.rate_limit_requests = rate_limit_requests;
            updated_fields.push("security.rate_limit_requests".to_string());
        }
        if let Some(rate_limit_window) = security_update.rate_limit_window {
            new_config.security.rate_limit_window = rate_limit_window;
            updated_fields.push("security.rate_limit_window".to_string());
        }
        if let Some(enable_hsts) = security_update.enable_hsts {
            new_config.security.enable_hsts = enable_hsts;
            updated_fields.push("security.enable_hsts".to_string());
            requires_restart.push("security.enable_hsts".to_string());
        }
        if let Some(hsts_max_age) = security_update.hsts_max_age {
            new_config.security.hsts_max_age = hsts_max_age;
            updated_fields.push("security.hsts_max_age".to_string());
            requires_restart.push("security.hsts_max_age".to_string());
        }
    }

    if let Some(storage_update) = req.storage {
        if let Some(data_dir) = storage_update.data_dir {
            new_config.storage.data_dir = PathBuf::from(data_dir);
            updated_fields.push("storage.data_dir".to_string());
            requires_restart.push("storage.data_dir".to_string());
        }
        if let Some(temp_dir) = storage_update.temp_dir {
            new_config.storage.temp_dir = PathBuf::from(temp_dir);
            updated_fields.push("storage.temp_dir".to_string());
            requires_restart.push("storage.temp_dir".to_string());
        }
        if let Some(max_disk_usage) = storage_update.max_disk_usage {
            new_config.storage.max_disk_usage = max_disk_usage;
            updated_fields.push("storage.max_disk_usage".to_string());
            requires_restart.push("storage.max_disk_usage".to_string());
        }
    }

    new_config.validate().map_err(|e| {
        tracing::error!(error = %e, "Configuration validation failed");
        TingError::InvalidRequest(format!("Invalid configuration: {}", e))
    })?;

    let mut config = state.config.write().await;
    *config = new_config;
    drop(config);

    let message = if requires_restart.is_empty() {
        "Configuration updated successfully. Changes are now in effect.".to_string()
    } else {
        format!(
            "Configuration updated successfully. {} parameter(s) require system restart to take effect.",
            requires_restart.len()
        )
    };

    Ok(Json(UpdateConfigResponse {
        message,
        updated_fields,
        requires_restart,
    }))
}
