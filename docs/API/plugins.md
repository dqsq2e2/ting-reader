# 插件管理

## 数据结构

### LocalizedText

```json
{
  "zh": "中文文本",
  "en": "English text"
}
```

### ScraperSearchField

```json
{
  "key": "title",
  "label": "书名",
  "label_i18n": {
    "zh": "书名",
    "en": "Title"
  },
  "required": true,
  "type": "text",
  "field_type": "text",
  "placeholder": "输入书名",
  "placeholder_i18n": {
    "zh": "输入书名",
    "en": "Enter title"
  },
  "default_from": "book.title"
}
```

### ScraperCapabilities

```json
{
  "auto_scrape": true,
  "search_fields": [
    {
      "key": "title",
      "label": "书名",
      "label_i18n": {
        "zh": "书名",
        "en": "Title"
      },
      "required": true,
      "type": "text",
      "placeholder": "输入书名",
      "placeholder_i18n": {
        "zh": "输入书名",
        "en": "Enter title"
      },
      "default_from": "book.title"
    }
  ],
  "result_fields": ["title", "author", "cover_url", "intro", "tags"],
  "result_field_labels": {
    "title": {
      "zh": "书名",
      "en": "Title"
    },
    "cover_url": {
      "zh": "封面",
      "en": "Cover"
    },
    "intro": {
      "zh": "简介",
      "en": "Description"
    },
    "description": {
      "zh": "简介",
      "en": "Description"
    }
  }
}
```

### PluginInfo

插件管理 API 中的展示/业务摘要字段由服务端从 `capabilities` 派生；插件能力以 manifest 中的 `capabilities` 为准。

```json
{
  "id": "string",
  "name": "string",
  "version": "string",
  "plugin_type": "scraper | format | utility",
  "runtime": "wasm | javascript | native | null",
  "author": "string | null",
  "description": "string | null",
  "description_i18n": {
    "zh": "中文描述",
    "en": "English description"
  },
  "is_enabled": true,
  "state": "loading | loaded | active | unloading | unloaded | failed",
  "error": "string | null",
  "stats": {
    "total_calls": 0,
    "successful_calls": 0,
    "failed_calls": 0,
    "avg_execution_time_ms": 0.0
  },
  "config_schema": {
    "type": "object",
    "properties": {
      "api_key": {
        "type": "string",
        "title": "API 密钥",
        "title_i18n": {
          "zh": "API 密钥",
          "en": "API Key"
        },
        "description": "用于访问 API 的密钥",
        "description_i18n": {
          "zh": "用于访问 API 的密钥",
          "en": "Key used to access the API"
        }
      }
    }
  },
  "permissions": ["network_access: example.com"],
  "license": "string | null",
  "repo": "string | null",
  "capabilities": [
    {
      "id": "metadata.search",
      "kind": "metadata_provider",
      "invoke": "search",
      "auto_scrape": true,
      "search_fields": [],
      "result_fields": []
    }
  ],
  "scraper": {
    "auto_scrape": true,
    "search_fields": [],
    "result_fields": [],
    "result_field_labels": {}
  }
}
```

### StorePlugin

```json
{
  "id": "ximalaya-scraper-wasm",
  "name": "ximalaya scraper",
  "description": "从喜马拉雅获取有声书元数据（WASM 实现）",
  "description_i18n": {
    "zh": "从喜马拉雅获取有声书元数据（WASM 实现）",
    "en": "Fetch audiobook metadata from Ximalaya (WASM implementation)"
  },
  "version": "1.0.2",
  "download_url": "/plugins/ximalaya-scraper-wasm.tr",
  "size": "347.63 KB",
  "date": "2026-06-27T14:20:49.000Z",
  "runtime": "wasm",
  "license": "MIT",
  "author": "Ting Reader Team",
  "repo": "dqsq2e2/example-plugin",
  "permissions": ["network_access: www.ximalaya.com"],
  "dependencies": ["ffmpeg-utils"],
  "min_core_version": "1.4.8",
  "config_schema": {},
  "capabilities": [
    {
      "id": "metadata.search",
      "kind": "metadata_provider",
      "invoke": "search",
      "auto_scrape": true,
      "search_fields": [],
      "result_fields": []
    }
  ],
  "downloads": [
    {
      "name": "Download Plugin",
      "url": "https://www.tingreader.cn/plugins/ximalaya-scraper-wasm.tr"
    }
  ]
}
```

