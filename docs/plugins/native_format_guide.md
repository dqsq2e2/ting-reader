# Native 插件开发指南

Native 插件使用 Rust 语言编写并编译为动态链接库（`.dll`, `.so`, `.dylib`），适合平台相关能力：格式处理、流式解密、系统库调用、FFmpeg 供应等。Native 插件通过 `capabilities` 声明能力，发布时需要按服务端平台分别打包。

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

提供插件声明文件 `plugin.yml`（详情请参考 [插件开发指南](./plugin-dev.md)）。格式插件通过 `format_handler` 能力声明支持的扩展名：

```yaml
id: my-format-plugin
name: My Format Plugin
version: 1.0.0
runtime: native
entry_point: my_format_plugin.dll
capabilities:
  - id: format.handler
    kind: format_handler
    invoke: get_stream_url
    extensions:
      - m4a
      - flac
      - wav
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
        "write_metadata" => write_metadata(params_json),
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
    // params 包含: file_path (文件绝对路径), extract_cover (布尔值，是否需要提取封面)
    let path = params["file_path"].as_str().ok_or("Missing path")?;
    let extract_cover = params.get("extract_cover").and_then(|v| v.as_bool()).unwrap_or(true);
    
    // 读取元数据...
    // 如果 extract_cover 为 true 且需要从音频中提取封面并写入磁盘，请在此时处理
    // 提取成功后返回 cover_url (可以是本地路径或 URL)
    Ok(serde_json::json!({ "title": "...", "artist": "..." }))
}

fn write_metadata(params: Value) -> Result<Value, String> {
    let path = params["file_path"].as_str().ok_or("Missing path")?;
    // params 包含: title, artist, album, genre, description, cover_path
    // 更新元数据...
    Ok(serde_json::json!({ "status": "success" }))
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

### 1.4 HostGateway（可选）

Native 插件需要读取书籍、存储库文件、缓存或受控数据库实体时，应通过 HostGateway，而不是直接访问 Ting Reader 数据库文件。插件可选导出 `plugin_set_host_api` 接收宿主函数表；在 `plugin_invoke` 执行期间调用 `host_invoke(method, params_json, result_ptr)`，并使用 `host_free` 释放返回字符串。

HostGateway 方法、权限、返回格式和错误码见 [HostGateway 能力调用详解](./hostgateway.md)。调用依赖当前认证用户上下文；公共路由、初始化和后台无用户上下文的场景会被拒绝。

## 2. 部署
将编译好的动态库文件放在 `plugin.yml` 同级，然后使用 `trpack build` 打包为 `.tr` 文件。Native 插件必须与宿主程序的操作系统和架构匹配，常用平台名包括 `windows-x86_64`、`linux-x86_64`、`linux-aarch64`。

```bash
trpack validate .
trpack build . --platform-tag windows-x86_64 --output dist/my-format-plugin-1.0.0-windows-x86_64.tr
trpack verify dist/my-format-plugin-1.0.0-windows-x86_64.tr
```

如果插件随包携带 FFmpeg 等二进制工具，建议放在 `bin/` 目录，并用 `tool_provider` 能力返回实际路径，不要让核心后端写死工具位置：

```bash
trpack build . --include bin --platform-tag windows-x86_64 --output dist/my-tool-plugin-1.0.0-windows-x86_64.tr
```

## 3. 高级功能：流式解密
为了支持大文件播放，建议实现 `get_decryption_plan` 和 `decrypt_chunk` 方法，允许播放器按需解密文件的特定部分，而不是一次性解密整个文件。

### 3.1 解密计划 (Decryption Plan) 规范
在返回的 `DecryptionPlan` 中，`segments` 数组描述了文件的物理结构。
- 对于 `type: "encrypted"` 的段，`length` 必须是该段在**物理文件中的真实字节长度**。后端在建立流时，会主动、完整地读取这部分物理字节并调用 `decrypt_chunk` 进行解密，然后**自动测量解密后的实际逻辑长度**。
- 这意味着，即使解密后的数据比物理数据小（由于去除了 AES 填充等），你也不需要在 `length` 中预测解密后的长度，直接填物理长度即可。后端会通过预解密机制自动修正逻辑偏移量，从而完美支持浏览器的任意 `Range` 请求（拖拽进度条）。

如果插件解密后的数据大小与原始加密段大小不同（例如因为去除了填充或解压缩），建议在 `DecryptionPlan` 中提供 `total_size` 字段（整个文件的最终逻辑大小）。如果未提供，后端将根据各段逻辑长度自动计算。

```rust
fn get_decryption_plan(params: Value) -> Result<Value, String> {
    // 返回文件的加密段和明文段分布
    Ok(serde_json::json!({
        "segments": [
            // offset 和 length 必须是物理文件中的真实偏移和真实长度！
            { "type": "encrypted", "offset": 1024, "length": 5000 },
            { "type": "plain", "offset": 6024, "length": -1 }
        ],
        "total_size": 123456 // 可选：解密后的总大小（字节）
    }))
}
```

## 4. 转码支持 (可选)
如果你的插件不需要 FFmpeg 转码（即可以直接输出 PCM/WAV/AAC 流），或者需要明确告知宿主程序不支持某些转码操作，请实现 `get_stream_url` 方法。

对于不需要转码支持的插件（例如自身负责解码的 Native 插件），应返回空对象以避免后端日志报错：

```rust
fn get_stream_url(_params: Value) -> Result<Value, String> {
    // 返回空对象表示不提供特殊的转码命令，后端将回退到默认处理逻辑（如 Standard Stream）
    Ok(serde_json::json!({}))
}
```

并在 `plugin_invoke` 中注册该方法：

```rust
match method_str {
    // ...
    "get_stream_url" => get_stream_url(params_json),
    // ...
}
```
