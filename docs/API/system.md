# 系统管理

## 公共接口

### GET /api/health

健康检查（无需认证）。

**响应：** `200 OK`

```json
{
  "status": "healthy | unhealthy",
  "components": {
    "database": {
      "status": "healthy | unhealthy",
      "message": "string",
      "details": {}
    },
    "plugin_system": {
      "status": "healthy | unhealthy",
      "message": "string",
      "details": {
        "total_plugins": 0,
        "active_plugins": 0,
        "failed_plugins": 0
      }
    }
  },
  "timestamp": "RFC3339",
  "version": "string"
}
```

---

### GET /api/stats

获取系统统计信息（无需认证）。

**响应：** `200 OK`

```json
{
  "total_books": 0,
  "total_chapters": 0,
  "total_duration": 0,
  "last_scan_time": "RFC3339 | null"
}
```

---

## 系统指标

### GET /api/v1/system/metrics

获取系统指标。支持 JSON 和 Prometheus 格式。

**请求头：**

| Header | 说明 |
|--------|------|
| `Accept` | `application/json`（默认）或 `text/plain`（Prometheus 格式） |

**响应（JSON）：** `200 OK`

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

---

## 系统配置

### GET /api/v1/system/config

获取系统配置。

**响应：** `200 OK`

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
    "format": "text",
    "output": "stdout",
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

---

### PUT /api/v1/system/config

更新系统配置。

**请求体：** 与响应结构相同，所有字段均为可选。

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

**响应：** `200 OK`

```json
{
  "message": "Configuration updated successfully. 2 parameter(s) require system restart to take effect.",
  "updated_fields": ["server.host", "server.port"],
  "requires_restart": ["server.host", "server.port"]
}
```

---

## 系统更新

### GET /api/v1/system/check-update

检查系统更新。

**响应：** `200 OK` — 返回更新信息 JSON

---

## 系统日志

### GET /api/v1/system/logs

获取系统日志。

**查询参数：**

| 参数 | 类型 | 说明 |
|------|------|------|
| level | string | 日志级别过滤（可选） |
| search | string | 搜索关键词（可选） |
| page | number | 页码（可选） |
| page_size | number | 每页数量（可选） |

**响应：** `200 OK`

```json
{
  "logs": [],
  "total": 0,
  "page": 1,
  "page_size": 50
}
```

---

### DELETE /api/v1/system/logs

清除系统日志。

**响应：** `200 OK`

```json
{
  "message": "System logs cleared successfully"
}
```

---

### GET /api/v1/system/logs/export

导出系统日志。

**响应：** 日志文件下载
