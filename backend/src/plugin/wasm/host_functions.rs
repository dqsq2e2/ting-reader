//! HTTP host functions for WASM runtime
//!
//! Registers `ting_env` host functions (http_request, http_post, etc.)
//! that let WASM plugins make HTTP requests via a blocking reqwest client.

use super::plugin::PluginState;
use std::time::Duration;
use wasmtime::*;

const ERR_PERMISSION_DENIED: i32 = -7;
const ERR_INVALID_MEMORY: i32 = -1;
const ERR_INVALID_UTF8: i32 = -2;
const ERR_INVALID_JSON: i32 = -3;
const ERR_MISSING_GATEWAY: i32 = -8;
const ERR_MISSING_USER: i32 = -9;
const ERR_MISSING_RUNTIME: i32 = -10;
const ERR_HOST_PANIC: i32 = -11;
const ERR_SERIALIZE_RESPONSE: i32 = -12;

/// Register all `ting_env` host functions on a linker
pub fn add_host_functions(linker: &mut Linker<PluginState>) -> Result<(), anyhow::Error> {
    // ting_http_request(url_ptr, url_len) -> handle (>0) or error (<0)
    linker
        .func_wrap(
            "ting_env",
            "http_request",
            |mut caller: Caller<'_, PluginState>, url_ptr: i32, url_len: i32| -> i32 {
                let mem = match caller.get_export("memory") {
                    Some(Extern::Memory(mem)) => mem,
                    _ => return -1,
                };
                let ctx = caller.as_context();
                let data = mem.data(&ctx);
                let url = match std::str::from_utf8(
                    &data[url_ptr as usize..(url_ptr + url_len) as usize],
                ) {
                    Ok(s) => s,
                    Err(_) => return -2,
                };

                tracing::info!("Plugin requested URL: {}", url);
                if !is_network_allowed(&caller.data().allowed_domains, url) {
                    tracing::warn!(url = %url, "WASM plugin network access denied");
                    return ERR_PERMISSION_DENIED;
                }

                let url_clone = url.to_string();
                let resp_result = std::thread::spawn(move || {
                    let client = match reqwest::blocking::Client::builder()
                        .user_agent("TingReader/1.0")
                        .timeout(Duration::from_secs(30))
                        .build()
                    {
                        Ok(c) => c,
                        Err(_) => return Err(-3i32),
                    };

                    let resp = match client.get(&url_clone).send() {
                        Ok(r) => r,
                        Err(_) => return Err(-4i32),
                    };

                    if !resp.status().is_success() {
                        return Err(-(resp.status().as_u16() as i32));
                    }

                    resp.bytes().map(|b| b.to_vec()).map_err(|_| -5)
                })
                .join();

                let body = match resp_result {
                    Ok(Ok(b)) => b,
                    Ok(Err(e)) => return e,
                    Err(_) => return -6,
                };

                if let Ok(body_str) = std::str::from_utf8(&body) {
                    tracing::info!(
                        "Plugin received response (length={}): {:.200}...",
                        body.len(),
                        body_str
                    );
                }

                let handle = (caller.data().http_responses.len() as u32) + 1;
                caller.data_mut().http_responses.insert(handle, body);
                handle as i32
            },
        )
        .map_err(|e| anyhow::anyhow!("Failed to define http_request: {}", e))?;

    // ting_http_post(url_ptr, url_len, body_ptr, body_len) -> handle (>0) or error (<0)
    linker
        .func_wrap(
            "ting_env",
            "http_post",
            |mut caller: Caller<'_, PluginState>,
             url_ptr: i32,
             url_len: i32,
             body_ptr: i32,
             body_len: i32|
             -> i32 {
                let mem = match caller.get_export("memory") {
                    Some(Extern::Memory(mem)) => mem,
                    _ => return -1,
                };
                let ctx = caller.as_context();
                let data = mem.data(&ctx);
                let url = match std::str::from_utf8(
                    &data[url_ptr as usize..(url_ptr + url_len) as usize],
                ) {
                    Ok(s) => s,
                    Err(_) => return -2,
                };
                let req_body = data[body_ptr as usize..(body_ptr + body_len) as usize].to_vec();

                tracing::info!("Plugin POST request URL: {}", url);
                if !is_network_allowed(&caller.data().allowed_domains, url) {
                    tracing::warn!(url = %url, "WASM plugin network access denied");
                    return ERR_PERMISSION_DENIED;
                }

                let url_clone = url.to_string();
                let resp_result = std::thread::spawn(move || {
                    let client = match reqwest::blocking::Client::builder()
                        .user_agent("TingReader/1.0")
                        .timeout(Duration::from_secs(30))
                        .build()
                    {
                        Ok(c) => c,
                        Err(_) => return Err(-3i32),
                    };

                    let resp = match client
                        .post(&url_clone)
                        .header("Content-Type", "application/x-www-form-urlencoded")
                        .body(req_body)
                        .send()
                    {
                        Ok(r) => r,
                        Err(_) => return Err(-4i32),
                    };

                    if !resp.status().is_success() {
                        return Err(-(resp.status().as_u16() as i32));
                    }
                    resp.bytes().map(|b| b.to_vec()).map_err(|_| -5)
                })
                .join();

                let body = match resp_result {
                    Ok(Ok(b)) => b,
                    Ok(Err(e)) => return e,
                    Err(_) => return -6,
                };
                if let Ok(body_str) = std::str::from_utf8(&body) {
                    tracing::info!(
                        "Plugin received response (length={}): {:.200}...",
                        body.len(),
                        body_str
                    );
                }
                let handle = (caller.data().http_responses.len() as u32) + 1;
                caller.data_mut().http_responses.insert(handle, body);
                handle as i32
            },
        )
        .map_err(|e| anyhow::anyhow!("Failed to define http_post: {}", e))?;

    // ting_http_get_with_token(url_ptr, url_len, token_ptr, token_len) -> handle (>0) or error (<0)
    linker
        .func_wrap(
            "ting_env",
            "http_get_with_token",
            |mut caller: Caller<'_, PluginState>,
             url_ptr: i32,
             url_len: i32,
             token_ptr: i32,
             token_len: i32|
             -> i32 {
                let mem = match caller.get_export("memory") {
                    Some(Extern::Memory(mem)) => mem,
                    _ => return -1,
                };
                let ctx = caller.as_context();
                let data = mem.data(&ctx);
                let url = match std::str::from_utf8(
                    &data[url_ptr as usize..(url_ptr + url_len) as usize],
                ) {
                    Ok(s) => s,
                    Err(_) => return -2,
                };
                let token = match std::str::from_utf8(
                    &data[token_ptr as usize..(token_ptr + token_len) as usize],
                ) {
                    Ok(s) => s,
                    Err(_) => return -2,
                };

                tracing::info!("Plugin GET (auth) request URL: {}", url);
                if !is_network_allowed(&caller.data().allowed_domains, url) {
                    tracing::warn!(url = %url, "WASM plugin network access denied");
                    return ERR_PERMISSION_DENIED;
                }

                let url_clone = url.to_string();
                let token_clone = token.to_string();
                let resp_result = std::thread::spawn(move || {
                    let client = match reqwest::blocking::Client::builder()
                        .user_agent("TingReader/1.0")
                        .timeout(Duration::from_secs(30))
                        .build()
                    {
                        Ok(c) => c,
                        Err(_) => return Err(-3i32),
                    };

                    let mut req = client.get(&url_clone);
                    if !token_clone.is_empty() {
                        req = req.header("Authorization", format!("Bearer {}", token_clone));
                    }
                    let resp = match req.send() {
                        Ok(r) => r,
                        Err(_) => return Err(-4i32),
                    };
                    if !resp.status().is_success() {
                        return Err(-(resp.status().as_u16() as i32));
                    }
                    resp.bytes().map(|b| b.to_vec()).map_err(|_| -5)
                })
                .join();

                let body = match resp_result {
                    Ok(Ok(b)) => b,
                    Ok(Err(e)) => return e,
                    Err(_) => return -6,
                };
                if let Ok(body_str) = std::str::from_utf8(&body) {
                    tracing::info!(
                        "Plugin received response (length={}): {:.200}...",
                        body.len(),
                        body_str
                    );
                }
                let handle = (caller.data().http_responses.len() as u32) + 1;
                caller.data_mut().http_responses.insert(handle, body);
                handle as i32
            },
        )
        .map_err(|e| anyhow::anyhow!("Failed to define http_get_with_token: {}", e))?;

    // ting_http_request_with_headers(...)
    linker
        .func_wrap(
            "ting_env",
            "http_request_with_headers",
            |mut caller: Caller<'_, PluginState>,
             url_ptr: i32,
             url_len: i32,
             method_ptr: i32,
             method_len: i32,
             headers_ptr: i32,
             headers_len: i32,
             body_ptr: i32,
             body_len: i32|
             -> i32 {
                let mem = match caller.get_export("memory") {
                    Some(Extern::Memory(mem)) => mem,
                    _ => return -1,
                };
                let ctx = caller.as_context();
                let data = mem.data(&ctx);
                let url = match std::str::from_utf8(
                    &data[url_ptr as usize..(url_ptr + url_len) as usize],
                ) {
                    Ok(s) => s,
                    Err(_) => return -2,
                };
                let method = match std::str::from_utf8(
                    &data[method_ptr as usize..(method_ptr + method_len) as usize],
                ) {
                    Ok(s) => s,
                    Err(_) => return -2,
                };
                let headers_json = match std::str::from_utf8(
                    &data[headers_ptr as usize..(headers_ptr + headers_len) as usize],
                ) {
                    Ok(s) => s,
                    Err(_) => return -2,
                };
                let req_body = if body_len > 0 {
                    data[body_ptr as usize..(body_ptr + body_len) as usize].to_vec()
                } else {
                    vec![]
                };

                tracing::info!("Plugin custom request URL: {} Method: {}", url, method);
                if !is_network_allowed(&caller.data().allowed_domains, url) {
                    tracing::warn!(url = %url, "WASM plugin network access denied");
                    return ERR_PERMISSION_DENIED;
                }

                let url_clone = url.to_string();
                let method_clone = method.to_string();
                let headers_json_clone = headers_json.to_string();
                let req_body_clone = req_body.clone();
                let resp_result = std::thread::spawn(move || {
                    let client = match reqwest::blocking::Client::builder()
                        .user_agent("TingReader/1.0")
                        .timeout(Duration::from_secs(30))
                        .build()
                    {
                        Ok(c) => c,
                        Err(_) => return Err(-3i32),
                    };

                    let http_method = match method_clone.to_uppercase().as_str() {
                        "GET" => reqwest::Method::GET,
                        "POST" => reqwest::Method::POST,
                        "PUT" => reqwest::Method::PUT,
                        "DELETE" => reqwest::Method::DELETE,
                        "HEAD" => reqwest::Method::HEAD,
                        "OPTIONS" => reqwest::Method::OPTIONS,
                        "PATCH" => reqwest::Method::PATCH,
                        _ => reqwest::Method::GET,
                    };
                    let mut req = client.request(http_method, &url_clone);
                    if !headers_json_clone.is_empty() {
                        if let Ok(headers_map) = serde_json::from_str::<
                            std::collections::HashMap<String, String>,
                        >(&headers_json_clone)
                        {
                            for (k, v) in headers_map {
                                req = req.header(k, v);
                            }
                        }
                    }
                    if !req_body_clone.is_empty() {
                        req = req.body(req_body_clone);
                    }
                    let resp = match req.send() {
                        Ok(r) => r,
                        Err(_) => return Err(-4i32),
                    };
                    if !resp.status().is_success() {
                        return Err(-(resp.status().as_u16() as i32));
                    }
                    resp.bytes().map(|b| b.to_vec()).map_err(|_| -5)
                })
                .join();

                let body = match resp_result {
                    Ok(Ok(b)) => b,
                    Ok(Err(e)) => return e,
                    Err(_) => return -6,
                };
                if let Ok(body_str) = std::str::from_utf8(&body) {
                    tracing::info!(
                        "Plugin received response (length={}): {:.200}...",
                        body.len(),
                        body_str
                    );
                }
                let handle = (caller.data().http_responses.len() as u32) + 1;
                caller.data_mut().http_responses.insert(handle, body);
                handle as i32
            },
        )
        .map_err(|e| anyhow::anyhow!("Failed to define http_request_with_headers: {}", e))?;

    // ting_http_response_size(handle) -> size
    linker
        .func_wrap(
            "ting_env",
            "http_response_size",
            |caller: Caller<'_, PluginState>, handle: i32| -> i32 {
                if let Some(body) = caller.data().http_responses.get(&(handle as u32)) {
                    body.len() as i32
                } else {
                    -1
                }
            },
        )
        .map_err(|e| anyhow::anyhow!("Failed to define http_response_size: {}", e))?;

    // ting_http_read_body(handle, ptr, len) -> bytes_read
    linker
        .func_wrap(
            "ting_env",
            "http_read_body",
            |mut caller: Caller<'_, PluginState>, handle: i32, ptr: i32, len: i32| -> i32 {
                let body = if let Some(b) = caller.data().http_responses.get(&(handle as u32)) {
                    b.clone()
                } else {
                    return -1;
                };
                let copy_len = std::cmp::min(body.len(), len as usize);
                let mem = match caller.get_export("memory") {
                    Some(Extern::Memory(mem)) => mem,
                    _ => return -2,
                };
                if mem
                    .write(&mut caller, ptr as usize, &body[..copy_len])
                    .is_err()
                {
                    return -3;
                }
                caller.data_mut().http_responses.remove(&(handle as u32));
                copy_len as i32
            },
        )
        .map_err(|e| anyhow::anyhow!("Failed to define http_read_body: {}", e))?;

    // ting_host_invoke(method_ptr, method_len, params_ptr, params_len) -> handle (>0) or error (<0)
    linker
        .func_wrap(
            "ting_env",
            "host_invoke",
            |mut caller: Caller<'_, PluginState>,
             method_ptr: i32,
             method_len: i32,
             params_ptr: i32,
             params_len: i32|
             -> i32 {
                let method = match read_wasm_string(&mut caller, method_ptr, method_len) {
                    Ok(value) => value,
                    Err(code) => return code,
                };
                let params = match read_wasm_json(&mut caller, params_ptr, params_len) {
                    Ok(value) => value,
                    Err(code) => return code,
                };

                let plugin_id = caller.data().plugin_id.clone();
                let permissions = caller.data().permissions.clone();
                let gateway = match caller
                    .data()
                    .host_gateway
                    .as_ref()
                    .and_then(|handle| handle.get())
                {
                    Some(gateway) => gateway,
                    None => return ERR_MISSING_GATEWAY,
                };
                let user = match caller.data().current_user.clone() {
                    Some(user) => user,
                    None => return ERR_MISSING_USER,
                };
                let runtime_handle = match tokio::runtime::Handle::try_current() {
                    Ok(handle) => handle,
                    Err(_) => return ERR_MISSING_RUNTIME,
                };

                let result = std::thread::spawn(move || {
                    runtime_handle.block_on(async move {
                        gateway
                            .invoke_with_permissions(
                                &plugin_id,
                                &permissions,
                                &user,
                                &method,
                                params,
                            )
                            .await
                    })
                })
                .join();

                let response = match result {
                    Ok(Ok(value)) => value,
                    Ok(Err(error)) => serde_json::json!({ "error": error.to_string() }),
                    Err(_) => return ERR_HOST_PANIC,
                };

                let body = match serde_json::to_vec(&response) {
                    Ok(body) => body,
                    Err(_) => return ERR_SERIALIZE_RESPONSE,
                };
                store_host_response(&mut caller, body)
            },
        )
        .map_err(|e| anyhow::anyhow!("Failed to define host_invoke: {}", e))?;

    // ting_host_response_size(handle) -> size
    linker
        .func_wrap(
            "ting_env",
            "host_response_size",
            |caller: Caller<'_, PluginState>, handle: i32| -> i32 {
                if let Some(body) = caller.data().host_responses.get(&(handle as u32)) {
                    body.len() as i32
                } else {
                    -1
                }
            },
        )
        .map_err(|e| anyhow::anyhow!("Failed to define host_response_size: {}", e))?;

    // ting_host_read_body(handle, ptr, len) -> bytes_read
    linker
        .func_wrap(
            "ting_env",
            "host_read_body",
            |mut caller: Caller<'_, PluginState>, handle: i32, ptr: i32, len: i32| -> i32 {
                let body = if let Some(b) = caller.data().host_responses.get(&(handle as u32)) {
                    b.clone()
                } else {
                    return -1;
                };
                let copy_len = std::cmp::min(body.len(), len as usize);
                let mem = match caller.get_export("memory") {
                    Some(Extern::Memory(mem)) => mem,
                    _ => return -2,
                };
                if mem
                    .write(&mut caller, ptr as usize, &body[..copy_len])
                    .is_err()
                {
                    return -3;
                }
                caller.data_mut().host_responses.remove(&(handle as u32));
                copy_len as i32
            },
        )
        .map_err(|e| anyhow::anyhow!("Failed to define host_read_body: {}", e))?;

    Ok(())
}

