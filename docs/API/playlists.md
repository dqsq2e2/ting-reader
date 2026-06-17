# 书单

书单接口需要登录。普通用户只能访问自己的书单；管理员仍按当前登录用户的书单范围返回，但在书单内添加作品时拥有管理员级作品可见性。

支持路径：

- `/api/playlists...`
- `/api/v1/playlists...`

## 数据结构

### PlaylistResponse

```json
{
  "id": "string",
  "user_id": "string",
  "title": "string",
  "description": "string | null",
  "created_at": "RFC3339",
  "updated_at": "RFC3339",
  "book_ids": ["string"],
  "books": [],
  "items": []
}
```

字段说明：

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `book_ids` | string[] | 兼容字段。按书单顺序展开后的书籍 ID；系列会展开为系列内书籍。 |
| `books` | BookResponse[] | 兼容字段。按书单顺序展开后的书籍列表；系列会展开为系列内书籍。结构见 [books.md](books.md)。 |
| `items` | PlaylistItemResponse[] | 书单真实条目，保留 `book` / `series` 类型与手动排序。新前端应优先使用该字段。 |

### PlaylistItemRequest

```json
{
  "item_type": "book | series",
  "item_id": "string"
}
```

字段说明：

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `item_type` | string | 条目类型，只能是 `book` 或 `series`。 |
| `item_id` | string | 当 `item_type = book` 时为书籍 ID；当 `item_type = series` 时为系列 ID。 |

### PlaylistItemResponse

```json
{
  "item_type": "book",
  "item_id": "string",
  "order": 1,
  "book": {},
  "series": null
}
```

系列条目示例：

```json
{
  "item_type": "series",
  "item_id": "string",
  "order": 2,
  "book": null,
  "series": {
    "id": "string",
    "libraryId": "string",
    "title": "系列名称",
    "books": []
  }
}
```

说明：

- `order` 从 1 开始，表示书单内手动排序。
- `book` 和 `series` 二选一。
- 系列条目会作为系列保存在书单中，不会拆成多个书籍条目。
- `series` 内部结构为 `SeriesResponse`，结构见 [series.md](series.md)。当前 `SeriesResponse` 字段按既有实现返回。

### CreatePlaylistRequest

```json
{
  "title": "string",
  "description": "string | null",
  "items": [
    { "item_type": "book", "item_id": "book-id" },
    { "item_type": "series", "item_id": "series-id" }
  ]
}
```

兼容旧字段：

```json
{
  "title": "string",
  "book_ids": ["book-id"]
}
```

说明：

- 推荐使用 `items`，可以同时保存书籍和系列。
- 仅传 `book_ids` 时，后端会按旧逻辑转换为 `book` 条目。
- 不支持 `bookIds`、`itemType`、`itemId` 等 camelCase 请求字段；直接调用后端 API 请使用 snake_case。

### UpdatePlaylistRequest

```json
{
  "title": "string",
  "description": "string | null",
  "items": [
    { "item_type": "series", "item_id": "series-id" },
    { "item_type": "book", "item_id": "book-id" }
  ]
}
```

说明：

- 所有字段均可选。
- 传入 `items` 时，会整体替换书单内条目及顺序。
- 传入 `book_ids` 时，会整体替换为纯书籍条目，用于兼容旧调用方。
- `items` 优先级高于 `book_ids`。
- 不传 `items` 和 `book_ids` 时，仅更新标题、描述等元信息。

## GET /api/playlists

获取当前用户书单列表。

响应：`200 OK`

```json
[
  {
    "id": "string",
    "user_id": "string",
    "title": "通勤",
    "description": "路上听",
    "created_at": "RFC3339",
    "updated_at": "RFC3339",
    "book_ids": ["book-id"],
    "books": [],
    "items": [
      {
        "item_type": "book",
        "item_id": "book-id",
        "order": 1,
        "book": {},
        "series": null
      }
    ]
  }
]
```

## POST /api/playlists

创建书单。

请求体：`CreatePlaylistRequest`

响应：`201 Created`

返回创建后的 `PlaylistResponse`。

## GET /api/playlists/:id

获取单个书单详情。

路径参数：

| 参数 | 类型 | 说明 |
| --- | --- | --- |
| `id` | string | 书单 ID |

响应：`200 OK`

返回 `PlaylistResponse`。

## PUT /api/playlists/:id

更新书单。可用于修改信息、替换作品列表、手动排序。

路径参数：

| 参数 | 类型 | 说明 |
| --- | --- | --- |
| `id` | string | 书单 ID |

请求体示例：

```json
{
  "title": "睡前听",
  "description": "放松一点",
  "items": [
    { "item_type": "series", "item_id": "series-1" },
    { "item_type": "book", "item_id": "book-3" },
    { "item_type": "book", "item_id": "book-1" }
  ]
}
```

响应：`200 OK`

返回更新后的 `PlaylistResponse`。

## DELETE /api/playlists/:id

删除书单。

路径参数：

| 参数 | 类型 | 说明 |
| --- | --- | --- |
| `id` | string | 书单 ID |

响应：`204 No Content`
