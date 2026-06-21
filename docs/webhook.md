# Webhook 使用指南

通知与事件用于把 Ting Reader 内部事件推送到企业微信、ntfy、Gotify、自动化平台或其他 HTTP 服务。

入口：`我的` -> `设置与管理` -> `通知与事件`

所有配置接口仅管理员可访问，同时支持以下路径：

- `/api/system/notifications...`
- `/api/v1/system/notifications...`

## 功能概览

每个 Webhook 可以单独配置：

- Webhook URL
- 要监听的事件
- 任意 HTTP 请求头
- Body 模板
- 启用或停用状态
- 兼容旧版本的 `X-Ting-Webhook-Secret`

系统内置以下常见模板：

- 企业微信 Markdown
- 企业微信文本
- ntfy JSON
- Gotify JSON
- 原始事件 JSON
- 纯文本

配置完成后可以先点击“测试发送”，查看 HTTP 状态、服务响应和实际渲染的请求体，再决定是否保存。

## 配置步骤

1. 进入 `我的` -> `设置与管理` -> `通知与事件`。
2. 点击“添加 Webhook”。
3. 填写名称和 Webhook URL。
4. 选择一个常见模板，或自行编辑请求头和 Body 模板。
5. 选择需要监听的事件。
6. 点击“测试发送”确认目标服务可以正常接收。
7. 保存配置。

## 模板变量

Body 和请求头的值都支持变量。

### 原样输出

```text
{{title}}
{{message}}
{{event}}
{{occurred_at}}
{{notification}}
{{data.username}}
```

`{{notification}}` 等于标题和正文的组合：

```text
标题
正文
```

原样输出适合纯文本 Body 或普通请求头。如果把它直接放进 JSON 字符串，内容中的引号和换行可能破坏 JSON。

### JSON 安全输出

JSON 模板应使用 `json:` 前缀：

```text
{{json:title}}
{{json:message}}
{{json:notification}}
{{json:data.username}}
{{json:payload}}
```

它会输出完整的 JSON 值并自动处理引号、换行和反斜杠。例如：

```json
{
  "title": {{json:title}},
  "message": {{json:message}}
}
```

`{{json:payload}}` 会输出完整的 Ting Reader 事件对象，适合需要原始事件数据的接收端。

可用根变量：

| 变量 | 说明 |
| --- | --- |
| `event` | 事件 ID |
| `title` | 事件标题 |
| `message` | 人类可读的通知正文 |
| `occurred_at` | RFC3339 格式的事件时间 |
| `notification` | 标题和正文的组合文本 |
| `data` | 当前事件的详细数据 |
| `payload` | 完整事件对象 |

`data` 支持点路径，例如 `{{json:data.book_title}}`。

## 默认请求行为

系统始终发送 HTTP `POST` 请求，并默认加入：

```http
Content-Type: application/json
X-Ting-Event: user.login
```

如果配置了同名自定义请求头，自定义值会覆盖默认值。

旧版本的 `secret` 字段仍被兼容。如果存在，会加入：

```http
X-Ting-Webhook-Secret: your-secret
```

新配置建议直接使用自定义请求头，例如：

```http
Authorization: Bearer your-token
```

请求头的值也可以使用模板变量。

## 常见模板

### 企业微信 Markdown

Webhook URL：

```text
https://qyapi.weixin.qq.com/cgi-bin/webhook/send?key=机器人密钥
```

请求头：

```http
Content-Type: application/json
```

Body：

```json
{
  "msgtype": "markdown",
  "markdown": {
    "content": {{json:notification}}
  }
}
```

企业微信机器人即使业务失败也可能返回 HTTP 200。Ting Reader 会继续检查响应 JSON 中的 `errcode`，只有 `errcode` 为 `0` 才视为成功。

### 企业微信文本

```json
{
  "msgtype": "text",
  "text": {
    "content": {{json:notification}}
  }
}
```

### ntfy JSON

JSON 发布模式应填写 ntfy 服务根地址，不要把 Topic 重复写进 URL：

```text
https://ntfy.example.com
```

Body：

```json
{
  "topic": "ting-reader",
  "title": {{json:title}},
  "message": {{json:message}},
  "priority": 3,
  "tags": ["headphones"]
}
```

需要认证时可添加：

```http
Authorization: Bearer your-token
```

或：

```http
Authorization: Basic base64(username:password)
```

### Gotify JSON

Webhook URL：

```text
https://gotify.example.com/message?token=APPLICATION_TOKEN
```

Body：

```json
{
  "title": {{json:title}},
  "message": {{json:message}},
  "priority": 5
}
```

### 原始事件 JSON

请求头：

```http
Content-Type: application/json
```

Body：

```text
{{json:payload}}
```

渲染结果示例：

```json
{
  "event": "user.login",
  "title": "用户登录",
  "message": "用户 admin 登录成功",
  "data": {
    "user_id": "string",
    "username": "admin",
    "role": "admin"
  },
  "occurred_at": "2026-06-21T08:00:00Z"
}
```

### 纯文本

请求头：

```http
Content-Type: text/plain; charset=utf-8
```

Body：

```text
{{notification}}
```

## 支持的事件

