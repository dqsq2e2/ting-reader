# Native 格式插件开发指南

Native 插件使用 Rust 语言编写，编译为动态链接库（`.dll`, `.so`, `.dylib`）。它是性能最强、功能最完整的插件类型，专门用于处理复杂的音频格式（如加密格式）。

**注意**: Native 插件具有完全的系统访问权限，开发和使用时需谨慎。

## 1. 快速开始

### 1.1 项目结构
创建一个新的 Rust 库项目：
```bash
cargo new --lib my-format-plugin
```

编辑 `Cargo.toml`：
```toml
[package]
name = "my-format-plugin"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib"]  # 必须是动态库

[dependencies]
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
# 其他依赖...
```

### 1.2 核心代码 (src/lib.rs)
```rust
use std::ffi::{CStr, CString};
use std::os::raw::c_int;
use serde_json::Value;

// 1. 核心入口 plugin_invoke (必须!)
#[no_mangle]
pub unsafe extern "C" fn plugin_invoke(
    method: *const u8,
    params: *const u8,
    result_ptr: *mut *mut u8,
) -> c_int {
    let method_str = CStr::from_ptr(method as *const i8).to_str().unwrap();
    let params_str = CStr::from_ptr(params as *const i8).to_str().unwrap();
    let params_json: Value = serde_json::from_str(params_str).unwrap();

    let result = match method_str {
        "detect" => detect(params_json),
        "extract_metadata" => extract_metadata(params_json),
        "decrypt" => decrypt(params_json),
        // ... 其他方法
        _ => Err("Unknown method".to_string()),
    };

    match result {
        Ok(val) => {
            let json = serde_json::to_string(&val).unwrap();
            let c_string = CString::new(json).unwrap();
            *result_ptr = c_string.into_raw() as *mut u8;
            0 // 成功
        }
        Err(e) => -1 // 失败
    }
}

// 2. 核心方法实现
fn detect(params: Value) -> Result<Value, String> {
    let path = params["file_path"].as_str().ok_or("Missing path")?;
    // 读取文件头，判断是否支持
    let is_supported = check_magic_header(path);
    Ok(serde_json::json!({ "is_supported": is_supported }))
}

fn extract_metadata(params: Value) -> Result<Value, String> {
    // 读取元数据...
    Ok(serde_json::json!({ "title": "...", "artist": "..." }))
}

fn decrypt(params: Value) -> Result<Value, String> {
    // 解密文件...
    Ok(serde_json::json!({ "status": "success" }))
}

// 3. 内存释放导出 (必须!)
#[no_mangle]
pub unsafe extern "C" fn plugin_free(ptr: *mut u8) {
    if !ptr.is_null() {
        let _ = CString::from_raw(ptr as *mut i8);
    }
}
```

### 1.3 编译
```bash
cargo build --release
```
编译产物位于 `target/release/` 目录下（Windows 为 `.dll`，Linux 为 `.so`，macOS 为 `.dylib`）。

## 2. 部署
将编译好的动态库文件和 `plugin.json` 放入 `plugins/my-format-plugin/` 目录。
注意：Native 插件必须与宿主程序的操作系统和架构匹配。

## 3. 高级功能：流式解密
为了支持大文件播放，建议实现 `get_decryption_plan` 和 `decrypt_chunk` 方法，允许播放器按需解密文件的特定部分，而不是一次性解密整个文件。

```rust
fn get_decryption_plan(params: Value) -> Result<Value, String> {
    // 返回文件的加密段和明文段分布
    Ok(serde_json::json!({
        "segments": [
            { "type": "encrypted", "offset": 1024, "length": 5000 },
            { "type": "plain", "offset": 6024, "length": -1 }
        ]
    }))
}
```
