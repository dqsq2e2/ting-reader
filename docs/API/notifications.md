# 通知与事件

通知与事件接口用于配置 Webhook 监听系统事件。所有接口仅管理员可访问。

支持路径：

- `/api/system/notifications...`
- `/api/v1/system/notifications...`

## 事件类型

| 事件 ID | 说明 | 触发时机 |
| --- | --- | --- |
| `user.login` | 用户登录 | 用户登录成功 |
| `playback.play` | 播放开始 | 用户首次写入某作品/章节进度时触发，避免定时进度上报刷屏 |
| `library.created` | 新增媒体库 | 管理员创建媒体库 |
| `library.deleted` | 删除媒体库 | 管理员删除媒体库 |
| `book.created` | 作品入库 | 作品被创建或入库 |
| `book.deleted` | 删除作品 | 作品被删除 |
| `library.scan_completed` | 扫描完成 | 媒体库扫描任务完成 |

## Webhook Payload

事件触发后，服务端会向匹配的 Webhook URL 发送 `POST` 请求。

请求头：

| Header | 说明 |
| --- | --- |
| `Content-Type` | `application/json` |
| `X-Ting-Event` | 当前事件 ID |
| `X-Ting-Webhook-Secret` | 如果配置了 `secret`，会原样放入该请求头 |

请求体：

```json
{
  "event": "user.login",
  "title": "用户登录",
  "message": "用户 admin 登录成功",
  "data": {
    "userId": "string",
    "username": "admin",
    "role": "admin",
    "realIp": "127.0.0.1",
    "userAgent": "Mozilla/5.0 ...",
    "device": "Windows / Chrome",
    "loginMethod": "password | session_restore | jwt_token"
  },
  "occurred_at": "RFC3339"
}
```

## 数据结构

### NotificationWebhook

```json
{
  "id": "string",
  "name": "string",
  "url": "https://example.com/webhook",
  "enabled": true,
  "events": ["user.login", "playback.play"],
  "secret": "string | null",
  "created_at": "RFC3339",
  "updated_at": "RFC3339"
}
```

### NotificationWebhookRequest

```json
{
  "name": "string",
  "url": "https://example.com/webhook",
  "enabled": true,
  "events": ["user.login", "playback.play"],
  "secret": "string | null"
}
```

校验规则：

- `name` 不能为空。
- `url` 必须是 `http://` 或 `https://`。
- `events` 至少选择一个，且必须是支持的事件 ID。
- `secret` 可选；为空时不会发送 `X-Ting-Webhook-Secret`。

## 接口

### GET /api/system/notifications/events

获取可监听事件列表。

响应：`200 OK`

```json
[
  {
    "id": "user.login",
    "label": "用户登录",
    "description": "用户成功登录系统"
  }
]
```

### GET /api/system/notifications

获取 Webhook 配置列表。

响应：`200 OK`

```json
[
  {
    "id": "string",
    "name": "企业微信通知",
    "url": "https://example.com/webhook",
    "enabled": true,
    "events": ["user.login", "library.scan_completed"],
    "secret": "string | null",
    "created_at": "RFC3339",
    "updated_at": "RFC3339"
  }
]
```

### POST /api/system/notifications

创建 Webhook 配置。

请求体：`NotificationWebhookRequest`

响应：`201 Created`

### PUT /api/system/notifications/:id

更新 Webhook 配置。

路径参数：

| 参数 | 类型 | 说明 |
| --- | --- | --- |
| `id` | string | Webhook 配置 ID |

请求体：`NotificationWebhookRequest`

响应：`200 OK`

### DELETE /api/system/notifications/:id

删除 Webhook 配置。

路径参数：

| 参数 | 类型 | 说明 |
| --- | --- | --- |
| `id` | string | Webhook 配置 ID |

响应：`204 No Content`
