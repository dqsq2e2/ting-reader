//! Decryption Cache Service
//!
//! 通用的解密缓存服务，支持所有加密格式插件
//! 
//! 职责：
//! 1. 缓存管理：LRU 缓存策略，自动清理过期文件
//! 2. 临时文件管理：自动清理解密后的临时文件
//! 3. 插件协调：调用格式插件进行解密
//! 
//! 设计原则：
//! - 高内聚：缓存逻辑集中在核心系统
//! - 低耦合：插件只需实现解密接口，不关心缓存
//! - 可扩展：支持任意加密格式（XM, QMC, NCM 等）

use crate::core::error::{Result, TingError};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use std::time::{Duration, SystemTime};
use tracing::{debug, info, warn};

/// 解密缓存条目
#[derive(Debug, Clone)]
struct CacheEntry {
    /// 原始加密文件路径
    _encrypted_path: PathBuf,
    /// 解密后的文件路径
    decrypted_path: PathBuf,
    /// 文件大小
    file_size: u64,
    /// 最后访问时间
    last_accessed: SystemTime,
    /// 创建时间
    created_at: SystemTime,
}

/// 解密缓存服务配置
#[derive(Debug, Clone)]
pub struct DecryptionCacheConfig {
    /// 缓存目录
    pub cache_dir: PathBuf,
    /// 最大缓存大小（字节）
    pub max_cache_size: u64,
    /// 缓存过期时间
    pub cache_expiry: Duration,
    /// 是否启用缓存
    pub enable_cache: bool,
}

impl Default for DecryptionCacheConfig {
    fn default() -> Self {
        Self {
            cache_dir: PathBuf::from("./data/cache/decrypted"),
            max_cache_size: 1024 * 1024 * 1024, // 1 GB
            cache_expiry: Duration::from_secs(24 * 3600), // 24 hours
            enable_cache: true,
        }
    }
}

/// 解密缓存服务
/// 
/// 提供通用的解密文件缓存功能，支持所有加密格式
pub struct DecryptionCacheService {
    config: DecryptionCacheConfig,
    cache: Arc<RwLock<CacheState>>,
}

struct CacheState {
    entries: HashMap<String, CacheEntry>,
    current_size: u64,
}

impl DecryptionCacheService {
    /// 创建新的解密缓存服务
    pub fn new(config: DecryptionCacheConfig) -> Result<Self> {
        // 创建缓存目录
        if !config.cache_dir.exists() {
            fs::create_dir_all(&config.cache_dir)?;
        }

        Ok(Self {
            config,
            cache: Arc::new(RwLock::new(CacheState {
                entries: HashMap::new(),
                current_size: 0,
            })),
        })
    }

    /// 获取或解密文件
    /// 
    /// 如果文件已缓存，直接返回缓存路径
    /// 否则调用解密函数并缓存结果
    /// 
    /// # Arguments
    /// * `encrypted_path` - 加密文件路径
    /// * `format_hint` - 格式提示（如 "xm", "qmc", "ncm"）
    /// * `decrypt_fn` - 解密函数（由插件提供）
    /// 
    /// # Returns
    /// 解密后的文件路径
    pub async fn get_or_decrypt<F, Fut>(
        &self,
        encrypted_path: &Path,
        format_hint: &str,
        decrypt_fn: F,
    ) -> Result<PathBuf>
    where
        F: FnOnce(PathBuf, PathBuf) -> Fut,
        Fut: std::future::Future<Output = Result<()>>,
    {
        // 检查缓存
        if self.config.enable_cache {
            if let Some(cached_path) = self.get_from_cache(encrypted_path)? {
                info!("Cache hit for encrypted file: {:?}", encrypted_path);
                return Ok(cached_path);
            }
        }

        info!("Cache miss, decrypting file: {:?}", encrypted_path);

        // 生成临时文件路径
        let temp_path = self.generate_temp_path(encrypted_path, format_hint)?;

        // 调用解密函数
        decrypt_fn(encrypted_path.to_path_buf(), temp_path.clone()).await?;

        // 添加到缓存
        if self.config.enable_cache {
            self.add_to_cache(encrypted_path, &temp_path)?;
        }

        Ok(temp_path)
    }

