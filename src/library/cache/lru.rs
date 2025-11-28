// LRU Cache - in-memory cache with Least Recently Used eviction

use std::collections::HashMap;
use std::time::Instant;

/// Statistics about cache performance
#[derive(Debug, Clone, serde::Serialize)]
pub struct CacheStats {
    pub size_bytes: usize,
    pub size_limit: usize,
    pub entry_count: usize,
    pub hit_count: u64,
    pub miss_count: u64,
    pub eviction_count: u64,
}

impl CacheStats {
    /// Calculate hit rate as a percentage (0-100)
    pub fn hit_rate(&self) -> f64 {
        let total = self.hit_count + self.miss_count;
        if total == 0 {
            0.0
        } else {
            (self.hit_count as f64 / total as f64) * 100.0
        }
    }

    /// Calculate cache usage as a percentage (0-100)
    pub fn usage_percent(&self) -> f64 {
        if self.size_limit == 0 {
            0.0
        } else {
            (self.size_bytes as f64 / self.size_limit as f64) * 100.0
        }
    }
}

/// Information about a cache entry for debugging
#[derive(Debug, Clone, serde::Serialize)]
pub struct CacheEntryInfo {
    pub key: String,
    pub size_bytes: usize,
    pub access_count: u64,
    #[serde(skip)]
    pub last_access: Instant,
    #[serde(skip)]
    pub created_at: Instant,
}

/// Internal cache entry with metadata
#[derive(Debug, Clone)]
struct CacheEntry {
    key: String,
    value: Vec<u8>,       // Serialized data (MessagePack)
    size_bytes: usize,    // Memory footprint
    access_time: Instant, // For LRU tracking
    access_count: u64,    // Access counter for debugging
    created_at: Instant,  // Creation timestamp
}

/// LRU cache with automatic eviction when size limit exceeded
pub struct LruCache {
    entries: HashMap<String, CacheEntry>,
    size_limit_bytes: usize,
    current_size_bytes: usize,
    hit_count: u64,
    miss_count: u64,
    eviction_count: u64,
    logging_enabled: bool,
}

impl LruCache {
    /// Create new LRU cache with size limit in bytes
    pub fn new(size_limit_bytes: usize, logging_enabled: bool) -> Self {
        Self {
            entries: HashMap::new(),
            size_limit_bytes,
            current_size_bytes: 0,
            hit_count: 0,
            miss_count: 0,
            eviction_count: 0,
            logging_enabled,
        }
    }

    /// Get cached value by key
    pub fn get<T>(&mut self, key: &str) -> Option<T>
    where
        T: serde::de::DeserializeOwned,
    {
        if let Some(entry) = self.entries.get_mut(key) {
            // Update access time and counter
            entry.access_time = Instant::now();
            entry.access_count += 1;

            self.hit_count += 1;

            if self.logging_enabled {
                tracing::debug!("Cache hit: {} (access count: {})", key, entry.access_count);
            }

            // Deserialize value
            match rmp_serde::from_slice(&entry.value) {
                Ok(value) => Some(value),
                Err(e) => {
                    tracing::error!("Cache deserialization error for key {}: {}", key, e);
                    None
                }
            }
        } else {
            self.miss_count += 1;

            if self.logging_enabled {
                tracing::debug!("Cache miss: {}", key);
            }

            None
        }
    }

    /// Set cached value (evicts LRU entries if size limit exceeded)
    pub fn set<T>(&mut self, key: String, value: T)
    where
        T: serde::Serialize,
    {
        // Serialize value
        let serialized = match rmp_serde::to_vec(&value) {
            Ok(bytes) => bytes,
            Err(e) => {
                tracing::error!("Cache serialization error for key {}: {}", key, e);
                return;
            }
        };

        let value_size = serialized.len();

        // Skip if value is larger than total cache size (check BEFORE evicting)
        if value_size > self.size_limit_bytes {
            if self.logging_enabled {
                tracing::warn!(
                    "Cache entry too large to store: {} ({} bytes > {} bytes limit)",
                    key,
                    value_size,
                    self.size_limit_bytes
                );
            }
            return;
        }

        // Remove old entry if key exists (update scenario)
        if let Some(old_entry) = self.entries.remove(&key) {
            self.current_size_bytes -= old_entry.size_bytes;
        }

        // Evict LRU entries if needed to make room
        while self.current_size_bytes + value_size > self.size_limit_bytes
            && !self.entries.is_empty()
        {
            self.evict_lru();
        }

        // Insert new entry
        let now = Instant::now();
        let entry = CacheEntry {
            key: key.clone(),
            value: serialized,
            size_bytes: value_size,
            access_time: now,
            access_count: 0,
            created_at: now,
        };

        self.current_size_bytes += value_size;
        self.entries.insert(key, entry);
    }

