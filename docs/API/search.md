# 搜索与刮削

## GET /api/v1/search

搜索在线书籍。

**查询参数：**

| 参数 | 类型 | 说明 |
|------|------|------|
| q | string | 搜索关键词 |
| source | string | 数据源插件 ID（可选） |
| page | number | 页码，默认 1 |
| page_size | number | 每页数量，默认 20 |

**响应：** `200 OK`

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
      "tags": ["string"],
      "genre": "string | null",
      "subtitle": "string | null",
      "published_year": "string | null",
      "duration": 0,
      "chapter_count": 0
    }
  ],
  "total": 0,
  "page": 1,
  "page_size": 20
}
```

---

## GET /api/v1/scraper/sources

获取可用的刮削数据源列表。

**响应：** `200 OK`

```json
{
  "sources": [
    {
      "id": "string",
      "name": "string",
      "description": "string | null",
      "version": "string",
      "enabled": true,
      "auto_scrape": true,
      "search_fields": [
        {
          "key": "string",
          "label": "string",
          "required": false,
          "type": "string",
          "field_type": "string",
          "placeholder": "string",
          "default_from": "string"
        }
      ],
      "result_fields": ["string"]
    }
  ]
}
```

---

## POST /api/v1/scraper/search

使用刮削器搜索书籍。

**请求体：**

```json
{
  "query": "string (可选)",
  "search_params": { "key": "value" },
  "source": "string (插件 ID，可选)",
  "page": 1,
  "page_size": 20,
  "author": "string (可选)",
  "narrator": "string (可选)"
}
```

**响应：** `200 OK` — 与 `GET /api/v1/search` 响应格式相同
