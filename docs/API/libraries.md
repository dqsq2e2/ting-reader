# 媒体库管理

## GET /api/libraries

获取所有媒体库。管理员获取全部，普通用户仅获取有权限的。

**响应：** `200 OK`

```json
[
  {
    "id": "string",
    "name": "string",
    "libraryType": "local | webdav",
    "url": "string",
    "username": "string | null",
    "rootPath": "string",
    "lastScannedAt": "RFC3339 | null",
    "createdAt": "RFC3339",
    "scraperConfig": {}
  }
]
```

---

## POST /api/libraries

创建媒体库（管理员）。

**请求体：**

```json
{
  "name": "string",
  "library_type": "local | webdav",
  "path": "string (本地库路径，可选)",
  "webdav_url": "string (WebDAV 地址，可选)",
  "webdav_username": "string (可选)",
  "webdav_password": "string (可选)",
  "description": "string (可选)",
  "enabled": true,
  "root_path": "string (可选，默认 /)",
  "scraper_config": {}
}
```

**ScraperConfig 结构：**

```json
{
  "default_sources": ["string"],
  "cover_sources": ["string"],
  "intro_sources": ["string"],
  "author_sources": ["string"],
  "narrator_sources": ["string"],
  "tags_sources": ["string"],
  "nfo_writing_enabled": false,
  "metadata_writing_enabled": false,
  "prefer_audio_title": false,
  "metadata_priority": ["string"],
  "extract_audio_cover": false,
  "disable_watcher": false
}
```

**响应：** `201 Created` — 返回 `LibraryResponse`

---

## PATCH /api/libraries/:id

更新媒体库（管理员）。

**路径参数：**

| 参数 | 类型 | 说明 |
|------|------|------|
| id | string | 媒体库 ID |

**请求体：** 与创建请求相同，所有字段可选。

**响应：** `200 OK` — 返回 `LibraryResponse`

---

## DELETE /api/libraries/:id

删除媒体库（管理员）。会自动清理关联的缓存封面和监听器。

**路径参数：**

| 参数 | 类型 | 说明 |
|------|------|------|
| id | string | 媒体库 ID |

**响应：** `200 OK`

```json
{
  "success": true,
  "message": "Library deleted successfully"
}
```

---

## POST /api/libraries/:id/scan

触发媒体库扫描（管理员，异步任务）。

**路径参数：**

| 参数 | 类型 | 说明 |
|------|------|------|
| id | string | 媒体库 ID |

**响应：** `202 Accepted`

```json
{
  "task_id": "string",
  "status": "queued",
  "message": "Library scan started for '...'"
}
```

---

## POST /api/libraries/test-connection

测试 WebDAV 连接（管理员）。

**请求体：**

```json
{
  "url": "string",
  "username": "string (可选)",
  "password": "string (可选)",
  "root_path": "string (可选)"
}
```

**响应：** `200 OK`

```json
{
  "success": true,
  "message": "连接成功 (使用 PROPFIND 方法)"
}
```

---

## GET /api/storage/folders

获取存储目录列表（管理员）。

**查询参数：**

| 参数 | 类型 | 说明 |
|------|------|------|
| subPath | string | 子路径（可选） |

**响应：** `200 OK`

```json
[
  {
    "name": "string",
    "path": "string",
    "isDirectory": true
  }
]
```