    /// Evict least recently used entry
    fn evict_lru(&mut self) {
        // Find entry with oldest access time
        if let Some((lru_key, _)) = self
            .entries
            .iter()
            .min_by_key(|(_, entry)| entry.access_time)
        {
            let lru_key = lru_key.clone();

            if let Some(entry) = self.entries.remove(&lru_key) {
                self.current_size_bytes -= entry.size_bytes;
                self.eviction_count += 1;

                if self.logging_enabled {
                    tracing::debug!(
                        "Cache eviction: {} ({} bytes, {} accesses)",
                        lru_key,
                        entry.size_bytes,
                        entry.access_count
                    );
                }
            }
        }
    }

    /// Invalidate (remove) cache entry by key
    pub fn invalidate(&mut self, key: &str) {
        if let Some(entry) = self.entries.remove(key) {
            self.current_size_bytes -= entry.size_bytes;

            if self.logging_enabled {
                tracing::debug!("Cache invalidation: {}", key);
            }
        }
    }

    /// Clear all cache entries
    pub fn clear(&mut self) {
        let count = self.entries.len();
        self.entries.clear();
        self.current_size_bytes = 0;

        if self.logging_enabled && count > 0 {
            tracing::info!("Cache cleared: {} entries removed", count);
        }
    }

    /// Get cache statistics
    pub fn stats(&self) -> CacheStats {
        CacheStats {
            size_bytes: self.current_size_bytes,
            size_limit: self.size_limit_bytes,
            entry_count: self.entries.len(),
            hit_count: self.hit_count,
            miss_count: self.miss_count,
            eviction_count: self.eviction_count,
        }
    }

