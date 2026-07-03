# 插件开发指南

Ting Reader 插件开发的关键是：在 `plugin.yml` 声明 capability 和 permissions，在插件代码里通过 HostGateway 调用系统能力，再按 JavaScript、WASM、Native 的桥接差异完成入口和打包。开发者最常写的是“插件如何安全读取书籍、进度、媒体、库文件、缓存和任务”，而不是绕过宿主直接读数据库或拼路径。

先区分两个方向：

- Capability：注册插件能力，决定系统什么时候调用插件。
- HostGateway：插件访问宿主系统，决定插件怎么安全读取或操作系统数据。

## 1. 插件如何调用系统能力

插件不能直接读数据库、拼媒体路径或绕过用户权限。需要读取书籍、章节、进度、媒体地址、存储库文件、缓存或创建任务时，统一调用 HostGateway。HostGateway 会检查 manifest 权限、当前登录用户、用户能访问的书籍或存储库、管理员写权限，以及文件路径是否仍在存储库根目录内。

不同运行时调用的是同一组系统能力，只是桥接方式不同：

1. JavaScript 后台方法使用 `Ting.host.invoke(method, params)`。
2. `web_container` UI 使用 `postMessage` 发送 `method: "host.invoke"`，客户端再转发到 `/api/v1/plugin-host/invoke`。
3. WASM 使用 `ting_env.host_invoke`，再通过 `host_response_size` 和 `host_read_body` 读取 JSON 结果。
4. Native 动态库通过 `plugin_set_host_api` 接收 Host API，再调用 `host_invoke(method, params_json, result_json)`。

```js
async function invokeTool(args) {
  if (args.tool !== "books.search") {
    throw new Error("Unsupported tool: " + args.tool);
  }

  const books = await Ting.host.invoke("books.list", {
    query: args.query,
    limit: args.limit ?? 20
  });

  await Ting.host.invoke("cache.set", {
    key: `last-tool:${args.tool}`,
    value: { query: args.query, total: books.total ?? books.length ?? 0 }
  });

  return books;
}

globalThis.invokeTool = invokeTool;
```

## 2. 最小但完整的 manifest

```yaml
id: assistant-tools
name: Assistant Tools
version: 1.0.0
min_core_version: 1.4.8
runtime: javascript
entry_point: plugin.js
author: Your Name
description:
  zh: 提供客户端入口、工具和后台任务
  en: Provides client entries, tools, and background tasks

capabilities:
  - id: assistant.panel
    kind: ui_extension
    invoke: openAssistant
    slot: global.floating_action
    title: { zh: AI 助手, en: AI Assistant }
    render:
      mode: web_container
      entry: ui/index.html

  - id: books.tools
    kind: tool_provider
    invoke: invokeTool
    tools:
      - name: books.search
        description: Search books the current user can access

  - id: batch.summarize
    kind: task_handler
    invoke: runTask
    task_types: [book.summarize]

permissions:
  - type: books_read
  - type: progress_read
  - type: task_create
  - type: cache_read
  - type: cache_write
```

`id@version` 会成为运行时实例 id，例如 `assistant-tools@1.0.0`。如果没有显式写 `runtime`，后端会根据 `entry_point` 扩展名推断：`.js` 是 JavaScript，`.wasm` 是 WASM，`.dll/.so/.dylib` 是 Native。

## 3. Capability 类型

| kind | 用途 | 典型调用方 |
| --- | --- | --- |
| `metadata_provider` | 搜索和刮削书籍/有声书元数据 | 刮削服务、书籍详情页 |
| `format_handler` | 格式识别、播放 URL、解密、元数据读写 | 媒体流和格式服务 |
| `content_processor` | TXT/PDF 等文档探测、章节、片段、分页 | DocumentReader |
| `ui_extension` / `client_extension` | 客户端按钮、面板、表单或插件 Web UI | Web / Flutter |
| `http_route` | 插件 HTTP 路由，例如 RSS、feed、回调 | 浏览器、第三方服务 |
| `tool_provider` | 可发现、可调用的工具 | AI 助手、其他插件、客户端 |
| `plugin_store` | 插件商店源 | 插件管理页 |
| `task_handler` | 后台自定义任务 | 任务队列 |
| `event_handler` | 系统事件订阅 | 通知事件分发器 |

一个插件可以同时声明多个 capability。运行时文件不决定插件类别，`capabilities` 才决定插件能做什么。