`download_url` 可以是字符串，也可以是平台映射：

```json
{
  "download_url": {
    "linux-x86_64": "https://example.com/plugin-linux-x86_64.tr",
    "linux-aarch64": "https://example.com/plugin-linux-arm64.tr",
    "windows-x86_64": "https://example.com/plugin-windows-amd64.tr"
  }
}
```

## 已安装插件

### GET /api/v1/plugins

获取已安装插件列表。

响应：`200 OK`

```json
[
  {
    "id": "string",
    "name": "string",
    "version": "string",
    "plugin_type": "scraper",
    "runtime": "wasm",
    "author": "Ting Reader Team",
    "description": "从示例站点获取元数据",
    "description_i18n": {
      "zh": "从示例站点获取元数据",
      "en": "Fetch metadata from the example site"
    },
    "is_enabled": true,
    "state": "active",
    "error": null,
    "stats": {
      "total_calls": 0,
      "successful_calls": 0,
      "failed_calls": 0,
      "avg_execution_time_ms": 0.0
    },
    "config_schema": {},
    "permissions": ["network_access: example.com"],
    "license": "MIT",
    "repo": "owner/repo",
    "capabilities": [
      {
        "id": "metadata.search",
        "kind": "metadata_provider",
        "invoke": "search",
        "auto_scrape": true,
        "search_fields": [],
        "result_fields": []
      }
    ],
    "scraper": {
      "auto_scrape": true,
      "search_fields": [],
      "result_fields": [],
      "result_field_labels": {}
    }
  }
]
```

### GET /api/v1/plugins/:id

获取插件详情。

路径参数：

| 参数 | 类型 | 说明 |
| --- | --- | --- |
| `id` | string | 插件 ID |

响应：`200 OK`

```json
{
  "id": "string",
  "name": "string",
  "version": "string",
  "plugin_type": "format",
  "runtime": "native",
  "author": "Ting Reader Team",
  "description": "通过 FFmpeg 提供原生音频格式支持",
  "description_i18n": {
    "zh": "通过 FFmpeg 提供原生音频格式支持",
    "en": "Native audio format support via FFmpeg"
  },
  "license": "MIT",
  "repo": "owner/repo",
  "is_enabled": true,
  "state": "active",
  "error": null,
  "entry_point": "native_audio_support.dll",
  "dependencies": [
    {
      "plugin_name": "ffmpeg-utils",
      "version_requirement": "*"
    }
  ],
  "permissions": ["FileRead(\"./data/audio\")"],
  "supported_extensions": ["m4a", "flac"],
  "capabilities": [
    {
      "id": "format.audio",
      "kind": "format_handler",
      "invoke": "get_stream_url",
      "extensions": ["m4a", "flac"]
    }
  ],
  "config_schema": {},
  "scraper": null,
  "stats": {
    "total_calls": 0,
    "successful_calls": 0,
    "failed_calls": 0,
    "avg_execution_time_ms": 0.0
  }
}
```

### POST /api/v1/plugins/install

上传安装插件（`multipart/form-data`，最大 50MB）。

请求：字段名 `file`，值为 `.tr` 插件包。

`.tr` 包由 `trpack build` 生成，并会在安装前完成有效性校验。

响应：`201 Created`

```json
{
  "plugin_id": "string",
  "message": "Plugin xxx installed successfully"
}
```

如果包未签名或签名 key id 不在受信任列表中，后端返回 `428 Precondition Required`，客户端应显示安全提示。用户同意后，用同一个文件重新提交，并增加字段 `accept_unverified=true`。

```json
{
  "requires_confirmation": true,
  "verification_status": "unsigned",
  "plugin_id": "example-plugin",
  "plugin_name": "Example Plugin",
  "plugin_version": "1.0.0",
  "publisher": "未知发布者",
  "warning": "Example Plugin由未知发布者提供，未经Ting Reader验证。单击同意，即表示你同意全权负责因使用该插件而可能导致的任何设备损坏或数据丢失。"
}
```

