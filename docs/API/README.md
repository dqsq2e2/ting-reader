# Ting Reader API 文档

版本：v1  
基础 URL：`http://<host>:<port>`

## 模块索引

| 模块 | 文件 | 说明 |
| --- | --- | --- |
| 认证 | [auth.md](auth.md) | 注册、登录、JWT Token |
| 用户 | [users.md](users.md) | 当前用户、用户管理、个性化设置 |
| 播放进度 | [progress.md](progress.md) | 最近收听、清空历史、更新播放进度 |
| 收藏 | [favorites.md](favorites.md) | 收藏管理 |
| 书单 | [playlists.md](playlists.md) | 我的书单、作品排序与管理 |
| 媒体库 | [libraries.md](libraries.md) | 媒体库 CRUD、扫描、WebDAV 测试 |
| 系列 | [series.md](series.md) | 系列 CRUD |
| 书籍 | [books.md](books.md) | 书籍 CRUD、章节管理、刮削、合并 |
| 搜索与刮削 | [search.md](search.md) | 本地搜索、在线刮削、刮削源 |
| 插件 | [plugins.md](plugins.md) | 插件管理、插件商店 |
| 任务 | [tasks.md](tasks.md) | 异步任务管理 |
| 媒体流 | [media.md](media.md) | 音频流、HLS、封面代理、缓存 |
| 系统 | [system.md](system.md) | 健康检查、统计报表、指标、配置、日志 |
| 通知与事件 | [notifications.md](notifications.md) | Webhook 事件、自定义请求头、Body 模板与测试发送 |
| 工具 | [tools.md](tools.md) | 正则生成等工具接口 |
| WebSocket | [websocket.md](websocket.md) | 实时播放进度同步 |
| 错误处理 | [errors.md](errors.md) | 错误格式与状态码 |

## 通用约定

### URL 前缀

大部分新接口同时支持 `/api/...` 和 `/api/v1/...` 两种前缀。文档中优先写前端当前使用的 `/api` 路径；仅 v1 路径存在的接口会明确写 `/api/v1`。

### 字段命名

后端原始 API 使用 `snake_case`。前端的 `apiClient` 会自动把响应转为 `camelCase`，但第三方或脚本直接调用 API 时应以本文档中的 `snake_case` 为准。

### 鉴权方式

除公共接口外，请求需要携带 JWT：

```http
Authorization: Bearer <token>
```

### 认证流程

1. 调用 `POST /api/auth/login` 获取 Token。
2. 后续 HTTP 请求在 Header 中携带 `Authorization: Bearer <token>`。
3. WebSocket 通过 `?token=<token>` 参数认证。

### 权限模型

- 公共接口：`/api/health`、`/api/stats`、`/api/auth/*`、HLS 播放列表与分片、WebSocket 握手入口。
- 用户接口：普通登录用户可访问，例如播放进度、收藏、书单、个性化设置。
- 管理员接口：需要 `role = admin`，例如媒体库管理、用户管理、系统日志、数据统计、通知与事件、插件管理、缓存管理。

### 事件与统计

用户清空或删除最近收听只隐藏可见历史，不删除章节播放进度，也不删除后台统计使用的 `listening_events`。系统统计报表依赖独立收听事件表，因此不会因用户清空历史而回退。
