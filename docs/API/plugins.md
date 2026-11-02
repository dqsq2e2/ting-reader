# 插件管理与调用

以下接口需要登录；同时提供 `/api/...` 和 `/api/v1/...` 前缀。本文使用 `/api/v1`。安装、卸载、重新加载、修改配置、商店安装、清除商店缓存和查询日志需要管理员身份。读取接口按插件的 `admin_only` 和当前用户权限过滤。

## 插件身份和声明

清单中的 `id` 是稳定插件 ID，例如 `example-metadata`。已安装实例的 ID 包含版本，例如 `example-metadata@1.0.0`；管理、配置、资产及能力调用使用接口返回的实例 ID。商店安装使用商店条目的稳定 ID。

`capabilities` 保持清单中的结构化声明。每项包含唯一 `id` 和 `kind`，操作由能力定义确定。元数据插件声明 `operations: ["search"]`；工具能力声明 `invoke: "invokeTool"`。能力和输入输出说明见 [能力声明](../plugins/capabilities.md)。

权限使用对象：

```json
[
  { "type": "network_access", "domain": "example.com" },
  { "type": "books_read" },
  { "type": "file_read", "path": "books/*" }
]
```

`network_access`、`file_read/file_write`、`event_subscribe` 分别使用 `domain`、`path`、`event` 作用域；`capability_invoke` 使用目标 `plugin_id` 和 `capability_id`。当前用户、书库及资源授权仍由宿主校验。

## 已安装插件

### GET /api/v1/plugins

返回当前用户可见的已安装实例数组：

```json
[
  {
    "id": "example-metadata@1.0.0",
    "name": "Example Metadata",
    "version": "1.0.0",
    "runtime": "wasm",
    "author": "Example Author",
    "description": "元数据搜索示例",
    "description_i18n": { "zh": "元数据搜索示例", "en": "Metadata search example" },
    "min_core_version": "2.0.0",
    "admin_only": false,
    "is_enabled": true,
    "state": "active",
    "error": null,
    "stats": {
      "total_calls": 0,
      "successful_calls": 0,
      "failed_calls": 0,
      "avg_execution_time_ms": 0.0
    },
    "permissions": [{ "type": "network_access", "domain": "example.com" }],
    "capabilities": [{
      "id": "metadata.search",
      "kind": "metadata_provider",
      "operations": ["search"],
      "auto_scrape": true,
      "search_fields": [{ "key": "title", "label": { "zh": "书名", "en": "Title" }, "type": "text", "required": true }],
      "result_fields": [{ "key": "title", "label": { "zh": "书名", "en": "Title" } }]
    }],
    "scraper": {
      "auto_scrape": true,
      "search_fields": [{ "key": "title", "label": "书名", "label_i18n": { "zh": "书名", "en": "Title" }, "required": true, "type": "text" }],
      "result_fields": ["title"],
      "result_field_labels": { "title": { "zh": "书名", "en": "Title" } }
    }
  }
]
```

可选字段还包括 `license`、`repo`、`min_flutter_version` 和 `config_schema`。`scraper` 是供搜索表单使用的派生摘要；能力声明以 `capabilities` 为准。`runtime` 为 `javascript`、`wasm` 或 `native`。

`state` 为 `discovered/loading/loaded/initializing/active/executing/unloading/unloaded/failed`。`is_enabled` 表示实例已注册，判断是否可调用应同时检查 `state` 和 `error`。

失败状态的 `error` 是统一的 `error/message/details/trace_id` 对象，正常状态为 `null`。`error.message` 用于展示；`error.details.reason` 和 `error.details.retryable` 保留加载、依赖、超时等分类，格式见[错误处理](errors.md)。

### GET /api/v1/plugins/:id

返回指定实例的详情。除列表中的业务字段外，包含：

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `entry_point` | string | 包内业务入口 |
| `dependencies` | object[] | 每项包含 `plugin_name`、`version_requirement` |
| `supported_extensions` | string[] 或 null | 声明的扩展名摘要 |
| `config_schema` | object 或 null | 插件配置表单 schema |

### POST /api/v1/plugins/install

管理员以 `multipart/form-data` 上传插件包：

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `file` | 文件 | `trpack build` 生成并签名的 `.tr` 包 |
| `accept_unverified` | boolean 文本 | 默认 false；确认安装未受信任发布者的已签名包时设为 true |
| `confirmed_package_sha256` | string | 确认时提供 `428` 响应中实际插件包的 SHA-256，与 `accept_unverified=true` 一起提交 |

上传上限 50 MiB。宿主校验清单、最低版本、能力、入口、包结构、文件哈希和签名。最多 10,000 个条目，单文件展开后最大 128 MiB，总展开大小最大 256 MiB。JavaScript 依赖须在开发机完成构建，并作为相对模块纳入包。

