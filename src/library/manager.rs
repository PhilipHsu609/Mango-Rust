use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use arc_swap::ArcSwap;
use tokio::sync::Mutex;

use super::entry::Entry;
use super::title::Title;
use crate::error::Result;
use crate::Storage;

pub struct Library {
    /// Library root directory
    path: PathBuf,

    /// All titles indexed by ID
    titles: HashMap<String, Title>,

    /// Database storage for ID persistence
    storage: Storage,

    /// Cache for sorted lists and library data (uses Mutex for thread-safe interior mutability)
    cache: Mutex<super::cache::Cache>,

    /// In-memory cache for progress data (eliminates O(N) filesystem reads)
    progress_cache: super::progress_cache::ProgressCache,
}

impl Library {
    /// Create a new Library instance
    pub fn new(path: PathBuf, storage: Storage, config: &crate::Config) -> Self {
        Self {
            path,
            titles: HashMap::new(),
            storage,
            cache: Mutex::new(super::cache::Cache::new(config)),
            progress_cache: super::progress_cache::ProgressCache::new(),
        }
    }

    /// Convert absolute path to relative path (relative to library root)
    /// Example: "/home/user/library/Series/Chapter.zip" -> "Series/Chapter.zip"
    #[allow(dead_code)]
    fn to_relative_path(&self, absolute_path: &Path) -> Result<String> {
        absolute_path
            .strip_prefix(&self.path)
            .map(|p| p.to_string_lossy().to_string())
            .map_err(|_| {
                crate::error::Error::Internal(format!(
                    "Path {} is not within library root {}",
                    absolute_path.display(),
                    self.path.display()
                ))
            })
    }

    /// Try to load library from cache
    /// Returns Ok(true) if loaded from cache, Ok(false) if cache miss/invalid
    pub async fn try_load_from_cache(&mut self) -> Result<bool> {
        tracing::info!("Attempting to load library from cache");

        // Get database title count for validation
        let db_title_count =
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM titles WHERE unavailable = 0")
                .fetch_one(self.storage.pool())
                .await? as usize;

        // Try to load from cache
        let cache = self.cache.lock().await;
        match cache.load_library(&self.path, db_title_count).await? {
            Some(cached_data) => {
                drop(cache); // Release lock before modifying self.titles

                self.titles = cached_data.titles;
                let entry_count: usize = self.titles.values().map(|t| t.entries.len()).sum();

                tracing::info!(
                    "Library loaded from cache: {} titles, {} entries",
                    self.titles.len(),
                    entry_count
                );

                // Load progress cache for all titles
                self.load_progress_cache().await;

                Ok(true)
            }
            None => {
                tracing::info!("Cache miss or invalid - will perform full scan");
                Ok(false)
            }
        }
    }

