# JavaScript 刮削插件开发指南

JavaScript 插件是 Ting Reader 中开发最简单、调试最方便的插件类型。它运行在内置的轻量级 JavaScript 运行时中，非常适合编写 HTTP 请求驱动的刮削逻辑。

## 1. 快速开始

### 1.1 插件目录结构
创建一个新文件夹 `my-scraper-js`，并在其中创建两个文件：
- `plugin.json`: 插件配置文件
- `plugin.js`: 插件代码文件

### 1.2 配置文件 (plugin.json)
```json
{
  "name": "my-scraper-js",
  "version": "1.0.0",
  "plugin_type": "scraper",
  "author": "Your Name",
  "description": "A simple JS scraper example",
  "runtime": "javascript",
  "entry_point": "plugin.js",
  "permissions": [
    { "type": "network_access", "value": "api.example.com" }
  ]
}
```

### 1.3 核心代码 (plugin.js)
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
    const { query, page } = args;
    // 发送请求 (fetch 是内置的)
    const resp = await fetch(`https://api.example.com/search?q=${query}&page=${page}`);
    const data = await resp.json();
    
    // 转换数据结构
    const items = data.results.map(item => ({
        id: String(item.id),
        title: item.title,
        author: item.author_name,
        cover_url: item.cover_image,
        intro: item.description,
        tags: item.categories || [],
        narrator: null,
        chapter_count: item.total_chapters,
        duration: null
    }));
    
    // 如果搜索结果不完整，可以内部调用私有方法获取详情
    // if (items.length > 0) {
    //     const detail = await _fetchDetail(items[0].id);
    //     Object.assign(items[0], detail);
    // }

    return {
        items: items,
        total: data.total_count,
        page: page,
        page_size: items.length
    };
}

// 3. 内部辅助函数 (可选)
async function _fetchDetail(bookId) {
    // ...
    return { ... };
}

// 4. 导出函数 (必须!)
globalThis.initialize = initialize;
globalThis.shutdown = shutdown;
globalThis.search = search;
```

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
