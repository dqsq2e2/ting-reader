//! HTTP host functions for WASM runtime
//!
//! Registers `ting_env` host functions (http_request, http_post, etc.)
//! that let WASM plugins make HTTP requests via a blocking reqwest client.

use std::time::Duration;
use wasmtime::*;
use super::plugin::PluginState;

/// Register all `ting_env` host functions on a linker
pub fn add_host_functions(linker: &mut Linker<PluginState>) -> Result<(), anyhow::Error> {
    // ting_http_request(url_ptr, url_len) -> handle (>0) or error (<0)
    linker.func_wrap("ting_env", "http_request", |mut caller: Caller<'_, PluginState>, url_ptr: i32, url_len: i32| -> i32 {
        let mem = match caller.get_export("memory") {
            Some(Extern::Memory(mem)) => mem,
            _ => return -1,
        };
        let ctx = caller.as_context();
        let data = mem.data(&ctx);
        let url = match std::str::from_utf8(&data[url_ptr as usize..(url_ptr + url_len) as usize]) {
            Ok(s) => s,
            Err(_) => return -2,
        };

        tracing::info!("插件请求 URL: {}", url);

        let url_clone = url.to_string();
        let resp_result = std::thread::spawn(move || {
            let client = match reqwest::blocking::Client::builder()
                .user_agent("TingReader/1.0")
                .timeout(Duration::from_secs(30))
                .build() { Ok(c) => c, Err(_) => return Err(-3i32) };

            let resp = match client.get(&url_clone).send() {
                Ok(r) => r, Err(_) => return Err(-4i32)
            };

            if !resp.status().is_success() {
                return Err(-(resp.status().as_u16() as i32));
            }

            resp.bytes().map(|b| b.to_vec()).map_err(|_| -5)
        }).join();

        let body = match resp_result {
            Ok(Ok(b)) => b,
            Ok(Err(e)) => return e,
            Err(_) => return -6,
        };

        if let Ok(body_str) = std::str::from_utf8(&body) {
            tracing::info!("插件收到响应 (长度={}): {:.200}...", body.len(), body_str);
        }

        let handle = (caller.data().http_responses.len() as u32) + 1;
        caller.data_mut().http_responses.insert(handle, body);
        handle as i32
    }).map_err(|e| anyhow::anyhow!("Failed to define http_request: {}", e))?;

    // ting_http_post(url_ptr, url_len, body_ptr, body_len) -> handle (>0) or error (<0)
    linker.func_wrap("ting_env", "http_post", |mut caller: Caller<'_, PluginState>, url_ptr: i32, url_len: i32, body_ptr: i32, body_len: i32| -> i32 {
        let mem = match caller.get_export("memory") {
            Some(Extern::Memory(mem)) => mem,
            _ => return -1,
        };
        let ctx = caller.as_context();
        let data = mem.data(&ctx);
        let url = match std::str::from_utf8(&data[url_ptr as usize..(url_ptr + url_len) as usize]) {
            Ok(s) => s, Err(_) => return -2,
        };
        let req_body = data[body_ptr as usize..(body_ptr + body_len) as usize].to_vec();

        tracing::info!("插件 POST 请求 URL: {}", url);

        let url_clone = url.to_string();
        let resp_result = std::thread::spawn(move || {
            let client = match reqwest::blocking::Client::builder()
                .user_agent("TingReader/1.0")
                .timeout(Duration::from_secs(30))
                .build() { Ok(c) => c, Err(_) => return Err(-3i32) };

            let resp = match client.post(&url_clone)
                .header("Content-Type", "application/x-www-form-urlencoded")
                .body(req_body)
                .send() { Ok(r) => r, Err(_) => return Err(-4i32) };

            if !resp.status().is_success() {
                return Err(-(resp.status().as_u16() as i32));
            }
            resp.bytes().map(|b| b.to_vec()).map_err(|_| -5)
        }).join();

        let body = match resp_result {
            Ok(Ok(b)) => b,
            Ok(Err(e)) => return e,
            Err(_) => return -6,
        };
        if let Ok(body_str) = std::str::from_utf8(&body) {
            tracing::info!("插件收到响应 (长度={}): {:.200}...", body.len(), body_str);
        }
        let handle = (caller.data().http_responses.len() as u32) + 1;
        caller.data_mut().http_responses.insert(handle, body);
        handle as i32
    }).map_err(|e| anyhow::anyhow!("Failed to define http_post: {}", e))?;

    // ting_http_get_with_token(url_ptr, url_len, token_ptr, token_len) -> handle (>0) or error (<0)
    linker.func_wrap("ting_env", "http_get_with_token", |mut caller: Caller<'_, PluginState>, url_ptr: i32, url_len: i32, token_ptr: i32, token_len: i32| -> i32 {
        let mem = match caller.get_export("memory") {
            Some(Extern::Memory(mem)) => mem,
            _ => return -1,
        };
        let ctx = caller.as_context();
        let data = mem.data(&ctx);
        let url = match std::str::from_utf8(&data[url_ptr as usize..(url_ptr + url_len) as usize]) {
            Ok(s) => s, Err(_) => return -2,
        };
        let token = match std::str::from_utf8(&data[token_ptr as usize..(token_ptr + token_len) as usize]) {
            Ok(s) => s, Err(_) => return -2,
        };

        tracing::info!("插件 GET (Auth) 请求 URL: {}", url);

        let url_clone = url.to_string();
        let token_clone = token.to_string();
        let resp_result = std::thread::spawn(move || {
            let client = match reqwest::blocking::Client::builder()
                .user_agent("TingReader/1.0")
                .timeout(Duration::from_secs(30))
                .build() { Ok(c) => c, Err(_) => return Err(-3i32) };

            let mut req = client.get(&url_clone);
            if !token_clone.is_empty() {
                req = req.header("Authorization", format!("Bearer {}", token_clone));
            }
            let resp = match req.send() { Ok(r) => r, Err(_) => return Err(-4i32) };
            if !resp.status().is_success() {
                return Err(-(resp.status().as_u16() as i32));
            }
            resp.bytes().map(|b| b.to_vec()).map_err(|_| -5)
        }).join();

        let body = match resp_result {
            Ok(Ok(b)) => b,
            Ok(Err(e)) => return e,
            Err(_) => return -6,
        };
        if let Ok(body_str) = std::str::from_utf8(&body) {
            tracing::info!("插件收到响应 (长度={}): {:.200}...", body.len(), body_str);
        }
        let handle = (caller.data().http_responses.len() as u32) + 1;
        caller.data_mut().http_responses.insert(handle, body);
        handle as i32
    }).map_err(|e| anyhow::anyhow!("Failed to define http_get_with_token: {}", e))?;

    // ting_http_request_with_headers(...)
    linker.func_wrap("ting_env", "http_request_with_headers", |mut caller: Caller<'_, PluginState>, url_ptr: i32, url_len: i32, method_ptr: i32, method_len: i32, headers_ptr: i32, headers_len: i32, body_ptr: i32, body_len: i32| -> i32 {
        let mem = match caller.get_export("memory") {
            Some(Extern::Memory(mem)) => mem,
            _ => return -1,
        };
        let ctx = caller.as_context();
        let data = mem.data(&ctx);
        let url = match std::str::from_utf8(&data[url_ptr as usize..(url_ptr + url_len) as usize]) {
            Ok(s) => s, Err(_) => return -2,
        };
        let method = match std::str::from_utf8(&data[method_ptr as usize..(method_ptr + method_len) as usize]) {
            Ok(s) => s, Err(_) => return -2,
        };
        let headers_json = match std::str::from_utf8(&data[headers_ptr as usize..(headers_ptr + headers_len) as usize]) {
            Ok(s) => s, Err(_) => return -2,
        };
        let req_body = if body_len > 0 {
            data[body_ptr as usize..(body_ptr + body_len) as usize].to_vec()
        } else { vec![] };

        tracing::info!("插件自定义请求 URL: {} Method: {}", url, method);

        let url_clone = url.to_string();
        let method_clone = method.to_string();
        let headers_json_clone = headers_json.to_string();
        let req_body_clone = req_body.clone();
        let resp_result = std::thread::spawn(move || {
            let client = match reqwest::blocking::Client::builder()
                .user_agent("TingReader/1.0")
                .timeout(Duration::from_secs(30))
                .build() { Ok(c) => c, Err(_) => return Err(-3i32) };

            let http_method = match method_clone.to_uppercase().as_str() {
                "GET" => reqwest::Method::GET, "POST" => reqwest::Method::POST,
                "PUT" => reqwest::Method::PUT, "DELETE" => reqwest::Method::DELETE,
                "HEAD" => reqwest::Method::HEAD, "OPTIONS" => reqwest::Method::OPTIONS,
                "PATCH" => reqwest::Method::PATCH,
                _ => reqwest::Method::GET,
            };
            let mut req = client.request(http_method, &url_clone);
            if !headers_json_clone.is_empty() {
                if let Ok(headers_map) = serde_json::from_str::<std::collections::HashMap<String, String>>(&headers_json_clone) {
                    for (k, v) in headers_map { req = req.header(k, v); }
                }
            }
            if !req_body_clone.is_empty() { req = req.body(req_body_clone); }
            let resp = match req.send() { Ok(r) => r, Err(_) => return Err(-4i32) };
            if !resp.status().is_success() { return Err(-(resp.status().as_u16() as i32)); }
            resp.bytes().map(|b| b.to_vec()).map_err(|_| -5)
        }).join();

        let body = match resp_result {
            Ok(Ok(b)) => b,
            Ok(Err(e)) => return e,
            Err(_) => return -6,
        };
        if let Ok(body_str) = std::str::from_utf8(&body) {
            tracing::info!("插件收到响应 (长度={}): {:.200}...", body.len(), body_str);
        }
        let handle = (caller.data().http_responses.len() as u32) + 1;
        caller.data_mut().http_responses.insert(handle, body);
        handle as i32
    }).map_err(|e| anyhow::anyhow!("Failed to define http_request_with_headers: {}", e))?;

    // ting_http_response_size(handle) -> size
    linker.func_wrap("ting_env", "http_response_size", |caller: Caller<'_, PluginState>, handle: i32| -> i32 {
        if let Some(body) = caller.data().http_responses.get(&(handle as u32)) {
            body.len() as i32
        } else { -1 }
    }).map_err(|e| anyhow::anyhow!("Failed to define http_response_size: {}", e))?;

    // ting_http_read_body(handle, ptr, len) -> bytes_read
    linker.func_wrap("ting_env", "http_read_body", |mut caller: Caller<'_, PluginState>, handle: i32, ptr: i32, len: i32| -> i32 {
        let body = if let Some(b) = caller.data().http_responses.get(&(handle as u32)) {
            b.clone()
        } else { return -1; };
        let copy_len = std::cmp::min(body.len(), len as usize);
        let mem = match caller.get_export("memory") {
            Some(Extern::Memory(mem)) => mem,
            _ => return -2,
        };
        if mem.write(&mut caller, ptr as usize, &body[..copy_len]).is_err() {
            return -3;
        }
        caller.data_mut().http_responses.remove(&(handle as u32));
        copy_len as i32
    }).map_err(|e| anyhow::anyhow!("Failed to define http_read_body: {}", e))?;

    Ok(())
}
