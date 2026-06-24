# 播放进度

播放进度接口需要登录。

## GET /api/progress/recent

获取当前用户可见收听历史。后端返回所有有章节进度的历史记录（包括已播完章节），按 `updated_at` 倒序排列；同一本书的多个章节会作为多条记录返回。

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

- 最近收听来自 `progress` 表，是用户可见历史；历史可见性与章节播放进度分离。
- 清除或删除历史只会隐藏可见历史，不会删除章节进度。用户再次播放被隐藏的章节后，该章节会重新出现在历史列表中。
- 后台统计使用独立的 `listening_events`，不会因清空最近收听而删除。

## DELETE /api/progress/recent

清空当前用户全部可见收听历史。

响应：`204 No Content`

说明：

- 该接口只设置当前用户可见历史的隐藏标记，不删除 `progress` 记录和章节进度。
- 不删除 `listening_events`，因此管理员数据统计不受影响。

## POST /api/progress/recent/delete

隐藏当前用户选中的可见收听历史。用于按章节、整本书或全选删除历史记录。

请求体：

```json
{
  "progress_ids": ["string"],
  "chapter_ids": ["string"]
}
```

字段说明：

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `progress_ids` | string[] | 要隐藏的进度记录 ID，优先用于精确删除。 |
| `chapter_ids` | string[] | 要隐藏的章节 ID，可作为旧客户端或无进度记录 ID 时的兜底。 |

响应：`200 OK`

```json
{
  "deleted": 0
}
```

说明：

- `deleted` 表示本次被隐藏的可见历史记录数。
- 后端原始 API 字段为 `snake_case`；Web 前端内部会由 `apiClient` 自动转换为 `camelCase`。

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

- `progress`：当前用户章节播放进度。若对应记录此前被隐藏，会自动恢复为可见历史。
- `listening_events`：后台统计事件，不受用户清空历史影响。

请求体：

```json
{
  "book_id": "string",
  "chapter_id": "string | null",
  "position": 0.0,
  "duration": 0.0,
  "playback_start": 0.0
}
```

- `playback_start` 为可选字段，只在真正开始或恢复播放时发送，值为本次起播位置。
- 普通周期进度同步不应携带 `playback_start`。
- 后端播放日志由该字段触发，不再把音频预加载或流探测请求误记为播放开始。

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

- 携带 `playback_start` 时会记录 `audit::playback` 日志；同一次 WS/HTTP 起播上报会自动去重。
- 当前用户首次写入某本书/章节进度时，如果配置了 Webhook 监听，会触发 `playback.play` 事件。
- 不是每次定时进度上报都会触发 Webhook，避免通知刷屏。
