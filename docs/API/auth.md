# 认证

## POST /api/auth/register

注册新用户。第一个注册的用户自动成为管理员。

**请求体：**

```json
{
  "username": "string",
  "password": "string"
}
```

**响应：** `201 Created`

```json
{
  "success": true
}
```

---

## POST /api/auth/login

用户登录，获取 JWT Token。

**请求体：**

```json
{
  "username": "string",
  "password": "string"
}
```

**响应：** `200 OK`

```json
{
  "user": {
    "id": "string",
    "username": "string",
    "role": "admin | user"
  },
  "token": "string (JWT)"
}
```

登录成功会写入登录日志，并触发 `user.login` Webhook，`login_method` 为 `password`。

---

## POST /api/auth/session-restore

验证浏览器或客户端已保存的登录会话，并在新的浏览器会话首次恢复时记录登录日志。普通页面刷新应优先调用 `GET /api/me`，不要重复调用本接口。

支持路径：

- `/api/auth/session-restore`
- `/api/v1/auth/session-restore`

**认证方式：**

任选其一：

- `Authorization: Bearer <JWT>`
- Cookie：`ting_reader_token=<JWT>` 或 `auth_token=<JWT>`
- 请求体 `token`

**请求头：**

| Header | 说明 |
| --- | --- |
| `X-Ting-Session-Id` | 浏览器会话 ID，用于服务端去重；同一用户同一会话只记录一次恢复登录 |
| `X-Ting-Device` | 可选，客户端设备信息，会写入登录日志 |

**请求体：**

```json
{
  "token": "string (JWT，可选)",
  "session_id": "string，可选"
}
```

**响应：** `200 OK`

```json
{
  "user": {
    "id": "string",
    "username": "string",
    "role": "admin | user"
  },
  "token": "string (JWT)"
}
```

恢复成功可能触发 `user.login` Webhook，`login_method` 为 `session_restore`。服务端会按用户和会话 ID 做短期去重，避免刷新页面刷登录日志。

---

## POST /api/auth/token-login

使用已有 JWT Token 显式登录。该接口保留给 API/工具客户端使用，Web 登录页不再展示完整 JWT Token 登录入口。

支持路径：

- `/api/auth/token-login`
- `/api/v1/auth/token-login`

**请求体：**

```json
{
  "token": "string (JWT)"
}
```

**响应：** `200 OK`

```json
{
  "user": {
    "id": "string",
    "username": "string",
    "role": "admin | user"
  },
  "token": "string (JWT)"
}
```

登录成功会写入登录日志，并触发 `user.login` Webhook，`login_method` 为 `jwt_token`。

---

## GET /api/me

获取当前登录用户信息。用于普通刷新、页面切换和会话健康检查，不会记录登录日志。

**响应：** `200 OK`

```json
{
  "id": "string",
  "username": "string",
  "role": "admin | user"
}
```
