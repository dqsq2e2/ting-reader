# HostGateway 能力调用详解

HostGateway 是插件访问 Ting Reader 核心数据和受控能力的统一入口。插件需要读取书籍、章节、播放进度、媒体地址、库文件、缓存或创建任务时，应调用 HostGateway，而不是直接读取数据库、拼接媒体路径或绕过用户权限。

它和 Capability 的关系可以这样理解：

- Capability：注册插件能力，决定系统什么时候调用插件。
- HostGateway：插件访问宿主系统，决定插件怎么安全读取或操作系统数据。

HostGateway 每次调用都会检查：

- 插件 `plugin.yml` 是否声明了对应 `permissions`。
- 当前调用是否带有已认证用户上下文。
- 当前用户是否能访问目标书籍或存储库。
- 写入类能力是否需要管理员上下文。
- 库文件路径是否仍在存储库根目录内。

## 1. 调用入口和返回外壳

不同运行时调用的是同一组方法名和参数，只是桥接方式不同。

| 场景 | 调用方式 | 成功时拿到什么 |
| --- | --- | --- |
| JavaScript 后台方法 | `await Ting.host.invoke(method, params)` | HostGateway 方法返回的业务 JSON |
| `web_container` UI | `postMessage({ method: "host.invoke", params: { method, params } })` | `ting-plugin:response` 里的 `result` |
| HTTP 中转接口 | `POST /api/v1/plugin-host/invoke` | `{ "result": ... }` |
| WASM | `ting_env.host_invoke` + `host_response_size` + `host_read_body` | 读取到的 JSON 字节 |
| Native | `host_invoke(method, params_json, result_json)` | `result_json` 指向的 JSON 字符串 |

JavaScript 示例：

```javascript
const result = await Ting.host.invoke("books.list", {
  search: "三体",
  limit: 20
});
```

HTTP 中转请求：

```http
POST /api/v1/plugin-host/invoke
Content-Type: application/json

{
  "plugin_id": "assistant-tools@1.0.0",
  "method": "books.list",
  "params": {
    "search": "三体",
    "limit": 20
  }
}
```

HTTP 中转成功响应会包一层 `result`：

```json
{
  "result": {
    "items": [],
    "total": 0,
    "offset": 0,
    "limit": 20
  }
}
```

Web 容器桥接成功响应：

```json
{
  "type": "ting-plugin:response",
  "id": "request-id",
  "ok": true,
  "result": {
    "items": [],
    "total": 0,
    "offset": 0,
    "limit": 20
  }
}
```

HTTP 错误响应使用后端统一错误结构：

```json
{
  "error": "PermissionDenied",
  "message": "Permission denied: Plugin assistant-tools@1.0.0 lacks permission required for host method books.list",
  "trace_id": "f0b75a72-9f87-4f0b-b1bb-3df4c4fbb2b2"
}
```

WASM 和 Native 桥接层不会抛 HTTP 响应；宿主拒绝调用时，返回体通常是：

```json
{
  "error": "Permission denied: Plugin assistant-tools@1.0.0 lacks permission required for host method books.list"
}
```

WASM 的 `host_invoke` 返回响应句柄；负数表示桥接层错误，例如内存、JSON、缺少 HostGateway 或缺少用户上下文。Native 的 `host_invoke` 返回 `0` 表示成功，负数表示桥接层或 HostGateway 调用失败；无论成功或失败，只要 `result_json` 非空，都应读取 JSON 并在用完后调用 `host_free`。

## 2. 方法与权限速查

| 方法 | 权限 | 说明 |
| --- | --- | --- |
| `books.list` | `books_read` 或 `database_read` | 查询当前用户可访问的书籍 |
| `books.get` | `books_read` 或 `database_read` | 读取单本书籍 |
| `libraries.list` | `books_read` 或 `database_read` | 查询当前用户可访问的存储库 |
| `libraries.get` | `books_read` 或 `database_read` | 读取单个存储库 |
| `chapters.list` | `chapters_read` 或 `database_read` | 查询某本书的章节 |
| `chapters.get` | `chapters_read` 或 `database_read` | 读取单个章节 |
| `progress.recent` | `progress_read` 或 `database_read` | 读取当前用户最近播放进度 |
| `media.get_url` | `media_read_url` 或 `media_read` | 获取受控播放地址 |
| `metadata.write` | `metadata_write` | 创建元数据写入任务，需要管理员 |
| `library.file.list` | `file_read` | 列出本地存储库目录 |
| `library.file.stat` | `file_read` | 读取本地存储库文件或目录信息 |
| `library.file.read` | `file_read` | 读取本地存储库文件，最大 20 MB |
| `library.file.write` | `file_write` | 写入本地存储库文件，最大 50 MB，需要管理员 |
| `database.get` | `database_read` | 读取受控实体，不支持裸 SQL |
| `database.list` | `database_read` | 查询受控实体列表，不支持裸 SQL |
| `database.update` | `database_write` | 更新受控实体字段，需要管理员 |
| `tasks.create` | `task_create` | 创建插件自定义后台任务 |
| `cache.get` | `cache_read` 或 `cache_write` | 读取插件隔离缓存 |
| `cache.has` | `cache_read` 或 `cache_write` | 判断插件隔离缓存是否存在 |
| `cache.set` | `cache_write` | 写入插件隔离缓存 |
| `cache.delete` | `cache_write` | 删除插件隔离缓存 |

