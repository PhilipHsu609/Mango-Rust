// Cache module - unified caching API for library operations
//
// Provides two-tier caching:
// 1. Library Cache File - persistent disk cache for entire library structure
// 2. LRU Cache - in-memory runtime cache for computed data

mod file;
pub mod key;
mod lru;

pub use file::CachedLibraryData;
pub use lru::{CacheEntryInfo, CacheStats};

use crate::{error::Result, Config, Library};
use std::path::Path;

/// Cache facade providing unified caching API
pub struct Cache {
    lru_cache: lru::LruCache,
    file_manager: file::CacheFileManager,
    enabled: bool,
}

impl Cache {
    /// Create new cache from configuration
    pub fn new(config: &Config) -> Self {
        let size_bytes = config.cache_size_mbs * 1024 * 1024;
        let lru_cache = lru::LruCache::new(size_bytes, config.cache_log_enabled);
        let file_manager = file::CacheFileManager::new(config.library_cache_path.clone());

        Self {
            lru_cache,
            file_manager,
            enabled: config.cache_enabled,
        }
    }

    /// Get cached sorted titles
    pub fn get_sorted_titles(&mut self, key: &str) -> Option<Vec<String>> {
        if !self.enabled {
            return None;
        }
        self.lru_cache.get(key)
    }

    /// Cache sorted titles
    pub fn set_sorted_titles(&mut self, key: String, title_ids: Vec<String>) {
        if !self.enabled {
            return;
        }
        self.lru_cache.set(key, title_ids);
    }

    /// Invalidate progress-dependent title lists for a user.
    pub fn invalidate_progress(&mut self, username: &str) {
        if !self.enabled {
            return;
        }

        let prefix = format!("sorted_titles:{}:", username);
        self.invalidate_by_prefix(&prefix);
    }

    /// Invalidate all cache entries with the given prefix
    fn invalidate_by_prefix(&mut self, prefix: &str) {
        // Get all entries and find those with matching prefix
        let entries = self.lru_cache.entries();
        for entry in entries {
            if entry.key.starts_with(prefix) {
                self.lru_cache.invalidate(&entry.key);
            }
        }
    }

    /// Save library to cache file
    pub async fn save_library(&self, library: &Library) -> Result<()> {
        if !self.enabled {
            return Ok(());
        }
        self.file_manager.save(library).await
    }

    /// Get cloneable file manager for background save tasks
    pub fn file_manager(&self) -> file::CacheFileManager {
        self.file_manager.clone()
    }

    /// Load library from cache file
    pub async fn load_library(
        &self,
        expected_dir: &Path,
        db_title_count: usize,
    ) -> Result<Option<file::CachedLibraryData>> {
        if !self.enabled {
            return Ok(None);
        }

        // Load cached data
        let cached_data = match self.file_manager.load(expected_dir).await? {
            Some(data) => data,
            None => return Ok(None),
        };

        // Validate title count
        if cached_data.titles.len() != db_title_count {
            tracing::warn!(
                "Cache title count mismatch: cache has {}, database has {}. Invalidating cache.",
                cached_data.titles.len(),
                db_title_count
            );
            let _ = self.file_manager.delete().await;
            return Ok(None);
        }

        Ok(Some(cached_data))
    }

    /// Get cache statistics
    pub fn stats(&self) -> CacheStats {
        self.lru_cache.stats()
    }

    /// Get cache entries for debugging (admin page)
    pub fn entries(&self) -> Vec<lru::CacheEntryInfo> {
        self.lru_cache.entries()
    }

    /// Clear all cached data
    pub fn clear(&mut self) {
        if !self.enabled {
            return;
        }
        self.lru_cache.clear();
    }

