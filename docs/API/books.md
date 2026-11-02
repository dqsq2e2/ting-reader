# 书籍管理

## BookResponse 结构

```json
{
  "id": "string",
  "library_id": "string",
  "title": "string | null",
  "author": "string | null",
  "narrator": "string | null",
  "cover_url": "string | null",
  "theme_color": "string | null",
  "description": "string | null",
  "skip_intro": 0,
  "skip_outro": 0,
  "path": "string",
  "hash": "string",
  "tags": "string | null",
  "genre": "string | null",
  "year": 0,
  "created_at": "RFC3339",
  "library_type": "local | webdav | rss | null",
  "can_write_metadata_files": false,
  "is_favorite": false,
  "manual_corrected": false,
  "match_pattern": "string | null",
  "chapter_regex": "string | null"
}
```

---

## 书籍 CRUD

### GET /api/v1/books

获取书籍列表。

**查询参数：**

| 参数 | 类型 | 说明 |
|------|------|------|
| search | string | 搜索关键词（可选） |
| tag | string | 按标签过滤（可选） |
| library_id | string | 按媒体库过滤（可选） |

**响应：** `200 OK` — 返回 `BookResponse[]`

---

### GET /api/v1/books/:id

获取书籍详情。

**路径参数：**

| 参数 | 类型 | 说明 |
|------|------|------|
| id | string | 书籍 ID |

**响应：** `200 OK` — 返回 `BookResponse`（包含 `library_type` 和 `is_favorite`）

---

### POST /api/v1/books

创建书籍。

**请求体：**

```json
{
  "library_id": "string",
  "title": "string (可选)",
  "author": "string (可选)",
  "narrator": "string (可选)",
  "cover_url": "string (可选)",
  "theme_color": "string (可选)",
  "description": "string (可选)",
  "skip_intro": 0,
  "skip_outro": 0,
  "path": "string",
  "hash": "string",
  "tags": "string | string[] (可选)",
  "genre": "string (可选)",
  "chapter_regex": "string (可选)"
}
```

**响应：** `201 Created` — 返回 `BookResponse`

说明：如果配置了 Webhook 监听，会触发 `book.created`。

---

### PUT /api/v1/books/:id

更新书籍（全量更新）。

**路径参数：**

| 参数 | 类型 | 说明 |
|------|------|------|
| id | string | 书籍 ID |

**请求体：** 与创建相同，所有字段可选（未提供的字段保持原值）。

**响应：** `200 OK` — 返回 `BookResponse`

---

### PATCH /api/v1/books/:id

部分更新书籍（与 PUT 相同逻辑）。

---

### POST /api/v1/books/:id/cover

上传封面，可同时保存编辑的书籍元数据。仅管理员可用，也支持 `/api/books/:id/cover`。

**请求：** `multipart/form-data`

| 字段 | 类型 | 说明 |
|------|------|------|
| file | 文件 | 必填，JPEG、PNG 或 WebP，最大 10 MiB，最多 1600 万像素 |
| metadata | JSON 字符串 | 可选，使用更新书籍接口的字段；不允许修改 `library_id`、`path`、`hash` |

图片按实际内容校验并保存为 `cover.jpg`。本地库保存到书籍目录；WebDAV、RSS 保存到服务器 `temp/<书籍路径的 SHA-256>/`。WebDAV 与 `metadata.json`、`book.nfo` 共用目录；启用库配置 `webdav_metadata_writing_enabled` 后，编辑同步更新网盘中的元数据文件和文件封面。封面 URL 只保持引用。RSS 只保存封面，不生成元数据文件。

**响应：** `200 OK` — 返回更新后的 `BookResponse`，包含封面路径和重新计算的主题色。未指定 `manual_corrected` 时，保存会锁定元数据。

**错误：** `400` 图片或元数据无效，`403` 权限不足，`404` 书籍不存在，`413` 请求超过大小上限。

---

### DELETE /api/v1/books/:id

删除书籍。

**路径参数：**

| 参数 | 类型 | 说明 |
|------|------|------|
| id | string | 书籍 ID |

**响应：** `204 No Content`

说明：如果配置了 Webhook 监听，会触发 `book.deleted`。

---

## 书籍操作

### POST /api/v1/books/merge

合并两本书籍（管理员）。

**请求体：**

```json
{
  "source_book_id": "string",
  "target_book_id": "string"
}
```

**响应：** `200 OK`

```json
{
  "message": "Books merged successfully",
  "result": {}
}
```

---

### POST /api/v1/books/chapters/move

将章节移动到另一本书（管理员）。

**请求体：**

```json
{
  "target_book_id": "string",
  "chapter_ids": ["string"]
}
```

**响应：** `200 OK`

```json
{
  "message": "Chapters moved successfully"
}
```

---

### POST /api/books/:id/write-metadata

将书籍元数据写入音频文件（管理员，异步任务）。

本地库可以使用此操作；WebDAV 库需开启“元数据写入网盘”。书籍详情的 `can_write_metadata_files` 表示当前库是否允许该操作。WebDAV 音频逐章下载至临时文件，使用与本地库相同的格式处理器写入标签后上传回原路径，处理完清理临时文件。STRM 章节跳过；URL 封面保持引用，不下载为图片。

**路径参数：**

| 参数 | 类型 | 说明 |
|------|------|------|
| id | string | 书籍 ID |

**响应：** `200 OK`