fn read_wasm_bytes(
    caller: &mut Caller<'_, PluginState>,
    ptr: i32,
    len: i32,
) -> std::result::Result<Vec<u8>, i32> {
    if ptr < 0 || len < 0 {
        return Err(ERR_INVALID_MEMORY);
    }

    let mem = match caller.get_export("memory") {
        Some(Extern::Memory(mem)) => mem,
        _ => return Err(ERR_INVALID_MEMORY),
    };
    let start = ptr as usize;
    let len = len as usize;
    let end = start.checked_add(len).ok_or(ERR_INVALID_MEMORY)?;
    let ctx = caller.as_context();
    let data = mem.data(&ctx);
    if end > data.len() {
        return Err(ERR_INVALID_MEMORY);
    }

    Ok(data[start..end].to_vec())
}

fn read_wasm_string(
    caller: &mut Caller<'_, PluginState>,
    ptr: i32,
    len: i32,
) -> std::result::Result<String, i32> {
    String::from_utf8(read_wasm_bytes(caller, ptr, len)?).map_err(|_| ERR_INVALID_UTF8)
}

fn read_wasm_json(
    caller: &mut Caller<'_, PluginState>,
    ptr: i32,
    len: i32,
) -> std::result::Result<serde_json::Value, i32> {
    let bytes = read_wasm_bytes(caller, ptr, len)?;
    if bytes.is_empty() {
        return Ok(serde_json::json!({}));
    }
    serde_json::from_slice(&bytes).map_err(|_| ERR_INVALID_JSON)
}

