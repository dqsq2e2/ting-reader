# 用户

## 用户信息

### GET /api/me

获取当前登录用户信息。

**响应：** `200 OK`

```json
{
  "id": "string",
  "username": "string",
  "role": "admin | user"
}
```

---

### PATCH /api/me

更新当前用户信息（用户名或密码）。

**请求体：**

```json
{
  "username": "string (可选)",
  "password": "string (可选)"
}
```

**响应：** `200 OK`

```json
{
  "id": "string",
  "username": "string",
  "role": "admin | user"
}
```

---

## 用户设置

### GET /api/settings

获取当前用户设置。

**响应：** `200 OK`

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
  "settings_json": {},
  "updated_at": "RFC3339"
}
```

---

### POST /api/settings

更新用户设置（UPSERT）。

**请求体：**

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
  "widget_css": "string (仅管理员)"
}
```

> 支持额外字段通过 `flatten` 合并到 `settings_json` 中。

**响应：** `200 OK` — 返回完整的 `UserSettingsResponse`

---

## 用户管理（管理员）

### GET /api/users

获取所有用户列表（管理员）。

**响应：** `200 OK`

```json
[
  {
    "id": "string",
    "username": "string",
    "role": "admin | user",
    "created_at": "RFC3339",
    "libraries_accessible": ["string"],
    "books_accessible": ["string"]
  }
]
```

---

### POST /api/users

创建新用户（管理员）。

**请求体：**

```json
{
  "username": "string",
  "password": "string",
  "role": "user (可选，默认 user)",
  "libraries_accessible": ["string (可选)"],
  "books_accessible": ["string (可选)"]
}
```

**响应：** `201 Created`

```json
{
  "message": "User created successfully",
  "user": {
    "id": "string",
    "username": "string",
    "role": "string",
    "created_at": "RFC3339",
    "libraries_accessible": [],
    "books_accessible": []
  }
}
```

---

### PATCH /api/users/:id

更新用户信息（管理员）。

**路径参数：**

| 参数 | 类型 | 说明 |
|------|------|------|
| id | string | 用户 ID |

**请求体：**

```json
{
  "username": "string (可选)",
  "password": "string (可选)",
  "role": "string (可选)",
  "libraries_accessible": ["string (可选)"],
  "books_accessible": ["string (可选)"]
}
```

**响应：** `200 OK` — 返回 `UserActionResponse`

---

### DELETE /api/users/:id

删除用户（管理员，不能删除自己）。

**路径参数：**

| 参数 | 类型 | 说明 |
|------|------|------|
| id | string | 用户 ID |

**响应：** `200 OK`

```json
{
  "message": "User deleted successfully"
}
```
