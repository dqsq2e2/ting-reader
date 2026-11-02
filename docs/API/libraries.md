# 媒体库管理

## GET /api/libraries

获取所有媒体库。管理员获取全部，普通用户仅获取有权限的。

**响应：** `200 OK`

```json
[
  {
    "id": "string",
    "name": "string",
    "library_type": "local | webdav | rss",
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
  "library_type": "local | webdav | rss",
  "path": "string (本地库路径，可选；可为授权根内绝对路径或旧版 storage 相对路径)",
  "webdav_url": "string (WebDAV 地址，可选)",
  "rss_feed_url": "string (RSS/Atom 订阅地址，RSS 库必填)",
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
  "webdav_metadata_writing_enabled": false,
  "metadata_priority": ["filename", "local_metadata", "audio_metadata", "scraper"],
  "extract_audio_cover": false,
  "auto_generate_series": true,
  "disable_watcher": false,
  "cloud_mode": false
}
```

**响应：** `201 Created` — 返回 `LibraryResponse`

说明：

- 创建成功后会自动提交一次媒体库扫描任务。
- `metadata_priority` 按从前到后的顺序选择元数据来源。默认依次为文件/文件夹名、本地 JSON/NFO、音频内嵌元数据、刮削器；将 `filename` 后移即可优先采用其他来源的标题。文件夹名用于书名，文件名用于章节名；缺少标题时使用下一来源，最终回退到路径名称。手动修改及章节正则等显式规则仍优先生效。旧版标题开关自动迁移到此列表，不再单独设置。
- 本地和 WebDAV 库的 `auto_generate_series` 默认为 `true`，旧库未配置此字段时也默认开启。此开关只控制根据文件夹名自动创建、关联系列，不影响 `metadata.json` 中系列信息的正常导入。关闭后已有系列及其关联保留，仍可手动管理。修改后在下次扫描生效，无需重新建库。
- WebDAV 的 `webdav_metadata_writing_enabled` 控制是否将后续元数据编辑同步到网盘，默认关闭。开启后同步 `metadata.json`、`book.nfo` 和文件封面；封面 URL 保持引用，不上传图片，也不删除已有网盘封面。
- WebDAV 扫描始终优先使用远端 `metadata.json`、`book.nfo`，并将已有文件和封面下载到服务器 `temp/<书籍路径的 SHA-256>/`。编辑时维护本地副本；手动锁定的元数据不会被后续扫描覆盖。初次扫描不会写入网盘。
- RSS 库上传的封面也保存在 `temp/<哈希>/`，编辑元数据不生成 `metadata.json` 或 NFO。
- 本地媒体库路径会在后端解析为真实绝对路径并保存；未在授权本地根目录内的绝对路径会被拒绝。
- 旧版相对路径仍按 `storage.local_storage_root` 下的子路径解析，兼容已有库和旧客户端。
- 如果启用 NFO 或 metadata 写入，目标目录必须可写；只扫描/播放的只读目录仍可作为媒体库。
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

触发媒体库同步（管理员，异步任务）。本地、WebDAV 和 RSS 库执行文件/订阅扫描；`source` 在线库刷新该书源中已加入书籍的元数据和章节，不执行文件夹扫描。

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
| mode | string | `incremental` | 普通库使用 `incremental` 增量同步或 `full` 全量同步；`source` 在线库忽略该字段，统一刷新已加入书籍。 |

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
- 在线库同步完成后，任务消息使用 `source.library.sync.completed`，记录实际刷新书籍数量。
- 开启“自动生成系列”时，扫描会尝试识别同一父目录下的系列目录。支持 `书名之XX`、`书名第一卷`、`书名第1季`、`书名 S01`、`书名 Vol.1`、`书名 Season 1` 等命名；这些目录本身包含音频文件时会分别作为书籍入库，并自动关联到同一个系列。若目录名包含卷/季编号，会按编号设置系列排序。此开关不影响章节分组目录合并或章节排序。

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

获取本地存储目录列表（管理员）。

**查询参数：**

| 参数 | 类型 | 说明 |
|------|------|------|
| root | string | 授权根目录路径（可选；应来自 `/api/storage/roots` 返回值） |
| sub_path | string | `root` 下的相对子路径（可选） |

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

说明：

- 未传 `root` 时保留旧行为，默认浏览 `storage.local_storage_root`。
- `root` 必须是授权本地根目录；`sub_path` 必须是相对路径，不能包含 `..` 或绝对子路径。
- 返回的 `path` 是相对当前 `root` 的子路径，前端可用 `root + path` 组合成完整本地库路径。
- 符号链接解析后的真实路径如果逃逸授权根，会被跳过。

---

## GET /api/storage/roots

获取当前应用可访问的本地存储根目录（管理员）。

**响应：** `200 OK`

```json
[
  {
    "path": "/app/storage",
    "source": "legacy_storage",
    "readable": true,
    "writable": true
  },
  {
    "path": "/mnt/media",
    "source": "config",
    "readable": true,
    "writable": false
  }
]
```

| 字段 | 类型 | 说明 |
|------|------|------|
| path | string | 规范化后的本地根目录路径 |
| source | string | 来源：`fnos`、`config` 或 `legacy_storage` |
| readable | boolean | 当前进程是否可读取 |
| writable | boolean | 当前进程是否可写入 |
