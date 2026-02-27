# WASM 刮削插件开发指南

WASM (WebAssembly) 插件使用 Rust 语言编写，编译为 `.wasm` 文件。它提供了比 JS 更好的性能、更强的类型安全和更好的工程化支持。

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

### 1.2 核心代码 (src/lib.rs)
```rust
use std::ffi::{CStr, CString};
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
#[no_mangle]
pub extern "C" fn invoke(method_ptr: *const i8, params_ptr: *const i8) -> *mut i8 {
    let method = unsafe { CStr::from_ptr(method_ptr).to_string_lossy() };
    let params = unsafe { CStr::from_ptr(params_ptr).to_string_lossy() };

    let result = match method.as_ref() {
        "search" => handle_search(&params),
        _ => Err("Unknown method".to_string()),
    };

    let json = match result {
        Ok(v) => serde_json::to_string(&v).unwrap(),
        Err(e) => serde_json::json!({ "error": e }).to_string(),
    };
    CString::new(json).unwrap().into_raw()
}

// 3. 业务逻辑实现
fn handle_search(params_json: &str) -> Result<SearchResult, String> {
    // 解析 JSON 参数
    let params: SearchParams = serde_json::from_str(params_json).map_err(|e| e.to_string())?;
    
    // 发起 HTTP 请求 (需自行封装宿主提供的 http_request)
    let url = format!("https://api.example.com/search?q={}", params.query);
    let body = fetch_url(&url)?;
    
    // 解析响应并构造结果
    // ...
    
    Ok(SearchResult { ... })
}

// 4. 内存管理导出 (必须!)
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
```

### 1.3 编译
```bash
cargo build --target wasm32-wasip1 --release
```
编译产物位于 `target/wasm32-wasip1/release/my_scraper_wasm.wasm`。

## 2. 宿主函数
WASM 插件可以通过 `extern "C"` 调用宿主提供的功能：

```rust
#[link(wasm_import_module = "ting_env")]
extern "C" {
    fn http_request(url_ptr: *const u8, url_len: i32) -> i32;
    fn http_response_size(handle: i32) -> i32;
    fn http_read_body(handle: i32, ptr: *mut u8, len: i32) -> i32;
}
```

## 3. 部署
将编译好的 `.wasm` 文件和 `plugin.json` 放入 `plugins/my-scraper-wasm/` 目录即可。