## 4. 如何调用 capability

### 4.1 直接调用

客户端或其他受控入口可以调用：

```http
POST /api/v1/plugins/{pluginId}/capabilities/{capabilityId}/invoke
Content-Type: application/json

{
  "params": {
    "tool": "books.search",
    "query": "三体"
  }
}
```

后端会执行：

```text
capabilityId -> capability.invoke -> PluginManager.invoke_plugin(pluginId, method, params)
```

如果没有写 `invoke`，默认使用 capability 自己的 `id` 作为方法名。实际传给插件的参数会附加 `_context`：

```json
{
  "tool": "books.search",
  "query": "三体",
  "_context": {
    "plugin_id": "assistant-tools@1.0.0",
    "capability_id": "books.tools",
    "route": {
      "authenticated": true,
      "user": {
        "id": "user-id",
        "username": "admin",
        "role": "admin"
      }
    }
  }
}
```

### 4.2 HTTP 路由 capability

```yaml
capabilities:
  - id: rss.feed
    kind: http_route
    invoke: generateRssFeed
    route:
      method: GET
      path: /rss/:library_id.xml
      auth: signed
```

请求 `/api/v1/plugin-routes/rss/main.xml` 时，后端会匹配 `method + path`，解析动态参数，然后调用 `generateRssFeed`：

```json
{
  "method": "GET",
  "path": "/rss/main.xml",
  "params": { "library_id": "main" },
  "query": "",
  "headers": {},
  "body_text": null,
  "body_base64": "",
  "capability_id": "rss.feed",
  "plugin_id": "rss-plugin@1.0.0",
  "context": {
    "authenticated": true
  }
}
```

`auth` 支持：

| auth | 说明 |
| --- | --- |
| `user` | 默认值，只能走登录态 `/api/v1/plugin-routes/*path` |
| `public` | 可走公开前缀 `/api/v1/public/plugin-routes/*path` |
| `signed` | 必须带签名 URL |
| `public_or_signed` | 可公开访问，也可通过签名绑定用户上下文 |

签名 URL 使用：

```http
POST /api/v1/plugin-route-signatures

{
  "method": "GET",
  "path": "/rss/main.xml",
  "expires_in_seconds": 86400,
  "bind_current_user": true
}
```

### 4.3 任务和事件 capability

`task_handler` 会接收任务队列里的自定义任务：

```yaml
capabilities:
  - id: batch.summarize
    kind: task_handler
    invoke: runTask
    task_types:
      - book.summarize
```

插件收到：

```json
{
  "task_id": "task-id",
  "task_type": "book.summarize",
  "data": {},
  "capability_id": "batch.summarize"
}
```

`event_handler` 会接收通知事件：

```yaml
capabilities:
  - id: events.sync
    kind: event_handler
    invoke: onEvent
    events:
      - book.created
      - library.scan_completed
```

插件收到：

```json
{
  "event": "book.created",
  "title": "Book created",
  "message": "...",
  "data": {},
  "occurred_at": "2026-07-01T12:00:00Z",
  "capability_id": "events.sync",
  "plugin_id": "sync-plugin@1.0.0"
}
```

## 5. HostGateway

插件访问核心数据时使用 HostGateway。HostGateway 会同时检查：

- manifest 是否声明了对应权限；
- 当前调用是否有登录用户上下文；
- 当前用户是否能访问目标书籍或存储库；
- 写入类操作是否需要管理员上下文；
- 文件路径是否保持在允许的存储库根目录内。

完整的方法参数、响应格式、Web 容器桥接、WASM 句柄读取和 Native 错误码请看独立文档：[HostGateway 能力调用详解](./hostgateway.md)。本节只保留常用方法速查和最短调用示例。

### 5.1 方法和权限

