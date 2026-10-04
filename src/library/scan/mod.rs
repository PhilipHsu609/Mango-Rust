mod discovery;
mod filesystem;
mod ids;

pub(crate) use filesystem::date_added_timestamp;

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use tokio::sync::Mutex;

use crate::error::Result;
use crate::library::{Library, SharedLibrary, Title};
use crate::Storage;
use ids::{IdIndex, PendingIds};

/// Serialize scan-and-swap cycles so an older scan cannot replace a newer one.
pub(crate) static SCAN_LOCK: Mutex<()> = Mutex::const_new(());

/// Scan a replacement snapshot, optionally publishing completed roots incrementally.
/// Callers serialize scan-and-swap cycles with `SCAN_LOCK`. A failed scan restores
/// the original published snapshot; dropping the future cancels its scan workers.
pub async fn scan(
    library: &mut Library,
    previous: Option<Arc<Library>>,
    publisher: Option<SharedLibrary>,
) -> Result<()> {
    let original = publisher.as_ref().map(|publisher| publisher.load_full());
    match scan_inner(library, previous, publisher.as_ref()).await {
        Ok(()) => Ok(()),
        Err(error) => {
            if let (Some(publisher), Some(original)) = (publisher, original) {
                publisher.store(original);
            }
            Err(error)
        }
    }
}

async fn scan_inner(
    library: &mut Library,
    previous: Option<Arc<Library>>,
    publisher: Option<&SharedLibrary>,
) -> Result<()> {
    if let Some(previous) = &previous {
        library.metadata = Arc::clone(&previous.metadata);
    }

    let scan_start = std::time::Instant::now();
    tracing::info!("Starting library scan: {}", library.path.display());

    // Collect all directory paths first
    let mut title_paths = Vec::new();
    let mut dir_entries = tokio::fs::read_dir(&library.path).await?;
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
    let storage = library.storage.clone();
    let library_path = library.path.clone();
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
                filesystem::calculate_contents_signature(&fingerprint_path)
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

            let mut title = match discovery::title_from_directory(title_path.clone()).await {
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
            if let Err(error) = Box::pin(ids::assign_title_tree_ids(
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
                    ids::bulk_insert_ids(
                        &library.storage,
                        &pending_ids.titles,
                        &pending_ids.entries,
                    )
                    .await?;
                    pending_ids.titles.clear();
                    pending_ids.entries.clear();
                }

                library.cache.lock().await.clear();
                library.titles = Arc::new(partial.clone());
                publisher.store(Arc::new(library.clone()));
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
        ids::bulk_insert_ids(&library.storage, &pending_ids.titles, &pending_ids.entries).await?;
        tracing::info!(
            "Bulk inserted {} new titles and {} new entries to database",
            pending_ids.titles.len(),
            pending_ids.entries.len()
        );
    }

    library.titles = Arc::new(new_titles);

    // Refresh metadata snapshots for all titles.
    load_metadata(library, previous.as_deref()).await;

    // Mark items in database as unavailable if not found during scan
    ids::mark_unavailable(library).await?;
    if let Some(publisher) = publisher {
        library.cache.lock().await.clear();
        publisher.store(Arc::new(library.clone()));
    }

    let scan_duration = scan_start.elapsed();
    tracing::info!(
        "Library scan complete: {} titles, {} entries ({:.2}s)",
        title_count,
        entry_count,
        scan_duration.as_secs_f64()
    );

    // Save library to cache in background (non-blocking)
    save_to_cache_background(library).await;

    Ok(())
}

/// Save library to cache in background task (non-blocking)
async fn save_to_cache_background(library: &Library) {
    let file_manager = {
        let cache = library.cache.lock().await;
        if cache.stats().size_limit == 0 {
            return;
        }
        cache.file_manager()
    };
    let path = library.path.clone();
    let titles = Arc::clone(&library.titles);
    tokio::spawn(async move {
        match file_manager.save_shared(&path, &titles).await {
            Ok(_) => tracing::info!("Library cache saved successfully in background"),
            Err(e) => tracing::warn!("Failed to save library cache in background: {}", e),
        }
    });
}

/// Refresh metadata while aligning changed title trees with Mango keys.
pub(super) async fn load_metadata(library: &Library, previous: Option<&Library>) {
    let start = std::time::Instant::now();
    let mut loaded = 0;
    let mut errors = 0;
    if let Err(error) = library.metadata.refresh(&library.path).await {
        tracing::warn!("Failed to refresh library metadata: {}", error);
        errors += 1;
    }

    for (title_id, title) in library.titles.iter() {
        let unchanged = previous
            .and_then(|library| library.titles.get(title_id))
            .is_some_and(|old| {
                old.path == title.path && old.contents_signature == title.contents_signature
            });
        if !unchanged {
            if let Err(error) = library.metadata.populate_date_added(title).await {
                tracing::warn!(
                    "Failed to align info.json for title {}: {}",
                    title_id,
                    error
                );
                errors += 1;
            }
        }
    }

    // Refresh every directory, including nested and unchanged titles, with
    // the existing worker concurrency. The store locks only individual paths.
    const METADATA_WORKERS: usize = 20;
    let mut workers: Vec<Vec<(String, PathBuf)>> =
        (0..METADATA_WORKERS).map(|_| Vec::new()).collect();
    for (index, title) in library.all_titles().into_iter().enumerate() {
        workers[index % METADATA_WORKERS].push((title.id.clone(), title.path.clone()));
    }
    let mut tasks = tokio::task::JoinSet::new();
    for titles in workers.into_iter().filter(|titles| !titles.is_empty()) {
        let metadata = Arc::clone(&library.metadata);
        tasks.spawn(async move {
            let mut loaded = 0;
            let mut errors = 0;
            for (title_id, path) in titles {
                match metadata.refresh(&path).await {
                    Ok(_) => loaded += 1,
                    Err(error) => {
                        tracing::warn!(
                            "Failed to refresh metadata for title {}: {}",
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
                tracing::warn!("Metadata worker failed: {}", error);
                errors += 1;
            }
        }
    }
    tracing::info!(
        "Metadata refreshed: {} titles in {:.2}ms ({} errors)",
        loaded,
        start.elapsed().as_secs_f64() * 1000.0,
        errors
    );
}

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
            match scan(&mut new_lib, Some(previous), Some(Arc::clone(&library))).await {
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

#[cfg(test)]
mod tests;