fn store_host_response(caller: &mut Caller<'_, PluginState>, body: Vec<u8>) -> i32 {
    let handle = caller
        .data()
        .host_responses
        .keys()
        .copied()
        .max()
        .unwrap_or(0)
        .saturating_add(1);
    caller.data_mut().host_responses.insert(handle, body);
    handle as i32
}

pub(crate) fn is_network_allowed(allowed_domains: &[String], url: &str) -> bool {
    if allowed_domains.is_empty() {
        return false;
    }

    let Ok(parsed) = url::Url::parse(url) else {
        return false;
    };
    let Some(domain) = parsed.host_str() else {
        return false;
    };

    allowed_domains
        .iter()
        .any(|pattern| domain_matches(domain, pattern.trim()))
}

fn domain_matches(domain: &str, pattern: &str) -> bool {
    if pattern.is_empty() {
        return false;
    }

    if pattern == "*" {
        true
    } else if let Some(base) = pattern.strip_prefix("*.") {
        domain == base || domain.ends_with(&format!(".{}", base))
    } else {
        domain == pattern
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wasm_network_permission_denies_when_no_domains_declared() {
        assert!(!is_network_allowed(&[], "https://example.com/data"));
    }

    #[test]
    fn wasm_network_permission_allows_exact_and_wildcard_domains() {
        let allowed = vec!["api.example.com".to_string(), "*.trusted.test".to_string()];

        assert!(is_network_allowed(
            &allowed,
            "https://api.example.com/v1/search"
        ));
        assert!(is_network_allowed(
            &allowed,
            "https://cdn.trusted.test/resource"
        ));
        assert!(is_network_allowed(&allowed, "https://trusted.test/root"));
        assert!(!is_network_allowed(&allowed, "https://evil.example.com"));
        assert!(is_network_allowed(
            &["*".to_string()],
            "https://evil.example.com"
        ));
    }
}
