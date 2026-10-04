use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use arc_swap::ArcSwap;
use tokio::sync::Mutex;

use super::entry::Entry;
use super::ordering::{sort_snapshot_titles, SortMethod, SortOptions};
use super::title::Title;
use crate::error::Result;
use crate::Storage;

#[derive(Clone)]
pub struct Library {
    /// Library root directory
    pub(super) path: PathBuf,

    /// Root titles indexed by ID and shared between scan snapshots
    pub(super) titles: Arc<HashMap<String, Arc<Title>>>,

    /// Database storage for ID persistence
    pub(super) storage: Storage,

    /// Cache for sorted lists and library data
    pub(super) cache: Arc<Mutex<super::cache::Cache>>,

    /// Shared metadata persistence and immutable snapshots
    pub(super) metadata: Arc<super::metadata::MetadataStore>,
}

impl Library {
    /// Create a new Library instance
    pub fn new(path: PathBuf, storage: Storage, config: &crate::Config) -> Self {
        Self {
            path,
            titles: Arc::new(HashMap::new()),
            storage,
            cache: Arc::new(Mutex::new(super::cache::Cache::new(config))),
            metadata: Arc::new(super::metadata::MetadataStore::new()),
        }
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

                self.titles = Arc::new(
                    cached_data
                        .titles
                        .into_iter()
                        .map(|(id, title)| (id, Arc::new(title)))
                        .collect(),
                );
                let entry_count: usize = self.titles.values().map(|t| t.entries.len()).sum();

                tracing::info!(
                    "Library loaded from cache: {} titles, {} entries",
                    self.titles.len(),
                    entry_count
                );

                // Load metadata snapshots for all titles.
                super::scan::load_metadata(self, None).await;

                Ok(true)
            }
            None => {
                tracing::info!("Cache miss or invalid - will perform full scan");
                Ok(false)
            }
        }
    }

    /// Get all titles (sorted by name)
    pub fn get_titles(&self) -> Vec<&Title> {
        self.get_titles_sorted(SortMethod::default(), true)
    }

    /// Get all titles sorted by specified method
    pub fn get_titles_sorted(&self, method: SortMethod, ascending: bool) -> Vec<&Title> {
        let mut titles: Vec<&Title> = self.titles.values().map(Arc::as_ref).collect();

        sort_snapshot_titles(&mut titles, SortOptions { method, ascending });

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
            SortMethod::TimeAdded => "added",
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
                    result.push(title.as_ref());
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

    /// Get a title by ID, including descendants.
    pub fn get_title(&self, id: &str) -> Option<&Title> {
        self.titles
            .values()
            .find_map(|title| find_title(title.as_ref(), id))
    }

    /// Get all titles in tree order, including root titles.
    pub fn all_titles(&self) -> Vec<&Title> {
        let mut titles = Vec::new();
        for title in self.get_titles() {
            titles.push(title);
            titles.extend(title.deep_titles());
        }
        titles
    }

    /// Return a title's ancestors in root-to-parent order.
    pub fn parent_titles<'a>(&'a self, title: &'a Title) -> Vec<&'a Title> {
        let mut parents = Vec::new();
        let mut parent_id = title.parent_id.as_deref();
        while let Some(id) = parent_id {
            let Some(parent) = self.get_title(id) else {
                break;
            };
            parents.push(parent);
            parent_id = parent.parent_id.as_deref();
        }
        parents.reverse();
        parents
    }

    /// Get a specific entry by title ID and entry ID.
    pub fn get_entry(&self, title_id: &str, entry_id: &str) -> Option<&Entry> {
        self.get_title(title_id)?
            .entries
            .iter()
            .find(|entry| entry.id == entry_id)
    }

    /// Get library root path
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Invalidate progress-dependent title sorting for a user.
    pub async fn invalidate_cache_for_progress(&self, username: &str) {
        self.cache.lock().await.invalidate_progress(username);
    }
    /// Get cache reference for admin/debug access
    pub fn cache(&self) -> &Mutex<super::cache::Cache> {
        &self.cache
    }

    /// Metadata persistence and cached access shared by scan snapshots.
    pub fn metadata(&self) -> &super::metadata::MetadataStore {
        &self.metadata
    }

    /// Get all titles as a HashMap
    pub fn titles(&self) -> &HashMap<String, Arc<Title>> {
        &self.titles
    }

    /// Get total library statistics
    pub fn stats(&self) -> LibraryStats {
        let titles = self.all_titles();
        let title_count = titles.len();
        let entry_count: usize = titles.iter().map(|title| title.entries.len()).sum();
        let page_count: usize = titles
            .iter()
            .map(|title| title.entries.iter().map(|entry| entry.pages).sum::<usize>())
            .sum();

        LibraryStats {
            titles: title_count,
            entries: entry_count,
            pages: page_count,
        }
    }
}

fn find_title<'a>(title: &'a Title, id: &str) -> Option<&'a Title> {
    if title.id == id {
        return Some(title);
    }
    title
        .nested_titles
        .iter()
        .find_map(|nested| find_title(nested, id))
}

/// Library statistics
#[derive(Debug, Clone)]
pub struct LibraryStats {
    pub titles: usize,
    pub entries: usize,
    pub pages: usize,
}

/// Shared application library, published as immutable snapshots for lock-free reads.
pub type SharedLibrary = Arc<ArcSwap<Library>>;