成功返回 `201 Created`，安装后自动加载和初始化：

```json
{
  "plugin_id": "example-metadata@1.0.0",
  "message": "Plugin example-metadata@1.0.0 installed successfully"
}
```

未受信任发布者的有效签名包返回 `428 Precondition Required`：

```json
{
  "requires_confirmation": true,
  "verification_status": "untrusted",
  "plugin_id": "example-metadata",
  "plugin_name": "Example Metadata",
  "plugin_version": "1.0.0",
  "publisher": "未知发布者",
  "warning": "服务端提供的安装确认提示",
  "runtime": "javascript",
  "permissions": [{ "type": "network_access", "domain": "example.com" }],
  "capabilities": [],
  "package_sha256": "64位小写十六进制SHA-256",
  "package_changed": false
}
```

客户端展示包内实际请求的 `permissions` 及域名、路径、目标插件等范围，`capabilities` 作为功能摘要；无权限时明确显示未申请宿主权限。原生插件需说明声明权限无法约束其全部系统操作。确认后上传同一个包并同时提交 `accept_unverified=true` 和 `confirmed_package_sha256`。单独提交确认布尔值不能跳过确认。

重新提交的包摘要不一致或缺失时，返回当前包的 `428` 响应及 `package_changed: true`，客户端重新展示权限并确认。未签名或签名无效的包会被拒绝，确认不能绕过签名校验。升级保持同一发布者签名身份；发布者身份不同的包不能覆盖已安装实例。

### DELETE /api/v1/plugins/:id

管理员卸载实例，返回 `200 OK`：

```json
{ "message": "Plugin example-metadata@1.0.0 uninstalled successfully" }
```

### POST /api/v1/plugins/:id/reload

管理员重新创建已安装实例，返回 `200 OK` 和 `message`。重新加载使用已安装文件；更新代码应先重新编译、打包并安装。

## 插件配置

### GET /api/v1/plugins/:id/config

返回当前用户可见实例的配置，敏感字段按 schema 脱敏：

```json
{
  "plugin_id": "example-metadata@1.0.0",
  "config": { "endpoint": "https://example.com", "api_key": "" }
}
```

脱敏值以实际响应为准；客户端保存未修改的敏感字段时保留服务端返回的占位值，不把它当作业务密钥。

### PUT /api/v1/plugins/:id/config

管理员保存配置，服务端校验 schema、保留未修改的敏感值、加密存储并重新加载实例。

```json
{ "config": { "endpoint": "https://example.com", "api_key": "new-secret" } }
```

成功返回 `200 OK` 和 `message`。配置表单说明见 [配置指南](../plugins/plugin-config.md)。

## 插件商店

### GET /api/v1/store/plugins

返回商店条目数组；查询参数 `refresh=true` 刷新缓存。普通用户看不到 `admin_only: true` 的条目。

每项包含稳定 `id`、`name`、`description`、`description_i18n`、`version`、`runtime`、`capabilities`，以及可选的 `author/license/repo/permissions/dependencies/config_schema/min_core_version/min_flutter_version/admin_only/size/date/downloads`。

权限和能力结构与包内清单一致。下载地址必须对应该条目版本的签名包：

```json
{
  "id": "example-metadata",
  "name": "Example Metadata",
  "description": "元数据搜索示例",
  "version": "1.0.0",
  "runtime": "wasm",
  "min_core_version": "2.0.0",
  "download_url": "https://example.com/plugins/example-metadata-1.0.0.tr",
  "permissions": [{ "type": "network_access", "domain": "example.com" }],
  "capabilities": [{
    "id": "metadata.search", "kind": "metadata_provider", "operations": ["search"],
    "search_fields": [], "result_fields": []
  }]
}
```

Native 包的 `download_url` 可以是平台映射：

```json
{
  "linux-x86_64": "https://example.com/plugin-linux-amd64.tr",
  "linux-aarch64": "https://example.com/plugin-linux-arm64.tr",
  "windows-x86_64": "https://example.com/plugin-windows-amd64.tr",
  "macos-x86_64": "https://example.com/plugin-darwin-amd64.tr",
  "macos-aarch64": "https://example.com/plugin-darwin-arm64.tr"
}
```

### POST /api/v1/store/install

管理员请求安装指定稳定 ID：

```json
{ "plugin_id": "example-metadata", "accept_unverified": false }
```

成功返回 `201 Created`，结构与上传安装相同。发布者确认使用相同的 `428` 响应，确认后重新提交 `accept_unverified: true` 和响应中的 `confirmed_package_sha256`。商店重新下载后校验实际包摘要；包变化则重新要求确认。

仅下载 HTTPS 地址，最多跟随 5 次重定向，每跳检查地址；单包最大 50 MiB。下载结束后执行与上传相同的安装校验。

