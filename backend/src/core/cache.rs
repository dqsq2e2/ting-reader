//! Generic LRU cache with size-based eviction
//!
//! Used by AudioStreamer and DecryptionCacheService.

use std::collections::HashMap;
use std::hash::Hash;
use std::time::SystemTime;

/// An LRU cache entry with metadata
#[derive(Debug, Clone)]
pub struct CacheEntry<V> {
    pub value: V,
    pub size: u64,
    pub last_accessed: SystemTime,
}

/// Generic LRU cache with size-based eviction
///
/// Evicts least-recently-accessed entries when total size exceeds max_size.
pub struct LruCache<K, V> {
    entries: HashMap<K, CacheEntry<V>>,
    total_size: u64,
    max_size: u64,
}

impl<K, V> LruCache<K, V>
where
    K: Eq + Hash + Clone,
{
    pub fn new(max_size: u64) -> Self {
        Self {
            entries: HashMap::new(),
            total_size: 0,
            max_size,
        }
    }

    pub fn get(&mut self, key: &K) -> Option<&V> {
        if let Some(entry) = self.entries.get_mut(key) {
            entry.last_accessed = SystemTime::now();
            Some(&entry.value)
        } else {
            None
        }
    }

    pub fn get_mut(&mut self, key: &K) -> Option<(&mut V, &mut SystemTime)> {
        self.entries.get_mut(key).map(|entry| {
            entry.last_accessed = SystemTime::now();
            (&mut entry.value, &mut entry.last_accessed)
        })
    }

    pub fn insert(&mut self, key: K, value: V, size: u64) {
        // Evict oldest entries if adding this would exceed max_size
        while self.total_size + size > self.max_size && !self.entries.is_empty() {
            if let Some(oldest_key) = self.find_oldest_key() {
                if let Some(removed) = self.entries.remove(&oldest_key) {
                    self.total_size = self.total_size.saturating_sub(removed.size);
                }
            } else {
                break;
            }
        }

        self.entries.insert(
            key,
            CacheEntry {
                value,
                size,
                last_accessed: SystemTime::now(),
            },
        );
        self.total_size += size;
    }

    pub fn remove(&mut self, key: &K) -> Option<V> {
        if let Some(entry) = self.entries.remove(key) {
            self.total_size = self.total_size.saturating_sub(entry.size);
            Some(entry.value)
        } else {
            None
        }
    }

    pub fn contains_key(&self, key: &K) -> bool {
        self.entries.contains_key(key)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn total_size(&self) -> u64 {
        self.total_size
    }

    pub fn max_size(&self) -> u64 {
        self.max_size
    }

    pub fn iter(&self) -> impl Iterator<Item = (&K, &CacheEntry<V>)> {
        self.entries.iter()
    }

    fn find_oldest_key(&self) -> Option<K> {
        self.entries
            .iter()
            .min_by_key(|(_, entry)| entry.last_accessed)
            .map(|(key, _)| key.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lru_cache_basic() {
        let mut cache = LruCache::<String, String>::new(100);
        cache.insert("a".into(), "value_a".into(), 40);
        cache.insert("b".into(), "value_b".into(), 30);
        assert_eq!(cache.len(), 2);
        assert_eq!(cache.total_size(), 70);

        let val = cache.get(&"a".to_string());
        assert_eq!(val, Some(&"value_a".to_string()));
    }

    #[test]
    fn test_lru_cache_eviction() {
        let mut cache = LruCache::<String, String>::new(100);
        cache.insert("a".into(), "value_a".into(), 50);
        cache.insert("b".into(), "value_b".into(), 30);

        // Access "a" to make "b" the oldest
        cache.get(&"a".to_string());

        // Insert 40 more: 50 + 30 + 40 = 120 > 100, evicts oldest ("b")
        cache.insert("c".into(), "value_c".into(), 40);
        assert!(!cache.contains_key(&"b".to_string()));
        assert!(cache.contains_key(&"a".to_string()));
        assert!(cache.contains_key(&"c".to_string()));
    }

    #[test]
    fn test_lru_cache_remove() {
        let mut cache = LruCache::<String, String>::new(100);
        cache.insert("a".into(), "value_a".into(), 40);
        assert_eq!(cache.total_size(), 40);
        let removed = cache.remove(&"a".to_string());
        assert_eq!(removed, Some("value_a".to_string()));
        assert_eq!(cache.total_size(), 0);
        assert!(cache.is_empty());
    }
}
