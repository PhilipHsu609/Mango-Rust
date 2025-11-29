use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use tokio::sync::{Mutex, RwLock};

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
}

impl Library {
    /// Create a new Library instance
    pub fn new(path: PathBuf, storage: Storage, config: &crate::Config) -> Self {
        Self {
            path,
            titles: HashMap::new(),
            storage,
            cache: Mutex::new(super::cache::Cache::new(config)),
        }
    }

    /// Convert absolute path to relative path (relative to library root)
    /// Example: "/home/user/library/Series/Chapter.zip" -> "Series/Chapter.zip"
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
        let db_title_count = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM titles WHERE unavailable = 0",
        )
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
                Ok(true)
            }
            None => {
                tracing::info!("Cache miss or invalid - will perform full scan");
                Ok(false)
            }
        }
    }

    /// Scan the library directory for manga titles
    pub async fn scan(&mut self) -> Result<()> {
        tracing::info!("Starting library scan: {}", self.path.display());

        let mut new_titles = HashMap::new();
        let mut dir_entries = tokio::fs::read_dir(&self.path).await?;

        while let Some(entry) = dir_entries.next_entry().await? {
            let entry_path = entry.path();

            if entry_path.is_dir() {
                // Each top-level directory is a manga title (series)
                match Title::from_directory(entry_path.clone()).await {
                    Ok(mut title) => {
                        // Try to match with existing ID from database
                        if let Some(existing_id) = self.find_existing_id(&title).await? {
                            title.id = existing_id;
                            tracing::debug!(
                                "Matched existing title: {} ({})",
                                title.title,
                                title.id
                            );
                        } else {
                            // New title, persist ID to database
                            self.persist_title_id(&title).await?;
                            tracing::info!("Discovered new title: {} ({})", title.title, title.id);
                        }

                        // Persist entry IDs
                        for entry in &mut title.entries {
                            if let Some(existing_id) = self.find_existing_entry_id(entry).await? {
                                entry.id = existing_id;
                            } else {
                                self.persist_entry_id(entry).await?;
                                tracing::debug!("  New entry: {} ({})", entry.title, entry.id);
                            }
                        }

                        // Populate date_added for newly discovered entries
                        if let Err(e) = title.populate_date_added().await {
                            tracing::warn!(
                                "Failed to populate date_added for {}: {}",
                                title.title,
                                e
                            );
                        }

                        new_titles.insert(title.id.clone(), title);
                    }
                    Err(e) => {
                        tracing::warn!("Failed to scan title at {}: {}", entry_path.display(), e);
                    }
                }
            }
        }

        let title_count = new_titles.len();
        let entry_count: usize = new_titles.values().map(|t| t.entries.len()).sum();

        self.titles = new_titles;

        // Mark items in database as unavailable if not found during scan
        self.mark_unavailable().await?;

        tracing::info!(
            "Library scan complete: {} titles, {} entries",
            title_count,
            entry_count
        );

        // Save library to cache in background (non-blocking)
        self.save_to_cache_background().await;

        Ok(())
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
    async fn find_existing_id(&self, title: &Title) -> Result<Option<String>> {
        let relative_path = self.to_relative_path(&title.path)?;

        // Tier 1: Exact match (path + signature)
        if let Some(id) = sqlx::query_scalar::<_, String>(
            "SELECT id FROM titles WHERE path = ? AND signature = ? AND unavailable = 0"
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
    async fn find_existing_entry_id(&self, entry: &Entry) -> Result<Option<String>> {
        let relative_path = self.to_relative_path(&entry.path)?;

        // Tier 1: Exact match
        if let Some(id) = sqlx::query_scalar::<_, String>(
            "SELECT id FROM ids WHERE path = ? AND signature = ? AND unavailable = 0"
        )
        .bind(&relative_path)
        .bind(&entry.signature)
        .fetch_optional(self.storage.pool())
        .await?
        {
            return Ok(Some(id));
        }

        // Tier 2: Path-only match
        if let Some(id) = sqlx::query_scalar::<_, String>(
            "SELECT id FROM ids WHERE path = ? AND unavailable = 0",
        )
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
    async fn persist_title_id(&self, title: &Title) -> Result<()> {
        let relative_path = self.to_relative_path(&title.path)?;

        sqlx::query(
            "INSERT INTO titles (id, path, signature) VALUES (?, ?, ?)
             ON CONFLICT(id) DO UPDATE SET path = ?, signature = ?",
        )
        .bind(&title.id)
        .bind(&relative_path)
        .bind(&title.signature)
        .bind(&relative_path)
        .bind(&title.signature)
        .execute(self.storage.pool())
        .await?;

        Ok(())
    }

    /// Persist entry ID to database
    async fn persist_entry_id(&self, entry: &Entry) -> Result<()> {
        let relative_path = self.to_relative_path(&entry.path)?;

        sqlx::query(
            "INSERT INTO ids (path, id, signature) VALUES (?, ?, ?)
             ON CONFLICT(id) DO UPDATE SET path = ?, signature = ?",
        )
        .bind(&relative_path)
        .bind(&entry.id)
        .bind(&entry.signature)
        .bind(&relative_path)
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

        // Try to get cached sorted list
        let mut cache = self.cache.lock().await;
        let cache_key = super::cache::key::sorted_titles_key(
            username,
            &all_title_ids,
            sort_method_str,
            ascending,
        );

        if let Some(cached_ids) = cache.get_sorted_titles(&cache_key) {
            drop(cache); // Release lock before building result

            // Build result from cached IDs
            let mut result = Vec::with_capacity(cached_ids.len());
            for id in &cached_ids {
                if let Some(title) = self.titles.get(id) {
                    result.push(title);
                }
            }
            return result;
        }

        drop(cache); // Release lock before sorting

        // Cache miss - compute sort
        let sorted_titles = self.get_titles_sorted(method, ascending);

        // Extract IDs in sorted order
        let sorted_ids: Vec<String> = sorted_titles.iter().map(|t| t.id.clone()).collect();

        // Cache the sorted IDs
        let mut cache = self.cache.lock().await;
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

        // Try to get cached sorted list
        let mut cache = self.cache.lock().await;
        let cache_key = super::cache::key::sorted_entries_key(
            title_id,
            username,
            &all_entry_ids,
            sort_method_str,
            ascending,
        );

        if let Some(cached_ids) = cache.get_sorted_entries(&cache_key) {
            drop(cache); // Release lock before building result

            // Build result from cached IDs
            let mut result = Vec::with_capacity(cached_ids.len());
            for id in &cached_ids {
                if let Some(entry) = title.entries.iter().find(|e| e.id == *id) {
                    result.push(entry);
                }
            }
            return Some(result);
        }

        drop(cache); // Release lock before sorting

        // Cache miss - compute sort
        let sorted_entries = title.get_entries_sorted(method, ascending);

        // Extract IDs in sorted order
        let sorted_ids: Vec<String> = sorted_entries.iter().map(|e| e.id.clone()).collect();

        // Cache the sorted IDs
        let mut cache = self.cache.lock().await;
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

        // Collect IDs of all found titles
        let found_title_ids: HashSet<String> = self.titles.keys().cloned().collect();

        // Collect IDs of all found entries
        let found_entry_ids: HashSet<String> = self
            .titles
            .values()
            .flat_map(|title| title.entries.iter().map(|e| e.id.clone()))
            .collect();

        // Query all title IDs from database where unavailable = 0
        let all_title_ids: Vec<String> = sqlx::query_scalar::<_, String>(
            "SELECT id FROM titles WHERE unavailable = 0",
        )
        .fetch_all(self.storage.pool())
        .await?;

        // Query all entry IDs from database where unavailable = 0
        let all_entry_ids: Vec<String> = sqlx::query_scalar::<_, String>(
            "SELECT id FROM ids WHERE unavailable = 0",
        )
        .fetch_all(self.storage.pool())
        .await?;

        // Find titles that are in DB but not found during scan
        let missing_title_ids: Vec<String> = all_title_ids
            .into_iter()
            .filter(|id| !found_title_ids.contains(id))
            .collect();

        // Find entries that are in DB but not found during scan
        let missing_entry_ids: Vec<String> = all_entry_ids
            .into_iter()
            .filter(|id| !found_entry_ids.contains(id))
            .collect();

        if !missing_title_ids.is_empty() {
            tracing::info!("Marking {} titles as unavailable", missing_title_ids.len());

            // Mark titles as unavailable
            for id in missing_title_ids {
                sqlx::query("UPDATE titles SET unavailable = 1 WHERE id = ?")
                    .bind(&id)
                    .execute(self.storage.pool())
                    .await?;
            }
        }

        if !missing_entry_ids.is_empty() {
            tracing::info!("Marking {} entries as unavailable", missing_entry_ids.len());

            // Mark entries as unavailable
            for id in missing_entry_ids {
                sqlx::query("UPDATE ids SET unavailable = 1 WHERE id = ?")
                    .bind(&id)
                    .execute(self.storage.pool())
                    .await?;
            }
        }

        // Mark titles as available if they were previously unavailable but now found
        for id in found_title_ids {
            sqlx::query("UPDATE titles SET unavailable = 0 WHERE id = ? AND unavailable = 1")
                .bind(&id)
                .execute(self.storage.pool())
                .await?;
        }

        // Mark entries as available if they were previously unavailable but now found
        for id in found_entry_ids {
            sqlx::query("UPDATE ids SET unavailable = 0 WHERE id = ? AND unavailable = 1")
                .bind(&id)
                .execute(self.storage.pool())
                .await?;
        }

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
pub type SharedLibrary = Arc<RwLock<Library>>;

/// Spawn a background task that periodically scans the library
pub fn spawn_periodic_scanner(
    library: SharedLibrary,
    interval_minutes: u64,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut interval =
            tokio::time::interval(std::time::Duration::from_secs(interval_minutes * 60));

        loop {
            interval.tick().await;

            tracing::info!("Starting periodic library scan");
            let mut lib = library.write().await;
            if let Err(e) = lib.scan().await {
                tracing::error!("Periodic scan failed: {}", e);
            }
        }
    })
}
