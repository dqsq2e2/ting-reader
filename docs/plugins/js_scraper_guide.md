# JavaScript 刮削插件开发指南

JavaScript 插件是 Ting Reader 中开发最简单、调试最方便的插件类型。它运行在内置的轻量级 JavaScript 运行时中，非常适合编写 HTTP 请求驱动的刮削逻辑。

## 1. 快速开始

### 1.1 插件目录结构
创建一个新文件夹 `my-scraper-js`，并在其中创建两个文件：
- `plugin.json`: 插件配置文件（详情请参考 [插件开发指南](./plugin-dev.md)）
- `plugin.js`: 插件代码文件

### 1.2 核心代码 (plugin.js)
```javascript
// 1. 初始化
function initialize(context) {
    Ting.log.info('插件已加载');
}

function shutdown() {
    Ting.log.info('插件已卸载');
}

// 2. 搜索书籍
async function search(args) {
    // args 会包含 plugin.json 中 scraper.search_fields 声明的字段。
    // title 会同步写入 query，用于兼容旧插件。
    const { title, query, page, narrator, author } = args;
    const keyword = title || query;
    
    Ting.log.info(`搜索: ${keyword}, 页码: ${page}`);

    // 发送请求 (fetch 是内置的)
    const resp = await fetch(`https://api.example.com/search?q=${encodeURIComponent(keyword)}&page=${page}`);
    const data = await resp.json();
    
    // 转换数据结构
    const items = data.results.map(item => ({
        id: String(item.id),
        title: cleanText(item.title), // 建议进行文本清洗
        author: item.author_name,
        cover_url: item.cover_image,
        intro: item.description,
        tags: item.categories || [],
        narrator: item.narrator_name || null,
        chapter_count: item.total_chapters,
        duration: null
    }));
    
    // 3. 结果优化 (最佳实践)
    if (items.length > 0) {
        // 3.1 演播者/作者筛选与重排
        if (narrator) {
            const idx = items.findIndex(i => i.narrator && i.narrator.includes(narrator));
            if (idx > -1) {
                // 将匹配项移到首位
                const [match] = items.splice(idx, 1);
                items.unshift(match);
            }
        }

        // 3.2 首项增强 (Search Result Enhancement)
        // 搜索列表返回的信息通常不完整，建议主动获取第一条结果的详情来补充信息
        try {
            const detail = await _fetchDetail(items[0].id);
            Object.assign(items[0], detail);
            Ting.log.info('已增强第一条结果的元数据');
        } catch (e) {
            Ting.log.warn('增强元数据失败: ' + e.message);
        }
        // 3.3 图片 URL 清洗与防盗链绕过 (Cover Image Handling)
        // 建议去除图片 URL 后的缩放参数（如 !200），并升级为 https
        // 如果目标网站有防盗链，可以在 URL 后追加 #referer=目标网站地址，让阅读器后端代理请求
        items.forEach(item => {
            if (item.cover_url) {
                // 示例：移除 !200 等缩放后缀
                item.cover_url = item.cover_url.split('!')[0];
                // 示例：绕过防盗链 (将由后端使用指定的 referer 代理下载)
                item.cover_url = item.cover_url.replace('http:', 'https:') + '#referer=https://www.example.com/';
            }
        });
    }

    return {
        items: items,
        total: data.total_count,
        page: page,
        page_size: items.length
    };
}

// 4. 内部辅助函数
async function _fetchDetail(bookId) {
    // ... 获取详情的逻辑
    return { 
        intro: "详细简介...",
        tags: ["标签1", "标签2"]
    };
}

function cleanText(text) {
    // 移除广告、特殊符号等
    return text ? text.replace(/【.*?】/g, '').trim() : '';
}

// 5. 导出函数 (必须!)
globalThis.initialize = initialize;
globalThis.shutdown = shutdown;
globalThis.search = search;
```

### 1.3 plugin.json 中的 scraper 声明

刮削插件应在 `plugin.json` 中声明搜索字段和可返回字段。手动刮削弹窗会根据这些声明渲染输入框和字段选择按钮。

如果插件要出现在存储库自动刮削配置中，需要声明 `auto_scrape: true`，并且必须有必填书名字段。仅用于手动刮削的插件可以省略 `auto_scrape`，但仍至少需要一个搜索字段。

```json
{
  "plugin_type": "scraper",
  "scraper": {
    "auto_scrape": true,
    "search_fields": [
      { "key": "title", "label": "书名", "required": true, "default_from": "book.title" },
      { "key": "author", "label": "作者", "required": false, "default_from": "book.author" },
      { "key": "narrator", "label": "演播", "required": false, "default_from": "book.narrator" }
    ],
    "result_fields": ["title", "author", "narrator", "cover_url", "description", "tags"]
  }
}
```

`result_fields` 要按插件实际能力填写。比如插件不能稳定返回作者，就不要声明 `author`；前端也不会展示作者的采用按钮。

## 2. API 参考

### 全局对象 `Ting`
- `Ting.log.info(msg)`: 打印信息日志
- `Ting.log.warn(msg)`: 打印警告日志
- `Ting.log.error(msg)`: 打印错误日志

### 全局函数 `fetch`
完全兼容标准的 Fetch API。
```javascript
const response = await fetch('https://api.example.com', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ key: 'value' })
});
```

## 3. 常见问题
- **Q: 支持 npm 包吗？**
  A: 不支持直接 require/import npm 包。这是一个轻量级运行时。如果需要复杂依赖，请考虑打包成单文件或使用 WASM。
- **Q: 如何调试？**
  A: 使用 `Ting.log` 输出日志，日志会显示在 Ting Reader 的控制台或日志文件中。
