# 插件能力声明

`capabilities` 为必填的封闭声明数组。每项有插件内唯一的 `id` 和 `kind`。加载时检查字段、操作、工具 schema、重复路由及依赖；调用前核对 `(plugin_id, capability_id, operation)`，请求与响应均接受校验。业务代码不根据运行时或插件 ID 选择能力。

| kind | 声明字段 | 固定操作 |
| --- | --- | --- |
| `metadata_provider` | `operations`、`search_fields`、`result_fields`、可选 `filters_schema`、`auto_scrape`、`book_source` | `search`，可选 `detail`、`list_chapters`、`chapter_detail`、`resolve_audio`、`fetch_cover`；书源必须实现 `search/detail/resolve_audio` |
| `format_handler` | `extensions`、`operations` | `probe`、`extract_metadata`；可选 `get_metadata_read_size`、`write_metadata` |
| `tool_provider` | `invoke: invokeTool`、`tools`，每项有名称、双向 schema、说明、`side_effects` | `invokeTool` |
| `http_route` | `route.method/path/auth` | `handle` |
| `ui_extension` | `slots`、`contexts`、`title`、`render` | `open` |
| `plugin_store` | `operations` | `list_plugins`，可选 `get_plugin` |
| `content_processor` | `extensions`、`operations` | `probe/open/close/cancel`；可选 `extract_metadata/read_text/render_page/seek` |
| `task_handler` | `tasks`，每项有 `task_type/input_schema/output_schema/idempotent` | `run` |
| `event_handler` | `events`，每项有 `name/schema` | `handle` |

## 元数据搜索接口

`metadata_provider` 的 `search` 接收以下请求，例如：

```json
{
  "title": "示例书籍",
  "author": null,
  "narrator": null,
  "page": 1,
  "page_size": 20,
  "filters": {},
  "chapter_candidates": [],
  "context": null
}
```

| 请求字段 | 类型 | 说明 |
| --- | --- | --- |
| `title`、`author`、`narrator` | `string \| null` | 三个字段均须存在；未知值为 `null`，每项最多 512 字节 |
| `page` | 正整数 | 页码，从 1 开始 |
| `page_size` | 整数 | 每页 1–100 条 |
| `filters` | 对象 | 平台搜索条件；省略时为 `{}`，序列化后最多 8 KiB |
| `chapter_candidates` | 数组 | 章节标题处理时提供的 `{id, title}`；省略时为 `[]`，最多 500 条，每个非空 `id` 和 `title` 最多 512 字节 |
| `context` | `object \| null` | 宿主提供的搜索上下文；省略时为 `null`，最多 128 KiB。`mode` 为 `search`、`aggregate` 或 `title_cleanup`，可含 `candidates`（最多 100 项）、`merged_metadata`、`scanner_context`、`scraper_query`（最多 512 字节） |

成功结果的 `data` 为 `SearchPage`：

| 结果字段 | 类型 | 说明 |
| --- | --- | --- |
| `items` | 数组 | 搜索结果，每项为下表的书籍元数据 |
| `page`、`page_size` | 整数 | 页码从 1 开始，`page_size` 为 1–100，`items` 不超过该值；分页大小与源站实际分页一致 |
| `total` | 非负整数或 `null` | 已知总数；未知时为 `null` |
| `has_more` | `boolean \| null` | 是否还有下一页；未知时为 `null` |

每项书籍元数据包含：

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `title` | `string` | 非空标题，最多 512 字节 |
| `id`、`source_url` | `string \| null` | 源站标识和详情地址 |
| `author`、`narrator`、`cover_url`、`intro` | `string \| null` | 作者、演播者、封面地址和简介 |
| `subtitle`、`publisher`、`language`、`genre` | `string \| null` | 副标题、出版方、语言和分类 |
| `published_year` | 整数或 `null` | 出版年份，范围 0–65535 |
| `published_date` | `string \| null` | `YYYY-MM-DD` 格式的出版日期 |
| `isbn`、`asin` | `string \| null` | 书籍标识 |
| `explicit`、`abridged` | `boolean \| null` | 内容分级和是否删节 |
| `duration` | 非负整数或 `null` | 时长，单位为秒 |
| `score` | 数值或 `null` | 匹配评分，范围 0–1 |
| `tags` | 字符串数组 | 未知时为 `[]`，最多 64 项，每项非空且最多 256 字节 |
| `chapter_title_template` | `string \| null` | 章节标题模板 |
| `chapter_titles` | 字符串数组 | 未知时为 `[]`，最多 500 项，每项非空且最多 512 字节 |

