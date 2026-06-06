# 插件开发指南

Ting Reader 支持通过插件扩展功能，包括元数据刮削和音频格式支持。您可以根据需求选择适合的开发方式。

- **JavaScript 刮削插件**: 最简单的开发方式，使用 JavaScript 编写，运行在轻量级运行时中。适合编写 HTTP 请求驱动的刮削逻辑。
- **WASM 刮削插件**: 使用 Rust 编写并编译为 WebAssembly。提供比 JS 更好的性能和类型安全，支持复杂的解析逻辑。
- **Native 格式插件**: 使用 Rust 编写并编译为动态链接库。拥有完全的系统权限，适合处理复杂的音频格式解码和加密文件。

## 插件配置文件 (plugin.json)

每个插件都需要在根目录提供一个 `plugin.json` 配置文件，用于向 Ting Reader 声明插件的基本信息、入口、权限和配置项。

### 完整示例

```json
{
  "id": "my-scraper-wasm",
  "name": "My Scraper",
  "version": "1.0.0",
  "plugin_type": "scraper",
  "runtime": "wasm",
  "author": "Your Name",
  "description": "从示例网站获取有声书元数据",
  "description_en": "Fetch audiobook metadata from example.com",
  "entry_point": "my_scraper.wasm",
  "license": "MIT",
  "repo": "user/my-scraper-wasm",
  "min_core_version": "1.0.0",
  "dependencies": [
    { "plugin_name": "ffmpeg-utils", "version_requirement": "*" }
  ],
  "npm_dependencies": [
    { "name": "axios", "version": "^1.6.0" }
  ],
  "supported_extensions": ["xm", "m4a"],
  "scraper": {
    "auto_scrape": true,
    "search_fields": [
      {
        "key": "title",
        "label": "书名",
        "required": true,
        "type": "text",
        "default_from": "book.title"
      },
      {
        "key": "author",
        "label": "作者",
        "required": false,
        "type": "text",
        "default_from": "book.author"
      },
      {
        "key": "narrator",
        "label": "演播",
        "required": false,
        "type": "text",
        "default_from": "book.narrator"
      }
    ],
    "result_fields": [
      "title",
      "author",
      "narrator",
      "cover_url",
      "description",
      "tags",
      "genre"
    ]
  },
  "permissions": [
    { "type": "network_access", "value": "*.example.com" },
    { "type": "file_read", "value": "./data/audio" },
    { "type": "file_write", "value": "./data/audio" }
  ],
  "config_schema": {
    "type": "object",
    "properties": {
      "api_key": {
        "type": "string",
        "title": "API 密钥",
        "description": "用于访问 API 的密钥",
        "x-encrypted": true
      },
      "timeout": {
        "type": "integer",
        "title": "超时时间",
        "description": "请求超时时间（秒）",
        "default": 30
      }
    }
  }
}
```

### 字段说明