权限写在 manifest 中：

```yaml
permissions:
  - type: books_read
  - type: chapters_read
  - type: progress_read
  - type: media_read_url
  - type: cache_read
  - type: cache_write
  - type: file_read
    value: library
```

## 3. 书籍、存储库和章节

### books.list

参数：

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `search` | string | 搜索关键词 |
| `tag` | string | 标签过滤 |
| `library_id` | string | 存储库过滤 |
| `limit` | number | 默认 50，范围 1-200 |
| `offset` | number | 默认 0 |

示例：

```javascript
const books = await Ting.host.invoke("books.list", {
  search: "三体",
  limit: 10,
  offset: 0
});
```

返回：

```json
{
  "items": [
    {
      "id": "book-id",
      "title": "三体",
      "author": "刘慈欣",
      "narrator": "演播者",
      "library_id": "library-id",
      "cover_url": "/api/...",
      "description": "..."
    }
  ],
  "total": 1,
  "offset": 0,
  "limit": 10
}
```

### books.get

参数可以使用 `book_id` 或 `id`：

```javascript
const book = await Ting.host.invoke("books.get", {
  book_id: "book-id"
});
```

返回单个书籍对象。若当前用户没有访问权限，会返回 `PermissionDenied`。

### libraries.list / libraries.get

`libraries.list` 支持 `limit` 和 `offset`。管理员可以看到全部存储库，普通用户只能看到自己有权限访问的存储库。

```json
{
  "items": [
    {
      "id": "library-id",
      "name": "Audiobooks",
      "type": "local",
      "url": "",
      "root_path": "/data/audiobooks",
      "last_scanned_at": "2026-07-01T12:00:00Z",
      "created_at": "2026-07-01T08:00:00Z",
      "scraper_config": null
    }
  ],
  "total": 1,
  "offset": 0,
  "limit": 50
}
```

### chapters.list / chapters.get

`chapters.list` 必须传 `book_id`，返回分页结构：

```javascript
const chapters = await Ting.host.invoke("chapters.list", {
  book_id: "book-id",
  limit: 200
});
```

返回：

```json
{
  "items": [
    {
      "id": "chapter-id",
      "book_id": "book-id",
      "title": "第 1 章",
      "path": "001.mp3",
      "duration": 1800,
      "chapter_index": 1
    }
  ],
  "total": 1,
  "offset": 0,
  "limit": 200
}
```

## 4. 进度和媒体地址

### progress.recent

读取当前用户最近播放记录：

```javascript
const recent = await Ting.host.invoke("progress.recent", {
  limit: 20
});
```

返回：

```json
{
  "items": [
    {
      "id": "progress-id",
      "book_id": "book-id",
      "chapter_id": "chapter-id",
      "position": 362,
      "duration": 1800,
      "updated_at": "2026-07-01T12:00:00Z",
      "book_title": "三体",
      "cover_url": "/api/...",
      "library_id": "library-id",
      "chapter_title": "第 1 章",
      "chapter_duration": 1800
    }
  ],
  "limit": 20
}
```

### media.get_url

获取当前用户可访问章节的受控播放地址：

```javascript
const media = await Ting.host.invoke("media.get_url", {
  chapter_id: "chapter-id",
  transcode: "hls",
  seek: "120",
  download: false
});
```

`transcode` 只支持 `hls`、`mp3`、`wav`。返回的 URL 依赖当前登录态：

```json
{
  "chapter_id": "chapter-id",
  "book_id": "book-id",
  "url": "/api/stream/chapter-id?transcode=hls&seek=120",
  "requires_auth": true,
  "auth": "current_user"
}
```

## 5. 库文件读写

库文件方法只面向本地存储库，路径必须是相对路径，不能使用绝对路径或 `..` 跳出存储库根目录。