    /// 从缓存获取解密文件
    fn get_from_cache(&self, encrypted_path: &Path) -> Result<Option<PathBuf>> {
        let key = self.cache_key(encrypted_path);
        let mut cache = self.cache.write().map_err(|e| {
            TingError::InitializationError(format!("Failed to acquire cache lock: {}", e))
        })?;

        if let Some(entry) = cache.entries.get_mut(&key) {
            // 检查文件是否仍然存在
            if !entry.decrypted_path.exists() {
                cache.entries.remove(&key);
                return Ok(None);
            }

            // 检查是否过期
            if let Ok(elapsed) = entry.created_at.elapsed() {
                if elapsed > self.config.cache_expiry {
                    // 过期，删除
                    let _ = fs::remove_file(&entry.decrypted_path);
                    cache.entries.remove(&key);
                    return Ok(None);
                }
            }

            // 更新访问时间
            entry.last_accessed = SystemTime::now();
            debug!("Cache hit for: {:?}", encrypted_path);
            return Ok(Some(entry.decrypted_path.clone()));
        }

        debug!("Cache miss for: {:?}", encrypted_path);
        Ok(None)
    }

    /// 添加到缓存
    fn add_to_cache(&self, encrypted_path: &Path, decrypted_path: &Path) -> Result<()> {
        let key = self.cache_key(encrypted_path);
        let file_size = fs::metadata(decrypted_path)?.len();

        let mut cache = self.cache.write().map_err(|e| {
            TingError::InitializationError(format!("Failed to acquire cache lock: {}", e))
        })?;

        // 检查是否需要清理空间
        while cache.current_size + file_size > self.config.max_cache_size
            && !cache.entries.is_empty()
        {
            self.evict_oldest(&mut cache)?;
        }

        let entry = CacheEntry {
            _encrypted_path: encrypted_path.to_path_buf(),
            decrypted_path: decrypted_path.to_path_buf(),
            file_size,
            last_accessed: SystemTime::now(),
            created_at: SystemTime::now(),
        };

        cache.entries.insert(key, entry);
        cache.current_size += file_size;

        info!(
            "Added to cache: {:?} -> {:?} (size: {} MB)",
            encrypted_path,
            decrypted_path,
            file_size / 1024 / 1024
        );

        Ok(())
    }

    /// 驱逐最旧的缓存条目
    fn evict_oldest(&self, cache: &mut CacheState) -> Result<()> {
        let oldest_key = cache
            .entries
            .iter()
            .min_by_key(|(_, entry)| entry.last_accessed)
            .map(|(key, _)| key.clone());

        if let Some(key) = oldest_key {
            if let Some(entry) = cache.entries.remove(&key) {
                cache.current_size = cache.current_size.saturating_sub(entry.file_size);

                if entry.decrypted_path.exists() {
                    fs::remove_file(&entry.decrypted_path)?;
                    info!("Evicted and deleted: {:?}", entry.decrypted_path);
                }
            }
        }

        Ok(())
    }

    /// 清理过期的缓存条目
    pub fn cleanup_expired(&self) -> Result<()> {
        let mut cache = self.cache.write().map_err(|e| {
            TingError::InitializationError(format!("Failed to acquire cache lock: {}", e))
        })?;

        let mut to_remove = Vec::new();

        for (key, entry) in &cache.entries {
            if let Ok(elapsed) = entry.created_at.elapsed() {
                if elapsed > self.config.cache_expiry {
                    to_remove.push(key.clone());
                }
            }
        }

        for key in to_remove {
            if let Some(entry) = cache.entries.remove(&key) {
                cache.current_size = cache.current_size.saturating_sub(entry.file_size);

                if entry.decrypted_path.exists() {
                    fs::remove_file(&entry.decrypted_path)?;
                    info!("Cleaned up expired file: {:?}", entry.decrypted_path);
                }
            }
        }

        Ok(())
    }

    /// 清理所有缓存
    pub fn cleanup_all(&self) -> Result<()> {
        let mut cache = self.cache.write().map_err(|e| {
            TingError::InitializationError(format!("Failed to acquire cache lock: {}", e))
        })?;

        for (_, entry) in cache.entries.drain() {
            if entry.decrypted_path.exists() {
                fs::remove_file(&entry.decrypted_path)?;
                info!("Cleaned up: {:?}", entry.decrypted_path);
            }
        }

        cache.current_size = 0;
        Ok(())
    }

