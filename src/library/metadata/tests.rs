use super::MetadataStore;
use crate::library::{Entry, Title};
#[tokio::test]
async fn populate_date_added_uses_entry_titles_and_migrates_legacy_ids() {
    let dir = tempfile::tempdir().unwrap();
    let store = MetadataStore::new();
    store
        .update(dir.path(), |info| {
            info.set_date_added("Existing", 1_600_000_000);
            info.set_date_added("legacy-uuid", 1_500_000_000);
        })
        .await
        .unwrap();

    let title = Title {
        id: "title".to_string(),
        path: dir.path().to_path_buf(),
        title: "Title".to_string(),
        signature: String::new(),
        contents_signature: String::new(),
        mtime: 0,
        entries: vec![
            Entry {
                id: "new-entry".to_string(),
                path: dir.path().join("new.cbz"),
                title: "New".to_string(),
                signature: String::new(),
                mtime: 0,
                ctime: 1_700_000_000,
                pages: 0,
                image_files: Vec::new(),
                size_bytes: 0,
                err_msg: None,
            },
            Entry {
                id: "existing-entry".to_string(),
                path: dir.path().join("existing.cbz"),
                title: "Existing".to_string(),
                signature: String::new(),
                mtime: 0,
                ctime: 1_700_000_001,
                pages: 0,
                image_files: Vec::new(),
                size_bytes: 0,
                err_msg: None,
            },
            Entry {
                id: "legacy-uuid".to_string(),
                path: dir.path().join("legacy.cbz"),
                title: "Migrated".to_string(),
                signature: String::new(),
                mtime: 0,
                ctime: 1_700_000_002,
                pages: 0,
                image_files: Vec::new(),
                size_bytes: 0,
                err_msg: None,
            },
        ],
        parent_id: None,
        nested_titles: Vec::new(),
    };

    store.populate_date_added(&title).await.unwrap();

    let info = store.refresh(dir.path()).await.unwrap();
    assert_eq!(info.get_date_added("New"), Some(1_700_000_000));
    assert_eq!(info.get_date_added("Existing"), Some(1_600_000_000));
    assert_eq!(info.get_date_added("Migrated"), Some(1_700_000_002));
    assert!(!info.date_added.contains_key("legacy-uuid"));
}
#[tokio::test]
async fn whole_title_progress_updates_nested_titles() {
    let dir = tempfile::tempdir().unwrap();
    let root_path = dir.path().join("Series");
    let child_path = root_path.join("Volume 1");
    std::fs::create_dir_all(&child_path).unwrap();
    let child = Title {
        id: "child".to_string(),
        path: child_path.clone(),
        title: "Volume 1".to_string(),
        signature: String::new(),
        contents_signature: String::new(),
        mtime: 0,
        entries: vec![Entry {
            id: "entry".to_string(),
            path: child_path.join("chapter.cbz"),
            title: "Chapter 1".to_string(),
            signature: String::new(),
            mtime: 0,
            ctime: 0,
            pages: 3,
            image_files: Vec::new(),
            size_bytes: 0,
            err_msg: None,
        }],
        parent_id: Some("root".to_string()),
        nested_titles: Vec::new(),
    };
    let root = Title {
        id: "root".to_string(),
        path: root_path,
        title: "Series".to_string(),
        signature: String::new(),
        contents_signature: String::new(),
        mtime: 0,
        entries: Vec::new(),
        parent_id: None,
        nested_titles: vec![child],
    };

    let store = MetadataStore::new();
    store.read_all(&root, "reader").await.unwrap();
    let info = store.refresh(&child_path).await.unwrap();
    assert_eq!(info.get_progress("reader", "Chapter 1"), Some(3));
    assert!(info.get_last_read("reader", "Chapter 1").is_some());

    store.unread_all(&root, "reader").await.unwrap();
    let info = store.refresh(&child_path).await.unwrap();
    assert_eq!(info.get_progress("reader", "Chapter 1"), None);
}

#[tokio::test]
async fn bulk_progress_preserves_last_read_timestamp() {
    let directory = tempfile::tempdir().unwrap();
    let store = MetadataStore::new();
    store
        .update(directory.path(), |info| {
            info.set_progress("reader", "Chapter 1", 7);
            info.set_last_read("reader", "Chapter 1", 1_700_000_000);
        })
        .await
        .unwrap();
    store
        .save_bulk_progress(directory.path(), "reader", &[("Chapter 1".to_string(), 0)])
        .await
        .unwrap();
    let saved = store.refresh(directory.path()).await.unwrap();
    assert_eq!(saved.get_progress("reader", "Chapter 1"), Some(0));
    assert_eq!(
        saved.get_last_read("reader", "Chapter 1"),
        Some(1_700_000_000)
    );
}

