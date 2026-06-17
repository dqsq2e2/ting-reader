# 系统管理

除“公共接口”外，本页接口均需要登录；系统管理接口通常需要管理员权限。

## 公共接口

### GET /api/health

健康检查，无需认证。

响应：`200 OK`

```json
{
  "status": "healthy",
  "components": {
    "database": {
      "status": "healthy",
      "message": "Database is operational",
      "details": {
        "status": "connected"
      }
    },
    "plugin_system": {
      "status": "healthy",
      "message": "Plugin system is operational",
      "details": {
        "total_plugins": 0,
        "active_plugins": 0,
        "failed_plugins": 0
      }
    }
  },
  "timestamp": "RFC3339",
  "version": "1.4.2"
}
```

### GET /api/stats

公共馆藏统计，无需认证。

响应：`200 OK`

```json
{
  "total_books": 0,
  "total_chapters": 0,
  "total_duration": 0,
  "last_scan_time": "RFC3339 | null"
}
```

## 管理员统计报表

### GET /api/system/statistics

获取后台数据统计报表。普通用户不可访问。

响应：`200 OK`

```json
{
  "overview": {
    "total_books": 0,
    "total_chapters": 0,
    "total_duration": 0,
    "total_libraries": 0,
    "local_libraries": 0,
    "webdav_libraries": 0,
    "total_users": 0,
    "admin_users": 0,
    "active_users": 0,
    "total_progress_records": 0,
    "total_listen_seconds": 0.0
  },
  "library_breakdown": [
    {
      "id": "string",
      "name": "string",
      "library_type": "local | webdav",
      "total_books": 0,
      "total_chapters": 0,
      "total_duration": 0,
      "last_scanned_at": "RFC3339 | null"
    }
  ],
  "user_activity": [
    {
      "id": "string",
      "username": "string",
      "role": "admin | user",
      "listened_books": 0,
      "progress_records": 0,
      "listen_seconds": 0.0,
      "last_active_at": "RFC3339 | null"
    }
  ],
  "recent_activity": [
    {
      "date": "YYYY-MM-DD",
      "active_users": 0,
      "progress_updates": 0,
      "listen_seconds": 0.0
    }
  ],
  "top_books": [
    {
      "id": "string",
      "title": "string | null",
      "author": "string | null",
      "library_id": "string",
      "library_name": "string | null",
      "listeners": 0,
      "progress_updates": 0,
      "listen_seconds": 0.0
    }
  ],
  "generated_at": "RFC3339"
}
```

说明：

- 统计使用 `listening_events`，用户清空最近收听不会影响后台历史统计。
- `recent_activity` 默认返回最近 14 天有记录的活动点。
- `top_books` 当前最多返回 8 条热门作品。

## 系统指标

### GET /api/system/metrics

获取系统指标。支持 JSON 和 Prometheus 文本格式。

请求头：

| Header | 说明 |
| --- | --- |
| `Accept` | `application/json` 或 `text/plain` |

响应：`200 OK`

```json
{
  "system": {
    "total_requests": 0,
    "avg_response_time_ms": 0.0,
    "total_errors": 0,
    "error_rate": 0.0,
    "uptime_seconds": 0
  },
  "plugins": [
    {
      "plugin_id": "string",
      "plugin_name": "string",
      "total_calls": 0,
      "successful_calls": 0,
      "failed_calls": 0,
      "success_rate": 0.0,
      "min_execution_time_ms": null,
      "max_execution_time_ms": null,
      "avg_execution_time_ms": null,
      "p95_execution_time_ms": null,
      "memory_usage_bytes": null,
      "peak_memory_bytes": null,
      "error_distribution": {}
    }
  ],
  "task_queue": {
    "queued_tasks": 0,
    "running_tasks": 0,
    "completed_tasks": 0,
    "failed_tasks": 0,
    "cancelled_tasks": 0,
    "total_tasks": 0,
    "avg_processing_time_ms": 0.0,
    "failure_rate": 0.0
  },
  "database": {
    "active_connections": 0,
    "idle_connections": 0,
    "total_queries": 0,
    "avg_query_time_ms": 0.0
  },
  "timestamp": "RFC3339"
}
```