    /// Invalidate a specific cache entry by key
    pub fn invalidate(&mut self, key: &str) {
        if !self.enabled {
            return;
        }
        self.lru_cache.invalidate(key);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_config() -> Config {
        Config {
            host: "0.0.0.0".to_string(),
            port: 9000,
            base_url: "/".to_string(),
            session_secret: "test".to_string(),
            library_path: std::path::PathBuf::from("/tmp/library"),
            db_path: std::path::PathBuf::from("/tmp/test.db"),
            queue_db_path: std::path::PathBuf::from("/tmp/queue.db"),
            scan_interval_minutes: 0,
            thumbnail_generation_interval_hours: 0,
            log_level: "info".to_string(),
            upload_path: std::path::PathBuf::from("/tmp/uploads"),
            plugin_path: std::path::PathBuf::from("/tmp/plugins"),
            download_timeout_seconds: 30,
            library_cache_path: std::path::PathBuf::from("/tmp/cache.bin"),
            cache_enabled: true,
            cache_size_mbs: 100,
            cache_log_enabled: false,
            disable_login: false,
            default_username: String::new(),
            auth_proxy_header_name: String::new(),
            plugin_update_interval_hours: 24,
        }
    }

    #[test]
    fn test_cache_new() {
        let config = create_test_config();
        let cache = Cache::new(&config);

        let stats = cache.stats();
        assert_eq!(stats.size_limit, 100 * 1024 * 1024);
        assert_eq!(stats.entry_count, 0);
    }

    #[test]
    fn test_cache_disabled() {
        let mut config = create_test_config();
        config.cache_enabled = false;

        let mut cache = Cache::new(&config);

        // Set should be no-op when disabled
        cache.set_sorted_titles("key".to_string(), vec!["id1".to_string()]);
        assert!(cache.get_sorted_titles("key").is_none());

        // Invalidation should be no-op
        cache.invalidate_progress("user1");
        cache.clear();
    }

    #[test]
    fn test_sorted_titles_cache() {
        let config = create_test_config();
        let mut cache = Cache::new(&config);

        let title_ids = vec!["id1".to_string(), "id2".to_string()];

        // Cache miss
        assert!(cache.get_sorted_titles("key1").is_none());

        // Cache hit after set
        cache.set_sorted_titles("key1".to_string(), title_ids.clone());
        assert_eq!(cache.get_sorted_titles("key1"), Some(title_ids));
    }

    #[test]
    fn test_invalidate_progress() {
        let config = create_test_config();
        let mut cache = Cache::new(&config);
        let title_ids = vec!["title1".to_string()];
        let user1_key = key::sorted_titles_key("user1", &title_ids, "progress", true);
        let user2_key = key::sorted_titles_key("user2", &title_ids, "progress", true);

        cache.set_sorted_titles(user1_key.clone(), vec!["title1".to_string()]);
        cache.set_sorted_titles(user2_key.clone(), vec!["title1".to_string()]);

        cache.invalidate_progress("user1");

        assert!(cache.get_sorted_titles(&user1_key).is_none());
        assert_eq!(
            cache.get_sorted_titles(&user2_key),
            Some(vec!["title1".to_string()])
        );
    }

    #[test]
    fn test_clear() {
        let config = create_test_config();
        let mut cache = Cache::new(&config);

        cache.set_sorted_titles("key1".to_string(), vec!["t1".to_string()]);

        assert_eq!(cache.stats().entry_count, 1);

        cache.clear();

        assert_eq!(cache.stats().entry_count, 0);
        assert!(cache.get_sorted_titles("key1").is_none());
    }

    #[test]
    fn test_stats() {
        let config = create_test_config();
        let mut cache = Cache::new(&config);

        let stats_before = cache.stats();
        assert_eq!(stats_before.entry_count, 0);
        assert_eq!(stats_before.hit_count, 0);
        assert_eq!(stats_before.miss_count, 0);

        // Add entry
        cache.set_sorted_titles("key1".to_string(), vec!["t1".to_string()]);

        // Hit
        let _ = cache.get_sorted_titles("key1");
        // Miss
        let _ = cache.get_sorted_titles("key2");

        let stats_after = cache.stats();
        assert_eq!(stats_after.entry_count, 1);
        assert_eq!(stats_after.hit_count, 1);
        assert_eq!(stats_after.miss_count, 1);
    }
}
