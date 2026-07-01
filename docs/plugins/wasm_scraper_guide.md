# WASM 插件开发指南

WASM 插件使用 Rust 语言编写并编译为 `.wasm` 文件，适合可移植的元数据解析、内容处理和计算型工具。插件仍然通过 `capabilities` 暴露能力；网络请求、数据库、存储库文件、缓存和任务等宿主能力统一通过 `ting_env` 宿主函数访问，并受 manifest 权限和当前用户上下文约束。

## 1. 快速开始

### 1.1 项目结构
创建一个标准的 Rust 库项目：
```bash
cargo new --lib my-scraper-wasm
```

编辑 `Cargo.toml`：
```toml
[package]
name = "my-scraper-wasm"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib"]

[dependencies]
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
# 其他依赖...
```

提供插件声明文件 `plugin.yml`（详情请参考 [插件开发指南](./plugin-dev.md)）。元数据插件应在 `capabilities` 中声明搜索字段和可返回字段，例如：

```yaml
capabilities:
  - id: metadata.search
    kind: metadata_provider
    invoke: search
    auto_scrape: true
    search_fields:
      - key: title
        label:
          zh: 书名
          en: Title
        required: true
        default_from: book.title
      - key: author
        label:
          zh: 作者
          en: Author
        required: false
        default_from: book.author
      - key: narrator
        label:
          zh: 演播
          en: Narrator
        required: false
        default_from: book.narrator
    result_fields:
      - key: title
        label:
          zh: 书名
          en: Title
      - key: author
        label:
          zh: 作者
          en: Author
      - key: narrator
        label:
          zh: 演播
          en: Narrator
      - key: cover_url
        label:
          zh: 封面
          en: Cover
      - key: intro
        label:
          zh: 简介
          en: Description
      - key: tags
        label:
          zh: 标签
          en: Tags
```

`auto_scrape: true` 表示该插件可以用于存储库自动刮削。自动刮削插件必须声明必填书名字段；仅用于手动刮削的插件可以省略或设为 `false`，但仍至少需要一个搜索字段。

`result_fields` 只声明插件实际能稳定返回的字段。

### 1.2 核心代码 (src/lib.rs)
```rust
use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use serde::{Deserialize, Serialize};

// 1. 定义数据结构
#[derive(Serialize)]
struct SearchResult {
    items: Vec<BookItem>,
    total: u32,
    page: u32,
    page_size: u32,
}

#[derive(Serialize)]
struct BookItem {
    id: String,
    title: String,
    author: String,
    cover_url: Option<String>,
    intro: Option<String>,
    tags: Vec<String>,
    // ... 其他字段
}

// 2. 导出 invoke 函数 (必须!)
// 它是插件与宿主交互的唯一入口
#[no_mangle]
pub extern "C" fn invoke(method_ptr: *const c_char, params_ptr: *const c_char) -> *mut c_char {
    let method = unsafe { CStr::from_ptr(method_ptr).to_string_lossy() };
    let params_json = unsafe { CStr::from_ptr(params_ptr).to_string_lossy() };

    let result = match method.as_ref() {
        "search" => handle_search(&params_json).map(|r| serde_json::to_string(&r).unwrap()),
        // 未来扩展其他方法...
        _ => Err(format!("Unknown method: {}", method)),
    };

    let response_json = match result {
        Ok(json) => json,
        Err(e) => serde_json::json!({ "error": e }).to_string(),
    };

    CString::new(response_json).unwrap().into_raw()
}

// 3. 内存管理导出 (必须!)
// 宿主环境需要分配和释放 WASM 内存以传递字符串
#[no_mangle]
pub extern "C" fn alloc(len: usize) -> *mut u8 {
    let mut buf = Vec::with_capacity(len);
    let ptr = buf.as_mut_ptr();
    std::mem::forget(buf);
    ptr
}

#[no_mangle]
pub extern "C" fn dealloc(ptr: *mut u8, len: usize) {
    unsafe {
        let _ = Vec::from_raw_parts(ptr, 0, len);
    }
}

#[derive(Deserialize)]
struct SearchParams {
    // title 来自 metadata_provider capability 中 search_fields 的书名字段。
    title: Option<String>,
    query: String,
    author: Option<String>,
    narrator: Option<String>,
    page: u32,
    page_size: Option<u32>,
}

// 4. 业务逻辑实现
fn handle_search(params_json: &str) -> Result<SearchResult, String> {
    // 解析 JSON 参数
    let params: SearchParams = serde_json::from_str(params_json).map_err(|e| e.to_string())?;
    let keyword = params.title.as_deref().unwrap_or(&params.query);
    
    // 发起 HTTP 请求 (需自行封装宿主提供的 http_request)
    let url = format!("https://api.example.com/search?q={}", keyword);
    let body = fetch_url(&url)?;
    
    // 解析响应并构造结果
    // ...
    
    // 最佳实践：如果提供了 author 筛选，在此处进行过滤或重排
    if let Some(author) = params.author {
        // ... filter logic
    }

    Ok(SearchResult { 
        items: vec![], // ...
        total: 0,
        page: params.page,
        page_size: 20
    })
}
```