| 方法 | 权限 | 说明 |
| --- | --- | --- |
| `books.list` / `books.get` | `books_read` | 读取当前用户可访问的书籍 |
| `libraries.list` / `libraries.get` | `books_read` | 读取当前用户可访问的存储库 |
| `chapters.list` / `chapters.get` | `chapters_read` | 读取章节元数据 |
| `progress.recent` | `progress_read` | 读取最近播放进度 |
| `media.get_url` | `media_read_url` 或 `media_read` | 获取受控媒体播放地址 |
| `metadata.write` | `metadata_write` | 创建元数据写入任务，需要管理员上下文 |
| `library.file.list` / `stat` / `read` | `file_read` | 在用户可访问的存储库根目录内读文件 |
| `library.file.write` | `file_write` | 在存储库根目录内写文件，需要管理员上下文 |
| `database.get` / `database.list` | `database_read` | 受控实体读取，不是裸 SQL |
| `database.update` | `database_write` | 受控实体更新，需要管理员上下文 |
| `tasks.create` | `task_create` | 创建后台任务 |
| `cache.get` / `cache.has` | `cache_read` 或 `cache_write` | 读取插件隔离缓存 |
| `cache.set` / `cache.delete` | `cache_write` | 写入插件隔离缓存 |
| `playlists.list` / `playlists.get` | `playlists_read` 或 `playlists_write` | 读取当前用户的播放列表 |
| `playlists.create` / `playlists.update` / `playlists.delete` | `playlists_write` | 维护当前用户的播放列表 |
| `playlists.add_item` / `playlists.remove_item` | `playlists_write` | 追加或删除当前用户的播放列表条目 |
| `favorites.list` | `favorites_read` 或 `favorites_write` | 读取当前用户的收藏 |
| `favorites.add` / `favorites.remove` | `favorites_write` | 维护当前用户的收藏 |
| `user_settings.get` | `user_settings_read` 或 `user_settings_write` | 读取当前用户的设置 |
| `user_settings.set` | `user_settings_write` | 写入当前用户的设置项（value 会 JSON 编码） |

### 5.2 JavaScript 调用示例

```js
async function recentBooks() {
  const context = Ting.host.getContext();
  const recent = await Ting.host.invoke("progress.recent", { limit: 20 });

  const book = context?.book_id
    ? await Ting.host.invoke("database.get", {
        entity: "book",
        id: context.book_id
      })
    : null;

  return { recent, book };
}

globalThis.recentBooks = recentBooks;
```

### 5.3 客户端 Web UI 调用示例

`web_container` UI 页面通过 `postMessage` 调用宿主桥。Web 使用 sandbox iframe，Flutter 使用 WebView，但消息协议一致。

```js
const requestId = crypto.randomUUID();

window.parent.postMessage({
  type: "ting-plugin:request",
  id: requestId,
  method: "host.invoke",
  params: {
    method: "progress.recent",
    params: { limit: 5 }
  }
}, "*");

window.addEventListener("message", (event) => {
  const message = event.data;
  if (message?.type === "ting-plugin:response" && message.id === requestId) {
    console.log(message.result);
  }
});
```

调用当前 capability：

```js
window.parent.postMessage({
  type: "ting-plugin:request",
  id: crypto.randomUUID(),
  method: "capability.invoke",
  params: {
    params: { action: "refresh" }
  }
}, "*");
```

## 6. 客户端 UI 扩展

UI 插件不能直接注入 React 或 Dart。它们声明平台无关的 slot 和 render mode，Web 与 Flutter 客户端读取同一份 manifest。

```yaml
capabilities:
  - id: assistant.float
    kind: ui_extension
    invoke: openAssistant
    slot: global.floating_action
    title: { zh: AI 助手, en: AI Assistant }
    render:
      mode: web_container
      entry: ui/index.html

  - id: book.note
    kind: ui_extension
    invoke: saveNote
    slot: book.detail_action
    title: { zh: 记录想法, en: Quick Note }
    render:
      mode: schema
      submit_label: Save
      schema:
        fields:
          - name: note
            label: Note
            type: textarea
            required: true
```

| slot | 说明 |
| --- | --- |
| `global.floating_action` | 全局悬浮入口 |
| `global.panel` | 全局插件面板入口 |
| `settings.section` | 设置页入口 |
| `book.detail_action` | 书籍详情页动作 |
| `reader.toolbar_action` | 阅读器工具栏动作 |
| `reader.side_panel` | 阅读器侧边栏 |
| `reader.document_viewer` | 文档阅读器入口 |

| render.mode | 说明 |
| --- | --- |
| `action` | 点击后直接调用 capability |
| `schema` | 插件声明表单字段，客户端用原生控件渲染 |
| `web_container` | 插件提供 HTML/JS，客户端用 iframe 或 WebView 承载 |
| `builtin` | 客户端内置通用组件，如 `host_method`、`capability_result`、`document_reader` |

## 7. 内容处理与 DocumentReader

