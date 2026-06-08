# 插件管理

## 已安装插件

### GET /api/v1/plugins

获取已安装插件列表。

**响应：** `200 OK`

```json
[
  {
    "id": "string",
    "name": "string",
    "version": "string",
    "plugin_type": "scraper | format | utility",
    "runtime": "wasm | javascript | native | null",
    "author": "string | null",
    "description": "string | null",
    "is_enabled": true,
    "state": "loading | loaded | active | unloading | unloaded | failed",
    "stats": {
      "total_calls": 0,
      "successful_calls": 0,
      "failed_calls": 0,
      "avg_execution_time_ms": 0.0
    },
    "config_schema": {},
    "permissions": ["string"],
    "license": "string | null",
    "repo": "string | null"
  }
]
```

---

### GET /api/v1/plugins/:id

获取插件详情。

**路径参数：**

| 参数 | 类型 | 说明 |
|------|------|------|
| id | string | 插件 ID |

**响应：** `200 OK`

```json
{
  "id": "string",
  "name": "string",
  "version": "string",
  "plugin_type": "scraper | format | utility",
  "runtime": "string | null",
  "author": "string | null",
  "description": "string | null",
  "license": "string | null",
  "repo": "string | null",
  "is_enabled": true,
  "state": "string",
  "entry_point": "string",
  "dependencies": [
    {
      "plugin_name": "string",
      "version_requirement": "string"
    }
  ],
  "permissions": ["string"],
  "supported_extensions": ["string"],
  "config_schema": {},
  "stats": {
    "total_calls": 0,
    "successful_calls": 0,
    "failed_calls": 0,
    "avg_execution_time_ms": 0.0
  }
}
```

---

### POST /api/v1/plugins/install

上传安装插件（multipart/form-data，最大 50MB）。

**请求：** `multipart/form-data`，字段名 `file`，值为 `.zip` 插件包。

**响应：** `201 Created`

```json
{
  "plugin_id": "string",
  "message": "Plugin xxx installed successfully"
}
```

---

### DELETE /api/v1/plugins/:id

卸载插件。

**路径参数：**

| 参数 | 类型 | 说明 |
|------|------|------|
| id | string | 插件 ID |

**响应：** `200 OK`

```json
{
  "message": "Plugin xxx uninstalled successfully"
}
```

---

### POST /api/v1/plugins/:id/reload

重新加载插件。

**路径参数：**

| 参数 | 类型 | 说明 |
|------|------|------|
| id | string | 插件 ID |

**响应：** `200 OK`

```json
{
  "message": "Plugin xxx reloaded successfully"
}
```

---

## 插件配置

### GET /api/v1/plugins/:id/config

获取插件配置。

**路径参数：**

| 参数 | 类型 | 说明 |
|------|------|------|
| id | string | 插件 ID |

**响应：** `200 OK`

```json
{
  "plugin_id": "string",
  "config": {}
}
```

---

### PUT /api/v1/plugins/:id/config

更新插件配置。

**路径参数：**

| 参数 | 类型 | 说明 |
|------|------|------|
| id | string | 插件 ID |

**请求体：**

```json
{
  "config": {}
}
```

**响应：** `200 OK`

```json
{
  "message": "Plugin xxx configuration updated successfully"
}
```

---

## 插件商店

### GET /api/v1/store/plugins

获取商店中的插件列表。

**响应：** `200 OK` — 返回商店插件列表

---

### POST /api/v1/store/install

从商店安装插件。

**请求体：**

```json
{
  "plugin_id": "string"
}
```

**响应：** `201 Created`

```json
{
  "plugin_id": "string",
  "message": "Plugin xxx installed successfully from store"
}
```

---

### POST /api/v1/store/cache/clear

清除插件商店缓存。

**响应：** `200 OK`

```json
{
  "message": "Plugin cache cleared successfully"
}
```
