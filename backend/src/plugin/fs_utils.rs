//! File system utilities shared across the plugin subsystem

use std::path::{Path, PathBuf};
use tracing::warn;

/// Recursively copy a directory and its contents
pub fn copy_dir_recursive(src: &Path, dst: &Path) -> std::io::Result<()> {
    if !dst.exists() {
        std::fs::create_dir_all(dst)?;
    }

    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());

        if file_type.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            std::fs::copy(&src_path, &dst_path)?;
        }
    }

    Ok(())
}

/// Recursively calculate the total size of a directory in bytes
pub fn calculate_dir_size(path: &Path) -> std::io::Result<u64> {
    let mut total_size = 0u64;

    if path.is_file() {
        return Ok(std::fs::metadata(path)?.len());
    }

    for entry in std::fs::read_dir(path)? {
        let entry = entry?;
        let entry_path = entry.path();

        if entry_path.is_dir() {
            total_size += calculate_dir_size(&entry_path)?;
        } else {
            total_size += std::fs::metadata(&entry_path)?.len();
        }
    }

    Ok(total_size)
}

/// Compute SHA-256 checksum of all files in a directory
pub fn checksum_directory(path: &Path) -> std::io::Result<String> {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    let mut files: Vec<PathBuf> = Vec::new();

    for entry in walkdir::WalkDir::new(path).sort_by_file_name() {
        let entry = entry?;
        if entry.file_type().is_file() {
            files.push(entry.path().to_path_buf());
        }
    }

    for file in &files {
        hasher.update(file.to_string_lossy().as_bytes());
        let content = std::fs::read(file)?;
        hasher.update(&content);
    }

    Ok(format!("{:x}", hasher.finalize()))
}

/// Remove a directory and all its contents, logging any errors
pub fn remove_dir_all_silent(path: &Path) {
    if let Err(e) = std::fs::remove_dir_all(path) {
        warn!("Failed to remove directory {:?}: {}", path, e);
    }
}