```yaml
capabilities:
  - id: document.reader
    kind: content_processor
    invoke: documentInvoke
    matches:
      extensions: [txt, pdf]
    operations:
      - probe
      - extract_metadata
      - list_sections
      - read_chunk
      - render_page
permissions:
  - type: media_read
```

客户端打开文档时会先对候选处理器调用 `probe`，选择 `supported=true` 且 `confidence` 最高的处理器。后续 `extract_metadata`、`list_sections`、`read_chunk`、`render_page` 会固定到同一个 capability，避免多个文档插件同时存在时错配。

## 8. 插件商店源

插件商店也是普通 capability：

```yaml
capabilities:
  - id: official.plugin_store
    kind: plugin_store
    invoke: listPlugins
```

`listPlugins(params)` 返回插件数组或 `{ plugins: [...] }`。用户刷新商店时，后端会传入 `{ force_refresh: true }` 并跳过缓存。

```js
async function listPlugins(params) {
  const response = await fetch(Ting.config.source_url);
  const data = await response.json();
  return Array.isArray(data) ? { plugins: data } : data;
}

globalThis.listPlugins = listPlugins;
```

如果插件源允许用户配置任意 HTTPS URL，应在 manifest 中明确声明网络权限，让安装前的权限提示准确展示风险。

## 9. 运行时典型示例

### 9.1 JavaScript

```js
async function invokeTool(args) {
  if (args.tool === "books.search") {
    return await Ting.host.invoke("books.list", {
      query: args.query,
      limit: args.limit || 20
    });
  }
  throw new Error("Unknown tool: " + args.tool);
}

globalThis.invokeTool = invokeTool;
```

JavaScript 插件可以声明 `npm_dependencies`：

```yaml
npm_dependencies:
  cheerio: "^1.0.0"
  dayjs: "^1.11.0"
```

运行时只允许 `require` manifest 中声明过的包和插件目录内的相对模块。

### 9.2 WASM

```rust
#[link(wasm_import_module = "ting_env")]
extern "C" {
    fn host_invoke(
        method_ptr: *const u8,
        method_len: i32,
        params_ptr: *const u8,
        params_len: i32
    ) -> i32;
    fn host_response_size(handle: i32) -> i32;
    fn host_read_body(handle: i32, ptr: *mut u8, len: i32) -> i32;
}

#[no_mangle]
pub extern "C" fn invoke(method: *const i8, params: *const i8) -> *mut i8 {
    // 解码 method 和 JSON params。
    // 按 capability.invoke 分发。
    // 需要宿主数据时调用 host_invoke("books.list", {...})。
    todo!()
}
```

WASM 适合可移植 Rust 逻辑、内容处理和计算型任务。网络、缓存、HostGateway 都通过宿主函数调用。

### 9.3 Native

```rust
#[no_mangle]
pub unsafe extern "C" fn plugin_invoke(
    method: *const u8,
    params: *const u8,
    result_ptr: *mut *mut u8,
) -> i32 {
    // 解码 method 和 JSON params。
    // 按 capability.invoke 分发。
    // 需要宿主数据时使用 plugin_set_host_api 注入的 Host API。
    0
}

#[no_mangle]
pub unsafe extern "C" fn plugin_free(ptr: *mut u8) {
    // 释放 plugin_invoke 分配的字符串。
}
```

Native 适合格式处理、流式解密、系统库调用、平台二进制工具供应。发布时通常要按平台分别构建，例如 `windows-x86_64`、`linux-x86_64`、`linux-aarch64`。

## 10. 插件配置 `config_schema`

插件需要让管理员填写 API 密钥、接口地址、模型名、开关、枚举选项或数值参数时，在 `plugin.yml` 里声明 `config_schema`。Ting Reader 会根据这个 schema：

- 在插件管理页显示“配置”按钮和表单；
- 从 `default` 提取初始配置；
- 保存配置到后端插件配置目录；
- 按 schema 校验配置；
- 自动加密敏感字段；
- 在插件运行时把解密后的配置注入 `Ting.config`。

### 10.1 完整写法

推荐使用 JSON Schema object 形式：

