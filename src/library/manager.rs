use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use arc_swap::ArcSwap;
use tokio::sync::Mutex;

use super::entry::Entry;
use super::title::Title;
use crate::error::Result;
use crate::Storage;

/// Serialize scan-and-swap cycles so an older scan cannot replace a newer one.
pub(crate) static SCAN_LOCK: Mutex<()> = Mutex::const_new(());

struct StoredId {
    id: String,
    signature: Option<String>,
    unavailable: i64,
}

/// IDs owned by one scan worker until its completed title reaches the collector.
#[derive(Default)]
struct PendingIds {
    titles: Vec<(String, String, String)>,
    entries: Vec<(String, String, String)>,
}

struct IdIndex {
    by_path: HashMap<String, StoredId>,
    by_signature: HashMap<String, Vec<(String, String)>>,
}

impl IdIndex {
    async fn load(storage: &Storage, table: &'static str) -> Result<Self> {
        let query = match table {
            "titles" => "SELECT id, path, signature, unavailable FROM titles",
            "ids" => "SELECT id, path, signature, unavailable FROM ids",
            _ => unreachable!("ID table is selected internally"),
        };
        let rows: Vec<(String, String, Option<String>, i64)> =
            sqlx::query_as(query).fetch_all(storage.pool()).await?;
        let mut by_path = HashMap::with_capacity(rows.len());
        let mut by_signature: HashMap<String, Vec<(String, String)>> = HashMap::new();
        for (id, path, signature, unavailable) in rows {
            if let Some(signature) = &signature {
                by_signature
                    .entry(signature.clone())
                    .or_default()
                    .push((id.clone(), path.clone()));
            }
            by_path.insert(
                path,
                StoredId {
                    id,
                    signature,
                    unavailable,
                },
            );
        }
        Ok(Self {
            by_path,
            by_signature,
        })
    }

    fn find(&self, path: &str, signature: &str) -> Option<(&str, bool)> {
        if let Some(stored) = self.by_path.get(path) {
            let unchanged =
                stored.signature.as_deref() == Some(signature) && stored.unavailable == 0;
            return Some((&stored.id, !unchanged));
        }
        self.by_signature
            .get(signature)?
            .iter()
            .max_by(|(_, left), (_, right)| {
                path_component_similarity(left, path)
                    .total_cmp(&path_component_similarity(right, path))
            })
            .map(|(id, _)| (id.as_str(), true))
    }
}

#[derive(Clone)]
pub struct Library {
    /// Library root directory
    path: PathBuf,

    /// Root titles indexed by ID and shared between scan snapshots
    titles: Arc<HashMap<String, Arc<Title>>>,

    /// Database storage for ID persistence
    storage: Storage,

    /// Cache for sorted lists and library data
    cache: Arc<Mutex<super::cache::Cache>>,

    /// In-memory cache for progress data
    progress_cache: Arc<super::progress_cache::ProgressCache>,
}

