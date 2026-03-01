#[cfg(target_os = "linux")]
use std::os::raw::c_int;

/// Release memory to the OS (Linux only)
/// 
/// This function calls `malloc_trim(0)` to release free memory back to the OS.
/// This is useful for long-running processes that allocate and free large amounts of memory,
/// as the glibc allocator may hold onto memory for reuse.
pub fn release_memory() {
    #[cfg(target_os = "linux")]
    {
        // Use libc::malloc_trim if available, or declare it manually
        // We use manual declaration to ensure compatibility even if libc crate version differs slightly
        extern "C" {
            fn malloc_trim(pad: usize) -> c_int;
        }
        
        unsafe {
            let _ = malloc_trim(0);
        }
    }
}