| 事件 ID | 说明 | 触发时机 |
| --- | --- | --- |
| `user.login` | 用户登录 | 用户登录成功 |
| `playback.play` | 播放开始 | 用户首次写入某作品或章节进度 |
| `library.created` | 新增媒体库 | 管理员创建媒体库 |
| `library.deleted` | 删除媒体库 | 管理员删除媒体库 |
| `book.created` | 作品入库 | 作品被创建或入库 |
| `book.deleted` | 删除作品 | 作品被删除 |
| `library.scan_completed` | 扫描完成 | 媒体库扫描任务完成 |

## 常见事件 data

### user.login

```json
{
  "user_id": "string",
  "username": "admin",
  "role": "admin",
  "real_ip": "127.0.0.1",
  "user_agent": "Mozilla/5.0 ...",
  "device": "Windows / Chrome",
  "login_method": "password"
}
```

### playback.play

`playback.play` 不是定时进度上报，只表示开始播放等低频事件。

```json
{
  "user_id": "string",
  "username": "string",
  "book_id": "string",
  "book_title": "string",
  "chapter_id": "string | null",
  "chapter_title": "string | null",
  "position": 0.0,
  "duration": 0.0
}
```

### library.scan_completed

```json
{
  "library_id": "string",
  "library_name": "string",
  "library_type": "local | webdav",
  "path": "string",
  "task_id": "string",
  "books_created": 0,
  "books_updated": 0,
  "books_deleted": 0,
  "errors": 0
}
```

## 测试发送

测试发送使用固定的 `webhook.test` 示例事件，不需要先保存配置：

```json
{
  "event": "webhook.test",
  "title": "听悦测试通知",
  "message": "如果你看到这条消息，说明 Webhook 配置正常。",
  "data": {
    "username": "admin",
    "book_title": "示例有声书",
    "chapter_title": "第一章"
  }
}
```

测试结果会展示：

- 是否成功
- HTTP 状态码
- 目标服务响应
- 实际渲染的请求体
- 网络错误或企业微信业务错误

## API 数据结构

### NotificationWebhook

```json
{
  "id": "string",
  "name": "企业微信通知",
  "url": "https://qyapi.weixin.qq.com/cgi-bin/webhook/send?key=...",
  "enabled": true,
  "events": ["user.login", "library.scan_completed"],
  "secret": null,
  "headers": {
    "Content-Type": "application/json"
  },
  "body_template": "{\"msgtype\":\"markdown\",\"markdown\":{\"content\":{{json:notification}}}}",
  "created_at": "RFC3339",
  "updated_at": "RFC3339"
}
```

校验规则：

- `name` 不能为空。
- `url` 必须使用 `http://` 或 `https://`。
- `events` 至少选择一个，且必须是支持的事件 ID。
- 请求头名称和值必须是合法 HTTP Header。
- `body_template` 不能为空，且不能引用未知变量。

## API 接口

| 方法 | 路径 | 说明 |
| --- | --- | --- |
| `GET` | `/api/system/notifications/events` | 获取可监听事件 |
| `GET` | `/api/system/notifications` | 获取 Webhook 列表 |
| `POST` | `/api/system/notifications` | 创建 Webhook |
| `PUT` | `/api/system/notifications/:id` | 更新 Webhook |
| `DELETE` | `/api/system/notifications/:id` | 删除 Webhook |
| `POST` | `/api/system/notifications/test` | 测试发送 |

### 测试发送请求

```json
{
  "name": "企业微信通知",
  "url": "https://qyapi.weixin.qq.com/cgi-bin/webhook/send?key=...",
  "enabled": true,
  "events": ["user.login"],
  "headers": {
    "Content-Type": "application/json"
  },
  "body_template": "{\"msgtype\":\"markdown\",\"markdown\":{\"content\":{{json:notification}}}}"
}
```

响应：

```json
{
  "success": true,
  "status": 200,
  "response_body": "{\"errcode\":0,\"errmsg\":\"ok\"}",
  "rendered_body": "{\"msgtype\":\"markdown\",\"markdown\":{\"content\":\"听悦测试通知\\n如果你看到这条消息，说明 Webhook 配置正常。\"}}",
  "error": null
}
```

## 发送行为

- Webhook 异步发送，不阻塞登录、播放和扫描等主流程。
- 单次请求超时时间为 8 秒。
- HTTP 非 `2xx` 会视为失败。
- 企业微信响应中的非零 `errcode` 会视为失败。
- 同一个事件可以配置多个 Webhook。
- 当前没有自动重试队列。
- 发送结果记录在 `audit::notification` 日志中。

## 排错

### 没收到请求

- 确认配置已启用并选择了正确事件。
- 确认 URL 能从 Ting Reader 服务端访问，而不仅是浏览器可以访问。
- Docker 部署时，`127.0.0.1` 指向容器本身。
- 查看“系统日志”中的“通知记录”。

### 企业微信返回 HTTP 200 但测试失败

展开测试结果查看 `errcode` 和 `errmsg`。常见原因是机器人 URL 错误、机器人被删除或请求体不符合消息类型协议。

### JSON 模板无效

- JSON 字符串值使用 `{{json:变量}}`，不要写成 `"{{变量}}"`。
- 使用测试发送查看实际请求体。
- 检查逗号、括号和固定文本中的引号。

### ntfy 没有收到消息

- JSON 模式 URL 应填写服务根地址。
- `topic` 应放在 Body 中。
- 私有服务检查 `Authorization` 请求头。

### Gotify 返回 401

- 确认 URL 使用 Application Token，而不是 Client Token。
- URL 通常是 `/message?token=APPLICATION_TOKEN`。
