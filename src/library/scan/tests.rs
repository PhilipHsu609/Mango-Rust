use super::{scan, Library};
use crate::library::storage::StorageReport;
use crate::{Config, Storage};
use std::sync::Arc;

async fn scan_config() -> (tempfile::TempDir, Config, Storage) {
    let temp = tempfile::tempdir().unwrap();
    let library_path = temp.path().join("library");
    std::fs::create_dir(&library_path).unwrap();
    let db_path = temp.path().join("test.db");
    std::fs::File::create(&db_path).unwrap();
    let storage = Storage::new(&format!("sqlite://{}", db_path.display()))
        .await
        .unwrap();
    let config = Config {
        host: "127.0.0.1".to_string(),
        port: 0,
        base_url: "/".to_string(),
        session_secret: "test".to_string(),
        library_path,
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
    (temp, config, storage)
}

#[tokio::test]
async fn rescan_detects_changes_and_preserves_ids() {
    let (_temp, config, storage) = scan_config().await;
    let library_path = &config.library_path;
    let title_path = library_path.join("Series");
    std::fs::create_dir(&title_path).unwrap();
    let archive_path = title_path.join("Chapter 1.cbz");
    let bad_archive = b"not a valid archive";
    std::fs::write(&archive_path, bad_archive).unwrap();

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
    assert!(entry.err_msg.is_some());
    let original_id = entry.id.clone();
    let original_title_id = title.id.clone();

    let library = Arc::new(library);
    let mut rescanned = Library::new(library_path.clone(), storage.clone(), &config);
    scan(&mut rescanned, Some(library), None).await.unwrap();
    let entry = &rescanned.get_titles()[0].entries[0];
    assert_eq!(entry.id, original_id);
    assert!(entry.err_msg.is_some());
    assert_eq!(entry.pages, 0);

    // Metadata edited outside the application must be visible after an unchanged scan.
    let mut info = crate::library::metadata::TitleInfo {
        display_name: "Updated series".to_string(),
        ..Default::default()
    };
    info.set_progress("reader", "Chapter 1", 4);
    tokio::fs::write(
        title_path.join("info.json"),
        serde_json::to_vec_pretty(&info).unwrap(),
    )
    .await
    .unwrap();
    let mut refreshed = Library::new(library_path.clone(), storage.clone(), &config);
    scan(&mut refreshed, Some(Arc::new(rescanned)), None)
        .await
        .unwrap();
    let refreshed_info = refreshed.metadata().cached(&title_path).unwrap();
    assert_eq!(refreshed_info.display_name, "Updated series");
    assert_eq!(refreshed_info.get_progress("reader", "Chapter 1"), Some(4));

    // Nested additions and removals must invalidate the parent without changing IDs.
    let pages = title_path.join("Volume/Pages");
    std::fs::create_dir_all(&pages).unwrap();
    std::fs::write(pages.join("001.png"), b"page").unwrap();
    let mut changed = Library::new(library_path.clone(), storage.clone(), &config);
    scan(&mut changed, Some(Arc::new(refreshed)), None)
        .await
        .unwrap();
    let title = changed.get_titles()[0];
    assert_eq!(title.id, original_title_id);
    assert_eq!(title.nested_titles[0].entries[0].pages, 1);
    assert_eq!(title.entries[0].id, original_id);

    std::fs::write(&archive_path, b"longer invalid archive").unwrap();
    let mut replaced = Library::new(library_path.clone(), storage.clone(), &config);
    scan(&mut replaced, Some(Arc::new(changed)), None)
        .await
        .unwrap();
    assert_eq!(replaced.get_titles()[0].entries[0].size_bytes, 22);
    assert_eq!(replaced.get_titles()[0].entries[0].id, original_id);

    std::fs::remove_file(pages.join("001.png")).unwrap();
    let mut removed = Library::new(library_path.clone(), storage.clone(), &config);
    scan(&mut removed, Some(Arc::new(replaced)), None)
        .await
        .unwrap();
    assert!(removed.get_titles()[0].nested_titles.is_empty());

    let renamed_path = title_path.join("Chapter 2.cbz");
    std::fs::rename(&archive_path, &renamed_path).unwrap();
    let mut renamed = Library::new(library_path.clone(), storage, &config);
    scan(&mut renamed, Some(Arc::new(removed)), None)
        .await
        .unwrap();
    let title = renamed.get_titles()[0];
    assert_eq!(title.id, original_title_id);
    assert_eq!(title.entries[0].title, "Chapter 2");
    assert_eq!(title.entries[0].path, renamed_path);
    assert_eq!(title.entries[0].id, original_id);
}

#[tokio::test]
async fn rescan_publishes_completed_roots_before_finishing() {
    let (_temp, config, storage) = scan_config().await;
    let title_path = config.library_path.join("Series");
    std::fs::create_dir(&title_path).unwrap();
    std::fs::write(title_path.join("Chapter.cbz"), b"invalid archive").unwrap();
    let mut previous = Library::new(config.library_path.clone(), storage.clone(), &config);
    scan(&mut previous, None, None).await.unwrap();
    let preserved_title_id = previous.get_titles()[0].id.clone();
    let previous_report = previous.storage_report().unwrap();
    let previous = Arc::new(previous);
    for index in 0..100 {
        let title_path = config.library_path.join(format!("New {index:03}"));
        std::fs::create_dir(&title_path).unwrap();
        std::fs::write(title_path.join("Chapter.cbz"), b"invalid archive").unwrap();
    }
    let shared = Arc::new(arc_swap::ArcSwap::from(Arc::clone(&previous)));
    let publisher = Arc::clone(&shared);
    let scan_task = tokio::spawn(async move {
        let mut scanning = Library::new(config.library_path.clone(), storage, &config);
        scan(&mut scanning, Some(previous), Some(publisher))
            .await
            .unwrap();
    });

    let mut observed_partial_scan = false;
    while !scan_task.is_finished() {
        let current = shared.load();
        let visible_titles = current.get_titles().len();
        let preserved_title_is_visible = current.get_title(&preserved_title_id).is_some();
        let report = current.storage_report().unwrap();
        drop(current);
        if (1..101).contains(&visible_titles) && preserved_title_is_visible {
            assert!(Arc::ptr_eq(&report, &previous_report));
            assert_eq!(report.total_bytes, 15);
            observed_partial_scan = true;
            break;
        }
        tokio::task::yield_now().await;
    }
    scan_task.await.unwrap();
    assert!(observed_partial_scan);
    let completed = shared.load();
    assert_eq!(completed.get_titles().len(), 101);
    assert!(completed.get_title(&preserved_title_id).is_some());
    let report = completed.storage_report().unwrap();
    assert!(!Arc::ptr_eq(&report, &previous_report));
    assert_eq!(report.total_bytes, 101 * 15);
    assert!(report.complete);
}

#[cfg(unix)]
#[tokio::test]
async fn moved_archives_keep_ids_from_matching_trailing_paths() {
    let (_temp, config, storage) = scan_config().await;
    let old_root = config.library_path.join("old");
    let volume = old_root.join("volume-1");
    let other = old_root.join("chapter-2");
    std::fs::create_dir_all(&volume).unwrap();
    std::fs::create_dir(&other).unwrap();
    let archive = volume.join("chapter-2.cbz");
    std::fs::write(&archive, b"same archive contents").unwrap();
    std::fs::hard_link(&archive, other.join("chapter-2.cbz")).unwrap();
    let mut previous = Library::new(config.library_path.clone(), storage.clone(), &config);
    scan(&mut previous, None, None).await.unwrap();
    let original = previous.get_titles()[0]
        .deep_entries()
        .into_iter()
        .find(|entry| entry.path == volume.join("chapter-2.cbz"))
        .unwrap()
        .id
        .clone();

    // Both signatures match; retain the ID of the corresponding volume, not its sibling.
    let new_root = config.library_path.join("new");
    std::fs::rename(&old_root, &new_root).unwrap();
    std::fs::remove_dir_all(new_root.join("chapter-2")).unwrap();
    let mut moved = Library::new(config.library_path.clone(), storage, &config);
    scan(&mut moved, Some(Arc::new(previous)), None)
        .await
        .unwrap();
    let entries = moved.get_titles()[0].deep_entries();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].path, new_root.join("volume-1/chapter-2.cbz"));
    assert_eq!(entries[0].id, original);
}

