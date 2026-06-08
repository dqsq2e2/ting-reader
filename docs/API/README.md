# Ting-Reader API 文档

> 版本: v1 | 基础 URL: `http://<host>:<port>`

## 模块索引

| 模块 | 文件 | 说明 |
|------|------|------|
| [认证](auth.md) | auth.md | 注册、登录、JWT Token |
| [用户](users.md) | users.md | 用户信息、用户管理、用户设置 |
| [播放进度](progress.md) | progress.md | 获取/更新播放进度 |
| [收藏](favorites.md) | favorites.md | 收藏管理 |
| [媒体库](libraries.md) | libraries.md | 媒体库 CRUD、扫描、WebDAV 连接测试 |
| [系列](series.md) | series.md | 系列 CRUD |
| [书籍](books.md) | books.md | 书籍 CRUD、章节管理、刮削、合并 |
| [搜索与刮削](search.md) | search.md | 在线搜索、刮削数据源 |
| [插件](plugins.md) | plugins.md | 插件管理、插件商店 |
| [任务](tasks.md) | tasks.md | 异步任务管理 |
| [媒体流](media.md) | media.md | 音频流、HLS、封面代理、缓存 |
| [系统](system.md) | system.md | 健康检查、统计、指标、配置、日志 |
| [工具](tools.md) | tools.md | 正则生成等工具接口 |
| [WebSocket](websocket.md) | websocket.md | 实时进度同步 |
| [错误处理](errors.md) | errors.md | 错误格式与状态码 |

## 通用约定

### URL 前缀

大部分 API 同时支持 `/api/...` 和 `/api/v1/...` 两种前缀（文档中以 `/api` 为准）。

### 字段命名

- **请求体**：使用 `snake_case`
- **响应体**：使用 `snake_case`（后端原始格式）

> 注意：前端 JavaScript 客户端会自动将响应字段转为 camelCase，但直接调用 API 时响应为 snake_case。第三方客户端开发应以本文档中的 snake_case 为准。

### 鉴权方式

除公共接口外，所有请求需在 Header 中携带 JWT Token：

```
Authorization: Bearer <token>
```

### 认证流程

1. 调用 `POST /api/auth/login` 获取 Token
2. 后续请求在 Header 中携带 `Authorization: Bearer <token>`
3. WebSocket 通过 `?token=<token>` 参数认证

### 权限模型

- **公共接口**：无需认证（`/api/health`、`/api/stats`、`/api/auth/*`、HLS 流、WebSocket）
- **用户接口**：需认证，普通用户可访问
- **管理员接口**：需认证且角色为 `admin`（用户管理、媒体库管理、缓存管理、任务管理等）
