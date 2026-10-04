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
        library.progress_cache = Arc::clone(&previous.progress_cache);
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

    // Load progress cache for all titles
    load_progress_cache(library, previous.as_deref()).await;

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

/// Load progress data for all titles into the cache
pub(super) async fn load_progress_cache(library: &Library, previous: Option<&Library>) {
    let start = std::time::Instant::now();
    let mut loaded = 0;
    let mut errors = 0;

    for (title_id, title) in library.titles.iter() {
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
    for (index, title) in library.all_titles().into_iter().enumerate() {
        workers[index % PROGRESS_WORKERS].push((title.id.clone(), title.path.clone()));
    }
    let mut tasks = tokio::task::JoinSet::new();
    for titles in workers.into_iter().filter(|titles| !titles.is_empty()) {
        let progress_cache = Arc::clone(&library.progress_cache);
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
mod scan_regression_tests {
    use super::{scan, Library};
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
        scan(&mut library, None, None).await.unwrap();
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
        scan(&mut rescanned, Some(Arc::clone(&library)), None)
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
        scan(&mut refreshed, Some(rescanned), None).await.unwrap();
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
        scan(&mut changed, Some(refreshed), None).await.unwrap();
        let title = changed.get_titles()[0];
        assert_eq!(title.id, library.get_titles()[0].id);
        assert_eq!(title.nested_titles[0].entries[0].pages, 1);
        assert_eq!(title.entries[0].id, original_id);

        let changed = Arc::new(changed);
        std::fs::write(&archive_path, b"longer invalid archive").unwrap();
        let mut replaced = Library::new(library_path.clone(), storage.clone(), &config);
        scan(&mut replaced, Some(changed), None).await.unwrap();
        assert_eq!(replaced.get_titles()[0].entries[0].size_bytes, 22);
        assert_eq!(replaced.get_titles()[0].entries[0].id, original_id);

        let replaced = Arc::new(replaced);
        std::fs::remove_file(pages.join("001.png")).unwrap();
        let mut removed = Library::new(library_path.clone(), storage.clone(), &config);
        scan(&mut removed, Some(replaced), None).await.unwrap();
        assert!(removed.get_titles()[0].nested_titles.is_empty());

        let removed = Arc::new(removed);
        let renamed_path = title_path.join("Chapter 2.cbz");
        std::fs::rename(&archive_path, &renamed_path).unwrap();
        let mut renamed = Library::new(library_path.clone(), storage.clone(), &config);
        scan(&mut renamed, Some(removed), None).await.unwrap();
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
            scan(&mut scanning, Some(previous), Some(publisher))
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