### library.file.list

```javascript
const files = await Ting.host.invoke("library.file.list", {
  library_id: "library-id",
  path: "三体",
  limit: 100
});
```

返回：

```json
{
  "library_id": "library-id",
  "path": "三体",
  "entries": [
    {
      "name": "001.mp3",
      "path": "三体/001.mp3",
      "is_file": true,
      "is_dir": false,
      "size": 1048576,
      "modified_unix": 1782888000
    }
  ],
  "limit": 100
}
```

### library.file.read

```javascript
const file = await Ting.host.invoke("library.file.read", {
  library_id: "library-id",
  path: "三体/info.json",
  as_text: true
});
```

返回：

```json
{
  "library_id": "library-id",
  "path": "三体/info.json",
  "size": 128,
  "data_base64": "eyJ0aXRsZSI6IuS4ieS9kyJ9",
  "text": "{\"title\":\"三体\"}",
  "entry": {
    "name": "info.json",
    "path": "三体/info.json",
    "is_file": true,
    "is_dir": false,
    "size": 128,
    "modified_unix": 1782888000
  }
}
```

### library.file.write

写入需要管理员上下文。参数可以传 `text` 或 `data_base64`，默认不覆盖已有文件。

```javascript
const written = await Ting.host.invoke("library.file.write", {
  library_id: "library-id",
  path: "三体/plugin-note.json",
  text: "{\"source\":\"plugin\"}",
  overwrite: true
});
```

返回：

```json
{
  "library_id": "library-id",
  "path": "三体/plugin-note.json",
  "size": 19,
  "entry": {
    "name": "plugin-note.json",
    "path": "三体/plugin-note.json",
    "is_file": true,
    "is_dir": false,
    "size": 19,
    "modified_unix": 1782888000
  }
}
```

## 6. 受控数据库和元数据写入

HostGateway 不提供裸 SQL。`database.get`、`database.list` 和 `database.update` 只支持受控实体：`book/books`、`chapter/chapters`、`library/libraries`，`database.list` 额外支持 `progress`。

```javascript
const book = await Ting.host.invoke("database.get", {
  entity: "book",
  id: "book-id"
});
```

`database.update` 需要管理员上下文，并且只能更新白名单字段。

```javascript
const updated = await Ting.host.invoke("database.update", {
  entity: "book",
  id: "book-id",
  patch: {
    title: "三体",
    author: "刘慈欣",
    tags: "科幻,中文"
  }
});
```

`metadata.write` 创建核心元数据写入任务：

```javascript
const task = await Ting.host.invoke("metadata.write", {
  book_id: "book-id"
});
```

返回：

```json
{
  "task_id": "task-id",
  "task_type": "write_metadata",
  "status": "queued",
  "book_id": "book-id"
}
```

## 7. 任务与缓存

### tasks.create

插件只能创建自定义任务，`library_scan` 和 `write_metadata` 属于核心保留任务类型。

```javascript
const task = await Ting.host.invoke("tasks.create", {
  "task_type": "plugin.summarize",
  "name": "生成书籍摘要",
  "priority": "normal",
  "data": {
    "book_id": "book-id"
  }
});
```

返回：

```json
{
  "task_id": "task-id",
  "task_type": "plugin.summarize",
  "status": "queued",
  "handler_count": 1
}
```

`priority` 支持 `low`、`normal`、`high`，其他值按 `normal` 处理。只有已经声明对应 `task_handler.task_types` 的插件任务才能被创建。

### cache.get / cache.set / cache.has / cache.delete

缓存按插件实例隔离，`assistant-tools@1.0.0` 和 `assistant-tools@1.0.1` 是不同命名空间。

```javascript
await Ting.host.invoke("cache.set", {
  key: "last-search",
  value: {
    query: "三体",
    total: 3
  }
});

const cached = await Ting.host.invoke("cache.get", {
  key: "last-search"
});
```

命中返回：

```json
{
  "hit": true,
  "key": "last-search",
  "value": {
    "query": "三体",
    "total": 3
  },
  "created_at": "2026-07-01T12:00:00Z",
  "updated_at": "2026-07-01T12:00:00Z"
}
```

未命中返回：

```json
{
  "hit": false,
  "key": "last-search",
  "value": null
}
```

`cache.has` 返回 `{ "key": "...", "hit": true }`，`cache.delete` 返回 `{ "key": "...", "deleted": true }`。

## 8. Web 容器桥接

`web_container` UI 页面不能直接拿到后端对象，需要通过 `postMessage` 调用宿主。