### 1.3 编译
```bash
rustup target add wasm32-unknown-unknown
cargo build --target wasm32-unknown-unknown --release
```
编译产物位于 `target/wasm32-unknown-unknown/release/my_scraper_wasm.wasm`。

## 2. 宿主函数
WASM 插件可以通过 `extern "C"` 调用宿主提供的功能。由于沙箱隔离，插件不能绕过宿主函数直接访问网络、数据库或存储库文件。

```rust
#[link(wasm_import_module = "ting_env")]
extern "C" {
    /// 发起 HTTP GET 请求，返回请求句柄（≥0）或错误码（<0）。
    fn http_request(url_ptr: *const u8, url_len: i32) -> i32;
    /// 发起 HTTP POST 请求，返回请求句柄（≥0）或错误码（<0）。
    fn http_post(url_ptr: *const u8, url_len: i32, body_ptr: *const u8, body_len: i32) -> i32;
    /// 发起带有 Bearer Token 的 HTTP GET 请求
    fn http_get_with_token(url_ptr: *const u8, url_len: i32, token_ptr: *const u8, token_len: i32) -> i32;
    /// 发起自定义 HTTP 请求（支持指定 method 和 headers JSON）
    fn http_request_with_headers(
        url_ptr: *const u8, url_len: i32,
        method_ptr: *const u8, method_len: i32,
        headers_ptr: *const u8, headers_len: i32,
        body_ptr: *const u8, body_len: i32
    ) -> i32;
    /// 获取响应体长度（字节）。
    fn http_response_size(handle: i32) -> i32;
    /// 读取响应体到缓冲区，返回实际读取的字节数（≤ len）或错误码。
    fn http_read_body(handle: i32, ptr: *mut u8, len: i32) -> i32;

    /// 调用 HostGateway，返回响应句柄（≥0）或错误码（<0）。
    fn host_invoke(method_ptr: *const u8, method_len: i32, params_ptr: *const u8, params_len: i32) -> i32;
    /// 获取 HostGateway 响应体长度（字节）。
    fn host_response_size(handle: i32) -> i32;
    /// 读取 HostGateway 响应体到缓冲区。
    fn host_read_body(handle: i32, ptr: *mut u8, len: i32) -> i32;
}
```

`host_invoke` 的 `method` 使用 [HostGateway 能力调用详解](./hostgateway.md) 中的方法名，例如 `books.list`、`database.get`、`library.file.read`。返回体是 JSON；如果宿主拒绝调用，响应可能包含 `{ "error": "..." }`。

### 2.1 封装示例：发起自定义网络请求
宿主提供的函数需要手动进行指针传递，在业务逻辑中，推荐像下面这样封装一层 Rust 函数，以方便直接使用 `&str` 等数据类型：

```rust
fn fetch_url_custom(url: &str, method: &str, headers_json: &str, body_data: &[u8]) -> Result<Vec<u8>, String> {
    let handle = unsafe {
        http_request_with_headers(
            url.as_ptr(), url.len() as i32,
            method.as_ptr(), method.len() as i32,
            headers_json.as_ptr(), headers_json.len() as i32,
            body_data.as_ptr(), body_data.len() as i32
        )
    };
    
    if handle < 0 {
        return Err(format!("HTTP custom request failed: {}", -handle));
    }

    let size = unsafe { http_response_size(handle) };
    if size < 0 {
        return Err("Failed to get response size".to_string());
    }

    let mut body = vec![0u8; size as usize];
    let read = unsafe { http_read_body(handle, body.as_mut_ptr(), size) };
    if read < 0 {
        return Err("Failed to read body".to_string());
    }
    
    Ok(body)
}
```

## 3. 部署
将编译好的 `.wasm` 文件放在 `plugin.yml` 同级，然后使用 `trpack build` 打包为 `.tr` 文件：

```bash
trpack validate .
trpack build . --output dist/my-scraper-wasm.tr
trpack verify dist/my-scraper-wasm.tr
```
