# Webhook 使用指南

通知与事件用于把 Ting Reader 内部事件推送到外部系统。例如：用户登录、开始播放、作品入库、作品删除、媒体库扫描完成等。

入口：`我的` -> `设置与管理` -> `通知与事件`

API 同时支持：

- `/api/system/notifications...`
- `/api/v1/system/notifications...`

所有配置接口仅管理员可访问。

## Webhook 是什么

Webhook 是一种“事件发生后主动通知外部服务”的机制。

普通接口是外部系统主动来查询 Ting Reader；Webhook 则是 Ting Reader 在事件发生时，主动向你配置的 URL 发送一个 HTTP `POST` 请求。

典型用途：

- 用户登录后通知企业微信、钉钉、飞书或自建服务。
- 媒体库扫描完成后通知管理员。
- 有新作品入库或作品被删除时同步到外部系统。
- 开始播放时触发自动化流程。

## 密钥是干什么的

Webhook 配置里的“密钥”是一个共享密钥，用来让接收端确认请求来自 Ting Reader。

如果配置了密钥，Ting Reader 发送 Webhook 时会加上请求头：

```http
X-Ting-Webhook-Secret: <你配置的密钥>
```

接收端收到请求后，应当比较这个 Header 是否等于自己保存的密钥。

重要说明：

- 密钥不会加密请求体。
- 密钥不是签名，目前不会生成 HMAC。
- 密钥会原样放在 Header 中发送。
- 密钥为空时，不会发送 `X-Ting-Webhook-Secret`。
- 建议 Webhook URL 使用 `https://`，避免密钥和事件内容在网络中明文传输。
- 建议使用随机长字符串作为密钥，例如 32 位以上随机值。

## 配置步骤

1. 进入 `我的` -> `设置与管理` -> `通知与事件`。
2. 点击 `新增 Webhook`。
3. 填写配置名称，例如 `企业微信通知`。
4. 填写 Webhook URL，例如 `https://example.com/ting-reader/webhook`。
5. 可选填写密钥。
6. 选择要监听的事件。
7. 保持“开启监听”为启用状态。
8. 保存配置。

保存后，匹配事件触发时，Ting Reader 会向该 URL 发送通知。

## 请求格式

事件触发后，Ting Reader 会向 Webhook URL 发送：

```http
POST /your-webhook-path HTTP/1.1
Content-Type: application/json
X-Ting-Event: user.login
X-Ting-Webhook-Secret: your-secret
```

请求体统一格式：

```json
{
  "event": "user.login",
  "title": "用户登录",
  "message": "用户 admin 登录成功",
  "data": {
    "userId": "1959b6b7-0d97-4876-8505-c9825f209619",
    "username": "admin",
    "role": "admin",
    "realIp": "127.0.0.1",
    "userAgent": "Mozilla/5.0 ...",
    "device": "Windows / Chrome"
  },
  "occurred_at": "2026-06-11T09:34:10.381846100+00:00"
}
```

字段说明：

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `event` | string | 事件 ID |
| `title` | string | 事件标题 |
| `message` | string | 可直接展示的人类可读消息 |
| `data` | object | 事件详情，不同事件结构不同 |
| `occurred_at` | string | 事件发生时间，RFC3339 格式 |

## 接收端校验示例

### Node.js / Express

```js
import express from 'express';

const app = express();
app.use(express.json());

const WEBHOOK_SECRET = process.env.TING_WEBHOOK_SECRET;

app.post('/ting-reader/webhook', (req, res) => {
  const secret = req.header('X-Ting-Webhook-Secret');

  if (WEBHOOK_SECRET && secret !== WEBHOOK_SECRET) {
    return res.status(401).json({ error: 'invalid webhook secret' });
  }

  const event = req.header('X-Ting-Event');
  const payload = req.body;

  console.log('Ting Reader event:', event, payload);

  res.sendStatus(204);
});

app.listen(8787);
```

### Python / FastAPI

```python
import os
from fastapi import FastAPI, Header, HTTPException, Request

app = FastAPI()
WEBHOOK_SECRET = os.getenv("TING_WEBHOOK_SECRET")

@app.post("/ting-reader/webhook")
async def ting_reader_webhook(
    request: Request,
    x_ting_event: str | None = Header(default=None),
    x_ting_webhook_secret: str | None = Header(default=None),
):
    if WEBHOOK_SECRET and x_ting_webhook_secret != WEBHOOK_SECRET:
        raise HTTPException(status_code=401, detail="invalid webhook secret")

    payload = await request.json()
    print("Ting Reader event:", x_ting_event, payload)
    return {"ok": True}
```

## 支持的事件

| 事件 ID | 说明 | 触发时机 |
| --- | --- | --- |
| `user.login` | 用户登录 | 用户登录成功 |
| `playback.play` | 播放开始 | 用户首次写入某作品/章节进度时触发，避免 WS/HTTP 定时进度保存刷屏 |
| `library.created` | 新增媒体库 | 管理员创建媒体库 |
| `library.deleted` | 删除媒体库 | 管理员删除媒体库 |
| `book.created` | 作品入库 | 作品被创建或入库 |
| `book.deleted` | 删除作品 | 作品被删除 |
| `library.scan_completed` | 扫描完成 | 媒体库扫描任务完成 |

