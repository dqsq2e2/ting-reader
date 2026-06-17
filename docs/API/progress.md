# 播放进度

播放进度接口需要登录。

## GET /api/progress/recent

获取当前用户最近收听记录。当前后端最多返回 `100` 条，每本书只返回最近的一条进度记录。

响应：`200 OK`

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

说明：

- 最近收听来自 `progress` 表，是用户可见历史和当前播放进度。
- 后台统计使用独立的 `listening_events`，不会因清空最近收听而删除。

## DELETE /api/progress/recent

清空当前用户可见收听历史和播放进度。

响应：`204 No Content`

说明：

- 该接口只删除当前用户 `progress` 记录。
- 不删除 `listening_events`，因此管理员数据统计不受影响。

## GET /api/progress/:bookId

获取指定书籍的最近播放进度。

路径参数：

| 参数 | 类型 | 说明 |
| --- | --- | --- |
| `bookId` | string | 书籍 ID |

响应：`200 OK`

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

未找到记录时返回 `404 Not Found`。

## POST /api/progress

更新播放进度。该接口会写入：

- `progress`：当前用户可见进度，可被 `DELETE /api/progress/recent` 清除。
- `listening_events`：后台统计事件，不受用户清空历史影响。

请求体：

```json
{
  "book_id": "string",
  "chapter_id": "string | null",
  "position": 0.0,
  "duration": 0.0
}
```

响应：`200 OK`

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

校验：

- `book_id` 必须存在。
- 如果传入 `chapter_id`，章节必须存在且属于该书籍。

事件：

- 当当前用户首次写入某本书/章节进度时，会记录 `audit::playback` 日志。
- 如果配置了 Webhook 监听，会触发 `playback.play` 事件。
- 不是每次定时进度上报都会触发 Webhook，避免通知刷屏。