impl Library {
    /// Create a new Library instance
    pub fn new(path: PathBuf, storage: Storage, config: &crate::Config) -> Self {
        Self {
            path,
            titles: Arc::new(HashMap::new()),
            storage,
            cache: Arc::new(Mutex::new(super::cache::Cache::new(config))),
            progress_cache: Arc::new(super::progress_cache::ProgressCache::new()),
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

                // Load progress cache for all titles
                self.load_progress_cache(None).await;

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
        self.scan_with_previous(None).await
    }

    pub async fn scan_with_previous(&mut self, previous: Option<Arc<Library>>) -> Result<()> {
        self.scan_inner(previous, None).await
    }

    /// Publish immutable library snapshots as root directories finish scanning.
    pub async fn scan_with_previous_and_publish(
        &mut self,
        previous: Option<Arc<Library>>,
        publisher: SharedLibrary,
    ) -> Result<()> {
        let original = publisher.load_full();
        match self
            .scan_inner(previous, Some(Arc::clone(&publisher)))
            .await
        {
            Ok(()) => Ok(()),
            Err(error) => {
                publisher.store(original);
                Err(error)
            }
        }
    }

    async fn scan_inner(
        &mut self,
        previous: Option<Arc<Library>>,
        publisher: Option<SharedLibrary>,
    ) -> Result<()> {
        if let Some(previous) = &previous {
            self.progress_cache = Arc::clone(&previous.progress_cache);
        }

        let scan_start = std::time::Instant::now();
        tracing::info!("Starting library scan: {}", self.path.display());

        // Collect all directory paths first
        let mut title_paths = Vec::new();
        let mut dir_entries = tokio::fs::read_dir(&self.path).await?;
        while let Some(entry) = dir_entries.next_entry().await? {
            let entry_path = entry.path();
            if entry_path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| !name.starts_with('.'))
                && entry_path.is_dir()
            {
                title_paths.push(entry_path);
            }
        }

        tracing::info!("Found {} directories to scan", title_paths.len());

        let mut pending_ids = PendingIds::default();
        // Most rescans need no ID lookups; load the tables only if a title changed.
        let indexes = Arc::new(tokio::sync::OnceCell::new());

        // Process titles in parallel with controlled concurrency
        let concurrency_limit = 20; // Increased from 5 to 20 for better parallelism
        let semaphore = Arc::new(tokio::sync::Semaphore::new(concurrency_limit));
        let storage = self.storage.clone();
        let library_path = self.path.clone();
        let previous_titles: HashMap<&Path, &str> = previous
            .as_deref()
            .map(|library| {
                library
                    .titles
                    .values()
                    .map(|title| (title.path.as_path(), title.id.as_str()))
                    .collect()
            })
            .unwrap_or_default();

        let root_paths: HashSet<PathBuf> = title_paths.iter().cloned().collect();
        let root_count = title_paths.len();
        let publish_batch = root_count.div_ceil(20).max(1);
        let mut tasks = tokio::task::JoinSet::new();
        for title_path in title_paths {
            let sem = semaphore.clone();
            let storage_clone = storage.clone();
            let lib_path = library_path.clone();
            let indexes = indexes.clone();
            let old_id = previous_titles
                .get(title_path.as_path())
                .map(|id| id.to_string());
            let prior = previous.as_ref().map(Arc::clone);

            tasks.spawn(async move {
                let _permit = sem.acquire().await.unwrap();

                let fingerprint_path = title_path.clone();
                let fingerprint = match tokio::task::spawn_blocking(move || {
                    super::title::calculate_contents_signature(&fingerprint_path)
                })
                .await
                {
                    Ok(Ok(signature)) => signature,
                    Ok(Err(error)) => {
                        tracing::warn!("Failed to fingerprint {}: {}", title_path.display(), error);
                        return None;
                    }
                    Err(error) => {
                        tracing::warn!(
                            "Fingerprint task failed for {}: {}",
                            title_path.display(),
                            error
                        );
                        return None;
                    }
                };
                if let (Some(prior), Some(id)) = (prior, old_id) {
                    if let Some(old_title) = prior.titles.get(&id) {
                        if old_title.contents_signature == fingerprint {
                            return Some((Arc::clone(old_title), PendingIds::default()));
                        }
                    }
                }

                let mut title = match Title::from_directory(title_path.clone()).await {
                    Ok(t) => t,
                    Err(e) => {
                        tracing::warn!("Failed to scan title at {}: {}", title_path.display(), e);
                        return None;
                    }
                };
                title.contents_signature = fingerprint;
                if title.entries.is_empty() && title.nested_titles.is_empty() {
                    return None;
                }

                let (title_index, entry_index) = match indexes
                    .get_or_try_init(|| async {
                        Ok::<_, crate::error::Error>((
                            IdIndex::load(&storage_clone, "titles").await?,
                            IdIndex::load(&storage_clone, "ids").await?,
                        ))
                    })
                    .await
                {
                    Ok(indexes) => indexes,
                    Err(error) => {
                        tracing::warn!("Failed to load ID index: {}", error);
                        return None;
                    }
                };
                let mut pending_ids = PendingIds::default();
                if let Err(error) = Box::pin(Self::assign_title_tree_ids(
                    &mut title,
                    None,
                    &lib_path,
                    &storage_clone,
                    title_index,
                    entry_index,
                    &mut pending_ids,
                ))
                .await
                {
                    tracing::warn!(
                        "Failed to assign IDs for {}: {}",
                        title_path.display(),
                        error
                    );
                    return None;
                }

                Some((Arc::new(title), pending_ids))
            });
        }

        // Collect results as tasks complete so progress reflects actual work,
        // not the launch order of root directories.
        let mut new_titles = HashMap::new();
        let mut partial_titles: Option<HashMap<String, Arc<Title>>> = None;
        let mut completed_roots = 0;
        let mut published_roots = 0;
        let mut progress_interval = tokio::time::interval(std::time::Duration::from_secs(10));
        progress_interval.tick().await;
        while completed_roots < root_count {
            let mut publish_tick = false;
            tokio::select! {
                joined = tasks.join_next() => {
                    let Some(joined) = joined else {
                        break;
                    };
                    completed_roots += 1;
                    match joined {
                        Ok(Some((title, ids))) => {
                            pending_ids.titles.extend(ids.titles);
                            pending_ids.entries.extend(ids.entries);
                            new_titles.insert(title.id.clone(), title);
                        }
                        Ok(None) => {}
                        Err(error) => {
                            tracing::warn!("Library scan task failed: {}", error);
                        }
                    }
                }
                _ = progress_interval.tick() => {
                    tracing::info!(
                        "Library scan progress: {}/{} root directories completed ({:.1}s)",
                        completed_roots,
                        root_count,
                        scan_start.elapsed().as_secs_f64()
                    );
                    publish_tick = true;
                }
            }
            if let Some(publisher) = &publisher {
                if completed_roots > published_roots
                    && (publish_tick || completed_roots - published_roots >= publish_batch)
                {
                    let partial = partial_titles.get_or_insert_with(|| {
                        let mut titles = previous
                            .as_deref()
                            .map(|library| library.titles.as_ref().clone())
                            .unwrap_or_default();
                        titles.retain(|_, title| root_paths.contains(&title.path));
                        titles
                    });
                    partial.extend(
                        new_titles
                            .iter()
                            .map(|(id, title)| (id.clone(), Arc::clone(title))),
                    );
                    if !pending_ids.titles.is_empty() || !pending_ids.entries.is_empty() {
                        self.bulk_insert_ids(&pending_ids.titles, &pending_ids.entries)
                            .await?;
                        pending_ids.titles.clear();
                        pending_ids.entries.clear();
                    }

                    self.cache.lock().await.clear();
                    self.titles = Arc::new(partial.clone());
                    publisher.store(Arc::new(self.clone()));
                    published_roots = completed_roots;
                }
            }
        }

        let title_count: usize = new_titles
            .values()
            .map(|title| 1 + title.deep_titles().len())
            .sum();
        let entry_count: usize = new_titles
            .values()
            .map(|title| title.deep_entries().len())
            .sum();
        // Bulk insert all new IDs in a single transaction

        if !pending_ids.titles.is_empty() || !pending_ids.entries.is_empty() {
            self.bulk_insert_ids(&pending_ids.titles, &pending_ids.entries)
                .await?;
            tracing::info!(
                "Bulk inserted {} new titles and {} new entries to database",
                pending_ids.titles.len(),
                pending_ids.entries.len()
            );
        }

        self.titles = Arc::new(new_titles);

        // Load progress cache for all titles
        self.load_progress_cache(previous.as_deref()).await;

        // Mark items in database as unavailable if not found during scan
        self.mark_unavailable().await?;
        if let Some(publisher) = publisher {
            self.cache.lock().await.clear();
            publisher.store(Arc::new(self.clone()));
        }

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

    async fn assign_title_tree_ids(
        title: &mut Title,
        parent_id: Option<String>,
        library_path: &Path,
        storage: &Storage,
        title_index: &IdIndex,
        entry_index: &IdIndex,
        pending_ids: &mut PendingIds,
    ) -> Result<()> {
        title.parent_id = parent_id;
        if let Some(id) =
            Self::find_existing_title_id(library_path, title, storage, title_index).await?
        {
            title.id = id;
        } else {
            let relative_path = title
                .path
                .strip_prefix(library_path)
                .map_err(|_| {
                    crate::error::Error::Internal(format!(
                        "Path {} is not within library root {}",
                        title.path.display(),
                        library_path.display()
                    ))
                })?
                .to_string_lossy()
                .to_string();
            pending_ids
                .titles
                .push((title.id.clone(), relative_path, title.signature.clone()));
        }

        for entry in &mut title.entries {
            if let Some(id) =
                Self::find_existing_entry_id(library_path, entry, storage, entry_index).await?
            {
                entry.id = id;
            } else {
                let relative_path = entry
                    .path
                    .strip_prefix(library_path)
                    .map_err(|_| {
                        crate::error::Error::Internal(format!(
                            "Path {} is not within library root {}",
                            entry.path.display(),
                            library_path.display()
                        ))
                    })?
                    .to_string_lossy()
                    .to_string();
                pending_ids.entries.push((
                    entry.id.clone(),
                    relative_path,
                    entry.signature.clone(),
                ));
            }
        }

        let parent_id = title.id.clone();
        for nested in &mut title.nested_titles {
            Box::pin(Self::assign_title_tree_ids(
                nested,
                Some(parent_id.clone()),
                library_path,
                storage,
                title_index,
                entry_index,
                pending_ids,
            ))
            .await?;
        }
        Ok(())
    }

    async fn find_existing_title_id(
        library_path: &Path,
        title: &Title,
        storage: &Storage,
        index: &IdIndex,
    ) -> Result<Option<String>> {
        let relative_path = title
            .path
            .strip_prefix(library_path)
            .map_err(|_| {
                crate::error::Error::Internal(format!(
                    "Path {} is not within library root {}",
                    title.path.display(),
                    library_path.display()
                ))
            })?
            .to_string_lossy()
            .to_string();

        Self::find_existing_id("titles", &relative_path, &title.signature, storage, index).await
    }

    async fn find_existing_entry_id(
        library_path: &Path,
        entry: &Entry,
        storage: &Storage,
        index: &IdIndex,
    ) -> Result<Option<String>> {
        let relative_path = entry
            .path
            .strip_prefix(library_path)
            .map_err(|_| {
                crate::error::Error::Internal(format!(
                    "Path {} is not within library root {}",
                    entry.path.display(),
                    library_path.display()
                ))
            })?
            .to_string_lossy()
            .to_string();

        Self::find_existing_id("ids", &relative_path, &entry.signature, storage, index).await
    }

    async fn find_existing_id(
        table: &'static str,
        path: &str,
        signature: &str,
        storage: &Storage,
        index: &IdIndex,
    ) -> Result<Option<String>> {
        let Some((id, should_update)) = index.find(path, signature) else {
            return Ok(None);
        };
        if should_update {
            let update_query = match table {
                "titles" => {
                    "UPDATE titles SET path = ?, signature = ?, unavailable = 0 WHERE id = ?"
                }
                "ids" => "UPDATE ids SET path = ?, signature = ?, unavailable = 0 WHERE id = ?",
                _ => unreachable!("ID table is selected internally"),
            };
            sqlx::query(update_query)
                .bind(path)
                .bind(signature)
                .bind(id)
                .execute(storage.pool())
                .await?;
        }
        Ok(Some(id.to_owned()))
    }

    /// Save library to cache in background task (non-blocking)
    async fn save_to_cache_background(&self) {
        let file_manager = {
            let cache = self.cache.lock().await;
            if cache.stats().size_limit == 0 {
                return;
            }
            cache.file_manager()
        };
        let path = self.path.clone();
        let titles = Arc::clone(&self.titles);
        tokio::spawn(async move {
            match file_manager.save_shared(&path, &titles).await {
                Ok(_) => tracing::info!("Library cache saved successfully in background"),
                Err(e) => tracing::warn!("Failed to save library cache in background: {}", e),
            }
        });
    }

    /// Get all titles (sorted by name)
    pub fn get_titles(&self) -> Vec<&Title> {
        self.get_titles_sorted(SortMethod::default(), true)
    }

    /// Get all titles sorted by specified method
    pub fn get_titles_sorted(&self, method: SortMethod, ascending: bool) -> Vec<&Title> {
        let mut titles: Vec<&Title> = self.titles.values().map(Arc::as_ref).collect();

        use super::{sort_by_mtime, sort_by_name};

        match method {
            SortMethod::TimeModified => sort_by_mtime(&mut titles, ascending),
            SortMethod::Name | SortMethod::TimeAdded | SortMethod::Progress | SortMethod::Auto => {
                // Mango falls back to title order when time_added is used for titles.
                sort_by_name(&mut titles, ascending);
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

    /// Get progress cache reference for fast progress lookups
    pub fn progress_cache(&self) -> &super::progress_cache::ProgressCache {
        &self.progress_cache
    }

    /// Load progress data for all titles into the cache
    async fn load_progress_cache(&self, previous: Option<&Library>) {
        let start = std::time::Instant::now();
        let mut loaded = 0;
        let mut errors = 0;

        for (title_id, title) in self.titles.iter() {
            if previous
                .and_then(|library| library.titles.get(title_id))
                .is_some_and(|old| {
                    old.path == title.path && old.contents_signature == title.contents_signature
                })
            {
                continue;
            }
            if let Err(error) = title.populate_date_added().await {
                tracing::warn!(
                    "Failed to align info.json for title {}: {}",
                    title_id,
                    error
                );
                errors += 1;
            }
        }

        // Distribute independent info.json reads across workers. A missing
        // info.json returns without yielding, so sequential async reads make
        // the scan pay filesystem latency once per title.
        const PROGRESS_WORKERS: usize = 20;
        let mut workers: Vec<Vec<(String, PathBuf)>> =
            (0..PROGRESS_WORKERS).map(|_| Vec::new()).collect();
        for (index, title) in self.all_titles().into_iter().enumerate() {
            workers[index % PROGRESS_WORKERS].push((title.id.clone(), title.path.clone()));
        }
        let mut tasks = tokio::task::JoinSet::new();
        for titles in workers.into_iter().filter(|titles| !titles.is_empty()) {
            let progress_cache = Arc::clone(&self.progress_cache);
            tasks.spawn(async move {
                let mut loaded = 0;
                let mut errors = 0;
                for (title_id, path) in titles {
                    match progress_cache.load_title(&title_id, &path).await {
                        Ok(()) => loaded += 1,
                        Err(error) => {
                            tracing::warn!(
                                "Failed to load progress cache for title {}: {}",
                                title_id,
                                error
                            );
                            errors += 1;
                        }
                    }
                }
                (loaded, errors)
            });
        }
        while let Some(result) = tasks.join_next().await {
            match result {
                Ok((worker_loaded, worker_errors)) => {
                    loaded += worker_loaded;
                    errors += worker_errors;
                }
                Err(error) => {
                    tracing::warn!("Progress cache worker failed: {}", error);
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

    /// Mark database entries as unavailable if their files no longer exist
    /// This is called after scan completes to detect missing files
    async fn mark_unavailable(&self) -> Result<()> {
        use std::collections::HashSet;

        const CHUNK_SIZE: usize = 500; // Well under SQLite's 999 limit

        let all_titles = self.all_titles();
        let found_title_ids: HashSet<String> =
            all_titles.iter().map(|title| title.id.clone()).collect();
        let found_entry_ids: HashSet<String> = all_titles
            .iter()
            .flat_map(|title| title.entries.iter().map(|entry| entry.id.clone()))
            .collect();

        let mut tx = self.storage.pool().begin().await?;

        // 1. Find and mark missing titles as unavailable
        let db_titles: Vec<String> =
            sqlx::query_scalar("SELECT id FROM titles WHERE unavailable = 0")
                .fetch_all(&mut *tx)
                .await?;
        let missing_titles: Vec<&String> = db_titles
            .iter()
            .filter(|id| !found_title_ids.contains(*id))
            .collect();

        for chunk in missing_titles.chunks(CHUNK_SIZE) {
            Self::batch_update_unavailable(&mut tx, "titles", chunk, 1).await?;
        }

        // 2. Mark missing entries, including entries beneath removed titles.
        let db_entries: Vec<String> =
            sqlx::query_scalar("SELECT id FROM ids WHERE unavailable = 0")
                .fetch_all(&mut *tx)
                .await?;
        let missing_entries: Vec<&String> = db_entries
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

fn find_title<'a>(title: &'a Title, id: &str) -> Option<&'a Title> {
    if title.id == id {
        return Some(title);
    }
    title
        .nested_titles
        .iter()
        .find_map(|nested| find_title(nested, id))
}

/// Sorting methods for titles and entries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SortMethod {
    /// Sort alphabetically by name/title.
    Name,
    /// Sort by modification time.
    TimeModified,
    /// Sort by added time.
    TimeAdded,
    /// Sort by reading progress.
    Progress,
    /// Smart chapter detection.
    #[default]
    Auto,
}

impl SortMethod {
    /// Parse from string parameter (for API routes)
    /// Matches original Mango API: "title", "modified", "auto"
    pub fn parse(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "title" | "name" => SortMethod::Name,
            "modified" | "time" | "time_modified" => SortMethod::TimeModified,
            "added" | "time_added" => SortMethod::TimeAdded,
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

/// Shared application library, published as immutable snapshots for lock-free reads.
pub type SharedLibrary = Arc<ArcSwap<Library>>;

/// Spawn a background task that periodically scans and publishes completed roots.
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
            let _scan_guard = SCAN_LOCK.lock().await;

            tracing::info!("Starting periodic library scan");
            let periodic_start = std::time::Instant::now();

            // Build new library instance in background (no lock held)
            let mut new_lib = Library::new(config.library_path.clone(), storage.clone(), &config);

            let previous = library.load_full();
            match new_lib
                .scan_with_previous_and_publish(Some(previous), Arc::clone(&library))
                .await
            {
                Ok(_) => {
                    let periodic_duration = periodic_start.elapsed();
                    let stats = new_lib.stats();

                    // Publish the completed library snapshot.
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
fn path_component_similarity(left: &str, right: &str) -> f64 {
    let left = Path::new(left);
    let right = Path::new(right);
    let component_count = left.components().count().min(right.components().count());
    if component_count == 0 {
        return 0.0;
    }

    let matching_components = left
        .components()
        .rev()
        .zip(right.components().rev())
        .filter(|(left, right)| left == right)
        .count();
    matching_components as f64 / component_count as f64
}

#[cfg(test)]
mod sort_method_tests {
    use super::SortMethod;

    #[test]
    fn parses_mango_date_added_sort_name() {
        assert_eq!(SortMethod::parse("time_added"), SortMethod::TimeAdded);
        assert_eq!(SortMethod::parse("added"), SortMethod::TimeAdded);
    }
}

#[cfg(test)]
mod path_similarity_tests {
    use super::path_component_similarity;

    #[test]
    fn moved_path_prefers_matching_trailing_components() {
        let moved_chapter =
            path_component_similarity("old/volume-1/chapter-2.cbz", "new/volume-1/chapter-2.cbz");
        let other_chapter =
            path_component_similarity("old/chapter-2/chapter-2.cbz", "new/volume-1/chapter-2.cbz");

        assert!(moved_chapter > other_chapter);
    }
}

#[cfg(test)]
mod scan_regression_tests {
    use super::Library;
    use crate::{Config, Storage};
    use std::sync::Arc;

    #[tokio::test]
    async fn rescan_detects_changes_preserves_ids_and_publishes_partial_roots() {
        let temp = tempfile::tempdir().unwrap();
        let library_path = temp.path().join("library");
        let title_path = library_path.join("Series");
        std::fs::create_dir_all(&title_path).unwrap();
        let archive_path = title_path.join("Chapter 1.cbz");
        let bad_archive = b"not a valid archive";
        std::fs::write(&archive_path, bad_archive).unwrap();

        let db_path = temp.path().join("test.db");
        std::fs::File::create(&db_path).unwrap();
        let storage = Storage::new(&format!("sqlite://{}", db_path.display()))
            .await
            .unwrap();
        let config = Config {
            host: "127.0.0.1".to_string(),
            port: 9000,
            base_url: "/".to_string(),
            session_secret: "test".to_string(),
            library_path: library_path.clone(),
            db_path,
            queue_db_path: temp.path().join("queue.db"),
            scan_interval_minutes: 0,
            thumbnail_generation_interval_hours: 0,
            log_level: "info".to_string(),
            upload_path: temp.path().join("uploads"),
            plugin_path: temp.path().join("plugins"),
            download_timeout_seconds: 30,
            library_cache_path: temp.path().join("library-cache.bin"),
            cache_enabled: false,
            cache_size_mbs: 0,
            cache_log_enabled: false,
            disable_login: false,
            default_username: String::new(),
            auth_proxy_header_name: String::new(),
            plugin_update_interval_hours: 24,
        };
        let mut library = Library::new(library_path.clone(), storage.clone(), &config);
        library.scan().await.unwrap();
        let title = library.get_titles()[0];
        assert_eq!(title.entries.len(), 1);
        let entry = &title.entries[0];
        assert_eq!(entry.path, archive_path);
        assert_eq!(entry.title, "Chapter 1");
        assert_eq!(entry.size_bytes, bad_archive.len() as u64);
        assert_eq!(entry.pages, 0);
        assert!(entry.image_files.is_empty());
        assert!(entry
            .err_msg
            .as_deref()
            .unwrap()
            .starts_with("Archive error:"));
        let original_id = entry.id.clone();

        // The persistent cache uses MessagePack to serialize the complete title tree.
        let serialized = rmp_serde::to_vec(title).unwrap();
        let cached: crate::library::Title = rmp_serde::from_slice(&serialized).unwrap();
        assert_eq!(cached.entries[0].err_msg, entry.err_msg);
        assert_eq!(cached.entries[0].id, original_id);

        let library = Arc::new(library);
        let mut rescanned = Library::new(library_path.clone(), storage.clone(), &config);
        rescanned
            .scan_with_previous(Some(Arc::clone(&library)))
            .await
            .unwrap();
        let entry = &rescanned.get_titles()[0].entries[0];
        assert_eq!(entry.id, original_id);
        assert!(entry.err_msg.is_some());
        assert_eq!(entry.pages, 0);

        // An out-of-band info.json edit must be visible after an unchanged scan.
        let mut info = crate::library::progress::TitleInfo {
            display_name: "Updated series".to_string(),
            ..Default::default()
        };
        info.set_progress("reader", "Chapter 1", 4);
        info.save(&title_path).await.unwrap();
        let rescanned = Arc::new(rescanned);
        let mut refreshed = Library::new(library_path.clone(), storage.clone(), &config);
        refreshed.scan_with_previous(Some(rescanned)).await.unwrap();
        assert_eq!(
            refreshed
                .progress_cache()
                .get_display_name(&library.get_titles()[0].id),
            Some("Updated series".to_string())
        );
        assert_eq!(
            refreshed.progress_cache().get_progress(
                &library.get_titles()[0].id,
                "reader",
                "Chapter 1"
            ),
            Some(4)
        );

        // Changes inside a nested directory must invalidate the parent title.
        let nested = title_path.join("Volume");
        let pages = nested.join("Pages");
        std::fs::create_dir_all(&pages).unwrap();
        std::fs::write(pages.join("001.png"), b"page").unwrap();
        let refreshed = Arc::new(refreshed);
        let mut changed = Library::new(library_path.clone(), storage.clone(), &config);
        changed.scan_with_previous(Some(refreshed)).await.unwrap();
        let title = changed.get_titles()[0];
        assert_eq!(title.id, library.get_titles()[0].id);
        assert_eq!(title.nested_titles[0].entries[0].pages, 1);
        assert_eq!(title.entries[0].id, original_id);

        let changed = Arc::new(changed);
        std::fs::write(&archive_path, b"longer invalid archive").unwrap();
        let mut replaced = Library::new(library_path.clone(), storage.clone(), &config);
        replaced.scan_with_previous(Some(changed)).await.unwrap();
        assert_eq!(replaced.get_titles()[0].entries[0].size_bytes, 22);
        assert_eq!(replaced.get_titles()[0].entries[0].id, original_id);

        let replaced = Arc::new(replaced);
        std::fs::remove_file(pages.join("001.png")).unwrap();
        let mut removed = Library::new(library_path.clone(), storage.clone(), &config);
        removed.scan_with_previous(Some(replaced)).await.unwrap();
        assert!(removed.get_titles()[0].nested_titles.is_empty());

        let removed = Arc::new(removed);
        let renamed_path = title_path.join("Chapter 2.cbz");
        std::fs::rename(&archive_path, &renamed_path).unwrap();
        let mut renamed = Library::new(library_path.clone(), storage.clone(), &config);
        renamed.scan_with_previous(Some(removed)).await.unwrap();
        assert_eq!(renamed.get_titles()[0].entries[0].title, "Chapter 2");
        assert_eq!(renamed.get_titles()[0].entries[0].id, original_id);
        let preserved_title_id = renamed.get_titles()[0].id.clone();

        let previous = Arc::new(renamed);
        for index in 0..100 {
            let title_path = library_path.join(format!("New {index:03}"));
            std::fs::create_dir_all(&title_path).unwrap();
            std::fs::write(title_path.join("Chapter.cbz"), b"invalid archive").unwrap();
        }
        let shared = Arc::new(arc_swap::ArcSwap::from(Arc::clone(&previous)));
        let publisher = Arc::clone(&shared);
        let scan_path = library_path.clone();
        let scan_storage = storage.clone();
        let scan_config = config.clone();
        let scan_task = tokio::spawn(async move {
            let mut scanning = Library::new(scan_path, scan_storage, &scan_config);
            scanning
                .scan_with_previous_and_publish(Some(previous), publisher)
                .await
                .unwrap();
        });

        let mut observed_partial_scan = false;
        while !scan_task.is_finished() {
            let current = shared.load();
            let visible_titles = current.get_titles().len();
            let preserved_title_is_visible = current.get_title(&preserved_title_id).is_some();
            drop(current);
            if (1..101).contains(&visible_titles) && preserved_title_is_visible {
                observed_partial_scan = true;
                break;
            }
            tokio::task::yield_now().await;
        }
        scan_task.await.unwrap();
        assert!(
            observed_partial_scan,
            "completed roots should be visible before the scan finishes"
        );
    }
}
