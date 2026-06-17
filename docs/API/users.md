# 用户

## 当前用户

### GET /api/me

获取当前登录用户信息。

响应：`200 OK`

```json
{
  "id": "string",
  "username": "string",
  "role": "admin | user"
}
```

### PATCH /api/me

更新当前用户信息。

请求体：

```json
{
  "username": "string",
  "password": "string"
}
```

所有字段均可选。

响应：`200 OK`

```json
{
  "id": "string",
  "username": "string",
  "role": "admin | user"
}
```

## 用户设置

### GET /api/settings

获取当前用户设置。

响应：`200 OK`

```json
{
  "user_id": "string",
  "playback_speed": 1.0,
  "theme": "auto",
  "auto_play": true,
  "skip_intro": 0,
  "skip_outro": 0,
  "sleep_timer_default": 0,
  "auto_preload": true,
  "auto_cache": false,
  "widget_css": "string | null",
  "settings_json": {
    "homeLayout": {
      "showHero": true,
      "showStats": true,
      "showRecommended": true,
      "showRecent": true,
      "showRecentlyAdded": true,
      "showCollections": true
    }
  },
  "updated_at": "RFC3339"
}
```

说明：

- 普通用户响应中会隐藏 `widget_css`，并强制 `auto_cache = false`。
- `settings_json` 为扩展配置容器，前端当前会把首页展示配置写入 `homeLayout`。

### POST /api/settings

更新当前用户设置。接口为 UPSERT。

请求体：

```json
{
  "playback_speed": 1.0,
  "theme": "auto | light | dark",
  "auto_play": true,
  "skip_intro": 0,
  "skip_outro": 0,
  "sleep_timer_default": 0,
  "auto_preload": true,
  "auto_cache": false,
  "widget_css": "string",
  "homeLayout": {
    "showHero": true,
    "showStats": true,
    "showRecommended": true,
    "showRecent": true,
    "showRecentlyAdded": true,
    "showCollections": true
  }
}
```

说明：

- `auto_cache` 和 `widget_css` 仅管理员可更新，普通用户提交会被忽略。
- 除系统字段外，额外字段会合并进 `settings_json`。
- 为避免递归，`settings_json` / `settingsJson` / `user_id` / `updated_at` 不会写入扩展配置。

响应：`200 OK`

返回完整 `UserSettingsResponse`。

## 用户管理（管理员）

### GET /api/users

获取所有用户列表。

响应：`200 OK`

```json
[
  {
    "id": "string",
    "username": "string",
    "role": "admin | user",
    "created_at": "RFC3339",
    "libraries_accessible": ["library-id"],
    "books_accessible": ["book-id"]
  }
]
```

### POST /api/users

创建用户。

请求体：

```json
{
  "username": "string",
  "password": "string",
  "role": "user",
  "libraries_accessible": ["library-id"],
  "books_accessible": ["book-id"]
}
```

响应：`201 Created`

```json
{
  "message": "User created successfully",
  "user": {
    "id": "string",
    "username": "string",
    "role": "user",
    "created_at": "RFC3339",
    "libraries_accessible": [],
    "books_accessible": []
  }
}
```

### PATCH /api/users/:id

更新用户信息与访问权限。

路径参数：

| 参数 | 类型 | 说明 |
| --- | --- | --- |
| `id` | string | 用户 ID |

请求体：

```json
{
  "username": "string",
  "password": "string",
  "role": "admin | user",
  "libraries_accessible": ["library-id"],
  "books_accessible": ["book-id"]
}
```

所有字段均可选。

响应：`200 OK`

返回 `UserActionResponse`。

### DELETE /api/users/:id

删除用户。管理员不能删除自己。

路径参数：

| 参数 | 类型 | 说明 |
| --- | --- | --- |
| `id` | string | 用户 ID |

响应：`200 OK`

```json
{
  "message": "User deleted successfully"
}
```
