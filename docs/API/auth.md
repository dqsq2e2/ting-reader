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