## 系统配置

### GET /api/system/config

获取系统配置。

响应：`200 OK`

```json
{
  "server": {
    "host": "0.0.0.0",
    "port": 3000,
    "max_connections": 100,
    "request_timeout": 30
  },
  "database": {
    "path": "string",
    "connection_pool_size": 5,
    "busy_timeout": 5000
  },
  "plugins": {
    "plugin_dir": "string",
    "enable_hot_reload": false,
    "max_memory_per_plugin": 52428800,
    "max_execution_time": 30000
  },
  "task_queue": {
    "max_concurrent_tasks": 2,
    "default_retry_count": 3,
    "task_timeout": 3600
  },
  "logging": {
    "level": "info",
    "format": "text | json",
    "output": "stdout | file",
    "log_file": "string | null",
    "max_file_size": 10485760,
    "max_backups": 5
  },
  "security": {
    "enable_auth": true,
    "api_key": "***",
    "allowed_origins": ["*"],
    "rate_limit_requests": 100,
    "rate_limit_window": 60,
    "enable_hsts": false,
    "hsts_max_age": 31536000
  },
  "storage": {
    "data_dir": "string",
    "temp_dir": "string",
    "local_storage_root": "string",
    "max_disk_usage": 50
  }
}
```

### PUT /api/system/config

更新系统配置。请求体中所有字段均可选。

请求体示例：

```json
{
  "server": {
    "host": "0.0.0.0",
    "port": 3000
  },
  "security": {
    "enable_auth": true,
    "api_key": "new-key"
  }
}
```

响应：`200 OK`

```json
{
  "message": "Configuration updated successfully. 2 parameter(s) require system restart to take effect.",
  "updated_fields": ["server.host", "server.port"],
  "requires_restart": ["server.host", "server.port"]
}
```

## 系统更新

### GET /api/system/check-update

检查服务端更新。

响应：`200 OK`

返回更新服务的原始 JSON，例如：

```json
{
  "version": "v1.4.3",
  "downloadUrl": "https://...",
  "size": "string",
  "date": "RFC3339"
}
```

## 系统日志

### GET /api/system/logs

获取系统日志与任务日志。

查询参数：

| 参数 | 类型 | 说明 |
| --- | --- | --- |
| `level` | string | 日志级别过滤，如 `INFO`、`WARN`、`ERROR` |
| `module` | string | 模块过滤，如 `audit`、`audit::login`、`audit::playback`、`audit::scan`、`audit::notification`、`all` |
| `page` | number | 页码，默认 `1` |
| `page_size` | number | 每页数量，默认 `50` |

响应：`200 OK`

```json
{
  "logs": [
    {
      "timestamp": "RFC3339",
      "level": "INFO",
      "module": "audit::login",
      "message": "用户 'admin' 登录成功",
      "fields": {
        "user_id": "string",
        "username": "admin",
        "real_ip": "127.0.0.1",
        "user_agent": "Mozilla/5.0 ...",
        "device": "Windows / Chrome"
      },
      "task_id": "string | null",
      "task_status": "queued | running | completed | failed | cancelled | null",
      "task_type": "string | null"
    }
  ],
  "total": 0,
  "page": 1,
  "page_size": 50
}
```

说明：

- 登录日志会记录 `real_ip`、`user_agent`、`device`。
- 扫描完成日志会记录媒体库 ID、名称、类型、路径、新增/更新/删除数量。
- `fields` 为结构化日志字段，不含 `message`。

### DELETE /api/system/logs

清空系统日志文件。

响应：`200 OK`

```json
{
  "message": "System logs cleared successfully"
}
```

### GET /api/system/logs/export

导出系统日志文本。

查询参数：

| 参数 | 类型 | 说明 |
| --- | --- | --- |
| `level` | string | 可选，仅导出指定级别 |

响应：`200 OK`，`text/plain` 文件下载。

## 通知与事件

Webhook 通知管理接口见 [notifications.md](notifications.md)。
