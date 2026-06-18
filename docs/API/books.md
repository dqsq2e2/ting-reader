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
  "library_type": "local | webdav | null",
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

> Web 端请求参数会统一从 camelCase 转为 snake_case；后端也兼容 `chapterType` 和 `targetChapterId`。

**响应：** `200 OK`

无分页参数时返回完整章节数组：

```json
[
  {
    "id": "string",
    "bookId": "string",
    "title": "string | null",
    "path": "string",
    "duration": 0,
    "chapterIndex": 0,
    "isExtra": 0,
    "createdAt": "RFC3339",
    "progressPosition": 0.0,
    "progressUpdatedAt": "RFC3339 | null"
  }
]
```

传入分页参数时返回分页对象：

```json
{
  "chapters": [
    {
      "id": "string",
      "bookId": "string",
      "title": "string | null",
      "path": "string",
      "duration": 0,
      "chapterIndex": 0,
      "isExtra": 0,
      "createdAt": "RFC3339",
      "progressPosition": 0.0,
      "progressUpdatedAt": "RFC3339 | null"
    }
  ],
  "total": 0,
  "mainTotal": 0,
  "extraTotal": 0,
  "offset": 0,
  "limit": 100,
  "chapterType": "main",
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