#[tokio::test]
async fn storage_counts_only_indexed_payloads_with_mixed_loose_ownership() {
    let (_temp, config, storage) = scan_config().await;
    let series = config.library_path.join("Series");
    let volume = series.join("Volume");
    let chapter = volume.join("Chapter");
    std::fs::create_dir_all(&chapter).unwrap();
    std::fs::write(series.join("Direct.cbz"), b"archive").unwrap();
    std::fs::write(volume.join("Nested.cbz"), b"nested archive").unwrap();
    std::fs::write(volume.join("001.png"), b"image").unwrap();
    std::fs::write(volume.join("002.jpg"), b"second image").unwrap();
    std::fs::write(chapter.join("001.png"), b"deep page").unwrap();
    let mut library = Library::new(config.library_path.clone(), storage.clone(), &config);
    assert!(library.storage_report().is_none());
    scan(&mut library, None, None).await.unwrap();
    let initial = library.storage_report().unwrap();
    assert_eq!(initial.total_bytes, 7 + 14 + 5 + 12 + 9);
    assert!(initial.complete);
    let root = &initial.titles[0];
    assert_eq!(root.size_bytes, initial.total_bytes);
    assert_eq!(root.entry_count, 4);
    // Volume's direct pages are owned by Series; Volume owns its archive and
    // the Chapter loose entry. Folder-based attribution would count pages twice.
    assert_eq!(root.children[0].size_bytes, 14 + 9);
    assert_eq!(root.children[0].entry_count, 2);
    assert!(root
        .issues
        .iter()
        .any(|issue| issue.path == "Series/Direct.cbz"));

    std::fs::write(series.join("info.json"), b"{}").unwrap();
    std::fs::write(series.join("custom-cover.png"), vec![0; 100]).unwrap();
    std::fs::write(series.join("unsupported.txt"), vec![0; 100]).unwrap();
    std::fs::write(volume.join("unsupported.txt"), vec![0; 100]).unwrap();
    std::fs::write(volume.join(".hidden.png"), vec![0; 100]).unwrap();
    std::fs::write(series.join(".hidden.cbz"), vec![0; 100]).unwrap();
    std::fs::write(config.library_path.join("unindexed.cbz"), vec![0; 100]).unwrap();
    std::fs::create_dir(series.join(".staging")).unwrap();
    std::fs::write(series.join(".staging/hidden.cbz"), vec![0; 100]).unwrap();
    let mut rescanned = Library::new(config.library_path.clone(), storage, &config);
    scan(&mut rescanned, Some(Arc::new(library)), None)
        .await
        .unwrap();
    assert_eq!(
        rescanned.storage_report().unwrap().total_bytes,
        initial.total_bytes
    );
    assert!(rescanned.storage_report().unwrap().complete);
}

