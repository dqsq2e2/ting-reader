# 媒体流

## 音频流

### GET /api/stream/:chapterId

流式播放章节音频。支持 Range 请求和转码。

**路径参数：**

| 参数 | 类型 | 说明 |
|------|------|------|
| chapterId | string | 章节 ID |

**查询参数：**

| 参数 | 类型 | 说明 |
|------|------|------|
| token | string | 认证 Token（可选） |
| transcode | string | 转码格式：`mp3`、`wav`、`hls`（可选） |
| seek | string | 跳转位置，如 `30.5`（秒，可选，仅转码模式） |

**请求头：**

| Header | 说明 |
|--------|------|
| `Range` | 标准 HTTP Range 头，如 `bytes=0-1023` |

**响应头：**

| Header | 说明 |
|--------|------|
| `Content-Type` | 音频 MIME 类型 |
| `Content-Length` | 内容长度 |
| `Content-Range` | Range 响应 |
| `Accept-Ranges` | `bytes` |
| `X-Audio-Duration` | 音频时长（秒，转码模式） |

**支持格式：** m4a, mp4, mp3, aac, flac, ogg, opus, wav, wma, strm

**转码说明：**
- `transcode=mp3`：通过 FFmpeg 转码为 MP3（128kbps）
- `transcode=wav`：通过 FFmpeg 转码为 WAV
- `transcode=hls`：转码为 HLS 流（见下方 HLS 章节）

**STRM 文件：** 自动解析 `.strm` 文件中的 URL 并代理或重定向。

---

## HLS 流

### GET /api/stream/hls/:sessionId/playlist.m3u8

获取 HLS 播放列表（无需认证，Session ID 提供安全保护）。

**路径参数：**

| 参数 | 类型 | 说明 |
|------|------|------|
| sessionId | string | HLS 会话 ID |

---

### GET /api/stream/hls/:sessionId/:filename

获取 HLS 分片（无需认证）。

**路径参数：**

| 参数 | 类型 | 说明 |
|------|------|------|
| sessionId | string | HLS 会话 ID |
| filename | string | 分片文件名，如 `segment_000.ts` |

---

### POST /api/stream/hls/:sessionId/seek

HLS 流跳转（无需认证）。

**路径参数：**

| 参数 | 类型 | 说明 |
|------|------|------|
| sessionId | string | HLS 会话 ID |

---

## 封面代理

### GET /api/proxy/cover

代理封面图片，支持本地文件、外部 URL 和 WebDAV。

**查询参数：**

| 参数 | 类型 | 说明 |
|------|------|------|
| path | string | 图片路径或 URL |
| libraryId | string | 媒体库 ID（可选） |
| bookId | string | 书籍 ID（可选） |

**特殊路径：**
- `embedded://first-chapter`：从音频文件提取封面（暂未实现）
- `http://...#referer=XXX`：带 Referer 的外部图片

**响应：** 图片二进制数据，带 `Cache-Control: public, max-age=31536000`

---

## 缓存管理

### GET /api/cache

获取缓存列表（管理员）。

**响应：** `200 OK`

```json
{
  "caches": [
    {
      "chapter_id": "string",
      "book_id": "string | null",
      "book_title": "string | null",
      "chapter_title": "string | null",
      "file_size": 0,
      "created_at": "RFC3339 | null",
      "cover_url": "string | null"
    }
  ],
  "total": 0,
  "total_size": 0
}
```

---

### POST /api/cache/:chapterId

缓存章节到本地（管理员，用于 WebDAV 远程文件）。

**路径参数：**

| 参数 | 类型 | 说明 |
|------|------|------|
| chapterId | string | 章节 ID |

**响应：** `200 OK`

```json
{
  "success": true,
  "message": "Chapter xxx cached successfully",
  "cacheInfo": {
    "chapterId": "string",
    "bookId": "string",
    "bookTitle": "string",
    "chapterTitle": "string",
    "fileSize": 0,
    "createdAt": "RFC3339",
    "coverUrl": "string"
  }
}
```

---

### DELETE /api/cache/:chapterId

删除指定章节缓存（管理员）。

**路径参数：**

| 参数 | 类型 | 说明 |
|------|------|------|
| chapterId | string | 章节 ID |

**响应：** `200 OK`

```json
{
  "success": true,
  "message": "Cache for chapter xxx deleted successfully"
}
```

---

### DELETE /api/cache

清除所有缓存（管理员）。

**响应：** `200 OK`

```json
{
  "success": true,
  "deleted_count": 0,
  "message": "Cleared 0 cached chapters"
}
```
