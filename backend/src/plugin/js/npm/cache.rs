//! npm dependency cache management

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use tracing::{debug, info};

use crate::plugin::fs_utils;

/// Cache entry for a dependency
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheEntry {
    pub package_name: String,
    pub version: String,
    pub cache_path: PathBuf,
    pub used_by: HashSet<String>,
    pub last_accessed: String,
    pub size_bytes: u64,
}

/// Cache statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheStatistics {
    pub total_packages: usize,
    pub total_size_bytes: u64,
    pub cache_hits: usize,
    pub cache_misses: usize,
    pub hit_rate: f64,
    pub plugins_count: usize,
    pub last_cleanup: Option<String>,
}

impl Default for CacheStatistics {
    fn default() -> Self {
        Self {
            total_packages: 0,
            total_size_bytes: 0,
            cache_hits: 0,
            cache_misses: 0,
            hit_rate: 0.0,
            plugins_count: 0,
            last_cleanup: None,
        }
    }
}

/// Cache registry (package_name@version -> CacheEntry)
pub type CacheRegistry = Arc<RwLock<HashMap<String, CacheEntry>>>;
pub type CacheStatsLock = Arc<RwLock<CacheStatistics>>;

pub fn get_cache_key(package_name: &str, version: &str) -> String {
    format!("{}@{}", package_name, version)
}

pub fn is_cached(cache_dir: &Option<PathBuf>, cache_registry: &CacheRegistry, package_name: &str, version: &str) -> bool {
    if cache_dir.is_none() {
        return false;
    }
    let cache_key = get_cache_key(package_name, version);
    let registry = cache_registry.read().unwrap();
    registry.contains_key(&cache_key)
}

pub fn update_hit_rate(stats: &mut CacheStatistics) {
    let total = stats.cache_hits + stats.cache_misses;
    stats.hit_rate = if total > 0 { stats.cache_hits as f64 / total as f64 } else { 0.0 };
}

pub fn add_to_cache(
    cache_dir: &Option<PathBuf>,
    cache_registry: &CacheRegistry,
    cache_stats: &CacheStatsLock,
    package_name: &str,
    version: &str,
    plugin_name: &str,
    source_path: &Path,
) -> Result<()> {
    let cache_dir = match cache_dir {
        Some(dir) => dir,
        None => {
            debug!("Cache directory not configured, skipping cache");
            return Ok(());
        }
    };

    if !cache_dir.exists() {
        std::fs::create_dir_all(cache_dir).context("Failed to create cache directory")?;
    }

    let cache_key = get_cache_key(package_name, version);
    let cache_path = cache_dir.join(&cache_key);

    if !cache_path.exists() {
        info!("Caching dependency: {}", cache_key);
        fs_utils::copy_dir_recursive(source_path, &cache_path)
            .context("Failed to copy dependency to cache")?;
        let size_bytes = fs_utils::calculate_dir_size(&cache_path)
            .context("Failed to calculate cache size")?;

        let mut used_by = HashSet::new();
        used_by.insert(plugin_name.to_string());

        let entry = CacheEntry {
            package_name: package_name.to_string(),
            version: version.to_string(),
            cache_path: cache_path.clone(),
            used_by,
            last_accessed: chrono::Utc::now().to_rfc3339(),
            size_bytes,
        };

        let mut registry = cache_registry.write().unwrap();
        registry.insert(cache_key.clone(), entry);

        let mut stats = cache_stats.write().unwrap();
        stats.total_packages += 1;
        stats.total_size_bytes += size_bytes;
        stats.cache_misses += 1;
        update_hit_rate(&mut stats);

        info!("Dependency cached successfully: {}", cache_key);
    } else {
        let mut registry = cache_registry.write().unwrap();
        if let Some(entry) = registry.get_mut(&cache_key) {
            entry.used_by.insert(plugin_name.to_string());
            entry.last_accessed = chrono::Utc::now().to_rfc3339();
            let mut stats = cache_stats.write().unwrap();
            stats.cache_hits += 1;
            update_hit_rate(&mut stats);
            info!("Using cached dependency: {}", cache_key);
        }
    }

    Ok(())
}

pub fn link_from_cache(
    cache_registry: &CacheRegistry,
    cache_stats: &CacheStatsLock,
    package_name: &str,
    version: &str,
    plugin_name: &str,
    target_path: &Path,
) -> Result<()> {
    let cache_key = get_cache_key(package_name, version);
    let registry = cache_registry.read().unwrap();
    let entry = registry.get(&cache_key).ok_or_else(|| {
        anyhow::anyhow!("Dependency not found in cache: {}", cache_key)
    })?;

    info!("Linking cached dependency {} to {}", cache_key, target_path.display());

    if let Some(parent) = target_path.parent() {
        if !parent.exists() {
            std::fs::create_dir_all(parent)?;
        }
    }

    fs_utils::copy_dir_recursive(&entry.cache_path, target_path)
        .context("Failed to copy from cache")?;

    drop(registry);
    let mut registry = cache_registry.write().unwrap();
    if let Some(entry) = registry.get_mut(&cache_key) {
        entry.used_by.insert(plugin_name.to_string());
        entry.last_accessed = chrono::Utc::now().to_rfc3339();
    }

    let mut stats = cache_stats.write().unwrap();
    stats.cache_hits += 1;
    update_hit_rate(&mut stats);

    Ok(())
}

