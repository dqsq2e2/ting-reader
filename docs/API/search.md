# 搜索与刮削

## GET /api/v1/search

搜索在线书籍。

查询参数：

| 参数 | 类型 | 说明 |
| --- | --- | --- |
| `q` | string | 搜索关键词 |
| `source` | string | 数据源插件 ID，可选 |
| `page` | number | 页码，默认 1 |
| `page_size` | number | 每页数量，默认 20 |

响应：`200 OK`

```json
{
  "items": [
    {
      "id": "string",
      "title": "string",
      "author": "string",
      "narrator": "string | null",
      "cover_url": "string | null",
      "intro": "string | null",
      "description": "string | null",
      "tags": ["string"],
      "genre": "string | null",
      "subtitle": "string | null",
      "published_year": "string | null",
      "published_date": "string | null",
      "publisher": "string | null",
      "isbn": "string | null",
      "asin": "string | null",
      "language": "string | null",
      "explicit": false,
      "abridged": false,
      "duration": 0,
      "chapter_count": 0
    }
  ],
  "total": 0,
  "page": 1,
  "page_size": 20
}
```

## GET /api/v1/scraper/sources

获取可用的刮削数据源列表。

响应：`200 OK`

```json
{
  "sources": [
    {
      "id": "ximalaya-scraper-wasm",
      "name": "ximalaya scraper",
      "description": "从喜马拉雅获取有声书元数据（WASM 实现）",
      "version": "1.0.2",
      "enabled": true,
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
          "field_type": "text",
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
        },
        "tags": {
          "zh": "标签",
          "en": "Tags"
        }
      }
    }
  ]
}
```

## POST /api/v1/scraper/search

使用刮削器搜索书籍。

请求体：

```json
{
  "query": "string",
  "search_params": {
    "title": "书名",
    "author": "作者"
  },
  "source": "ximalaya-scraper-wasm",
  "page": 1,
  "page_size": 20,
  "author": "string",
  "narrator": "string"
}
```

说明：

- `query` 可选。传入后，后端会补入 `search_params.title` 和 `search_params.query`。
- `search_params` 会按插件 `scraper.search_fields` 的 `key` 原样传给插件。
- `source` 可选。不传时会按可用刮削源聚合搜索。
- `author`、`narrator` 会补入对应搜索参数。

响应：`200 OK`

```json
{
  "items": [],
  "total": 0,
  "page": 1,
  "page_size": 20
}
```
