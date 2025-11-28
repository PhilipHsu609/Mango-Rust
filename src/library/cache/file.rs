// Cache File Manager - persistent library cache serialization

use crate::error::{Error, Result};
use crate::Library;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// Metadata about the cache file
#[derive(Debug, Clone)]
pub struct CacheFileMetadata {
    pub path: PathBuf,
    pub size_bytes: u64,
    pub modified: SystemTime,
    pub valid: bool,
}

/// Manager for library cache file operations
#[derive(Clone)]
pub struct CacheFileManager {
    cache_path: PathBuf,
}

/// Serializable library data (excludes database Storage)
#[derive(serde::Serialize, serde::Deserialize)]
pub struct CachedLibraryData {
    pub path: PathBuf,
    pub titles: std::collections::HashMap<String, crate::library::Title>,
}

impl CacheFileManager {
    /// Create new cache file manager
    pub fn new(cache_path: PathBuf) -> Self {
        Self { cache_path }
    }

    /// Save library to cache file (MessagePack + gzip)
    pub async fn save(&self, library: &Library) -> Result<()> {
        let cached_data = CachedLibraryData {
            path: library.path().to_path_buf(),
            titles: library.titles().clone(),
        };
        self.save_data(cached_data).await
    }

    /// Save cached library data to file (MessagePack + gzip)
    pub async fn save_data(&self, cached_data: CachedLibraryData) -> Result<()> {
        use flate2::write::GzEncoder;
        use flate2::Compression;
        use std::io::Write;

        // Serialize to MessagePack
        let serialized = rmp_serde::to_vec(&cached_data)
            .map_err(|e| Error::CacheSerialization(e.to_string()))?;

        // Compress with gzip
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder
            .write_all(&serialized)
            .map_err(|e| Error::CacheSerialization(e.to_string()))?;
        let compressed = encoder
            .finish()
            .map_err(|e| Error::CacheSerialization(e.to_string()))?;

        // Create parent directory if needed
        if let Some(parent) = self.cache_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        // Atomic write: write to temp file then rename
        let temp_path = self.cache_path.with_extension("tmp");
        tokio::fs::write(&temp_path, &compressed).await?;

        // Set file permissions to 0600 (owner read/write only)
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let perms = std::fs::Permissions::from_mode(0o600);
            std::fs::set_permissions(&temp_path, perms)?;
        }

        // Atomic rename
        tokio::fs::rename(&temp_path, &self.cache_path).await?;

        tracing::info!(
            "Library cache saved: {} ({} bytes compressed)",
            self.cache_path.display(),
            compressed.len()
        );

