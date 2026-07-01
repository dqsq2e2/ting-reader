use crate::plugin::wasm::sandbox::Permission;
use crate::plugin::{PluginHostGateway, PluginHostUser};
use serde_json::Value;
use std::cell::RefCell;
use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::sync::Arc;

pub const TING_NATIVE_HOST_API_VERSION: u32 = 1;

#[repr(C)]
pub struct TingNativeHostApi {
    pub version: u32,
    pub host_invoke: Option<
        unsafe extern "C" fn(
            method: *const c_char,
            params_json: *const c_char,
            result_json: *mut *mut c_char,
        ) -> i32,
    >,
    pub host_free: Option<unsafe extern "C" fn(ptr: *mut c_char)>,
}

pub type SetHostApiFn = unsafe extern "C" fn(api: *const TingNativeHostApi) -> i32;

#[derive(Clone)]
pub(crate) struct NativeHostInvocationContext {
    pub plugin_id: String,
    pub permissions: Vec<Permission>,
    pub user: Option<PluginHostUser>,
    pub host_gateway: Option<Arc<PluginHostGateway>>,
    pub runtime_handle: tokio::runtime::Handle,
}

thread_local! {
    static CURRENT_CONTEXT: RefCell<Option<NativeHostInvocationContext>> = const { RefCell::new(None) };
}

pub(crate) fn with_invocation_context<F, R>(context: NativeHostInvocationContext, f: F) -> R
where
    F: FnOnce() -> R,
{
    let previous = CURRENT_CONTEXT.with(|slot| slot.replace(Some(context)));
    let _guard = NativeHostContextGuard { previous };
    f()
}

struct NativeHostContextGuard {
    previous: Option<NativeHostInvocationContext>,
}

impl Drop for NativeHostContextGuard {
    fn drop(&mut self) {
        let previous = self.previous.take();
        CURRENT_CONTEXT.with(|slot| {
            slot.replace(previous);
        });
    }
}

static HOST_API: TingNativeHostApi = TingNativeHostApi {
    version: TING_NATIVE_HOST_API_VERSION,
    host_invoke: Some(native_host_invoke),
    host_free: Some(native_host_free),
};

pub fn native_host_api() -> *const TingNativeHostApi {
    &HOST_API as *const TingNativeHostApi
}

unsafe extern "C" fn native_host_invoke(
    method: *const c_char,
    params_json: *const c_char,
    result_json: *mut *mut c_char,
) -> i32 {
    if method.is_null() || params_json.is_null() || result_json.is_null() {
        return -1;
    }

    let method = match c_string_to_string(method) {
        Ok(value) => value,
        Err(code) => return code,
    };
    let params = match c_string_to_json(params_json) {
        Ok(value) => value,
        Err(code) => {
            write_error_result(result_json, "Invalid HostGateway params JSON");
            return code;
        }
    };

    let Some(context) = CURRENT_CONTEXT.with(|slot| slot.borrow().clone()) else {
        write_error_result(result_json, "Native HostGateway context is not active");
        return -2;
    };
    let Some(gateway) = context.host_gateway else {
        write_error_result(
            result_json,
            "HostGateway is not configured for this native plugin",
        );
        return -3;
    };
    let Some(user) = context.user else {
        write_error_result(
            result_json,
            "HostGateway requires an authenticated user context",
        );
        return -4;
    };

    let invoke_result = context.runtime_handle.block_on(async move {
        gateway
            .invoke_with_permissions(
                &context.plugin_id,
                &context.permissions,
                &user,
                &method,
                params,
            )
            .await
    });

    match invoke_result {
        Ok(value) => write_value_result(result_json, &value),
        Err(error) => {
            write_error_result(result_json, &error.to_string());
            -6
        }
    }
}

unsafe extern "C" fn native_host_free(ptr: *mut c_char) {
    if !ptr.is_null() {
        unsafe {
            drop(CString::from_raw(ptr));
        }
    }
}

fn c_string_to_string(ptr: *const c_char) -> std::result::Result<String, i32> {
    let value = unsafe { CStr::from_ptr(ptr) };
    value.to_str().map(ToOwned::to_owned).map_err(|_| -5)
}

fn c_string_to_json(ptr: *const c_char) -> std::result::Result<Value, i32> {
    let value = c_string_to_string(ptr)?;
    if value.trim().is_empty() {
        return Ok(serde_json::json!({}));
    }
    serde_json::from_str(&value).map_err(|_| -5)
}

fn write_value_result(result_json: *mut *mut c_char, value: &Value) -> i32 {
    let json = match serde_json::to_string(value) {
        Ok(json) => json,
        Err(_) => return -7,
    };
    write_result_string(result_json, json)
}

fn write_error_result(result_json: *mut *mut c_char, message: &str) {
    let _ = write_value_result(result_json, &serde_json::json!({ "error": message }));
}

fn write_result_string(result_json: *mut *mut c_char, value: String) -> i32 {
    let c_string = match CString::new(value) {
        Ok(value) => value,
        Err(_) => return -7,
    };
    unsafe {
        *result_json = c_string.into_raw();
    }
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_host_invoke_rejects_without_active_context() {
        let method = CString::new("books.list").unwrap();
        let params = CString::new("{}").unwrap();
        let mut result: *mut c_char = std::ptr::null_mut();

        let code = unsafe {
            native_host_invoke(
                method.as_ptr(),
                params.as_ptr(),
                &mut result as *mut *mut c_char,
            )
        };

        assert_eq!(code, -2);
        assert!(!result.is_null());
        let body = unsafe { CStr::from_ptr(result).to_string_lossy().into_owned() };
        assert!(body.contains("context is not active"));
        unsafe {
            native_host_free(result);
        }
    }

    #[test]
    fn native_host_api_exposes_versioned_callbacks() {
        let api = unsafe { native_host_api().as_ref() }.unwrap();

        assert_eq!(api.version, TING_NATIVE_HOST_API_VERSION);
        assert!(api.host_invoke.is_some());
        assert!(api.host_free.is_some());
    }
}