### POST /api/v1/store/cache/clear

管理员清除商店缓存，返回 `200 OK` 和 `message`。之后可通过 `GET /api/v1/store/plugins?refresh=true` 获取更新后的条目。

## 能力发现与调用

### GET /api/v1/plugin-capabilities

列出当前用户可见的已注册能力；可用 `kind` 查询参数过滤。九类能力为 `metadata_provider/format_handler/tool_provider/http_route/ui_extension/plugin_store/content_processor/task_handler/event_handler`。

```json
[
  {
    "plugin_id": "example-panel@1.0.0",
    "plugin_name": "Example Panel",
    "admin_only": false,
    "client_grant": "<opaque-client-grant>",
    "capability": {
      "id": "assistant.panel",
      "kind": "ui_extension",
      "slots": ["global.panel"],
      "contexts": ["global"],
      "title": { "zh": "助手", "en": "Assistant" },
      "render": {
        "mode": "web_container",
        "entry": "ui/panel.html",
        "bridge": { "capabilities": ["assistant.tools"], "host_methods": ["user_settings.get"] }
      }
    }
  }
]
```

只有 UI 注册项返回 `client_grant`。凭据绑定当前用户、插件实例、来源 UI 和有效期；由受信客户端保存，过期后重新获取。不要写入日志或发送给第三方。

### POST /api/v1/plugins/:plugin_id/capabilities/:capability_id/invoke

受信客户端转发 UI 能力请求：

```json
{
  "ui_capability_id": "assistant.panel",
  "ui_grant": "<opaque-client-grant>",
  "params": { "tool_name": "books.search", "params": { "query": "示例" } }
}
```

宿主验证来源 UI、签名、用户和 `render.bridge.capabilities`。UI 调用固定为 `open`，工具调用固定为 `invokeTool`；其他能力在 `params.operation` 中指定已声明的操作。宿主附加可信 `_context`，页面提交的数据不能覆盖该上下文。

只有内容处理能力的受控读取入口可以不携带来源 UI。其他客户端调用均需同时提供 `ui_capability_id` 和 `ui_grant`；内部搜索、格式、任务和事件调度使用各自业务入口。

成功返回 `200 OK`，`result` 保留插件结果信封：

```json
{ "result": { "ok": true, "data": { "items": [] } } }
```

### GET /api/v1/plugin-capabilities/content-processors

查询参数：`extension` 必填，`operation` 可选。操作为 `probe/open/close/cancel/extract_metadata/read_text/render_page/seek`，仅返回声明了对应操作的处理器。

### GET /api/v1/plugin-capabilities/tools

查询参数 `name` 可选，过滤工具名称。响应项包含插件实例、`admin_only`、能力声明及匹配的 `tool`。

### GET /api/v1/plugin-capabilities/task-handlers

查询参数 `task_type` 可选，返回匹配的任务能力注册项。

### GET /api/v1/plugin-capabilities/event-handlers

查询参数 `event` 可选，返回匹配的事件能力注册项。

## 刮削与书源

以下接口均需要登录。标记 `admin_only` 的插件只对管理员开放。

### GET /api/v1/scraper/sources

返回 `{ "sources": [...] }`。每项含 `id`（插件实例 ID）、`name`、`version`、`enabled`、`book_source`、`auto_scrape`、搜索字段和结果字段。搜索页仅将 `enabled: true` 且 `book_source: true` 的插件列为书源；未声明书源能力的旧刮削插件不变。

### POST /api/v1/scraper/search

```json
{
  "source": "example-source@1.0.0",
  "query": "示例书籍",
  "page": 1,
  "page_size": 20
}
```

返回 `{ "items": [...], "page": 1, "page_size": 20, "total": null, "has_more": false }`。分页大小以源站实际返回值为准。指定书源搜索时，结果中的 `added_book_id` 为当前用户可访问的已加入书籍 ID，未加入时为 `null`。常规刮削搜索的响应保持不变。

### POST /api/v1/scraper/books

```json
{
  "source": "example-source@1.0.0",
  "book": { "id": "book-1", "url": null }
}
```

后端调用书源 `detail`，验证元数据和首批章节后，在同一事务内持久化书籍、已返回章节以及当前用户的访问授权。书籍详情页的章节分页会按需调用 `list_chapters`，并对章节 ID 幂等写入，避免加入书架时一次拉取完整目录。未声明 `list_chapters` 的旧书源仍须在 `detail` 返回完整章节目录。系统自动创建 `source` 托管库，不需要用户创建存储库；该库会正常出现在存储库列表中，但不支持文件夹扫描，管理员可从编辑入口管理和删除其中已加入的书籍。失败时不留下半本书或空托管库。