```yaml
config_schema:
  type: object
  properties:
    api_base_url:
      type: string
      title:
        zh: API 地址
        en: API endpoint
      description:
        zh: OpenAI 兼容接口地址，可以填写服务根地址或完整 /v1/chat/completions 地址。
        en: OpenAI-compatible endpoint. A service root or full /v1/chat/completions URL is accepted.
      placeholder:
        zh: https://api.example.com/v1/chat/completions
        en: https://api.example.com/v1/chat/completions
      default: https://api.openai.com/v1/chat/completions

    api_key:
      type: string
      format: secret
      x-encrypted: true
      title:
        zh: API 密钥
        en: API key
      description:
        zh: 后端会加密保存，插件运行时通过 Ting.config.api_key 读取明文值。
        en: Stored encrypted by the backend. The plugin reads the plaintext value from Ting.config.api_key at runtime.

    model:
      type: string
      title:
        zh: 模型
        en: Model
      default: gpt-4.1-mini

    mode:
      type: string
      title:
        zh: 处理模式
        en: Processing mode
      default: balanced
      enum:
        - fast
        - balanced
        - accurate
      enum_labels:
        fast:
          zh: 快速
          en: Fast
        balanced:
          zh: 平衡
          en: Balanced
        accurate:
          zh: 准确
          en: Accurate

    temperature:
      type: number
      title:
        zh: 温度
        en: Temperature
      default: 0.2
      minimum: 0
      maximum: 1

    max_candidates:
      type: integer
      title:
        zh: 最大候选数
        en: Max candidates
      default: 8
      minimum: 1
      maximum: 20

    enabled:
      type: boolean
      title:
        zh: 启用增强处理
        en: Enable enhanced processing
      default: true
```

### 10.2 扁平简写

后端也接受扁平写法，会自动包装成 `type: object` + `properties`。适合简单插件：

```yaml
config_schema:
  api_key:
    type: string
    format: secret
    x-encrypted: true
    title: API Key
  source_url:
    type: string
    title: Source URL
    default: https://example.com/plugins.json
```

### 10.3 支持的表单字段

插件管理页当前按以下规则渲染配置表单：

| schema | 后台表单 |
| --- | --- |
| `type: string` | 文本输入框 |
| `type: string` + `enum` | 下拉选择框 |
| `type: number` | 数字输入框 |
| `type: integer` | 数字输入框，保存为数字 |
| `type: boolean` | 复选框 |
| `format: password` / `format: secret` | 密码输入框，并按敏感字段处理 |
| `x-encrypted: true` / `encrypted: true` | 密码输入框，并加密保存 |

`title`、`description`、`placeholder` 可以写字符串，也可以写中英文对象。`enum_labels` 可以按枚举值提供多语言显示名。

```yaml
title:
  zh: API 地址
  en: API endpoint
description:
  zh: 用于访问远程服务。
  en: Used to access the remote service.
placeholder:
  zh: 请输入地址
  en: Enter endpoint
enum_labels:
  clean:
    zh: 使用内置清洗
    en: Use built-in cleanup
```

支持 i18n 的常见字段包括顶层 `description`、搜索字段 `label/placeholder`、结果字段 `label`、配置项 `title/description/placeholder`、UI capability 的 `title/label`。

### 10.4 敏感字段和加密

敏感字段建议同时写 `format: secret` 和 `x-encrypted: true`：

```yaml
api_key:
  type: string
  format: secret
  x-encrypted: true
  title:
    zh: API 密钥
    en: API key
```

后端识别以下任一标记后都会加密保存：

- `x-encrypted: true`
- `encrypted: true`
- `format: password`
- `format: secret`

插件管理页读取配置时，敏感字段会显示为空或占位提示，不会把明文回显给浏览器。用户保存时如果保持密钥不变，前端会用内部占位符保留旧值；插件作者不需要处理这个占位符。

### 10.5 运行时读取配置

JavaScript 插件通过 `Ting.config` 读取配置：

```js
function readConfig() {
  const config = Ting.config || {};
  return {
    apiBaseUrl: String(config.api_base_url || "https://api.openai.com/v1/chat/completions"),
    apiKey: String(config.api_key || ""),
    model: String(config.model || "gpt-4.1-mini"),
    enabled: config.enabled !== false,
    maxCandidates: Number(config.max_candidates || 8),
  };
}

async function search(args) {
  const config = readConfig();
  if (!config.apiKey) {
    Ting.log?.warn?.("api_key is empty; returning fallback result.");
  }
}

globalThis.search = search;
```

WASM 和 Native 插件由宿主在调用时注入对应运行时上下文。需要配置时，优先通过运行时提供的插件配置上下文读取；不要直接读取后端配置文件，也不要假设配置文件路径。

### 10.6 保存和更新配置

插件管理页使用以下接口保存配置：