#[tokio::test]
async fn unknown_cached_sizes_are_remeasured_even_with_unchanged_fingerprints() {
    let (_temp, config, storage) = scan_config().await;
    let title_path = config.library_path.join("Series");
    std::fs::create_dir(&title_path).unwrap();
    std::fs::write(title_path.join("Zero.cbz"), b"").unwrap();
    let mut previous = Library::new(config.library_path.clone(), storage.clone(), &config);
    scan(&mut previous, None, None).await.unwrap();
    let original_id = previous.get_titles()[0].entries[0].id.clone();
    let titles = Arc::make_mut(&mut previous.titles);
    let title = Arc::make_mut(titles.values_mut().next().unwrap());
    title.entries[0].size_known = false;
    previous.storage_report = Some(Arc::new(crate::library::storage::aggregate(
        &previous.path,
        &previous.titles,
        None,
        true,
        Vec::new(),
    )));
    assert_eq!(
        previous.storage_report().unwrap().titles[0].measured_entries,
        0
    );
    let mut refreshed = Library::new(config.library_path.clone(), storage, &config);
    scan(&mut refreshed, Some(Arc::new(previous)), None)
        .await
        .unwrap();
    let entry = &refreshed.get_titles()[0].entries[0];
    assert_eq!(entry.id, original_id);
    assert!(entry.size_known);
    assert_eq!(entry.size_bytes, 0);
    let report = refreshed.storage_report().unwrap();
    assert!(report.complete);
    assert!(!report.stale);
    assert!(report.measured_at.is_some());
    assert_eq!(report.titles[0].measured_entries, 1);
}