```json
{ "book_id": "source-...", "already_added": false }
```

重复或并发加入不会重复建书；已有书籍返回相同的 `book_id` 和 `already_added: true`，并为当前用户补充该书的授权，不授予整个源库的访问权限。身份取自登录信息，不接受客户端传入用户或库 ID。

加入后继续使用现有书籍详情、章节和 `/api/stream/:chapterId` 接口。服务端在读取音频时调用 `resolve_audio`，携带源站请求头代理媒体并支持 Range，客户端不会拿到源站认证头。源插件停用或卸载时，未缓存的媒体无法重新解析；重新安装相同插件 ID 的兼容版本可继续播放。能力输入输出及当前支持的媒体类型见[书源声明](../plugins/capabilities.md#书源)。

## HostGateway

### POST /api/v1/plugin-host/invoke

由受信客户端转发页面的 Host 请求：

```json
{
  "plugin_id": "example-panel@1.0.0",
  "ui_capability_id": "assistant.panel",
  "ui_grant": "<opaque-client-grant>",
  "method": "user_settings.get",
  "params": {}
}
```

这三个身份字段均为必填。后端验证 UI 来源、凭据、`render.bridge.host_methods` 白名单、清单权限及当前用户对目标资源的授权。成功返回 `200 OK` 和 `{ "result": ... }`。

Host 包含书籍、媒体、配置、私有文件、持久存储、HTML、跨插件能力、任务、事件、缓存和用户数据入口；权限和资源生命周期见 [Host 接口](../plugins/hostgateway.md)。后台 JavaScript、WASM 和 Native 通过各自运行时的 Host 接口访问这些方法。

## UI 资产

### GET /api/v1/plugin-assets/:client_grant/:plugin_id/*path

读取 `active` 或 `executing` 实例的 `ui/`、`assets/` 文件。凭据绑定用户、实例和来源 UI；服务端再次检查用户、插件可见性、规范化路径、符号链接和目录范围。单文件上限 64 MiB，流式返回。

响应禁止缓存和直接执行，并使用 `nosniff`、禁止 framing 的响应头和限制性 CSP。HTML 由受信客户端读取，注入 CSP、资源基址及桥接脚本后放入 sandbox。

页面使用每文档随机 `bridgeToken` 和宿主创建的 `MessagePort`，通过 `window.__TING_PLUGIN_BRIDGE__.postMessage()` 调用。`bridgeToken` 用于页面通信；HTTP 凭据由客户端附加。页面代码、消息格式、主题和语言处理见 [界面与日志](../plugins/ui-logging-migration.md)。

## 插件 HTTP 路由

### ANY /api/v1/plugin-routes/*path

调用声明 `route.auth: authenticated` 的路由，需要用户登录。方法和路径必须匹配清单，由宿主调用固定 `handle` 操作。

### ANY /api/v1/public/plugin-routes/*path

调用声明 `route.auth: public` 的路由。声明 `require_signature: true` 时需要有效签名；签名绑定用户时恢复该用户的权限上下文。返回状态、允许的响应头和正文由插件的路由结果决定。

### POST /api/v1/plugin-route-signatures

为已声明的公共插件路由签名，默认绑定当前用户：

```json
{ "method": "GET", "path": "/rss/library-id.xml", "expires_in_seconds": 86400, "bind_current_user": true }
```

成功返回：

```json
{
  "path": "/rss/library-id.xml",
  "expires": 1790000000,
  "signature": "hex",
  "user_id": "user-id",
  "signed_url": "/api/v1/public/plugin-routes/rss/library-id.xml?expires=1790000000&signature=hex&user=user-id"
}
```

## 插件日志

### GET /api/v1/plugins/:id/logs

### GET /api/v1/plugin-logs

管理员查询指定实例或全部插件日志：

| 参数 | 说明 |
| --- | --- |
| `plugin_id` | 全局查询时按插件过滤 |
| `level` | 日志级别 |
| `source` | `code/lifecycle/runtime/gateway/security` |
| `q` | 检索消息和结构化字段 |
| `since`、`until` | RFC 3339 时间；起始不能晚于结束 |
| `page` | 从 1 开始，默认 1 |
| `page_size` | 默认 100，范围 1–500 |

响应为 `{ "logs": [...], "total": 0, "page": 1, "page_size": 100 }`。日志包含时间、级别、消息和结构化字段；插件身份、运行时及来源由宿主记录，业务 `op` 和结果数量等在 `plugin_fields` 中。

### GET /api/v1/plugins/:id/logs/export

### GET /api/v1/plugin-logs/export

管理员导出文本日志，支持上述过滤参数。响应 `Content-Type: text/plain; charset=utf-8`，带附件文件名。导出按过滤条件获取匹配记录，不按查询列表的页码截断。