    /// Scan the library directory for manga titles
    /// Uses parallel processing with controlled concurrency for improved performance
    pub async fn scan(&mut self) -> Result<()> {
        let scan_start = std::time::Instant::now();
        tracing::info!("Starting library scan: {}", self.path.display());

        // Collect all directory paths first
        let mut title_paths = Vec::new();
        let mut dir_entries = tokio::fs::read_dir(&self.path).await?;
        while let Some(entry) = dir_entries.next_entry().await? {
            let entry_path = entry.path();
            if entry_path.is_dir() {
                title_paths.push(entry_path);
            }
        }

        tracing::info!("Found {} directories to scan", title_paths.len());

        // Collections for bulk database inserts (matching original Mango pattern)
        let new_title_ids = Arc::new(tokio::sync::Mutex::new(Vec::new()));
        let new_entry_ids = Arc::new(tokio::sync::Mutex::new(Vec::new()));

        // Process titles in parallel with controlled concurrency
        let concurrency_limit = 20; // Increased from 5 to 20 for better parallelism
        let semaphore = Arc::new(tokio::sync::Semaphore::new(concurrency_limit));
        let storage = self.storage.clone();
        let library_path = self.path.clone();

        let mut tasks = Vec::new();

        for title_path in title_paths {
            let sem = semaphore.clone();
            let storage_clone = storage.clone();
            let lib_path = library_path.clone();
            let title_ids = new_title_ids.clone();
            let entry_ids = new_entry_ids.clone();

            let task = tokio::spawn(async move {
                let _permit = sem.acquire().await.unwrap();

                // Scan title directory
                let mut title = match Title::from_directory(title_path.clone()).await {
                    Ok(t) => t,
                    Err(e) => {
                        tracing::warn!("Failed to scan title at {}: {}", title_path.display(), e);
                        return None;
                    }
                };

                // Find or create title ID
                let existing_id = Self::find_existing_id_static(&lib_path, &title, &storage_clone)
                    .await
                    .ok()?;
                let is_new_title = existing_id.is_none();
                if let Some(id) = existing_id {
                    title.id = id;
                    tracing::debug!("Matched existing title: {} ({})", title.title, title.id);
                } else {
                    // New title - collect for bulk insert
                    let relative_path = title
                        .path
                        .strip_prefix(&lib_path)
                        .ok()?
                        .to_string_lossy()
                        .to_string();

                    title_ids.lock().await.push((
                        title.id.clone(),
                        relative_path,
                        title.signature.clone(),
                    ));
                    tracing::info!("Discovered new title: {} ({})", title.title, title.id);
                }

                // Find or create entry IDs
                for entry in &mut title.entries {
                    let existing_entry_id =
                        Self::find_existing_entry_id_static(&lib_path, entry, &storage_clone)
                            .await
                            .ok()?;
                    if let Some(id) = existing_entry_id {
                        entry.id = id;
                    } else {
                        // New entry - collect for bulk insert
                        let relative_path = entry
                            .path
                            .strip_prefix(&lib_path)
                            .ok()?
                            .to_string_lossy()
                            .to_string();

                        entry_ids.lock().await.push((
                            entry.id.clone(),
                            relative_path,
                            entry.signature.clone(),
                        ));

                        if is_new_title {
                            tracing::debug!("  New entry: {} ({})", entry.title, entry.id);
                        }
                    }
                }

                // Populate date_added
                if let Err(e) = title.populate_date_added().await {
                    tracing::warn!("Failed to populate date_added for {}: {}", title.title, e);
                }

                Some(title)
            });

            tasks.push(task);
        }

        // Collect results
        let mut new_titles = HashMap::new();
        for task in tasks {
            if let Ok(Some(title)) = task.await {
                new_titles.insert(title.id.clone(), title);
            }
        }

        let title_count = new_titles.len();
        let entry_count: usize = new_titles.values().map(|t| t.entries.len()).sum();

        // Bulk insert all new IDs in a single transaction
        let title_ids_vec = new_title_ids.lock().await;
        let entry_ids_vec = new_entry_ids.lock().await;

        if !title_ids_vec.is_empty() || !entry_ids_vec.is_empty() {
            self.bulk_insert_ids(&title_ids_vec, &entry_ids_vec).await?;
            tracing::info!(
                "Bulk inserted {} new titles and {} new entries to database",
                title_ids_vec.len(),
                entry_ids_vec.len()
            );
        }

        self.titles = new_titles;

        // Load progress cache for all titles
        self.load_progress_cache().await;

        // Mark items in database as unavailable if not found during scan
        self.mark_unavailable().await?;

        let scan_duration = scan_start.elapsed();
        tracing::info!(
            "Library scan complete: {} titles, {} entries ({:.2}s)",
            title_count,
            entry_count,
            scan_duration.as_secs_f64()
        );

        // Save library to cache in background (non-blocking)
        self.save_to_cache_background().await;

        Ok(())
    }

