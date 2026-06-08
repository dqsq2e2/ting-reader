# 错误处理

## 错误响应格式

所有错误响应遵循统一格式：

```json
{
  "error": "error_type",
  "message": "详细错误信息"
}
```

---

## HTTP 状态码

| 状态码 | 说明 |
|--------|------|
| 200 | 成功 |
| 201 | 创建成功 |
| 204 | 删除成功（无内容） |
| 400 | 请求无效 / 验证失败 |
| 401 | 未认证（Token 缺失或过期） |
| 403 | 权限不足（非管理员操作） |
| 404 | 资源不存在 |
| 500 | 服务器内部错误 |

---

## 错误类型

| error | 说明 |
|-------|------|
| `authentication_error` | 认证失败 |
| `permission_denied` | 权限不足 |
| `not_found` | 资源不存在 |
| `validation_error` | 数据验证失败 |
| `invalid_request` | 请求格式错误 |
| `plugin_not_found` | 插件不存在 |
| `plugin_execution_error` | 插件执行错误 |
| `task_error` | 任务错误 |
| `external_service_error` | 外部服务错误 |
