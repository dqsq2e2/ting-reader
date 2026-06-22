# WebSocket

## WS /api/ws

实时进度同步 WebSocket 端点。

**连接：** `ws://<host>:<port>/api/ws?token=<JWT_TOKEN>`

---

## 客户端 → 服务器消息

### progress_update

更新播放进度。

```json
{
  "type": "progress_update",
  "book_id": "string",
  "chapter_id": "string | null",
  "position": 0.0,
  "playback_start": 0.0
}
```

`playback_start` 可选，仅在真正开始或恢复播放时发送。周期进度心跳只发送
`position`，后端会对同一次 WS/HTTP 起播事件去重。

### ping

心跳检测。

```json
{
  "type": "ping"
}
```

---

## 服务器 → 客户端消息

### progress_updated

进度更新确认。

```json
{
  "type": "progress_updated",
  "book_id": "string",
  "chapter_id": "string | null",
  "position": 0.0,
  "updated_at": "RFC3339"
}
```

### pong

心跳响应。

```json
{
  "type": "pong"
}
```

### error

错误消息。

```json
{
  "type": "error",
  "message": "string"
}
```
