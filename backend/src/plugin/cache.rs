use crate::core::error::{Result, TingError};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::path::PathBuf;

const MAX_PLUGIN_CACHE_KEY_BYTES: usize = 512;
const MAX_PLUGIN_CACHE_VALUE_BYTES: usize = 1024 * 1024;

#[derive(Debug, Clone)]
pub struct PluginCache {
    root_dir: PathBuf,
}

#[derive(Debug, Serialize, Deserialize)]
struct PluginCacheRecord {
    key: String,
    value: Value,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct PluginCacheItem {
    pub key: String,
    pub value: Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl PluginCache {
    pub fn new(root_dir: PathBuf) -> Result<Self> {
        std::fs::create_dir_all(&root_dir).map_err(TingError::IoError)?;
        Ok(Self { root_dir })
    }

    pub async fn get(&self, plugin_id: &str, key: &str) -> Result<Option<PluginCacheItem>> {
        let path = self.cache_path(plugin_id, key)?;
        if !path.exists() {
            return Ok(None);
        }

        let bytes = tokio::fs::read(&path).await.map_err(TingError::IoError)?;
        let record: PluginCacheRecord = serde_json::from_slice(&bytes)
            .map_err(|e| TingError::DeserializationError(e.to_string()))?;
        Ok(Some(PluginCacheItem {
            key: record.key,
            value: record.value,
            created_at: record.created_at,
            updated_at: record.updated_at,
        }))
    }

    pub async fn set(&self, plugin_id: &str, key: &str, value: Value) -> Result<PluginCacheItem> {
        let path = self.cache_path(plugin_id, key)?;
        let now = Utc::now();
        let created_at = match self.get(plugin_id, key).await? {
            Some(existing) => existing.created_at,
            None => now,
        };

        let record = PluginCacheRecord {
            key: key.to_string(),
            value,
            created_at,
            updated_at: now,
        };
        let bytes = serde_json::to_vec(&record)
            .map_err(|e| TingError::SerializationError(e.to_string()))?;
        if bytes.len() > MAX_PLUGIN_CACHE_VALUE_BYTES {
            return Err(TingError::ResourceLimitExceeded(format!(
                "Plugin cache value exceeds {} bytes",
                MAX_PLUGIN_CACHE_VALUE_BYTES
            )));
        }

        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(TingError::IoError)?;
        }
        tokio::fs::write(&path, bytes)
            .await
            .map_err(TingError::IoError)?;

        Ok(PluginCacheItem {
            key: record.key,
            value: record.value,
            created_at: record.created_at,
            updated_at: record.updated_at,
        })
    }

    pub async fn delete(&self, plugin_id: &str, key: &str) -> Result<bool> {
        let path = self.cache_path(plugin_id, key)?;
        if !path.exists() {
            return Ok(false);
        }

        tokio::fs::remove_file(path)
            .await
            .map_err(TingError::IoError)?;
        Ok(true)
    }

    pub async fn has(&self, plugin_id: &str, key: &str) -> Result<bool> {
        Ok(self.cache_path(plugin_id, key)?.exists())
    }

    fn cache_path(&self, plugin_id: &str, key: &str) -> Result<PathBuf> {
        validate_plugin_cache_key(key)?;
        Ok(self
            .root_dir
            .join(hash_segment(plugin_id))
            .join(format!("{}.json", hash_segment(key))))
    }
}

fn validate_plugin_cache_key(key: &str) -> Result<()> {
    let bytes = key.as_bytes();
    if bytes.is_empty() {
        return Err(TingError::InvalidRequest(
            "Plugin cache key cannot be empty".to_string(),
        ));
    }
    if bytes.len() > MAX_PLUGIN_CACHE_KEY_BYTES {
        return Err(TingError::InvalidRequest(format!(
            "Plugin cache key exceeds {} bytes",
            MAX_PLUGIN_CACHE_KEY_BYTES
        )));
    }
    if key.chars().any(char::is_control) {
        return Err(TingError::InvalidRequest(
            "Plugin cache key cannot contain control characters".to_string(),
        ));
    }
    Ok(())
}

fn hash_segment(value: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(value.as_bytes());
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn plugin_cache_is_scoped_by_plugin_id() {
        let temp_dir = tempfile::tempdir().unwrap();
        let cache = PluginCache::new(temp_dir.path().to_path_buf()).unwrap();

        cache
            .set("plugin-a", "shared-key", serde_json::json!({"value": 1}))
            .await
            .unwrap();
        cache
            .set("plugin-b", "shared-key", serde_json::json!({"value": 2}))
            .await
            .unwrap();

        assert_eq!(
            cache
                .get("plugin-a", "shared-key")
                .await
                .unwrap()
                .unwrap()
                .value,
            serde_json::json!({"value": 1})
        );
        assert_eq!(
            cache
                .get("plugin-b", "shared-key")
                .await
                .unwrap()
                .unwrap()
                .value,
            serde_json::json!({"value": 2})
        );
    }

    #[test]
    fn plugin_cache_key_rejects_control_characters() {
        assert!(validate_plugin_cache_key("reader:last-open").is_ok());
        assert!(validate_plugin_cache_key("bad\nkey").is_err());
        assert!(validate_plugin_cache_key("").is_err());
    }
}
