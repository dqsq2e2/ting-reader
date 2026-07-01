# 通知与事件 API

通知与事件接口用于配置可自定义请求头和 Body 模板的 HTTP Webhook。所有接口仅管理员可访问。

同时支持：

- `/api/system/notifications...`
- `/api/v1/system/notifications...`

## 事件类型

| 事件 ID | 说明 |
| --- | --- |
| `user.login` | 用户登录 |
| `playback.play` | 开始播放 |
| `library.created` | 新增媒体库 |
| `library.deleted` | 删除媒体库 |
| `book.created` | 作品入库 |
| `book.deleted` | 删除作品 |
| `library.scan_completed` | 扫描完成 |

## 模板语法

原样输出：

```text
{{title}}
{{message}}
{{event}}
{{occurred_at}}
{{notification}}
{{data.username}}
```

JSON 安全输出：

```text
{{json:title}}
{{json:message}}
{{json:notification}}
{{json:data.username}}
{{json:payload}}
```

`{{json:变量}}` 输出合法 JSON 值并自动转义。`{{json:payload}}` 输出完整事件对象。

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
  "headers": {
    "Content-Type": "application/json",
    "Authorization": "Bearer token"
  },
  "body_template": "{\"title\":{{json:title}},\"message\":{{json:message}}}",
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
  "secret": null,
  "headers": {
    "Content-Type": "application/json"
  },
  "body_template": "{{json:payload}}"
}
```

规则：

- `name` 不能为空。
- `url` 必须使用 `http://` 或 `https://`。
- `events` 至少包含一个受支持事件。
- `headers` 可选，请求头名称和值必须合法。
- `body_template` 可选，默认 `{{json:payload}}`。
- `secret` 存在时发送 `X-Ting-Webhook-Secret`。

## 接口

### GET /api/system/notifications/events

获取可监听事件。

### GET /api/system/notifications

获取全部 Webhook 配置。

### POST /api/system/notifications

创建配置，响应 `201 Created`。

### PUT /api/system/notifications/:id

更新配置，响应更新后的 `NotificationWebhook`。

### DELETE /api/system/notifications/:id

删除配置，响应 `204 No Content`。

### POST /api/system/notifications/test

使用固定的 `webhook.test` 示例事件测试当前配置，无需先保存。

请求体：`NotificationWebhookRequest`

响应：

```json
{
  "success": true,
  "status": 200,
  "response_body": "{\"errcode\":0,\"errmsg\":\"ok\"}",
  "rendered_body": "{\"msgtype\":\"text\",\"text\":{\"content\":\"听悦测试通知\\n如果你看到这条消息，说明 Webhook 配置正常。\"}}",
  "error": null
}
```

网络请求未完成时 `status` 为 `0`，错误信息位于 `error`。

## 请求行为

- 请求方法固定为 `POST`。
- 默认请求头为 `Content-Type: application/json` 和 `X-Ting-Event`。
- 自定义请求头可以覆盖同名默认头。
- 请求头值也支持模板变量。
- 超时时间为 8 秒。
- HTTP 非 `2xx` 视为失败。
- 响应 JSON 包含非零 `errcode` 时视为业务失败。