#[tokio::test]
async fn individual_zero_progress_sets_last_read() {
    let directory = tempfile::tempdir().unwrap();
    let store = MetadataStore::new();
    store
        .save_progress(directory.path(), "reader", "Chapter 1", 0)
        .await
        .unwrap();
    let saved = store.refresh(directory.path()).await.unwrap();
    assert_eq!(saved.get_progress("reader", "Chapter 1"), Some(0));
    assert!(saved.get_last_read("reader", "Chapter 1").is_some());
}

#[tokio::test]
async fn failed_persistence_does_not_publish_mutation() {
    let directory = tempfile::tempdir().unwrap();
    let store = MetadataStore::new();
    let before = store.read(directory.path()).await.unwrap();
    // The old snapshot is valid, but the directory is now absent: load defaults,
    // mutate, then fail specifically when attempting to persist info.json.
    std::fs::remove_dir(directory.path()).unwrap();
    let result = store
        .update(directory.path(), |info| {
            info.display_name = "Unsaved".to_string();
            info.set_progress("reader", "Chapter 1", 5);
        })
        .await;
    assert!(result.is_err());
    let after = store.cached(directory.path()).unwrap();
    assert!(std::sync::Arc::ptr_eq(&before, &after));
    assert!(after.display_name.is_empty());
    assert_eq!(after.get_progress("reader", "Chapter 1"), None);
}

#[tokio::test]
async fn concurrent_updates_preserve_unrelated_fields() {
    let directory = tempfile::tempdir().unwrap();
    let store = MetadataStore::new();
    let (display, progress) = tokio::join!(
        store.update(directory.path(), |info| info.display_name =
            "Series".to_string()),
        store.save_progress(directory.path(), "reader", "Chapter 1", 5),
    );
    display.unwrap();
    progress.unwrap();
    let saved = store.refresh(directory.path()).await.unwrap();
    assert_eq!(saved.display_name, "Series");
    assert_eq!(saved.get_progress("reader", "Chapter 1"), Some(5));
}

#[tokio::test]
async fn absent_sort_prefers_auto_ascending() {
    let directory = tempfile::tempdir().unwrap();
    assert_eq!(
        MetadataStore::new()
            .get_and_save_sort(directory.path(), "admin", None, None)
            .await
            .unwrap(),
        ("auto".to_string(), true),
    );
}

#[tokio::test]
async fn missing_corrupt_and_empty_comment_metadata_keep_defaults() {
    let directory = tempfile::tempdir().unwrap();
    let store = MetadataStore::new();
    assert_eq!(
        store.read(directory.path()).await.unwrap().comment,
        "Generated by Mango. DO NOT EDIT!"
    );
    tokio::fs::write(directory.path().join("info.json"), "not json")
        .await
        .unwrap();
    assert!(store
        .refresh(directory.path())
        .await
        .unwrap()
        .progress
        .is_empty());
    tokio::fs::write(
        directory.path().join("info.json"),
        r#"{"comment":"","display_name":"External"}"#,
    )
    .await
    .unwrap();
    let loaded = store.refresh(directory.path()).await.unwrap();
    assert_eq!(loaded.comment, "Generated by Mango. DO NOT EDIT!");
    assert_eq!(loaded.display_name, "External");
}

#[tokio::test]
async fn cached_reads_share_snapshots_and_updates_reload_external_fields() {
    let directory = tempfile::tempdir().unwrap();
    let store = MetadataStore::new();
    let before = store.read(directory.path()).await.unwrap();
    let again = store.read(directory.path()).await.unwrap();
    assert!(std::sync::Arc::ptr_eq(&before, &again));

    tokio::fs::write(
        directory.path().join("info.json"),
        r#"{"display_name":"External","cover_url":"/external.jpg"}"#,
    )
    .await
    .unwrap();
    store
        .save_progress(directory.path(), "reader", "Chapter 1", 2)
        .await
        .unwrap();
    let saved = store.refresh(directory.path()).await.unwrap();
    assert_eq!(saved.display_name, "External");
    assert_eq!(saved.cover_url, "/external.jpg");
    assert_eq!(saved.get_progress("reader", "Chapter 1"), Some(2));
    assert!(before.display_name.is_empty());
}

#[tokio::test]
async fn sort_preferences_cover_root_and_title_paths_without_cross_talk() {
    let directory = tempfile::tempdir().unwrap();
    let title_path = directory.path().join("Series");
    std::fs::create_dir(&title_path).unwrap();
    let store = MetadataStore::new();
    assert_eq!(
        store
            .get_and_save_sort(directory.path(), "reader", Some("modified"), Some("0"))
            .await
            .unwrap(),
        ("modified".to_string(), false),
    );
    assert_eq!(
        store
            .get_and_save_sort(&title_path, "reader", Some("title"), Some("invalid"))
            .await
            .unwrap(),
        ("title".to_string(), true),
    );
    assert_eq!(
        store
            .get_and_save_sort(directory.path(), "reader", None, Some("1"))
            .await
            .unwrap(),
        ("modified".to_string(), false),
    );
    assert_eq!(
        store
            .refresh(&title_path)
            .await
            .unwrap()
            .get_sort_by("reader"),
        Some(("title".to_string(), true)),
    );
}