pub fn get_cache_statistics(
    cache_registry: &CacheRegistry,
    cache_stats: &CacheStatsLock,
) -> CacheStatistics {
    let stats = cache_stats.read().unwrap();
    let registry = cache_registry.read().unwrap();
    let mut all_plugins = HashSet::new();
    for entry in registry.values() {
        all_plugins.extend(entry.used_by.iter().cloned());
    }
    CacheStatistics {
        total_packages: stats.total_packages,
        total_size_bytes: stats.total_size_bytes,
        cache_hits: stats.cache_hits,
        cache_misses: stats.cache_misses,
        hit_rate: stats.hit_rate,
        plugins_count: all_plugins.len(),
        last_cleanup: stats.last_cleanup.clone(),
    }
}

pub fn clear_cache(
    cache_dir: &Option<PathBuf>,
    cache_registry: &CacheRegistry,
    cache_stats: &CacheStatsLock,
) -> Result<()> {
    let cache_dir = match cache_dir {
        Some(dir) => dir,
        None => return Ok(()),
    };
    info!("Clearing all cache");

    if cache_dir.exists() {
        std::fs::remove_dir_all(cache_dir).context("Failed to remove cache directory")?;
        std::fs::create_dir_all(cache_dir).context("Failed to recreate cache directory")?;
    }

    cache_registry.write().unwrap().clear();
    let mut stats = cache_stats.write().unwrap();
    *stats = CacheStatistics { last_cleanup: Some(chrono::Utc::now().to_rfc3339()), ..Default::default() };

    info!("Cache cleared successfully");
    Ok(())
}

pub fn cleanup_cache_for_plugin(
    cache_dir: &Option<PathBuf>,
    cache_registry: &CacheRegistry,
    cache_stats: &CacheStatsLock,
    plugin_name: &str,
) -> Result<usize> {
    if cache_dir.is_none() {
        return Ok(0);
    }
    info!("Cleaning up cache for plugin: {}", plugin_name);
    let mut removed_count = 0;
    let mut packages_to_remove = Vec::new();

    {
        let mut registry = cache_registry.write().unwrap();
        for (cache_key, entry) in registry.iter_mut() {
            entry.used_by.remove(plugin_name);
            if entry.used_by.is_empty() {
                packages_to_remove.push((cache_key.clone(), entry.cache_path.clone(), entry.size_bytes));
            }
        }
    }

    for (cache_key, cache_path, size_bytes) in packages_to_remove {
        info!("Removing unused cached package: {}", cache_key);
        if cache_path.exists() {
            std::fs::remove_dir_all(&cache_path)
                .with_context(|| format!("Failed to remove cached package at {}", cache_path.display()))?;
        }
        cache_registry.write().unwrap().remove(&cache_key);
        let mut stats = cache_stats.write().unwrap();
        stats.total_packages = stats.total_packages.saturating_sub(1);
        stats.total_size_bytes = stats.total_size_bytes.saturating_sub(size_bytes);
        removed_count += 1;
    }

    if removed_count > 0 {
        cache_stats.write().unwrap().last_cleanup = Some(chrono::Utc::now().to_rfc3339());
        info!("Removed {} unused packages from cache", removed_count);
    }

    Ok(removed_count)
}

pub fn cleanup_all_unused(
    cache_dir: &Option<PathBuf>,
    cache_registry: &CacheRegistry,
    cache_stats: &CacheStatsLock,
) -> Result<usize> {
    if cache_dir.is_none() {
        return Ok(0);
    }
    info!("Cleaning up all unused cached packages");
    let mut removed_count = 0;
    let mut packages_to_remove = Vec::new();

    {
        let registry = cache_registry.read().unwrap();
        for (cache_key, entry) in registry.iter() {
            if entry.used_by.is_empty() {
                packages_to_remove.push((cache_key.clone(), entry.cache_path.clone(), entry.size_bytes));
            }
        }
    }

    for (cache_key, cache_path, size_bytes) in packages_to_remove {
        info!("Removing unused cached package: {}", cache_key);
        if cache_path.exists() {
            std::fs::remove_dir_all(&cache_path)
                .with_context(|| format!("Failed to remove cached package at {}", cache_path.display()))?;
        }
        cache_registry.write().unwrap().remove(&cache_key);
        let mut stats = cache_stats.write().unwrap();
        stats.total_packages = stats.total_packages.saturating_sub(1);
        stats.total_size_bytes = stats.total_size_bytes.saturating_sub(size_bytes);
        removed_count += 1;
    }

    if removed_count > 0 {
        cache_stats.write().unwrap().last_cleanup = Some(chrono::Utc::now().to_rfc3339());
        info!("Removed {} unused packages from cache", removed_count);
    }

    Ok(removed_count)
}
