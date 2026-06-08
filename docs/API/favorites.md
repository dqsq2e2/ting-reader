# 收藏

## GET /api/favorites

获取当前用户的收藏书籍列表。

**响应：** `200 OK` — 返回 `BookResponse[]`（见 [书籍](books.md) 章节）

---

## POST /api/favorites/:bookId

添加书籍到收藏。

**路径参数：**

| 参数 | 类型 | 说明 |
|------|------|------|
| bookId | string | 书籍 ID |

**响应：** `201 Created`

```json
{
  "message": "Book added to favorites"
}
```

---

## DELETE /api/favorites/:bookId

取消收藏。

**路径参数：**

| 参数 | 类型 | 说明 |
|------|------|------|
| bookId | string | 书籍 ID |

**响应：** `200 OK`

```json
{
  "message": "Book removed from favorites"
}
```