#[tokio::test]
async fn failed_refresh_retains_previous_observation_and_marks_it_historical() {
    let (_temp, config, storage) = scan_config().await;
    std::fs::create_dir(config.library_path.join("Series")).unwrap();
    std::fs::write(config.library_path.join("Series/1.cbz"), b"archive").unwrap();
    let mut previous = Library::new(config.library_path.clone(), storage.clone(), &config);
    scan(&mut previous, None, None).await.unwrap();
    let report = previous.storage_report().unwrap();
    let previous = Arc::new(previous);
    let shared = Arc::new(arc_swap::ArcSwap::from(Arc::clone(&previous)));
    std::fs::rename(
        &config.library_path,
        config.library_path.with_extension("gone"),
    )
    .unwrap();
    let mut refreshed = Library::new(config.library_path.clone(), storage.clone(), &config);
    assert!(scan(
        &mut refreshed,
        Some(Arc::clone(&previous)),
        Some(Arc::clone(&shared))
    )
    .await
    .is_err());
    let retained = shared.load().storage_report().unwrap();
    assert_eq!(retained.total_bytes, report.total_bytes);
    assert_eq!(retained.measured_at, report.measured_at);
    assert_eq!(retained.titles[0].id, report.titles[0].id);
    assert!(retained.stale);
    let failures = |report: &StorageReport| {
        report
            .issues
            .iter()
            .filter(|issue| issue.message.contains("Refresh failed"))
            .count()
    };
    assert_eq!(failures(&retained), 1);

    // Failing again must replace the notice, not stack another one on top.
    let mut again = Library::new(config.library_path.clone(), storage, &config);
    let latest = shared.load_full();
    assert!(scan(&mut again, Some(latest), Some(Arc::clone(&shared)))
        .await
        .is_err());
    assert_eq!(failures(&shared.load().storage_report().unwrap()), 1);
}

#[cfg(unix)]
#[tokio::test]
async fn symbolic_links_are_followed_and_counted_like_any_other_entry() {
    use std::os::unix::fs::symlink;
    let (temp, config, storage) = scan_config().await;
    let external = temp.path().join("external");
    std::fs::create_dir_all(external.join("Linked Series")).unwrap();
    std::fs::write(external.join("Linked Series/1.cbz"), vec![0; 100]).unwrap();
    std::fs::write(external.join("2.cbz"), vec![0; 40]).unwrap();
    let title_path = config.library_path.join("Series");
    std::fs::create_dir(&title_path).unwrap();
    std::fs::write(title_path.join("1.cbz"), b"archive").unwrap();
    symlink(external.join("2.cbz"), title_path.join("linked.cbz")).unwrap();
    symlink(
        external.join("Linked Series"),
        config.library_path.join("Linked Series"),
    )
    .unwrap();
    let mut library = Library::new(config.library_path.clone(), storage.clone(), &config);
    scan(&mut library, None, None).await.unwrap();
    // Mango follows links, so they stay in the catalog instead of becoming missing items.
    assert_eq!(library.get_titles().len(), 2);
    let report = library.storage_report().unwrap();
    assert_eq!(report.total_bytes, 7 + 40 + 100);
    assert!(report.complete);

    // A symlinked library root is the usual container setup and must still scan.
    let linked_root = temp.path().join("library-link");
    symlink(&config.library_path, &linked_root).unwrap();
    let mut via_link = Library::new(linked_root, storage, &config);
    scan(&mut via_link, None, None).await.unwrap();
    assert_eq!(via_link.get_titles().len(), 2);
}

#[tokio::test]
async fn cache_startup_reports_historical_sizes_without_a_fabricated_timestamp() {
    let (_temp, mut config, storage) = scan_config().await;
    std::fs::create_dir(config.library_path.join("Series")).unwrap();
    std::fs::write(config.library_path.join("Series/1.cbz"), b"archive").unwrap();
    let mut library = Library::new(config.library_path.clone(), storage.clone(), &config);
    scan(&mut library, None, None).await.unwrap();
    let cache = library.cache().lock().await.file_manager();
    cache.save(&library).await.unwrap();
    config.cache_enabled = true;
    config.cache_size_mbs = 4;
    let mut loaded = Library::new(config.library_path.clone(), storage, &config);
    assert!(loaded.try_load_from_cache().await.unwrap());
    let report = loaded.storage_report().unwrap();
    assert_eq!(report.total_bytes, 7);
    assert_eq!(report.measured_at, None);
    assert!(report.stale);
    assert!(report.complete);
    assert_eq!(report.titles[0].measured_entries, 1);
}