        Ok(())
    }

    /// Load library from cache file
    pub async fn load(&self, expected_dir: &Path) -> Result<Option<CachedLibraryData>> {
        use flate2::read::GzDecoder;
        use std::io::Read;

        // Check if cache file exists
        if !self.cache_path.exists() {
            tracing::debug!("Cache file does not exist: {}", self.cache_path.display());
            return Ok(None);
        }

        // Read compressed file
        let compressed = match tokio::fs::read(&self.cache_path).await {
            Ok(data) => data,
            Err(e) => {
                tracing::warn!("Failed to read cache file: {}", e);
                return Ok(None);
            }
        };

        // Decompress
        let mut decoder = GzDecoder::new(&compressed[..]);
        let mut serialized = Vec::new();
        if let Err(e) = decoder.read_to_end(&mut serialized) {
            tracing::warn!("Failed to decompress cache file: {}", e);
            // Delete corrupt cache
            let _ = tokio::fs::remove_file(&self.cache_path).await;
            return Ok(None);
        }

        // Deserialize
        let cached_data: CachedLibraryData = match rmp_serde::from_slice(&serialized) {
            Ok(data) => data,
            Err(e) => {
                tracing::warn!("Failed to deserialize cache file: {}", e);
                // Delete corrupt cache
                let _ = tokio::fs::remove_file(&self.cache_path).await;
                return Ok(None);
            }
        };

        // Validate directory path matches
        if cached_data.path != expected_dir {
            tracing::warn!(
                "Cache directory mismatch: cached={}, expected={}",
                cached_data.path.display(),
                expected_dir.display()
            );
            // Delete invalid cache
            let _ = tokio::fs::remove_file(&self.cache_path).await;
            return Ok(None);
        }

        tracing::info!(
            "Library cache loaded: {} titles from {}",
            cached_data.titles.len(),
            self.cache_path.display()
        );

        Ok(Some(cached_data))
    }

    /// Validate cache file against current configuration
    pub async fn validate(&self, library: &Library, db_title_count: usize) -> Result<()> {
        // Load cache to validate
        let cached_data = match self.load(library.path()).await? {
            Some(data) => data,
            None => {
                return Err(Error::CacheCorrupted(
                    "Cache file does not exist or is invalid".to_string(),
                ))
            }
        };

        // Validate title count matches database
        if cached_data.titles.len() != db_title_count {
            return Err(Error::CacheCorrupted(format!(
                "Title count mismatch: cache has {}, database has {}",
                cached_data.titles.len(),
                db_title_count
            )));
        }

        Ok(())
    }

    /// Delete cache file
    pub async fn delete(&self) -> Result<()> {
        if self.cache_path.exists() {
            tokio::fs::remove_file(&self.cache_path).await?;
            tracing::info!("Cache file deleted: {}", self.cache_path.display());
        }
        Ok(())
    }

    /// Get cache file metadata
    pub async fn metadata(&self) -> Result<CacheFileMetadata> {
        if !self.cache_path.exists() {
            return Ok(CacheFileMetadata {
                path: self.cache_path.clone(),
                size_bytes: 0,
                modified: SystemTime::now(),
                valid: false,
            });
        }

        let metadata = tokio::fs::metadata(&self.cache_path).await?;

        Ok(CacheFileMetadata {
            path: self.cache_path.clone(),
            size_bytes: metadata.len(),
            modified: metadata.modified()?,
            valid: true,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Storage;
    use tempfile::TempDir;

    async fn create_test_library(path: PathBuf) -> Library {
        // Create a test storage
        let temp_db = tempfile::NamedTempFile::new().unwrap();
        let db_path = temp_db.path().to_str().unwrap();
        let storage = Storage::new(db_path).await.unwrap();

        // Create test config for cache initialization
        let config = crate::Config {
            host: "0.0.0.0".to_string(),
            port: 9000,
            base_url: "/".to_string(),
            session_secret: "test".to_string(),
            library_path: path.clone(),
            db_path: PathBuf::from(db_path),
            queue_db_path: PathBuf::from("/tmp/test_queue.db"),
            scan_interval_minutes: 0,
            thumbnail_generation_interval_hours: 0,
            log_level: "info".to_string(),
            upload_path: PathBuf::from("/tmp/uploads"),
            plugin_path: PathBuf::from("/tmp/plugins"),
            download_timeout_seconds: 30,
            library_cache_path: PathBuf::from("/tmp/test_cache.bin"),
            cache_enabled: true,
            cache_size_mbs: 100,
            cache_log_enabled: false,
            disable_login: false,
            default_username: None,
            auth_proxy_header_name: None,
            plugin_update_interval_hours: 24,
        };

        // Create library with test data
        // Add some test titles (empty for now, but structure is in place)
        Library::new(path, storage, &config)
    }

    #[tokio::test]
    async fn test_save_load_roundtrip() {
        let temp_dir = TempDir::new().unwrap();
        let library_path = temp_dir.path().join("library");
        let cache_path = temp_dir.path().join("cache.bin");

        // Create test library
        let library = create_test_library(library_path.clone()).await;

        // Save to cache
        let manager = CacheFileManager::new(cache_path.clone());
        manager.save(&library).await.unwrap();

        // Verify cache file exists
        assert!(cache_path.exists(), "Cache file should be created");

        // Load from cache
        let loaded = manager.load(&library_path).await.unwrap();
        assert!(loaded.is_some(), "Should load cached data");

        let loaded_data = loaded.unwrap();
        assert_eq!(loaded_data.path, library_path);
        assert_eq!(loaded_data.titles.len(), library.titles().len());
    }

    #[tokio::test]
    async fn test_load_nonexistent_cache() {
        let temp_dir = TempDir::new().unwrap();
        let cache_path = temp_dir.path().join("nonexistent.bin");
        let library_path = temp_dir.path().join("library");

        let manager = CacheFileManager::new(cache_path);
        let result = manager.load(&library_path).await.unwrap();

        assert!(result.is_none(), "Should return None for nonexistent cache");
    }

    #[tokio::test]
    async fn test_directory_mismatch_invalidates_cache() {
        let temp_dir = TempDir::new().unwrap();
        let library_path1 = temp_dir.path().join("library1");
        let library_path2 = temp_dir.path().join("library2");
        let cache_path = temp_dir.path().join("cache.bin");

        // Create and save library with path1
        let library = create_test_library(library_path1).await;
        let manager = CacheFileManager::new(cache_path.clone());
        manager.save(&library).await.unwrap();

        // Try to load with path2 (different directory)
        let result = manager.load(&library_path2).await.unwrap();
        assert!(
            result.is_none(),
            "Should invalidate cache for directory mismatch"
        );

        // Cache file should be deleted
        assert!(!cache_path.exists(), "Invalid cache should be deleted");
    }

    #[tokio::test]
    async fn test_corrupt_file_handling() {
        let temp_dir = TempDir::new().unwrap();
        let cache_path = temp_dir.path().join("cache.bin");
        let library_path = temp_dir.path().join("library");

        // Write corrupt data
        tokio::fs::write(&cache_path, b"corrupt data")
            .await
            .unwrap();

        // Try to load corrupt cache
        let manager = CacheFileManager::new(cache_path.clone());
        let result = manager.load(&library_path).await.unwrap();

        assert!(result.is_none(), "Should return None for corrupt cache");
        assert!(!cache_path.exists(), "Corrupt cache should be deleted");
    }

    #[tokio::test]
    async fn test_delete_operation() {
        let temp_dir = TempDir::new().unwrap();
        let library_path = temp_dir.path().join("library");
        let cache_path = temp_dir.path().join("cache.bin");

        // Create and save library
        let library = create_test_library(library_path).await;
        let manager = CacheFileManager::new(cache_path.clone());
        manager.save(&library).await.unwrap();

        assert!(cache_path.exists(), "Cache file should exist");

        // Delete cache
        manager.delete().await.unwrap();
        assert!(!cache_path.exists(), "Cache file should be deleted");

        // Delete again should not error
        manager.delete().await.unwrap();
    }

    #[tokio::test]
    async fn test_metadata_extraction() {
        let temp_dir = TempDir::new().unwrap();
        let library_path = temp_dir.path().join("library");
        let cache_path = temp_dir.path().join("cache.bin");

        let manager = CacheFileManager::new(cache_path.clone());

        // Metadata for nonexistent file
        let meta = manager.metadata().await.unwrap();
        assert!(!meta.valid);
        assert_eq!(meta.size_bytes, 0);

        // Create cache file
        let library = create_test_library(library_path).await;
        manager.save(&library).await.unwrap();

        // Metadata for existing file
        let meta = manager.metadata().await.unwrap();
        assert!(meta.valid);
        assert!(meta.size_bytes > 0);
        assert_eq!(meta.path, cache_path);
    }

    #[tokio::test]
    async fn test_atomic_write() {
        let temp_dir = TempDir::new().unwrap();
        let library_path = temp_dir.path().join("library");
        let cache_path = temp_dir.path().join("cache.bin");

        let library = create_test_library(library_path).await;
        let manager = CacheFileManager::new(cache_path.clone());

        // Save should use atomic write (temp file + rename)
        manager.save(&library).await.unwrap();

        // Temp file should not exist
        let temp_path = cache_path.with_extension("tmp");
        assert!(!temp_path.exists(), "Temp file should not exist after save");

        // Final cache file should exist
        assert!(cache_path.exists(), "Cache file should exist");
    }

    #[tokio::test]
    async fn test_validation_success() {
        let temp_dir = TempDir::new().unwrap();
        let library_path = temp_dir.path().join("library");
        let cache_path = temp_dir.path().join("cache.bin");

        let library = create_test_library(library_path).await;
        let manager = CacheFileManager::new(cache_path);

        // Save library
        manager.save(&library).await.unwrap();

        // Validate should succeed (title count matches)
        let db_title_count = library.titles().len();
        manager.validate(&library, db_title_count).await.unwrap();
    }

    #[tokio::test]
    async fn test_validation_title_count_mismatch() {
        let temp_dir = TempDir::new().unwrap();
        let library_path = temp_dir.path().join("library");
        let cache_path = temp_dir.path().join("cache.bin");

        let library = create_test_library(library_path).await;
        let manager = CacheFileManager::new(cache_path);

        // Save library
        manager.save(&library).await.unwrap();

        // Validate with wrong title count should fail
        let wrong_count = library.titles().len() + 10;
        let result = manager.validate(&library, wrong_count).await;

        assert!(result.is_err(), "Should error on title count mismatch");
    }
}