| 字段 | 类型 | 必填 | 说明 |
|------|------|:---:|------|
| `id` | string | 是 | 插件全局唯一标识符，小写字母+连字符（如 `"my-scraper-js"`） |
| `name` | string | 是 | 插件显示名称 |
| `version` | string | 是 | 版本号，遵循语义化版本（如 `"1.0.0"`） |
| `plugin_type` | string | 是 | 插件类型：`"scraper"` / `"format"` / `"utility"` |
| `author` | string | 是 | 作者名称 |
| `description` | string | 是 | 功能简述（中文） |
| `entry_point` | string | 是 | 入口文件名（如 `"plugin.js"` / `"xm_format.dll"` / `"ypshuo_scraper.wasm"`） |
| `description_en` | string | 否 | 英文描述，国际化展示时使用 |
| `runtime` | string | 否 | 运行环境：`"wasm"` / `"javascript"` / `"native"`。未指定时根据 entry_point 扩展名自动推断 |
| `license` | string | 否 | 开源许可证（如 `"MIT"`） |
| `repo` | string | 否 | 插件仓库，推荐使用 GitHub `owner/name` 格式（如 `"dqsq2e2/ypshuo-scraper-wasm"`） |
| `min_core_version` | string | 否 | 要求的最小核心版本号 |
| `dependencies` | array | 否 | 依赖的其他插件列表，见下方[依赖格式](#依赖格式) |
| `npm_dependencies` | array | 否 | （仅 JavaScript 插件）NPM 依赖包列表 |
| `supported_extensions` | array | 否 | （格式插件）支持的音频文件扩展名列表 |
| `scraper` | object | 否 | （刮削插件）自动刮削注册、搜索字段和返回字段声明 |
| `permissions` | array | 否 | 插件运行所需权限列表，见下方[权限类型](#权限类型) |
| `config_schema` | object | 否 | 用户可配置项结构，使用 [JSON Schema](https://json-schema.org/) 规范 |

### 依赖格式

支持两种写法：

**简单字符串**（任意版本均可）：
```json
"dependencies": ["ffmpeg-utils"]
```

**详细对象**（指定版本要求）：
```json
"dependencies": [
  { "plugin_name": "ffmpeg-utils", "version_requirement": "^1.0.0" }
]
```

### Scraper 能力声明

`plugin_type` 为 `scraper` 的插件建议声明 `scraper` 对象。该对象决定书籍详情页手动刮削弹窗如何展示搜索表单、搜索结果，以及哪些字段可以被用户单独采用。

刮削插件可以选择是否注册为自动刮削插件：

- `auto_scrape: true`：插件会出现在存储库自动刮削配置中，也能用于手动刮削。此时必须声明一个必填的书名搜索字段，通常为 `key: "title"` 且 `default_from: "book.title"`。
- `auto_scrape: false` 或省略：插件只用于书籍详情页的手动刮削，不会出现在存储库自动刮削配置中。此时 `search_fields` 至少需要声明一个搜索字段即可，可以不是书名。
- 旧插件如果完全没有声明 `scraper`，系统会使用兼容默认值，并视为可自动刮削。

```json
"scraper": {
  "auto_scrape": true,
  "search_fields": [
    {
      "key": "title",
      "label": "书名",
      "required": true,
      "type": "text",
      "default_from": "book.title"
    },
    {
      "key": "author",
      "label": "作者",
      "required": false,
      "type": "text",
      "default_from": "book.author"
    },
    {
      "key": "narrator",
      "label": "演播",
      "required": false,
      "type": "text",
      "default_from": "book.narrator"
    }
  ],
  "result_fields": ["title", "author", "narrator", "cover_url", "description", "tags"]
}
```

`search_fields` 中的字段会原样传给插件的 `search(args)`。如果搜索参数中包含 `title` 或 `query`，系统会同时写入兼容字段 `title` 和 `query`，旧插件仍可继续读取 `args.query`。

加载校验规则：

- 所有刮削插件至少需要一个 `search_fields` 字段。
- `auto_scrape: true` 的插件必须有必填书名字段；否则插件会加载失败。
- 仅手动刮削插件可以使用任意搜索字段组合，手动搜索时只要求至少一个搜索参数非空。

`result_fields` 必须只声明插件实际能稳定返回的字段。手动刮削弹窗只允许用户选择这些字段；某条搜索结果中字段为空时，该字段会显示为未返回并不可采用。

常用返回字段：

| 字段 | 类型 | 说明 |
|------|------|------|
| `title` | string | 书名 |
| `author` | string | 作者 |
| `narrator` | string | 演播者 |
| `cover_url` | string | 封面 URL |
| `description` / `intro` | string | 简介。插件返回 `intro` 时，前端会按简介字段展示 |
| `tags` | string[] | 标签 |
| `genre` | string | 类型/分类 |
| `year` / `published_year` | string/number | 出版年份 |

如果旧插件没有声明 `scraper`，系统会使用兼容默认值：`auto_scrape` 为 `true`，搜索字段为 `title` 必填、`author` 和 `narrator` 选填；返回字段为常见元数据字段。新插件应显式声明实际能力。

### NPM 依赖格式

```json
"npm_dependencies": [
  { "name": "axios", "version": "^1.6.0" }
]
```

### 权限类型

| 权限类型 | value 含义 | 说明 |
|----------|-----------|------|
| `network_access` | 域名（支持 `*.example.com` 通配符） | 允许访问的网络域名 |
| `file_read` | 相对路径 | 允许读取的文件/目录 |
| `file_write` | 相对路径 | 允许写入的文件/目录 |
| `database_read` | （无） | 允许读取数据库 |
| `database_write` | （无） | 允许写入数据库 |
| `event_publish` | （无） | 允许发布事件 |

### 配置结构 (config_schema)

`config_schema` 遵循 [JSON Schema](https://json-schema.org/) 规范，必须包含最外层的 `"type": "object"` 和 `"properties"`。系统后台会根据此字段自动生成插件设置表单。

支持的属性：
- `type`: 字段类型（`string`, `integer`, `boolean`, `number` 等）
- `title`: 字段显示名称
- `description`: 字段说明
- `default`: 默认值
- `x-encrypted`: 设为 `true` 表示该字段为敏感信息，系统会使用 AES-256-GCM 加密存储

**注意**：即使使用简化的 flat 格式（如下），系统也会自动规范化为 JSON Schema 格式：

```json
// 简化格式（自动规范化）
{
  "ffmpeg_path": {
    "type": "string",
    "title": "FFmpeg 路径",
    "default": ""
  }
}

// 等价的标准格式
{
  "type": "object",
  "properties": {
    "ffmpeg_path": {
      "type": "string",
      "title": "FFmpeg 路径",
      "default": ""
    }
  }
}
```

### 运行时 (runtime) 自动推断

如果未指定 `runtime` 字段，系统会根据 `entry_point` 的扩展名自动推断：

| entry_point 扩展名 | 推断的 runtime |
|--------------------|---------------|
| `.wasm` | `wasm` |
| `.js` | `javascript` |
| `.dll` / `.so` / `.dylib` | `native` |