#[cfg(unix)]
#[tokio::test]
async fn unreadable_root_regions_do_not_produce_complete_smaller_reports() {
    use std::os::unix::fs::PermissionsExt;
    let (_temp, mut config, storage) = scan_config().await;
    let title = config.library_path.join("Series");
    std::fs::create_dir(&title).unwrap();
    std::fs::write(title.join("1.cbz"), b"archive").unwrap();
    let permissions = std::fs::metadata(&title).unwrap().permissions();
    std::fs::set_permissions(&title, std::fs::Permissions::from_mode(0)).unwrap();
    // Privileged test processes can bypass mode bits, so this scenario only
    // applies when the operating system actually refuses the directory read.
    if std::fs::read_dir(&title).is_ok() {
        std::fs::set_permissions(&title, permissions).unwrap();
        return;
    }
    let mut library = Library::new(config.library_path.clone(), storage.clone(), &config);
    let scanned = scan(&mut library, None, None).await;
    std::fs::set_permissions(&title, permissions).unwrap();
    scanned.unwrap();
    let report = library.storage_report().unwrap();
    assert_eq!(report.total_bytes, 0);
    assert!(!report.complete);
    assert!(report.issues.iter().any(|issue| issue.path == "Series"));

    // Cached partial coverage must stay partial after restart, even though its
    // omitted title is absent from the serialized catalog.
    let cache = library.cache().lock().await.file_manager();
    cache.save(&library).await.unwrap();
    config.cache_enabled = true;
    config.cache_size_mbs = 4;
    let mut loaded = Library::new(config.library_path.clone(), storage.clone(), &config);
    assert!(loaded.try_load_from_cache().await.unwrap());
    assert!(!loaded.storage_report().unwrap().complete);
    assert!(loaded.storage_report().unwrap().stale);

    let mut refreshed = Library::new(config.library_path.clone(), storage, &config);
    scan(&mut refreshed, Some(Arc::new(loaded)), None)
        .await
        .unwrap();
    assert_eq!(refreshed.storage_report().unwrap().total_bytes, 7);
    assert!(refreshed.storage_report().unwrap().complete);
}

#[tokio::test]
async fn loose_entries_keep_their_payload_kind_with_archive_named_directories() {
    let (_temp, config, storage) = scan_config().await;
    let pages = config.library_path.join("Series/Chapter.cbz");
    std::fs::create_dir_all(&pages).unwrap();
    std::fs::write(pages.join("001.png"), b"image").unwrap();
    let mut library = Library::new(config.library_path.clone(), storage, &config);
    scan(&mut library, None, None).await.unwrap();
    let entry = &library.get_titles()[0].entries[0];
    assert!(entry.is_directory);
    let report = library.storage_report().unwrap();
    assert_eq!(report.total_bytes, 5);
    assert_eq!(report.titles[0].measured_entries, 1);
    assert!(report.complete);
}

#[cfg(unix)]
#[tokio::test]
async fn distinct_hard_linked_entry_paths_count_separately() {
    let (_temp, config, storage) = scan_config().await;
    let title = config.library_path.join("Series");
    std::fs::create_dir(&title).unwrap();
    std::fs::write(title.join("1.cbz"), b"archive").unwrap();
    std::fs::hard_link(title.join("1.cbz"), title.join("2.cbz")).unwrap();
    let mut library = Library::new(config.library_path.clone(), storage, &config);
    scan(&mut library, None, None).await.unwrap();
    let entries = &library.get_titles()[0].entries;
    assert_eq!(entries.len(), 2);
    assert_ne!(entries[0].id, entries[1].id);
    let report = library.storage_report().unwrap();
    assert_eq!(report.total_bytes, 14);
    assert_eq!(report.titles[0].measured_entries, 2);
    assert!(report.complete);
}