```http
GET /api/v1/plugins/:id/config
PUT /api/v1/plugins/:id/config
Content-Type: application/json

{
  "config": {
    "api_key": "sk-...",
    "model": "gpt-4.1-mini",
    "enabled": true
  }
}
```

保存后后端会校验 schema、加密敏感字段并通知插件管理器。已运行的插件需要重新加载或由宿主热更新后才能拿到新配置；配置类问题排查时优先执行“保存配置”后再“重新加载插件”。

### 10.7 常见坑

- `config_schema` 只负责插件自身配置，不是存储库的刮削源配置；存储库配置仍写在 library 的 `scraper_config`。
- `default` 只用于初始化和补齐缺失字段，不会覆盖用户已经保存过的值。
- 加密字段不要写在普通 `description` 或日志里；插件日志也不要打印 API key。
- 当前插件管理页不渲染嵌套 object、array 和复杂表单；需要复杂配置时建议拆成多个简单字段，或提供 `ui_extension` 自定义配置面板。
- 修改 `plugin.yml` 里的 `config_schema` 后，需要重新打包/重装或重新加载插件，后台才会看到新的表单。

## 11. trpack 打包

官网 public 目录已经提供 `trpack` 二进制下载：

- Windows x86: [`trpack-1.0.2-windows-amd64`](https://www.tingreader.cn/trpack/trpack-1.0.2-windows-amd64)
- Linux x86: [`trpack-1.0.2-linux-amd64`](https://www.tingreader.cn/trpack/trpack-1.0.2-linux-amd64)
- Linux ARM: [`trpack-1.0.2-linux-arm64`](https://www.tingreader.cn/trpack/trpack-1.0.2-linux-arm64)
- Mac Intel: [`trpack-1.0.2-darwin-amd64`](https://www.tingreader.cn/trpack/trpack-1.0.2-darwin-amd64)
- Mac M系列: [`trpack-1.0.2-darwin-arm64`](https://www.tingreader.cn/trpack/trpack-1.0.2-darwin-arm64)

下载后建议重命名为 `trpack` 或 `trpack.exe`，并放到 PATH 中。Linux 和 macOS 需要先执行 `chmod +x trpack`。

`trpack` 的常用能力：

| 命令 | 用途 |
| --- | --- |
| `init` | 按模板创建插件项目，模板包括 `metadata`、`format`、`ui`、`route`、`content`、`tool` |
| `validate` | 校验插件目录和 `plugin.yml/plugin.yaml` |
| `build` / `pack` | 构建 `.tr` 包，可用 `--include` 添加额外文件、`--json` 输出机器可读摘要 |
| `keygen` | 生成 Ed25519 发布密钥，保持后续升级的发布者身份稳定 |
| `sign` | 给已有 `.tr` 包重新签名 |
| `inspect` | 查看 `.tr` 包元数据、文件表和签名摘要，支持 `--json` |
| `verify` | 校验 `.tr` 包结构、manifest、文件表和签名状态 |
| `unpack` | 解包到目录，便于调试包内文件 |
| `is-tr` | 快速判断文件是否是 `.tr` 插件包 |

```bash
trpack init my-plugin --template ui --id my-plugin --name "My Plugin"
trpack validate my-plugin
trpack build my-plugin --output dist/my-plugin.tr --json
trpack inspect dist/my-plugin.tr --json
trpack unpack dist/my-plugin.tr --output unpacked
trpack is-tr dist/my-plugin.tr
```

插件项目可以独立于主仓库维护。发布前至少执行创建、校验、打包和验证：

```bash
trpack init my-plugin --template tool --id my-plugin --name "My Plugin"
trpack validate my-plugin
trpack build my-plugin --output dist/my-plugin.tr
trpack verify dist/my-plugin.tr
```

公开发布插件时建议使用稳定签名密钥，确保后续升级保持同一发布者身份：

```bash
trpack keygen --key-id my-plugin-release --output keys/private.json --public-output keys/public.json
trpack build . --output dist/my-plugin.tr --sign-key keys/private.json
trpack sign dist/my-plugin.tr --key keys/private.json --output dist/my-plugin.signed.tr
trpack verify dist/my-plugin.signed.tr
```

安装器会检查 `.tr` 包格式、manifest、文件表、签名元数据、服务端版本要求、依赖插件和发布者身份。同一个插件 id 如果来自不同发布者身份，需要先卸载旧插件再安装。
