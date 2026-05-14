use super::*;
use super::plugin::StoreLimits;
use wasmtime::ResourceLimiter;

#[tokio::test]
async fn test_wasm_runtime_creation() {
    let runtime = WasmRuntime::new();
    assert!(runtime.is_ok());
}

#[tokio::test]
async fn test_sandbox_creation() {
    let runtime = WasmRuntime::new().unwrap();
    let permissions = vec![
        Permission::FileRead(std::path::PathBuf::from("/tmp")),
    ];
    let limits = ResourceLimits::default();

    let sandbox = runtime.create_sandbox(permissions, limits);
    assert!(sandbox.is_ok());
}

#[test]
fn test_store_limits() {
    let mut limits = StoreLimits::new(1024);

    assert!(limits.memory_growing(0, 512, None).unwrap());
    assert_eq!(limits.current_memory(), 512);

    assert!(limits.memory_growing(512, 1024, None).unwrap());
    assert_eq!(limits.current_memory(), 1024);

    assert!(!limits.memory_growing(1024, 2048, None).unwrap());
    assert_eq!(limits.current_memory(), 1024);
}
