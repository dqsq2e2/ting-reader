# 播放进度

## GET /api/progress/recent

获取当前用户最近的播放进度（最多 4 条）。

**响应：** `200 OK`

```json
[
  {
    "id": "string",
    "user_id": "string",
    "book_id": "string",
    "chapter_id": "string | null",
    "position": 0.0,
    "duration": 0.0,
    "updated_at": "RFC3339",
    "book_title": "string | null",
    "cover_url": "string | null",
    "library_id": "string | null",
    "chapter_title": "string | null",
    "chapter_duration": 0
  }
]
```

---

## GET /api/progress/:bookId

获取指定书籍的播放进度。

**路径参数：**

| 参数 | 类型 | 说明 |
|------|------|------|
| bookId | string | 书籍 ID |

**响应：** `200 OK`

```json
{
  "id": "string",
  "userId": "string",
  "bookId": "string",
  "chapterId": "string | null",
  "position": 0.0,
  "duration": 0.0,
  "updatedAt": "RFC3339",
  "bookTitle": "string | null",
  "coverUrl": "string | null",
  "libraryId": "string | null",
  "chapterTitle": "string | null",
  "chapterDuration": 0
}
```

---

## POST /api/progress

更新播放进度（UPSERT）。

**请求体：**

```json
{
  "book_id": "string",
  "chapter_id": "string (可选)",
  "position": 0.0,
  "duration": 0.0 (可选)"
}
```

**响应：** `200 OK`

```json
{
  "id": "string",
  "user_id": "string",
  "book_id": "string",
  "chapter_id": "string | null",
  "position": 0.0,
  "duration": 0.0,
  "updated_at": "RFC3339",
  "book_title": null,
  "cover_url": null,
  "library_id": null,
  "chapter_title": null,
  "chapter_duration": null
}
```