## 常见事件 data 示例

### user.login

```json
{
  "userId": "string",
  "username": "admin",
  "role": "admin",
  "realIp": "127.0.0.1",
  "userAgent": "Mozilla/5.0 ...",
  "device": "Windows / Chrome"
}
```

### playback.play

`playback.play` 不是每 2 秒的播放进度更新。它只用于“开始播放”这类低频事件，避免 WebSocket/HTTP 进度同步造成大量重复通知。

```json
{
  "userId": "string",
  "username": "string",
  "bookId": "string",
  "bookTitle": "string",
  "chapterId": "string | null",
  "chapterTitle": "string | null",
  "position": 0.0,
  "duration": 0.0
}
```

### library.scan_completed

```json
{
  "libraryId": "string",
  "libraryName": "string",
  "libraryType": "local | webdav",
  "path": "string",
  "taskId": "string",
  "booksCreated": 0,
  "booksUpdated": 0,
  "booksDeleted": 0,
  "errors": 0
}
```

### book.created / book.deleted

```json
{
  "actorId": "string",
  "actor": "admin",
  "bookId": "string",
  "bookTitle": "string",
  "author": "string | null",
  "narrator": "string | null",
  "libraryId": "string",
  "libraryName": "string | null"
}
```

### library.created / library.deleted

```json
{
  "actorId": "string",
  "actor": "admin",
  "libraryId": "string",
  "libraryName": "string",
  "libraryType": "local | webdav",
  "url": "string",
  "rootPath": "string"
}
```

## 发送行为

- Webhook 异步发送，不阻塞登录、播放、扫描等主流程。
- 单次请求超时时间为 8 秒。
- 返回 `2xx` 表示发送成功。
- 返回非 `2xx` 或请求失败时，Ting Reader 会记录 `audit::notification` 日志。
- 当前没有自动重试队列。
- 同一个事件可以配置多个 Webhook，服务端会逐个发送。
- 只有启用状态的 Webhook 会收到通知。

## API 数据结构

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
- `secret` 可选。

## API 接口

### 获取可监听事件

```http
GET /api/system/notifications/events
Authorization: Bearer <token>
```

响应：

```json
[
  {
    "id": "user.login",
    "label": "用户登录",
    "description": "用户成功登录系统"
  }
]
```

### 获取 Webhook 列表

```http
GET /api/system/notifications
Authorization: Bearer <token>
```

响应：

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

### 创建 Webhook

```http
POST /api/system/notifications
Authorization: Bearer <token>
Content-Type: application/json
```

请求体：

```json
{
  "name": "企业微信通知",
  "url": "https://example.com/webhook",
  "enabled": true,
  "events": ["user.login", "library.scan_completed"],
  "secret": "your-random-secret"
}
```

响应：`201 Created`

### 更新 Webhook

```http
PUT /api/system/notifications/:id
Authorization: Bearer <token>
Content-Type: application/json
```

请求体同创建接口。响应返回更新后的 Webhook。

### 删除 Webhook

```http
DELETE /api/system/notifications/:id
Authorization: Bearer <token>
```

响应：`204 No Content`

## curl 示例

先登录获取 Token：

```bash
TOKEN=$(curl -s http://localhost:3000/api/auth/login \
  -H 'Content-Type: application/json' \
  -d '{"username":"admin","password":"admin123"}' | jq -r '.token')
```

创建 Webhook：

```bash
curl http://localhost:3000/api/system/notifications \
  -H "Authorization: Bearer $TOKEN" \
  -H 'Content-Type: application/json' \
  -d '{
    "name": "本地测试",
    "url": "http://127.0.0.1:8787/hook",
    "enabled": true,
    "events": ["user.login"],
    "secret": "test-secret"
  }'
```

## 排错

### 没收到请求

- 确认 Webhook 已启用。
- 确认事件 ID 已被选中。
- 确认 URL 能从 Ting Reader 服务端访问，而不是只能从浏览器访问。
- 本机测试时推荐使用 `http://127.0.0.1:<port>/path`。
- 查看系统日志里的 `audit::notification`。

### 收到请求但校验失败

- 确认接收端读取的是 `X-Ting-Webhook-Secret`。
- 确认配置里的密钥没有多余空格。
- 如果配置密钥为空，服务端不会发送该 Header。

### 收到重复通知

- 播放进度的 WS/HTTP 定时同步不会触发 Webhook。
- `playback.play` 只在首次写入某作品/章节进度时触发。
- 浏览器音频拉流可能产生多次请求，但 Webhook 不直接依赖拉流日志。

### 外部服务返回失败

- Ting Reader 只认为 `2xx` 是成功。
- 非 `2xx` 会记录日志，但不会让原业务失败。
- 当前没有自动重试队列，需要接收端保持稳定可用。