JavaScript 使用 `publishSearch`，Rust 使用 `publish_search_for_request` 整理解析结果。对外结果包含上表全部字段，可空字段显式为 `null`，未知数组为 `[]`；平台内部的额外字段保留在插件内部。实现示例见[开发指南](./plugin-dev.md#4-实现业务)。

## 书源

普通刮削插件为已有音频补充元数据。将同一个 `metadata_provider` 声明为书源后，用户还可以在搜索页选择它，搜索远程书籍并加入书架，无需提前准备本地音频。省略 `book_source` 或设置为 `false` 时保持原有行为，无需修改旧插件。

```yaml
capabilities:
  - id: metadata.search
    kind: metadata_provider
    book_source: true
    operations: [search, detail, list_chapters, resolve_audio]
    search_fields:
      - key: title
        label: { zh: 书名, en: Title }
        required: true
        type: text
    result_fields:
      - key: title
        label: { zh: 书名, en: Title }
permissions:
  - type: network_access
    domain: books.example.com
  - type: network_access
    domain: media.example.com
```

书源仍使用现有插件运行时和能力调度，不引入其他书源格式的适配器。搜索、详情和媒体域名均须声明网络权限。一个插件声明多个元数据能力时，现有刮削入口使用第一项，`book_source` 必须声明在这一项上。

- `search`：请求及 `SearchPage` 与上文相同。每本可加入的书必须提供稳定的 `id`。
- `detail`：输入为 `{ "id": "book-1", "url": null }`。只有书源返回 `SourceBookDetail`，即 `{ "metadata": ScraperResult, "chapters_complete": boolean, "chapters": ChapterItem[] }`；普通刮削器的 `detail` 仍返回 `ScraperResult`。`chapters_complete` 缺省为 `true`，兼容旧书源；声明 `list_chapters` 的书源可以设为 `false`，只返回首批章节。
- `list_chapters`：输入为 `{ "source": { "id": "book-1", "url": null }, "page": 1, "page_size": 100 }`，返回 `{ "items": ChapterItem[], "page": 1, "page_size": 100, "total": number | null, "has_more": boolean | null }`。宿主只在书籍详情需要对应页时调用并缓存该页，避免加入书架时拉取完整目录。
- `resolve_audio`：输入为 `{ "chapter_id": "chapter-1" }`，输出为下面的媒体描述。播放、下载、缓存或转码需要读取音频时才解析，不将临时地址和认证头保存到书籍记录。

`metadata` 使用上文完整元数据字段，`id` 必须稳定、非空且不超过 512 字节，并与请求的 ID 一致。`chapters_complete: true` 时，`chapters` 为完整目录，包含 1–20000 项；为 `false` 时允许为空或只包含首批章节，但能力声明必须同时包含 `list_chapters`。每项包括 `id`、`title`、`index`、`duration` 和 `source_url`，未知时长和来源地址填 `null`。章节 ID、标题非空且最多 512 字节，ID 和排序索引在同一本书中唯一，索引、秒数不超过 `2147483647`。

```json
{
  "type": "proxy",
  "url": "https://media.example.com/audio/one.mp3",
  "headers": { "Referer": "https://books.example.com/" },
  "expires_at": null,
  "seekable": true,
  "mime": "audio/mpeg"
}
```

这些是成功信封的 `data`，仍需通过 SDK `success(data)` 或 Rust 的对应结果接口返回。

`headers` 可省略，最多 32 项；名称最多 128 字节、值最多 8192 字节，不接受换行、传输控制头或覆盖 `Host`、`Range`、`Accept-Encoding`。`expires_at` 为 RFC 3339 时间或 `null`，已过期地址会被拒绝。媒体代理始终在服务端携带这些头，并核对网络权限、DNS 解析地址及每次重定向，跨源跳转移除认证头和 Cookie。

当前书源播放入口支持绝对 HTTP(S) 直链音频，`type` 为 `direct` 或 `proxy`，两者均经服务端代理，支持 Range 和已有 MP3/HLS 输出转码。这里的 HLS 输出转码不同于源站返回 HLS 播放列表：源站 `.m3u8`、`type: hls` 和相对 Host 路由暂不支持，会明确报错。插件不能将播放列表伪装为直链音频。

加入书架使用现有书籍、章节及权限表。系统按稳定插件 ID 自动创建托管库，类型为 `source`，绑定信息保存在 `scraper_config`。在线托管库会像其他存储库一样出现在书架筛选和存储库管理中，但不参与文件夹扫描；在管理页的编辑入口可以查看并删除该在线库中已加入的书籍。同一源书按「插件 ID + 源书 ID」去重，插件升级不产生重复书籍；普通用户只获得自己加入的书的访问权限。

## 扩展格式

```yaml
capabilities:
  - id: format.handler
    kind: format_handler
    extensions: [example]
    operations: [probe, get_metadata_read_size, extract_metadata, write_metadata]
```

此声明只注册支持的扩展名和操作；格式解析和元数据规则由插件实现。宿主以扩展名筛选、`probe` 验证，使用宿主资源句柄调用；Rust 插件通过 `ting_plugin_sdk::contract::format` 和 `format_calls` 使用元数据及 staging 写回类型。普通音频和 FFmpeg 管理属于核心。

## UI 与权限

```yaml
capabilities:
  - id: assistant.panel
    kind: ui_extension
    slots: [app.sidebar_page, global.floating_action]
    contexts: [global]
    title: { zh: 助手, en: Assistant }
    render:
      mode: web_container
      entry: ui/panel.html
      bridge:
        capabilities: [assistant.tools]
        host_methods: [user_settings.get]
permissions:
  - type: user_settings_read
```

合法的 `slots` 为 `app.sidebar_page`、`global.floating_action`、`global.panel` 和 `book.detail_action`；`contexts` 为 `global/book/reader`；`render.mode` 为 `web_container/action`。`web_container` 的页面在宿主的 sandbox 中运行，`bridge` 白名单与当前授权一起校验。网络权限使用 `network_access.domain`，文件权限使用 `file_read.path` / `file_write.path`，事件订阅使用 `event_subscribe.event`；书籍和库的读取权限分别使用 `books_read` 与 `libraries_read`。

运行时具体写法见 [JavaScript](./js_runtime_guide.md)、[WASM](./wasm_runtime_guide.md) 和 [Native](./native_runtime_guide.md) 指南。
