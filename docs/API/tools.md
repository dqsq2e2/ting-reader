# 工具

## POST /api/v1/tools/regex/generate

根据文件名示例生成章节正则表达式。

**请求体：**

```json
{
  "filename": "第001集 传承",
  "chapter_number": "1",
  "chapter_title": "传承"
}
```

**响应：** `200 OK`

```json
{
  "regex": "^第(\\d+)集\\s(.+)$",
  "test_match": true,
  "captured_index": "001",
  "captured_title": "传承"
}
```
