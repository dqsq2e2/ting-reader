# JavaScript 插件开发指南

JavaScript 插件适合快速接入 HTTP API、解析网页、提供插件商店源、编写工具能力或轻量 UI 后端逻辑。运行时提供 `fetch`、`Ting.log`、`Ting.host.invoke` 和声明式 npm 依赖。

## 项目结构

```text
my-js-plugin/
  plugin.yml
  plugin.js
  ui/
    index.html
```

## plugin.yml 示例

```yaml
id: example-metadata-js
name: Example Metadata JS
version: 1.0.0
min_core_version: 1.4.8
runtime: javascript
entry_point: plugin.js
npm_dependencies:
  cheerio: "^1.0.0"
capabilities:
  - id: metadata.search
    kind: metadata_provider
    invoke: search
    auto_scrape: true
    search_fields:
      - key: title
        label: { zh: 书名, en: Title }
        type: text
        required: true
        default_from: book.title
    result_fields:
      - key: title
        label: { zh: 书名, en: Title }
      - key: author
        label: { zh: 作者, en: Author }
      - key: cover_url
        label: { zh: 封面, en: Cover }
permissions:
  - type: network_access
    value: "*.example.com"
```

`metadata_provider` 中的 `search_fields` 决定前端搜索表单，`result_fields` 决定搜索结果可采用字段。需要进入存储库自动刮削配置时设置 `auto_scrape: true`，并提供必填书名字段。

插件需要管理员填写 API 地址、密钥、开关或模型参数时，在 `plugin.yml` 顶层声明 `config_schema`。完整写法见 [插件开发指南：插件配置 config_schema](./plugin-dev.md#10-插件配置-config_schema)。

```yaml
config_schema:
  type: object
  properties:
    api_key:
      type: string
      format: secret
      x-encrypted: true
      title:
        zh: API 密钥
        en: API key
    source_url:
      type: string
      title:
        zh: 数据源地址
        en: Source URL
      default: https://example.com/api
```

JavaScript 运行时通过 `Ting.config` 读取解密后的配置：

```javascript
const apiKey = Ting.config?.api_key || "";
const sourceUrl = Ting.config?.source_url || "https://example.com/api";
```

## plugin.js 示例

```javascript
const cheerio = require('cheerio');

async function search(args) {
  const keyword = args.title || args.query;
  const html = await (await fetch(
    'https://www.example.com/search?q=' + encodeURIComponent(keyword)
  )).text();
  const $ = cheerio.load(html);

  const items = $('.book').map((_, item) => ({
    id: $(item).attr('data-id'),
    title: $(item).find('.title').text().trim(),
    author: $(item).find('.author').text().trim() || null,
    cover_url: normalizeCover($(item).find('img').attr('src')),
    intro: $(item).find('.intro').text().trim() || null,
  })).get();

  return {
    items,
    total: items.length,
    page: args.page || 1,
    page_size: items.length,
  };
}

function normalizeCover(url) {
  if (!url) return null;
  return url.replace(/^http:/, 'https:').split('!')[0];
}

globalThis.search = search;
```

## npm 依赖

在 `npm_dependencies` 中声明的包会在插件加载前安装。运行时提供 CommonJS 风格的 `require`，只允许加载：

- manifest 中声明过的 npm 包。
- 插件目录内的相对模块，例如 `require('./helper')`。

```yaml
npm_dependencies:
  cheerio: "^1.0.0"
  dayjs: "^1.11.0"
```

```javascript
const dayjs = require('dayjs');
const helper = require('./helper');
```

如果依赖包是 ESM-only，建议在插件项目构建阶段打包成 CommonJS 文件，或选择支持 CommonJS 的版本。

## HostGateway

服务端 JS 插件可以通过 `Ting.host.invoke(method, params)` 访问核心数据。调用会按 manifest 权限和当前用户上下文校验。

```yaml
permissions:
  - type: books_read
  - type: database_read
  - type: cache_write
  - type: file_read
    value: library
```

```javascript
async function recentBooks() {
  const context = Ting.host.getContext();
  const recent = await Ting.host.invoke('progress.recent', { limit: 20 });
  const book = context?.book_id
    ? await Ting.host.invoke('database.get', { entity: 'book', id: context.book_id })
    : null;
  return { recent, book };
}

globalThis.recentBooks = recentBooks;
```

常用方法、请求参数、返回格式和错误处理见 [HostGateway 能力调用详解](./hostgateway.md)。

## 打包

```bash
trpack validate .
trpack build . --output dist/example-metadata-js.tr
trpack verify dist/example-metadata-js.tr
```