    /// Get all cache entries (for debug page)
    pub fn entries(&self) -> Vec<CacheEntryInfo> {
        self.entries
            .values()
            .map(|entry| CacheEntryInfo {
                key: entry.key.clone(),
                size_bytes: entry.size_bytes,
                access_count: entry.access_count,
                last_access: entry.access_time,
                created_at: entry.created_at,
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread::sleep;
    use std::time::Duration;

    #[test]
    fn test_basic_get_set() {
        let mut cache = LruCache::new(1000, false);

        cache.set("key".to_string(), vec![1, 2, 3]);
        let result: Option<Vec<i32>> = cache.get("key");

        assert_eq!(result, Some(vec![1, 2, 3]));
    }

    #[test]
    fn test_cache_miss() {
        let mut cache = LruCache::new(1000, false);
        let result: Option<Vec<i32>> = cache.get("nonexistent");

        assert_eq!(result, None);
    }

    #[test]
    fn test_eviction_respects_access_order() {
        // Cache that can hold exactly 2 entries
        let mut cache = LruCache::new(60, false);

        // Insert A and B
        cache.set("A".to_string(), vec![0u8; 20]);
        sleep(Duration::from_millis(10));
        cache.set("B".to_string(), vec![0u8; 20]);
        sleep(Duration::from_millis(10));

        // Access A to make it more recently used than B
        let _: Option<Vec<u8>> = cache.get("A");
        sleep(Duration::from_millis(10));

        // Insert C - should evict B (least recently accessed), not A
        cache.set("C".to_string(), vec![0u8; 20]);

        // Verify eviction happened
        let stats = cache.stats();
        assert!(
            stats.eviction_count > 0,
            "Should have evicted at least one entry"
        );

        // A should survive (was recently accessed)
        assert!(
            cache.get::<Vec<u8>>("A").is_some(),
            "A should survive (was recently accessed)"
        );
        // B should be evicted (least recently accessed)
        assert!(
            cache.get::<Vec<u8>>("B").is_none(),
            "B should be evicted (least recently accessed)"
        );
        // C should exist (just inserted)
        assert!(
            cache.get::<Vec<u8>>("C").is_some(),
            "C should exist (just inserted)"
        );
    }

    #[test]
    fn test_update_existing_key_size_accounting() {
        let mut cache = LruCache::new(200, false);

        cache.set("key".to_string(), vec![0u8; 40]);
        let size_after_insert = cache.stats().size_bytes;

        cache.set("key".to_string(), vec![0u8; 40]); // Same size
        let size_after_update = cache.stats().size_bytes;

        assert_eq!(
            size_after_insert, size_after_update,
            "Updating same key shouldn't double size"
        );
    }

    #[test]
    fn test_update_existing_key_different_size() {
        let mut cache = LruCache::new(200, false);

        cache.set("key".to_string(), vec![0u8; 40]);
        let size1 = cache.stats().size_bytes;

        cache.set("key".to_string(), vec![0u8; 80]); // Bigger
        let size2 = cache.stats().size_bytes;

        assert!(size2 > size1, "Size should increase with larger value");
        assert_eq!(cache.stats().entry_count, 1, "Should still be one entry");
    }

    #[test]
    fn test_oversized_value_does_not_evict_existing() {
        let mut cache = LruCache::new(100, false);

        cache.set("small".to_string(), vec![0u8; 30]);
        assert_eq!(cache.stats().entry_count, 1);

        // Try to insert something larger than total cache
        cache.set("huge".to_string(), vec![0u8; 200]);

        // CRITICAL: The small entry should NOT have been evicted
        assert!(
            cache.get::<Vec<u8>>("small").is_some(),
            "Oversized insert should not evict existing entries"
        );
        assert_eq!(
            cache.stats().entry_count,
            1,
            "Should still have only the small entry"
        );
    }

    #[test]
    fn test_multiple_evictions_for_large_insert() {
        let mut cache = LruCache::new(100, false);

        // Insert 5 small items
        for i in 0..5 {
            cache.set(format!("k{}", i), vec![0u8; 10]);
            sleep(Duration::from_millis(5));
        }

        // Insert one large item that needs multiple evictions
        cache.set("big".to_string(), vec![0u8; 60]);

        // Should have evicted oldest entries until big fits
        let stats = cache.stats();
        assert!(stats.size_bytes <= 100, "Should not exceed limit");
        assert!(
            cache.get::<Vec<u8>>("big").is_some(),
            "Big item should exist"
        );
        assert!(stats.eviction_count > 0, "Should have evicted some entries");
    }

    #[test]
    fn test_clear_resets_size_tracking() {
        let mut cache = LruCache::new(1000, false);

        cache.set("a".to_string(), vec![0u8; 100]);
        cache.set("b".to_string(), vec![0u8; 100]);

        let hits_before = cache.stats().hit_count;

        cache.clear();

        let stats = cache.stats();
        assert_eq!(stats.size_bytes, 0, "Size should be zero after clear");
        assert_eq!(stats.entry_count, 0, "Entry count should be zero");
        assert_eq!(stats.hit_count, hits_before, "Hit count should persist");
    }

    #[test]
    fn test_invalidate_updates_size() {
        let mut cache = LruCache::new(1000, false);

        cache.set("key".to_string(), vec![0u8; 100]);
        let size_before = cache.stats().size_bytes;

        cache.invalidate("key");
        let size_after = cache.stats().size_bytes;

        assert!(
            size_after < size_before,
            "Size should decrease after invalidation"
        );
        assert_eq!(cache.stats().entry_count, 0);
    }

    #[test]
    fn test_invalidate_nonexistent_key() {
        let mut cache = LruCache::new(1000, false);
        cache.set("exists".to_string(), vec![0u8; 50]);

        cache.invalidate("does_not_exist"); // Should not panic

        assert_eq!(cache.stats().entry_count, 1, "Existing entry should remain");
    }

    #[test]
    fn test_statistics_tracking() {
        let mut cache = LruCache::new(1000, false);

        cache.set("key".to_string(), "value".to_string());

        let _: Option<String> = cache.get("key"); // Hit
        let _: Option<String> = cache.get("key"); // Hit
        let _: Option<String> = cache.get("miss"); // Miss

        let stats = cache.stats();
        assert_eq!(stats.hit_count, 2);
        assert_eq!(stats.miss_count, 1);
    }

    #[test]
    fn test_size_limit_enforcement() {
        let mut cache = LruCache::new(100, false);

        // Insert items until we trigger eviction
        cache.set("k1".to_string(), vec![0u8; 30]);
        cache.set("k2".to_string(), vec![0u8; 30]);
        cache.set("k3".to_string(), vec![0u8; 30]);

        // Cache should never exceed limit
        let stats = cache.stats();
        assert!(
            stats.size_bytes <= 100,
            "Cache should not exceed size limit"
        );
    }

    #[test]
    fn test_eviction_counter() {
        let mut cache = LruCache::new(50, false);

        cache.set("k1".to_string(), vec![0u8; 30]);
        cache.set("k2".to_string(), vec![0u8; 30]); // Should evict k1

        let stats = cache.stats();
        assert_eq!(stats.eviction_count, 1, "Should have one eviction");
    }

    #[test]
    fn test_empty_string_key() {
        let mut cache = LruCache::new(1000, false);

        cache.set("".to_string(), vec![1, 2, 3]);
        let result: Option<Vec<i32>> = cache.get("");

        assert_eq!(result, Some(vec![1, 2, 3]), "Empty string key should work");
    }

    #[test]
    fn test_entries_list() {
        let mut cache = LruCache::new(1000, false);

        cache.set("key1".to_string(), vec![1, 2, 3]);
        cache.set("key2".to_string(), vec![4, 5, 6]);

        let entries = cache.entries();
        assert_eq!(entries.len(), 2, "Should have 2 entries");

        let keys: Vec<&str> = entries.iter().map(|e| e.key.as_str()).collect();
        assert!(keys.contains(&"key1"));
        assert!(keys.contains(&"key2"));
    }
}