```json
{
  "message": "Metadata write task submitted",
  "task_id": "string"
}
```

---

## 章节管理

### GET /api/v1/books/:id/chapters

获取书籍的章节列表（含播放进度）。不传分页参数时返回完整章节数组；传入分页、分组或定位参数时返回分页对象，适合大型章节列表按需加载。

**路径参数：**

| 参数 | 类型 | 说明 |
|------|------|------|
| id | string | 书籍 ID |

**查询参数：**

| 参数 | 类型 | 说明 |
|------|------|------|
| offset | number | 分页起始偏移（可选，默认 `0`） |
| limit | number | 每页数量（可选，默认 `100`，范围 `1-500`） |
| chapter_type | string | 章节类型（可选）：`main` 正文、`extra` 番外、`all` 全部 |
| order | string | 排序方向（可选）：`asc` 正序、`desc` 逆序 |
| target_chapter_id | string | 目标章节 ID（可选）。传入后返回包含该章节的分页，并自动解析正文/番外类型 |

**响应：** `200 OK`

无分页参数时返回完整章节数组：

```json
[
  {
    "id": "string",
    "book_id": "string",
    "title": "string | null",
    "path": "string",
    "duration": 0,
    "chapter_index": 0,
    "is_extra": 0,
    "start_time": 0,
    "end_time": 38173,
    "created_at": "RFC3339",
    "progress_position": 0.0,
    "progress_updated_at": "RFC3339 | null"
  }
]
```

`start_time` / `end_time`（毫秒）仅出现在单文件 M4B/M4A 的内嵌章节上，表示章节在 `path` 同一个音频文件内的起止位置；此时 `duration` 为章节区间时长。普通“一文件一章节”的书籍不返回这两个字段。

传入分页参数时返回分页对象：

```json
{
  "chapters": [
    {
      "id": "string",
      "book_id": "string",
      "title": "string | null",
      "path": "string",
      "duration": 0,
      "chapter_index": 0,
      "is_extra": 0,
      "start_time": 0,
      "end_time": 38173,
      "created_at": "RFC3339",
      "progress_position": 0.0,
      "progress_updated_at": "RFC3339 | null"
    }
  ],
  "total": 0,
  "main_total": 0,
  "extra_total": 0,
  "offset": 0,
  "limit": 100,
  "chapter_type": "main",
  "order": "asc"
}
```

---

### PUT /api/v1/books/:id/chapters/batch

批量更新章节（管理员）。

**路径参数：**

| 参数 | 类型 | 说明 |
|------|------|------|
| id | string | 书籍 ID |

**请求体：**

```json
{
  "updates": [
    {
      "id": "string (章节 ID)",
      "title": "string (可选)",
      "chapter_index": 0,
      "is_extra": 0
    }
  ]
}
```

**响应：** `200 OK`

```json
{
  "message": "Chapters updated successfully"
}
```

---

### PATCH /api/v1/chapters/:id

更新章节信息。

**路径参数：**

| 参数 | 类型 | 说明 |
|------|------|------|
| id | string | 章节 ID |

**请求体：**

```json
{
  "title": "string (可选)",
  "path": "string (可选)",
  "duration": 0,
  "chapter_index": 0,
  "is_extra": 0
}
```

**响应：** `200 OK` — 返回 `ChapterResponse`

---

### GET /api/v1/tags

获取所有标签。

**响应：** `200 OK`

```json
["标签1", "标签2", "标签3"]
```

---

## 刮削

### POST /api/v1/books/:id/scrape-diff

获取刮削差异（管理员）。

**路径参数：**

| 参数 | 类型 | 说明 |
|------|------|------|
| id | string | 书籍 ID |

**请求体：**

```json
{
  "query": "string (搜索关键词，通常为书名)",
  "author": "string (可选)",
  "narrator": "string (可选)"
}
```

**响应：** `200 OK`

```json
{
  "current": {
    "title": "string",
    "author": "string",
    "narrator": "string",
    "description": "string",
    "cover_url": "string | null",
    "tags": ["string"],
    "genre": "string | null"
  },
  "scraped": {
    "title": "string",
    "author": "string",
    "narrator": "string",
    "description": "string",
    "cover_url": "string | null",
    "tags": ["string"],
    "genre": "string | null"
  },
  "chapter_changes": []
}
```

---

### POST /api/v1/books/:id/scrape-apply

应用刮削结果（管理员）。

**路径参数：**

| 参数 | 类型 | 说明 |
|------|------|------|
| id | string | 书籍 ID |

**请求体（方式一 - 按字段选择）：**

```json
{
  "fields": {
    "title": { "value": "新标题" },
    "author": { "value": "新作者" },
    "cover_url": { "value": "http://...", "source": "plugin-id", "external_id": "ext-id" }
  }
}
```

**请求体（方式二 - 全量应用）：**

```json
{
  "apply_metadata": true,
  "metadata": {
    "title": "string",
    "author": "string",
    "narrator": "string | null",
    "intro": "string",
    "cover_url": "string | null",
    "tags": ["string"],
    "genre": "string | null",
    "subtitle": "string | null",
    "published_year": "string | null",
    "published_date": "string | null",
    "publisher": "string | null",
    "isbn": "string | null",
    "asin": "string | null",
    "language": "string | null",
    "explicit": false,
    "abridged": false
  }
}
```

**响应：** `200 OK` — 返回更新后的 `BookResponse`