    /// 获取缓存统计信息
    pub fn stats(&self) -> Result<CacheStats> {
        let cache = self.cache.read().map_err(|e| {
            TingError::InitializationError(format!("Failed to acquire cache lock: {}", e))
        })?;

        Ok(CacheStats {
            total_entries: cache.entries.len(),
            total_size: cache.current_size,
            max_size: self.config.max_cache_size,
            cache_dir: self.config.cache_dir.clone(),
        })
    }

    /// 启动后台清理任务
    pub fn start_cleanup_task(self: Arc<Self>) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(3600)); // 每小时清理一次

            loop {
                interval.tick().await;

                if let Err(e) = self.cleanup_expired() {
                    warn!("Failed to cleanup expired cache: {}", e);
                } else {
                    info!("Completed periodic cache cleanup");
                }
            }
        })
    }

    // Private helper methods

    /// 生成缓存键
    fn cache_key(&self, path: &Path) -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        path.to_string_lossy().hash(&mut hasher);
        format!("{:x}", hasher.finish())
    }

    /// 生成临时文件路径
    fn generate_temp_path(&self, encrypted_path: &Path, format_hint: &str) -> Result<PathBuf> {
        let key = self.cache_key(encrypted_path);
        
        // 根据格式提示确定输出扩展名
        let extension = match format_hint.to_lowercase().as_str() {
            "xm" => "mp3",
            "qmc" | "qmc0" | "qmc3" => "mp3",
            "ncm" => "mp3",
            _ => "audio",
        };
        
        let filename = format!("decrypted_{}_{}.{}", format_hint, key, extension);
        Ok(self.config.cache_dir.join(filename))
    }
}

/// 缓存统计信息
#[derive(Debug, Clone)]
pub struct CacheStats {
    pub total_entries: usize,
    pub total_size: u64,
    pub max_size: u64,
    pub cache_dir: PathBuf,
}

impl Drop for DecryptionCacheService {
    fn drop(&mut self) {
        // 自动清理所有临时文件
        if let Err(e) = self.cleanup_all() {
            warn!("Failed to cleanup cache on drop: {}", e);
        } else {
            info!("Cleaned up all cached files");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_cache_key_generation() {
        let service = DecryptionCacheService::new(DecryptionCacheConfig::default()).unwrap();
        
        let path1 = Path::new("/path/to/file1.xm");
        let path2 = Path::new("/path/to/file2.xm");

        let key1 = service.cache_key(path1);
        let key2 = service.cache_key(path2);

        assert_ne!(key1, key2);
        assert_eq!(key1, service.cache_key(path1)); // 一致性
    }

    #[test]
    fn test_service_creation() {
        let temp_dir = TempDir::new().unwrap();
        let mut config = DecryptionCacheConfig::default();
        config.cache_dir = temp_dir.path().to_path_buf();

        let service = DecryptionCacheService::new(config);
        assert!(service.is_ok());
        assert!(temp_dir.path().exists());
    }

    #[test]
    fn test_temp_path_generation() {
        let temp_dir = TempDir::new().unwrap();
        let mut config = DecryptionCacheConfig::default();
        config.cache_dir = temp_dir.path().to_path_buf();

        let service = DecryptionCacheService::new(config).unwrap();
        
        let path = Path::new("/test/file.xm");
        let temp_path = service.generate_temp_path(path, "xm").unwrap();
        
        assert!(temp_path.to_string_lossy().contains("decrypted_xm_"));
        assert!(temp_path.to_string_lossy().ends_with(".mp3"));
    }

    #[tokio::test]
    async fn test_cache_stats() {
        let temp_dir = TempDir::new().unwrap();
        let mut config = DecryptionCacheConfig::default();
        config.cache_dir = temp_dir.path().to_path_buf();

        let service = DecryptionCacheService::new(config).unwrap();
        let stats = service.stats().unwrap();

        assert_eq!(stats.total_entries, 0);
        assert_eq!(stats.total_size, 0);
    }
}
