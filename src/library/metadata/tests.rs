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
                size_known: false,
                is_directory: false,
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
                size_known: false,
                is_directory: false,
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
                size_known: false,
                is_directory: false,
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
            size_known: false,
            is_directory: false,
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
    let before = store
        .update(directory.path(), |info| {
            info.display_name = "Saved".to_string();
            info.set_progress("reader", "Chapter 1", 2);
        })
        .await
        .unwrap();
    // Retain the saved snapshot while making the next persistence attempt fail.
    std::fs::remove_file(directory.path().join("info.json")).unwrap();
    std::fs::remove_dir(directory.path()).unwrap();
    let result = store
        .update(directory.path(), |info| {
            info.display_name = "Unsaved".to_string();
            info.set_progress("reader", "Chapter 1", 5);
        })
        .await;
    assert!(result.is_err());
    let after = store.cached(directory.path()).unwrap();
    assert_eq!(before.display_name, "Saved");
    assert_eq!(after.display_name, "Saved");
    assert_eq!(after.get_progress("reader", "Chapter 1"), Some(2));
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
async fn missing_and_corrupt_metadata_allow_later_persisted_updates() {
    let directory = tempfile::tempdir().unwrap();
    let store = MetadataStore::new();
    let missing = store.read(directory.path()).await.unwrap();
    assert_eq!(missing.get_progress("reader", "Chapter 1"), None);
    tokio::fs::write(directory.path().join("info.json"), "not json")
        .await
        .unwrap();
    let corrupt = store.refresh(directory.path()).await.unwrap();
    assert_eq!(corrupt.get_progress("reader", "Chapter 1"), None);
    assert!(!directory.path().join("info.json").exists());
    let backups = malformed_backups(directory.path());
    assert_eq!(backups.len(), 1);
    assert_eq!(std::fs::read_to_string(&backups[0]).unwrap(), "not json");
    store
        .save_progress(directory.path(), "reader", "Chapter 1", 3)
        .await
        .unwrap();
    let saved = MetadataStore::new().read(directory.path()).await.unwrap();
    assert_eq!(saved.get_progress("reader", "Chapter 1"), Some(3));
    assert_eq!(std::fs::read_to_string(&backups[0]).unwrap(), "not json");
}

fn malformed_backups(directory: &std::path::Path) -> Vec<std::path::PathBuf> {
    std::fs::read_dir(directory)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            path.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("info.json.malformed-")
        })
        .collect()
}

#[tokio::test]
async fn saves_replace_the_whole_file_and_leave_no_temp_files() {
    let directory = tempfile::tempdir().unwrap();
    tokio::fs::write(
        directory.path().join("info.json"),
        format!(
            r#"{{"display_name":"Old","comment":"{}"}}"#,
            "x".repeat(4096)
        ),
    )
    .await
    .unwrap();
    let store = MetadataStore::new();
    store
        .update(directory.path(), |info| {
            info.display_name = "New".to_string();
            info.comment = "short".to_string();
        })
        .await
        .unwrap();
    let names: Vec<_> = std::fs::read_dir(directory.path())
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(names, vec!["info.json"]);
    let saved = MetadataStore::new().read(directory.path()).await.unwrap();
    assert_eq!(saved.display_name, "New");
    assert_eq!(saved.comment, "short");
}

#[cfg(unix)]
#[tokio::test]
async fn unreadable_metadata_is_an_error_and_is_not_set_aside() {
    use std::os::unix::fs::PermissionsExt;
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("info.json");
    std::fs::write(&path, "not json").unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o000)).unwrap();
    // Privileged test processes can bypass mode bits.
    if std::fs::read(&path).is_ok() {
        return;
    }
    assert!(MetadataStore::new().read(directory.path()).await.is_err());
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
    assert!(malformed_backups(directory.path()).is_empty());
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "not json");
}

#[cfg(unix)]
#[tokio::test]
async fn failed_saves_keep_the_old_file_and_leave_no_temp_files() {
    use std::os::unix::fs::PermissionsExt;
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("info.json");
    std::fs::write(&path, r#"{"display_name":"Old"}"#).unwrap();
    let permissions = std::fs::metadata(directory.path()).unwrap().permissions();
    std::fs::set_permissions(directory.path(), std::fs::Permissions::from_mode(0o555)).unwrap();
    // Privileged test processes can bypass mode bits.
    if std::fs::write(directory.path().join("probe"), b"").is_ok() {
        std::fs::set_permissions(directory.path(), permissions).unwrap();
        return;
    }
    let saved = MetadataStore::new()
        .update(directory.path(), |info| {
            info.display_name = "New".to_string()
        })
        .await;
    std::fs::set_permissions(directory.path(), permissions).unwrap();
    assert!(saved.is_err());
    let names: Vec<_> = std::fs::read_dir(directory.path())
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(names, vec!["info.json"]);
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        r#"{"display_name":"Old"}"#
    );
}

#[tokio::test]
async fn cached_reads_preserve_snapshots_and_updates_reload_external_fields() {
    let directory = tempfile::tempdir().unwrap();
    let store = MetadataStore::new();
    let before = store.read(directory.path()).await.unwrap();

    tokio::fs::write(
        directory.path().join("info.json"),
        r#"{"display_name":"External","cover_url":"/external.jpg"}"#,
    )
    .await
    .unwrap();
    let cached = store.read(directory.path()).await.unwrap();
    assert!(cached.display_name.is_empty());
    assert_eq!(cached.get_progress("reader", "Chapter 1"), None);
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
