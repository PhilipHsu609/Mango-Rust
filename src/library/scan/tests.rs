use super::{scan, Library};
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
        drop(current);
        if (1..101).contains(&visible_titles) && preserved_title_is_visible {
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