    /// Bulk insert title and entry IDs in a single transaction
    /// Matches the pattern from original Mango for performance
    async fn bulk_insert_ids(
        &self,
        title_ids: &[(String, String, String)], // (id, path, signature)
        entry_ids: &[(String, String, String)], // (id, path, signature)
    ) -> Result<()> {
        let mut tx = self.storage.pool().begin().await?;

        // Insert all title IDs
        for (id, path, signature) in title_ids {
            sqlx::query(
                "INSERT INTO titles (id, path, signature, unavailable) VALUES (?, ?, ?, 0)
                 ON CONFLICT(path) DO UPDATE SET id = ?, signature = ?, unavailable = 0",
            )
            .bind(id)
            .bind(path)
            .bind(signature)
            .bind(id)
            .bind(signature)
            .execute(&mut *tx)
            .await?;
        }

        // Insert all entry IDs
        for (id, path, signature) in entry_ids {
            sqlx::query(
                "INSERT INTO ids (id, path, signature, unavailable) VALUES (?, ?, ?, 0)
                 ON CONFLICT(path) DO UPDATE SET id = ?, signature = ?, unavailable = 0",
            )
            .bind(id)
            .bind(path)
            .bind(signature)
            .bind(id)
            .bind(signature)
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        Ok(())
    }

    /// Static helper for finding existing title ID (for use in spawned tasks)
    async fn find_existing_id_static(
        library_path: &Path,
        title: &Title,
        storage: &Storage,
    ) -> Result<Option<String>> {
        let relative_path = title
            .path
            .strip_prefix(library_path)
            .map(|p| p.to_string_lossy().to_string())
            .map_err(|_| {
                crate::error::Error::Internal(format!(
                    "Path {} is not within library root {}",
                    title.path.display(),
                    library_path.display()
                ))
            })?;

        // Tier 1: Exact match
        if let Some(id) = sqlx::query_scalar::<_, String>(
            "SELECT id FROM titles WHERE path = ? AND signature = ? AND unavailable = 0",
        )
        .bind(&relative_path)
        .bind(&title.signature)
        .fetch_optional(storage.pool())
        .await?
        {
            return Ok(Some(id));
        }

        // Tier 2: Path-only match
        if let Some(id) = sqlx::query_scalar::<_, String>(
            "SELECT id FROM titles WHERE path = ? AND unavailable = 0",
        )
        .bind(&relative_path)
        .fetch_optional(storage.pool())
        .await?
        {
            // Update signature
            sqlx::query("UPDATE titles SET signature = ? WHERE id = ?")
                .bind(&title.signature)
                .bind(&id)
                .execute(storage.pool())
                .await?;

            return Ok(Some(id));
        }

        Ok(None)
    }

    /// Static helper for finding existing entry ID (for use in spawned tasks)
    async fn find_existing_entry_id_static(
        library_path: &Path,
        entry: &Entry,
        storage: &Storage,
    ) -> Result<Option<String>> {
        let relative_path = entry
            .path
            .strip_prefix(library_path)
            .map(|p| p.to_string_lossy().to_string())
            .map_err(|_| {
                crate::error::Error::Internal(format!(
                    "Path {} is not within library root {}",
                    entry.path.display(),
                    library_path.display()
                ))
            })?;

        // Tier 1: Exact match
        if let Some(id) = sqlx::query_scalar::<_, String>(
            "SELECT id FROM ids WHERE path = ? AND signature = ? AND unavailable = 0",
        )
        .bind(&relative_path)
        .bind(&entry.signature)
        .fetch_optional(storage.pool())
        .await?
        {
            return Ok(Some(id));
        }

        // Tier 2: Path-only match
        if let Some(id) =
            sqlx::query_scalar::<_, String>("SELECT id FROM ids WHERE path = ? AND unavailable = 0")
                .bind(&relative_path)
                .fetch_optional(storage.pool())
                .await?
        {
            // Update signature
            sqlx::query("UPDATE ids SET signature = ? WHERE id = ?")
                .bind(&entry.signature)
                .bind(&id)
                .execute(storage.pool())
                .await?;

            return Ok(Some(id));
        }

        Ok(None)
    }

    /// Save library to cache in background task (non-blocking)
    async fn save_to_cache_background(&self) {
        // Clone data needed for background save (to satisfy 'static requirement)
        let cached_data = super::cache::CachedLibraryData {
            path: self.path.clone(),
            titles: self.titles.clone(),
        };

        // Get file manager for background save
        let file_manager = {
            let cache = self.cache.lock().await;
            if cache.stats().size_limit == 0 {
                return; // Cache disabled
            }
            cache.file_manager()
        };

        // Spawn background task to save cache (non-blocking)
        tokio::spawn(async move {
            match file_manager.save_data(cached_data).await {
                Ok(_) => tracing::info!("Library cache saved successfully in background"),
                Err(e) => tracing::warn!("Failed to save library cache in background: {}", e),
            }
        });
    }

    /// Find existing title ID from database (by path or signature)
    #[allow(dead_code)]
    async fn find_existing_id(&self, title: &Title) -> Result<Option<String>> {
        let relative_path = self.to_relative_path(&title.path)?;

        // Tier 1: Exact match (path + signature)
        if let Some(id) = sqlx::query_scalar::<_, String>(
            "SELECT id FROM titles WHERE path = ? AND signature = ? AND unavailable = 0",
        )
        .bind(&relative_path)
        .bind(&title.signature)
        .fetch_optional(self.storage.pool())
        .await?
        {
            return Ok(Some(id));
        }

        // Tier 2: Path-only match (directory modified but not moved)
        if let Some(id) = sqlx::query_scalar::<_, String>(
            "SELECT id FROM titles WHERE path = ? AND unavailable = 0",
        )
        .bind(&relative_path)
        .fetch_optional(self.storage.pool())
        .await?
        {
            // Update signature
            sqlx::query("UPDATE titles SET signature = ? WHERE id = ?")
                .bind(&title.signature)
                .bind(&id)
                .execute(self.storage.pool())
                .await?;

            return Ok(Some(id));
        }

        // Tier 3: Signature-only match (directory moved/renamed)
        // Note: Commented out for now as we don't query by signature alone for titles
        // If needed in future, add: AND unavailable = 0
        // For Week 2, we'll skip path similarity matching (add in Week 5)

        Ok(None)
    }

    /// Find existing entry ID from database
    #[allow(dead_code)]
    async fn find_existing_entry_id(&self, entry: &Entry) -> Result<Option<String>> {
        let relative_path = self.to_relative_path(&entry.path)?;

        // Tier 1: Exact match
        if let Some(id) = sqlx::query_scalar::<_, String>(
            "SELECT id FROM ids WHERE path = ? AND signature = ? AND unavailable = 0",
        )
        .bind(&relative_path)
        .bind(&entry.signature)
        .fetch_optional(self.storage.pool())
        .await?
        {
            return Ok(Some(id));
        }

        // Tier 2: Path-only match
        if let Some(id) =
            sqlx::query_scalar::<_, String>("SELECT id FROM ids WHERE path = ? AND unavailable = 0")
                .bind(&relative_path)
                .fetch_optional(self.storage.pool())
                .await?
        {
            // Update signature
            sqlx::query("UPDATE ids SET signature = ? WHERE id = ?")
                .bind(&entry.signature)
                .bind(&id)
                .execute(self.storage.pool())
                .await?;

            return Ok(Some(id));
        }

        Ok(None)
    }

    /// Persist title ID to database
    #[allow(dead_code)]
    async fn persist_title_id(&self, title: &Title) -> Result<()> {
        let relative_path = self.to_relative_path(&title.path)?;

        sqlx::query(
            "INSERT INTO titles (id, path, signature, unavailable) VALUES (?, ?, ?, 0)
             ON CONFLICT(path) DO UPDATE SET id = ?, signature = ?, unavailable = 0",
        )
        .bind(&title.id)
        .bind(&relative_path)
        .bind(&title.signature)
        .bind(&title.id)
        .bind(&title.signature)
        .execute(self.storage.pool())
        .await?;

        Ok(())
    }

    /// Persist entry ID to database
    #[allow(dead_code)]
    async fn persist_entry_id(&self, entry: &Entry) -> Result<()> {
        let relative_path = self.to_relative_path(&entry.path)?;

        sqlx::query(
            "INSERT INTO ids (id, path, signature, unavailable) VALUES (?, ?, ?, 0)
             ON CONFLICT(path) DO UPDATE SET id = ?, signature = ?, unavailable = 0",
        )
        .bind(&entry.id)
        .bind(&relative_path)
        .bind(&entry.signature)
        .bind(&entry.id)
        .bind(&entry.signature)
        .execute(self.storage.pool())
        .await?;

        Ok(())
    }

    /// Get all titles (sorted by name)
    pub fn get_titles(&self) -> Vec<&Title> {
        self.get_titles_sorted(SortMethod::default(), true)
    }

    /// Get all titles sorted by specified method
    pub fn get_titles_sorted(&self, method: SortMethod, ascending: bool) -> Vec<&Title> {
        let mut titles: Vec<&Title> = self.titles.values().collect();

        use super::{sort_by_mtime, sort_by_name};

        match method {
            SortMethod::Name | SortMethod::Progress | SortMethod::Auto => {
                // Progress sorting is handled at route level (after calculating progress with username context)
                // Auto uses name sorting (future: smart chapter detection)
                sort_by_name(&mut titles, ascending);
            }
            SortMethod::TimeModified => {
                sort_by_mtime(&mut titles, ascending);
            }
        }

        titles
    }

    /// Get all titles sorted by specified method with caching
    /// This version uses cache when username is provided
    pub async fn get_titles_sorted_cached(
        &self,
        username: &str,
        method: SortMethod,
        ascending: bool,
    ) -> Vec<&Title> {
        // Generate cache key signature from current title IDs
        let mut all_title_ids: Vec<String> = self.titles.keys().cloned().collect();
        all_title_ids.sort(); // Consistent ordering for cache key

        let sort_method_str = match method {
            SortMethod::Name => "name",
            SortMethod::TimeModified => "modified",
            SortMethod::Progress => "progress",
            SortMethod::Auto => "auto",
        };

        // Acquire lock for entire cache operation (check-compute-store)
        // This prevents TOCTOU race condition where another thread could invalidate
        // the cache between our check and our write
        let mut cache = self.cache.lock().await;
        let cache_key = super::cache::key::sorted_titles_key(
            username,
            &all_title_ids,
            sort_method_str,
            ascending,
        );

        if let Some(cached_ids) = cache.get_sorted_titles(&cache_key) {
            drop(cache); // Can drop early on cache hit

            // Build result from cached IDs
            let mut result = Vec::with_capacity(cached_ids.len());
            for id in &cached_ids {
                if let Some(title) = self.titles.get(id) {
                    result.push(title);
                }
            }
            return result;
        }

        // Cache miss - compute sort while holding lock
        // Sorting is fast (<1ms for 1000 titles), so lock contention is acceptable
        // This ensures atomicity of check-compute-store operation
        let sorted_titles = self.get_titles_sorted(method, ascending);

        // Extract IDs in sorted order
        let sorted_ids: Vec<String> = sorted_titles.iter().map(|t| t.id.clone()).collect();

        // Store result (still holding lock)
        cache.set_sorted_titles(cache_key, sorted_ids);
        drop(cache);

        sorted_titles
    }

    /// Get a specific title by ID
    pub fn get_title(&self, id: &str) -> Option<&Title> {
        self.titles.get(id)
    }

    /// Get a specific entry by title ID and entry ID
    pub fn get_entry(&self, title_id: &str, entry_id: &str) -> Option<&Entry> {
        self.titles
            .get(title_id)?
            .entries
            .iter()
            .find(|e| e.id == entry_id)
    }

    /// Get sorted entries for a title with caching
    pub async fn get_entries_sorted_cached(
        &self,
        title_id: &str,
        username: &str,
        method: SortMethod,
        ascending: bool,
    ) -> Option<Vec<&Entry>> {
        let title = self.titles.get(title_id)?;

        // Generate cache key signature from current entry IDs
        let mut all_entry_ids: Vec<String> = title.entries.iter().map(|e| e.id.clone()).collect();
        all_entry_ids.sort(); // Consistent ordering for cache key

        let sort_method_str = match method {
            SortMethod::Name => "name",
            SortMethod::TimeModified => "modified",
            SortMethod::Progress => "progress",
            SortMethod::Auto => "auto",
        };

        // Acquire lock for entire cache operation (check-compute-store)
        // This prevents TOCTOU race condition where another thread could invalidate
        // the cache between our check and our write
        let mut cache = self.cache.lock().await;
        let cache_key = super::cache::key::sorted_entries_key(
            title_id,
            username,
            &all_entry_ids,
            sort_method_str,
            ascending,
        );

        if let Some(cached_ids) = cache.get_sorted_entries(&cache_key) {
            drop(cache); // Can drop early on cache hit

            // Build result from cached IDs
            let mut result = Vec::with_capacity(cached_ids.len());
            for id in &cached_ids {
                if let Some(entry) = title.entries.iter().find(|e| e.id == *id) {
                    result.push(entry);
                }
            }
            return Some(result);
        }

        // Cache miss - compute sort while holding lock
        // Sorting is fast (<1ms for typical entry counts), so lock contention is acceptable
        // This ensures atomicity of check-compute-store operation
        let sorted_entries = title.get_entries_sorted(method, ascending);

        // Extract IDs in sorted order
        let sorted_ids: Vec<String> = sorted_entries.iter().map(|e| e.id.clone()).collect();

        // Store result (still holding lock)
        cache.set_sorted_entries(cache_key, sorted_ids);
        drop(cache);

        Some(sorted_entries)
    }

    /// Get library root path
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Invalidate cache for a title after progress update
    pub async fn invalidate_cache_for_progress(&self, title_id: &str, username: &str) {
        let mut cache = self.cache.lock().await;
        cache.invalidate_progress(title_id, username);
    }

    /// Get cache reference for admin/debug access
    pub fn cache(&self) -> &Mutex<super::cache::Cache> {
        &self.cache
    }

    /// Get progress cache reference for fast progress lookups
    pub fn progress_cache(&self) -> &super::progress_cache::ProgressCache {
        &self.progress_cache
    }

    /// Load progress data for all titles into the cache
    async fn load_progress_cache(&self) {
        let start = std::time::Instant::now();
        let mut loaded = 0;
        let mut errors = 0;

        for (title_id, title) in &self.titles {
            match self.progress_cache.load_title(title_id, &title.path).await {
                Ok(_) => loaded += 1,
                Err(e) => {
                    tracing::warn!("Failed to load progress cache for title {}: {}", title_id, e);
                    errors += 1;
                }
            }
        }

        tracing::info!(
            "Progress cache loaded: {} titles in {:.2}ms ({} errors)",
            loaded,
            start.elapsed().as_secs_f64() * 1000.0,
            errors
        );
    }

    /// Get all titles as a HashMap
    pub fn titles(&self) -> &HashMap<String, Title> {
        &self.titles
    }

    /// Get total library statistics
    pub fn stats(&self) -> LibraryStats {
        let title_count = self.titles.len();
        let entry_count: usize = self.titles.values().map(|t| t.entries.len()).sum();
        let page_count: usize = self.titles.values().map(|t| t.total_pages()).sum();

        LibraryStats {
            titles: title_count,
            entries: entry_count,
            pages: page_count,
        }
    }

    /// Mark database entries as unavailable if their files no longer exist
    /// This is called after scan completes to detect missing files
    async fn mark_unavailable(&self) -> Result<()> {
        use std::collections::HashSet;

        const CHUNK_SIZE: usize = 500; // Well under SQLite's 999 limit

        let found_title_ids: HashSet<String> = self.titles.keys().cloned().collect();
        let found_entry_ids: HashSet<String> = self
            .titles
            .values()
            .flat_map(|t| t.entries.iter().map(|e| e.id.clone()))
            .collect();

        let mut tx = self.storage.pool().begin().await?;

        // 1. Find and mark missing titles as unavailable
        let db_title_ids: Vec<String> =
            sqlx::query_scalar::<_, String>("SELECT id FROM titles WHERE unavailable = 0")
                .fetch_all(&mut *tx)
                .await?;

        let missing_titles: Vec<&String> = db_title_ids
            .iter()
            .filter(|id| !found_title_ids.contains(*id))
            .collect();

        for chunk in missing_titles.chunks(CHUNK_SIZE) {
            Self::batch_update_unavailable(&mut tx, "titles", chunk, 1).await?;
        }

        // 2. Find and mark missing entries as unavailable
        let db_entry_ids: Vec<String> =
            sqlx::query_scalar::<_, String>("SELECT id FROM ids WHERE unavailable = 0")
                .fetch_all(&mut *tx)
                .await?;

        let missing_entries: Vec<&String> = db_entry_ids
            .iter()
            .filter(|id| !found_entry_ids.contains(*id))
            .collect();

        for chunk in missing_entries.chunks(CHUNK_SIZE) {
            Self::batch_update_unavailable(&mut tx, "ids", chunk, 1).await?;
        }

        // 3. Restore previously unavailable titles that are now found
        let unavailable_titles: Vec<String> =
            sqlx::query_scalar::<_, String>("SELECT id FROM titles WHERE unavailable = 1")
                .fetch_all(&mut *tx)
                .await?;

        let restored_titles: Vec<&String> = unavailable_titles
            .iter()
            .filter(|id| found_title_ids.contains(*id))
            .collect();

        for chunk in restored_titles.chunks(CHUNK_SIZE) {
            Self::batch_update_unavailable(&mut tx, "titles", chunk, 0).await?;
        }

        // 4. Restore previously unavailable entries that are now found
        let unavailable_entries: Vec<String> =
            sqlx::query_scalar::<_, String>("SELECT id FROM ids WHERE unavailable = 1")
                .fetch_all(&mut *tx)
                .await?;

        let restored_entries: Vec<&String> = unavailable_entries
            .iter()
            .filter(|id| found_entry_ids.contains(*id))
            .collect();

        for chunk in restored_entries.chunks(CHUNK_SIZE) {
            Self::batch_update_unavailable(&mut tx, "ids", chunk, 0).await?;
        }

        // Log what we did
        if !missing_titles.is_empty() {
            tracing::info!("Marked {} titles as unavailable", missing_titles.len());
        }
        if !missing_entries.is_empty() {
            tracing::info!("Marked {} entries as unavailable", missing_entries.len());
        }
        if !restored_titles.is_empty() {
            tracing::info!("Restored {} titles as available", restored_titles.len());
        }
        if !restored_entries.is_empty() {
            tracing::info!("Restored {} entries as available", restored_entries.len());
        }

        tx.commit().await?;
        Ok(())
    }

    /// Helper: batch UPDATE with IN clause
    /// Chunks are handled by caller to respect SQLite's parameter limit
    async fn batch_update_unavailable(
        tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
        table: &str,
        ids: &[&String],
        unavailable: i32,
    ) -> Result<()> {
        if ids.is_empty() {
            return Ok(());
        }

        let placeholders = ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let query_str = format!(
            "UPDATE {} SET unavailable = {} WHERE id IN ({})",
            table, unavailable, placeholders
        );

        let mut query = sqlx::query(&query_str);
        for id in ids {
            query = query.bind(*id);
        }
        query.execute(&mut **tx).await?;
        Ok(())
    }
}

/// Sorting methods for titles and entries
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SortMethod {
    /// Sort alphabetically by name/title
    #[default]
    Name,
    /// Sort by modification time
    TimeModified,
    /// Sort by reading progress
    Progress,
    /// Smart chapter detection (future enhancement)
    Auto,
}

impl SortMethod {
    /// Parse from string parameter (for API routes)
    /// Matches original Mango API: "title", "modified", "auto"
    pub fn parse(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "title" | "name" => SortMethod::Name,
            "modified" | "time" => SortMethod::TimeModified,
            "progress" => SortMethod::Progress,
            "auto" => SortMethod::Auto,
            _ => SortMethod::default(),
        }
    }

