//! Cache management module
//!
//! Provides functionality for managing cached chapter files.

use std::path::{Path, PathBuf};
use std::fs;
use std::io;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CacheError {
    #[error("IO error: {0}")]
    Io(#[from] io::Error),
    
    #[error("Cache file not found: {0}")]
    NotFound(String),
    
    #[error("Invalid cache path: {0}")]
    InvalidPath(String),
}

pub type Result<T> = std::result::Result<T, CacheError>;

/// Information about a cached chapter
#[derive(Debug, Clone)]
pub struct CacheInfo {
    pub chapter_id: String,
    pub file_size: u64,
    pub file_path: PathBuf,
    pub created_at: Option<std::time::SystemTime>,
}

/// Cache manager for handling chapter cache files
pub struct CacheManager {
    cache_dir: PathBuf,
}

impl CacheManager {
    /// Create a new cache manager with the specified cache directory
    pub fn new(cache_dir: PathBuf) -> Result<Self> {
        // Ensure cache directory exists
        if !cache_dir.exists() {
            fs::create_dir_all(&cache_dir)?;
        }
        
        Ok(Self { cache_dir })
    }
    
    /// Get the cache file path for a chapter
    pub fn get_cache_path(&self, chapter_id: &str) -> PathBuf {
        self.cache_dir.join(format!("{}.cache", chapter_id))
    }
    
    /// Check if a chapter is cached
    pub fn is_cached(&self, chapter_id: &str) -> bool {
        self.get_cache_path(chapter_id).exists()
    }
    
    /// Cache a chapter file (copy from source to cache)
    pub async fn cache_chapter(&self, chapter_id: &str, source_path: &Path) -> Result<CacheInfo> {
        let cache_path = self.get_cache_path(chapter_id);
        
        // Copy file to cache
        tokio::fs::copy(source_path, &cache_path).await?;
        
        // Get file metadata
        let metadata = tokio::fs::metadata(&cache_path).await?;
        let file_size = metadata.len();
        let created_at = metadata.created().ok();
        
        Ok(CacheInfo {
            chapter_id: chapter_id.to_string(),
            file_size,
            file_path: cache_path,
            created_at,
        })
    }
    
    /// Get information about a cached chapter
    pub async fn get_cache_info(&self, chapter_id: &str) -> Result<CacheInfo> {
        let cache_path = self.get_cache_path(chapter_id);
        
        if !cache_path.exists() {
            return Err(CacheError::NotFound(chapter_id.to_string()));
        }
        
        let metadata = tokio::fs::metadata(&cache_path).await?;
        let file_size = metadata.len();
        let created_at = metadata.created().ok();
        
        Ok(CacheInfo {
            chapter_id: chapter_id.to_string(),
            file_size,
            file_path: cache_path,
            created_at,
        })
    }
    
    /// List all cached chapters
    pub async fn list_cached(&self) -> Result<Vec<CacheInfo>> {
        let mut cached_chapters = Vec::new();
        
        let mut entries = tokio::fs::read_dir(&self.cache_dir).await?;
        
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            
            // Only process .cache files
            if path.extension().and_then(|s| s.to_str()) == Some("cache") {
                if let Some(file_stem) = path.file_stem().and_then(|s| s.to_str()) {
                    let chapter_id = file_stem.to_string();
                    
                    let metadata = entry.metadata().await?;
                    let file_size = metadata.len();
                    let created_at = metadata.created().ok();
                    
                    cached_chapters.push(CacheInfo {
                        chapter_id,
                        file_size,
                        file_path: path,
                        created_at,
                    });
                }
            }
        }
        
        Ok(cached_chapters)
    }
    
    /// Delete a cached chapter
    pub async fn delete_cache(&self, chapter_id: &str) -> Result<()> {
        let cache_path = self.get_cache_path(chapter_id);
        
        if !cache_path.exists() {
            return Err(CacheError::NotFound(chapter_id.to_string()));
        }
        
        tokio::fs::remove_file(cache_path).await?;
        Ok(())
    }
    
    /// Clear all cached chapters and temporary files
    pub async fn clear_all(&self) -> Result<usize> {
        let mut count = 0;
        let mut entries = tokio::fs::read_dir(&self.cache_dir).await?;
        
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            
            // Delete .cache files and .enc files (temporary downloads)
            let ext = path.extension().and_then(|s| s.to_str());
            if ext == Some("cache") || ext == Some("enc") {
                if let Err(e) = tokio::fs::remove_file(&path).await {
                    tracing::warn!("Failed to delete file {}: {}", path.display(), e);
                } else {
                    count += 1;
                }
            }
        }
        
        Ok(count)
    }
    
    /// Clean up orphaned cache files (chapters that no longer exist in database)
    pub async fn cleanup_orphaned(&self, valid_chapter_ids: &[String]) -> Result<usize> {
        let mut count = 0;
        
        // We need to iterate directory manually to find .enc files too, 
        // as list_cached only returns .cache files
        let mut entries = tokio::fs::read_dir(&self.cache_dir).await?;
        
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if let Some(file_stem) = path.file_stem().and_then(|s| s.to_str()) {
                let chapter_id = file_stem.to_string();
                let ext = path.extension().and_then(|s| s.to_str());
                
                // If chapter ID is not valid, delete both .cache and .enc
                if !valid_chapter_ids.contains(&chapter_id) && (ext == Some("cache") || ext == Some("enc")) {
                    if let Err(e) = tokio::fs::remove_file(&path).await {
                        tracing::warn!("Failed to delete orphaned file {}: {}", path.display(), e);
                    } else {
                        count += 1;
                    }
                }
            }
        }
        
        Ok(count)
    }

    /// Enforce cache limits (max files and max size)
    /// Removes oldest files first until limits are satisfied
    pub async fn enforce_limits(&self, max_files: usize, max_size_bytes: u64) -> Result<usize> {
        let mut cached_files = self.list_cached().await?;
        
        // Sort by creation time (oldest first)
        cached_files.sort_by(|a, b| {
            a.created_at.cmp(&b.created_at)
        });
        
        let mut current_count = cached_files.len();
        let mut current_size: u64 = cached_files.iter().map(|f| f.file_size).sum();
        let mut removed_count = 0;
        
        // Check if we need cleanup
        if current_count <= max_files && current_size <= max_size_bytes {
            return Ok(0);
        }
        
        tracing::info!(
            "Cache limits exceeded (Count: {}/{}, Size: {}/{}). Starting cleanup...",
            current_count, max_files, current_size, max_size_bytes
        );
        
        for file in cached_files {
            // Stop if both conditions are met
            if current_count <= max_files && current_size <= max_size_bytes {
                break;
            }
            
            // Remove file
            if let Err(e) = tokio::fs::remove_file(&file.file_path).await {
                tracing::warn!("Failed to delete cache file {}: {}", file.file_path.display(), e);
                continue;
            }
            
            current_count -= 1;
            current_size = current_size.saturating_sub(file.file_size);
            removed_count += 1;
            
            tracing::debug!("Removed old cache file: {:?} (Size: {})", file.file_path, file.file_size);
        }
        
        tracing::info!("Cache cleanup complete. Removed {} files.", removed_count);
        Ok(removed_count)
    }
}