### DELETE /api/v1/plugins/:id

卸载插件。

路径参数：

| 参数 | 类型 | 说明 |
| --- | --- | --- |
| `id` | string | 插件 ID |

响应：`200 OK`

```json
{
  "message": "Plugin xxx uninstalled successfully"
}
```

### POST /api/v1/plugins/:id/reload

重新加载插件。

路径参数：

| 参数 | 类型 | 说明 |
| --- | --- | --- |
| `id` | string | 插件 ID |

响应：`200 OK`

```json
{
  "message": "Plugin xxx reloaded successfully"
}
```

## 插件配置

### GET /api/v1/plugins/:id/config

获取插件配置。

路径参数：

| 参数 | 类型 | 说明 |
| --- | --- | --- |
| `id` | string | 插件 ID |

响应：`200 OK`

```json
{
  "plugin_id": "string",
  "config": {}
}
```

### PUT /api/v1/plugins/:id/config

更新插件配置。

路径参数：

| 参数 | 类型 | 说明 |
| --- | --- | --- |
| `id` | string | 插件 ID |

请求体：

```json
{
  "config": {
    "api_key": "string"
  }
}
```

响应：`200 OK`

```json
{
  "message": "Plugin xxx configuration updated successfully"
}
```

## 插件商店

### GET /api/v1/store/plugins

获取商店插件列表。服务端会查找已安装且启用的 `plugin_store` capability，调用该 capability 的 `invoke` 方法获取列表；如果未安装插件商店插件，返回空数组。

查询参数：

| 参数 | 类型 | 必填 | 说明 |
| --- | --- | --- | --- |
| `refresh` | boolean | 否 | 为 `true` 时跳过后端商店缓存，并向商店插件传入 `force_refresh: true`。 |

响应：`200 OK`

```json
[
  {
    "id": "ximalaya-scraper-wasm",
    "name": "ximalaya scraper",
    "description": "从喜马拉雅获取有声书元数据（WASM 实现）",
    "description_i18n": {
      "zh": "从喜马拉雅获取有声书元数据（WASM 实现）",
      "en": "Fetch audiobook metadata from Ximalaya (WASM implementation)"
    },
    "version": "1.0.2",
    "download_url": "https://www.tingreader.cn/plugins/ximalaya-scraper-wasm.tr",
    "runtime": "wasm",
    "author": "Ting Reader Team",
    "permissions": ["network_access: www.ximalaya.com"],
    "capabilities": [
      {
        "id": "metadata.search",
        "kind": "metadata_provider",
        "invoke": "search",
        "auto_scrape": true,
        "search_fields": [
          {
            "key": "title",
            "label": "书名",
            "label_i18n": {
              "zh": "书名",
              "en": "Title"
            },
            "required": true,
            "type": "text",
            "placeholder": "输入书名",
            "placeholder_i18n": {
              "zh": "输入书名",
              "en": "Enter title"
            },
            "default_from": "book.title"
          }
        ],
        "result_fields": ["title", "author", "cover_url", "intro"],
        "result_field_labels": {
          "title": {
            "zh": "书名",
            "en": "Title"
          },
          "intro": {
            "zh": "简介",
            "en": "Description"
          },
          "description": {
            "zh": "简介",
            "en": "Description"
          }
        }
      }
    ]
  }
]
```

### POST /api/v1/store/install

从商店安装插件。

请求体：

```json
{
  "plugin_id": "string"
}
```

响应：`201 Created`

```json
{
  "plugin_id": "string",
  "message": "Plugin xxx installed successfully from store"
}
```

### POST /api/v1/store/cache/clear

清除插件商店缓存。客户端执行“更新插件列表”时应先调用此接口，再以 `refresh=true` 拉取 `/api/v1/store/plugins`。

响应：`200 OK`

```json
{
  "message": "Plugin cache cleared successfully"
}
```

## 插件能力 API

### GET /api/v1/plugin-capabilities

