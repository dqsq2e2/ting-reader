# 系列管理

## GET /api/v1/series

获取系列列表。

**查询参数：**

| 参数 | 类型 | 说明 |
|------|------|------|
| library_id | string | 按媒体库过滤（可选） |

**响应：** `200 OK`

```json
[
  {
    "id": "string",
    "library_id": "string",
    "title": "string",
    "author": "string | null",
    "narrator": "string | null",
    "cover_url": "string | null",
    "description": "string | null",
    "created_at": "RFC3339",
    "updated_at": "RFC3339",
    "books": [BookResponse]
  }
]
```

---

## GET /api/v1/series/:id

获取系列详情。

**路径参数：**

| 参数 | 类型 | 说明 |
|------|------|------|
| id | string | 系列 ID |

**响应：** `200 OK` — 返回 `SeriesResponse`

---

## POST /api/v1/series

创建系列（管理员）。

**请求体：**

```json
{
  "library_id": "string",
  "title": "string",
  "book_ids": ["string"],
  "author": "string (可选，默认取第一本书)",
  "narrator": "string (可选)",
  "cover_url": "string (可选)",
  "description": "string (可选)"
}
```

**响应：** `201 Created` — 返回 `SeriesResponse`

---

## PUT /api/v1/series/:id

更新系列（管理员）。

**路径参数：**

| 参数 | 类型 | 说明 |
|------|------|------|
| id | string | 系列 ID |

**请求体：**

```json
{
  "title": "string (可选)",
  "author": "string (可选)",
  "narrator": "string (可选)",
  "cover_url": "string (可选)",
  "description": "string (可选)",
  "book_ids": ["string (可选，替换所有书籍)"]
}
```

**响应：** `200 OK` — 返回 `SeriesResponse`

---

## DELETE /api/v1/series/:id

删除系列（管理员）。

**路径参数：**

| 参数 | 类型 | 说明 |
|------|------|------|
| id | string | 系列 ID |

**响应：** `204 No Content`
