# 媒体库管理

## GET /api/libraries

获取所有媒体库。管理员获取全部，普通用户仅获取有权限的。

**响应：** `200 OK`

```json
[
  {
    "id": "string",
    "name": "string",
    "library_type": "local | webdav",
    "url": "string",
    "username": "string | null",
    "root_path": "string",
    "last_scanned_at": "RFC3339 | null",
    "created_at": "RFC3339",
    "scraper_config": {}
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

**scraper_config 结构：**

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
  "disable_watcher": false,
  "cloud_mode": false
}
```

**响应：** `201 Created` — 返回 `LibraryResponse`

说明：

- 创建成功后会自动提交一次媒体库扫描任务。
- 如果配置了 Webhook 监听，会触发 `library.created`；扫描完成后会触发 `library.scan_completed`。

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

说明：如果配置了 Webhook 监听，会触发 `library.deleted`。

---

## POST /api/libraries/:id/scan

触发媒体库同步扫描（管理员，异步任务）。

**路径参数：**

| 参数 | 类型 | 说明 |
|------|------|------|
| id | string | 媒体库 ID |

**请求体（可选）：**

```json
{
  "mode": "incremental | full"
}
```

| 字段 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| mode | string | `incremental` | `incremental` 为增量同步，仅处理新增/变更内容；`full` 为全量同步，忽略上次扫描时间并重新检查已有内容。 |

**响应：** `202 Accepted`

```json
{
  "task_id": "string",
  "status": "queued",
  "message": "Incremental scan started for '...'"
}
```

说明：
- 扫描任务完成后会在任务消息和 `audit::scan` 日志中记录媒体库名称、类型、路径、同步模式、新增/更新/删除数量；如果配置了 Webhook 监听，会触发 `library.scan_completed`。
- 扫描时会尝试识别同一父目录下的系列目录。支持 `书名之XX`、`书名第一卷`、`书名第1季`、`书名 S01`、`书名 Vol.1`、`书名 Season 1` 等命名；这些目录本身包含音频文件时会分别作为书籍入库，并自动关联到同一个系列。若目录名包含卷/季编号，会按编号设置系列排序。

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
| sub_path | string | 子路径（可选） |

**响应：** `200 OK`

```json
[
  {
    "name": "string",
    "path": "string",
    "is_directory": true
  }
]
```