列出已启用插件声明的 capability。可用 `kind` 过滤，例如 `ui_extension`、`client_extension`、`content_processor`、`tool_provider`、`task_handler`、`event_handler`、`http_route`。

查询参数：

| 参数 | 类型 | 必填 | 说明 |
| --- | --- | :---: | --- |
| `kind` | string | 否 | capability kind 过滤。 |

响应：`200 OK`

```json
[
  {
    "plugin_id": "advanced-capabilities-example@0.1.0",
    "plugin_name": "Advanced Capabilities Example",
    "capability": {
      "id": "assistant.settings",
      "kind": "ui_extension",
      "invoke": "saveAssistantSettings",
      "slot": "settings.section",
      "render": {
        "mode": "schema"
      }
    }
  }
]
```

### POST /api/v1/plugins/:plugin_id/capabilities/:capability_id/invoke

调用指定插件 capability。后端会自动附加可信 `_context`，包含插件、capability 和当前认证用户上下文。

请求体：

```json
{
  "params": {
    "slot": "book.detail_action",
    "context": {
      "book_id": "book-id"
    },
    "values": {
      "note": "example"
    }
  }
}
```

响应：`200 OK`

```json
{
  "result": {
    "ok": true
  }
}
```

### GET /api/v1/plugin-capabilities/content-processors

按扩展名查询内容处理插件。

查询参数：

| 参数 | 类型 | 必填 | 说明 |
| --- | --- | :---: | --- |
| `extension` | string | 是 | 文件扩展名，例如 `txt`、`pdf`。 |
| `operation` | string | 否 | 操作过滤：`probe`、`extract_metadata`、`list_sections`、`read_chunk`、`render_page`。 |

### GET /api/v1/plugin-capabilities/tools

查询 `tool_provider`。可用 `name` 过滤工具名。

### GET /api/v1/plugin-capabilities/task-handlers

查询 `task_handler`。可用 `task_type` 过滤任务类型。

### GET /api/v1/plugin-capabilities/event-handlers

查询 `event_handler`。可用 `event` 过滤事件名。

## HostGateway API

### POST /api/v1/plugin-host/invoke

由前端受控调用插件可访问的 HostGateway 方法。后端会同时校验：

- 插件 manifest 是否声明了对应权限。
- 当前用户是否有目标书籍/书库访问权限。
- 目标方法是否允许在当前认证上下文中调用。

请求体：

```json
{
  "plugin_id": "advanced-capabilities-example@0.1.0",
  "method": "progress.recent",
  "params": {
    "limit": 5
  }
}
```

响应：`200 OK`

```json
{
  "result": []
}
```

当前常用方法：

| 方法 | 权限 |
| --- | --- |
| `books.list` / `books.get` | `books_read` |
| `libraries.list` / `libraries.get` | `books_read` |
| `chapters.list` / `chapters.get` | `chapters_read` |
| `progress.recent` | `progress_read` |
| `media.get_url` | `media_read_url` 或 `media_read` |
| `metadata.write` | `metadata_write` + admin |
| `library.file.list` / `library.file.stat` / `library.file.read` | `file_read` |
| `library.file.write` | `file_write` + admin |
| `database.get` / `database.list` | `database_read` |
| `database.update` | `database_write` + admin |
| `tasks.create` | `task_create` |
| `cache.get` / `cache.has` | `cache_read` 或 `cache_write` |
| `cache.set` / `cache.delete` | `cache_write` |

## 插件路由签名

### POST /api/v1/plugin-route-signatures

为公共插件路由生成签名 URL。默认绑定当前用户，签名中包含 `user`，公共请求校验后会恢复 signed-user 上下文。RSS 订阅等外部客户端可用该 URL 访问当前用户有权限的内容。

请求体：

```json
{
  "method": "GET",
  "path": "/rss/library-id.xml",
  "expires_in_seconds": 86400,
  "bind_current_user": true
}
```

响应：`200 OK`

```json
{
  "path": "/rss/library-id.xml",
  "expires": 1790000000,
  "signature": "hex",
  "user_id": "user-id",
  "signed_url": "/api/v1/public/plugin-routes/rss/library-id.xml?expires=1790000000&signature=hex&user=user-id"
}
```