```javascript
const id = crypto.randomUUID();

window.parent.postMessage({
  type: "ting-plugin:request",
  id,
  method: "host.invoke",
  params: {
    method: "progress.recent",
    params: { limit: 5 }
  }
}, "*");

window.addEventListener("message", (event) => {
  const message = event.data;
  if (message?.type !== "ting-plugin:response" || message.id !== id) return;
  if (!message.ok) {
    console.error(message.error);
    return;
  }
  console.log(message.result);
});
```

调用当前 capability：

```javascript
window.parent.postMessage({
  type: "ting-plugin:request",
  id: crypto.randomUUID(),
  method: "capability.invoke",
  params: {
    params: {
      action: "refresh"
    }
  }
}, "*");
```

指定其他 capability：

```javascript
window.parent.postMessage({
  type: "ting-plugin:request",
  id: crypto.randomUUID(),
  method: "capability.invoke",
  params: {
    capabilityId: "assistant.panel",
    params: {
      action: "open"
    }
  }
}, "*");
```

## 9. WASM 桥接

WASM 插件通过 `ting_env` 导入函数调用 HostGateway：

```rust
#[link(wasm_import_module = "ting_env")]
extern "C" {
    fn host_invoke(
        method_ptr: *const u8,
        method_len: i32,
        params_ptr: *const u8,
        params_len: i32,
    ) -> i32;

    fn host_response_size(handle: i32) -> i32;
    fn host_read_body(handle: i32, ptr: *mut u8, len: i32) -> i32;
}
```

调用流程：

1. 把方法名和参数 JSON 写入 WASM 内存。
2. 调用 `host_invoke`，返回值大于 0 时是响应句柄。
3. 调用 `host_response_size(handle)` 获取响应字节数。
4. 分配缓冲区并调用 `host_read_body(handle, ptr, len)` 读取 JSON；读取后宿主会释放该句柄。
5. 解析 JSON。如果 JSON 中有 `error` 字段，应按调用失败处理。

桥接层常见负数错误码：

| 错误码 | 含义 |
| --- | --- |
| `-1` | WASM 内存访问失败 |
| `-2` | 字符串不是合法 UTF-8 |
| `-3` | 参数不是合法 JSON |
| `-8` | HostGateway 未配置 |
| `-9` | 当前调用缺少认证用户上下文 |
| `-10` | 当前线程没有 Tokio runtime |
| `-11` | 宿主调用线程异常 |
| `-12` | 宿主响应序列化失败 |

## 10. Native 桥接

Native 插件可以导出 `plugin_set_host_api` 接收宿主 API：

```rust
#[repr(C)]
pub struct TingNativeHostApi {
    pub version: u32,
    pub host_invoke: Option<unsafe extern "C" fn(
        method: *const c_char,
        params_json: *const c_char,
        result_json: *mut *mut c_char,
    ) -> i32>,
    pub host_free: Option<unsafe extern "C" fn(ptr: *mut c_char)>,
}
```

调用 `host_invoke` 时：

- `method` 是 `books.list`、`cache.get` 等 HostGateway 方法名。
- `params_json` 是 JSON 字符串；空字符串会按 `{}` 处理。
- `result_json` 返回业务 JSON 或 `{ "error": "..." }`。
- 用完 `result_json` 后必须调用 `host_free`。

常见负数错误码：

| 错误码 | 含义 |
| --- | --- |
| `-1` | 入参指针为空 |
| `-2` | Native HostGateway context 未激活 |
| `-3` | 插件没有配置 HostGateway |
| `-4` | 当前调用缺少认证用户上下文 |
| `-5` | 字符串或 JSON 参数解析失败 |
| `-6` | HostGateway 调用失败，详情在 `result_json.error` |
| `-7` | 响应 JSON 序列化或 CString 构造失败 |

## 11. 常见问题

### 为什么后台任务里调用 HostGateway 被拒绝？

HostGateway 依赖当前认证用户上下文。由系统初始化、公共路由或无用户上下文的后台路径触发时，读取书籍、进度、库文件等能力会被拒绝。需要外部访问又要带用户权限时，使用签名路由并设置 `bind_current_user: true`。

### 为什么 `library.file.read` 不能读绝对路径？

库文件方法只允许读取用户可访问存储库根目录内的相对路径。绝对路径和 `..` 会被拒绝，避免插件越权读取宿主文件系统。

### 什么时候用 `database.get`，什么时候用 `books.get`？

如果只需要书籍、章节、存储库的常规读取，优先使用语义化方法：`books.get`、`chapters.list`、`libraries.list`。`database.*` 适合需要以实体名分发的通用工具，但仍然是受控实体接口，不是 SQL。