    /// Parse sort method and ascend flag from query parameters
    /// Returns (SortMethod, bool) where bool is true for ascending
    pub fn from_params(sort: Option<&str>, ascend: Option<&str>) -> (Self, bool) {
        let method = sort.map(Self::parse).unwrap_or_default();
        let ascending = ascend
            .and_then(|s| s.parse::<i32>().ok())
            .map(|v| v != 0)
            .unwrap_or(true); // Default to ascending
        (method, ascending)
    }
}

/// Library statistics
#[derive(Debug, Clone)]
pub struct LibraryStats {
    pub titles: usize,
    pub entries: usize,
    pub pages: usize,
}

/// Create a shared Library instance that can be used across async tasks
/// Uses ArcSwap for lock-free reads and atomic swaps during scan
pub type SharedLibrary = Arc<ArcSwap<Library>>;

/// Spawn a background task that periodically scans the library
/// Uses double-buffer approach: builds new library in background, then atomically swaps
pub fn spawn_periodic_scanner(
    library: SharedLibrary,
    storage: Storage,
    config: Arc<crate::Config>,
    interval_minutes: u64,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut interval =
            tokio::time::interval(std::time::Duration::from_secs(interval_minutes * 60));

        loop {
            interval.tick().await;

            tracing::info!("Starting periodic library scan (double-buffer)");
            let periodic_start = std::time::Instant::now();

            // Build new library instance in background (no lock held)
            let mut new_lib = Library::new(config.library_path.clone(), storage.clone(), &config);

            match new_lib.scan().await {
                Ok(_) => {
                    let periodic_duration = periodic_start.elapsed();
                    let stats = new_lib.stats();

                    // Atomically swap the new library in
                    library.store(Arc::new(new_lib));

                    tracing::info!(
                        "Periodic library scan completed ({:.2}s) - {} titles, {} entries",
                        periodic_duration.as_secs_f64(),
                        stats.titles,
                        stats.entries
                    );
                }
                Err(e) => {
                    tracing::error!("Periodic scan failed: {}", e);
                    // Keep the old library on failure
                }
            }
        }
    })
}
