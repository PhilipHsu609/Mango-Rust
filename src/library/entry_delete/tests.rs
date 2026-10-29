use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, UNIX_EPOCH};

use super::{cleanup_after_rescan, delete, preview, DeleteError, DeleteRequest};
use crate::library::scan::{rescan_and_publish, SCAN_LOCK};
use crate::library::test_support::scan_config;
use crate::library::{Library, SharedLibrary};
use crate::{Config, Storage};

struct Fixture {
    _temp: tempfile::TempDir,
    config: Arc<Config>,
    storage: Storage,
    shared: SharedLibrary,
}

impl Fixture {
    /// A scanned library holding `files` (paths relative to the library root).
    async fn new(cache: bool, files: &[&str]) -> Self {
        let (temp, config, storage) = scan_config(cache).await;
        for file in files {
            let path = config.library_path.join(file);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, b"archive").unwrap();
        }
        let empty = Library::new(config.library_path.clone(), storage.clone(), &config);
        let shared = Arc::new(arc_swap::ArcSwap::from_pointee(empty));
        let fixture = Self {
            _temp: temp,
            config: Arc::new(config),
            storage,
            shared,
        };
        fixture.rescan().await;
        fixture
    }

    async fn rescan(&self) {
        let guard = SCAN_LOCK.lock().await;
        rescan_and_publish(&self.shared, &self.storage, &self.config, &guard)
            .await
            .unwrap();
    }

    fn root(&self) -> &Path {
        &self.config.library_path
    }

    /// (title id, entry id) of the entry at `relative`.
    fn ids(&self, relative: &str) -> (String, String) {
        let library = self.shared.load();
        let path = self.root().join(relative);
        library
            .all_titles()
            .into_iter()
            .find_map(|title| {
                title
                    .entries
                    .iter()
                    .find(|entry| entry.path == path)
                    .map(|entry| (title.id.clone(), entry.id.clone()))
            })
            .unwrap_or_else(|| panic!("{relative} is not in the library"))
    }

    fn title_id(&self, relative: &str) -> String {
        let path = self.root().join(relative);
        self.shared
            .load()
            .all_titles()
            .into_iter()
            .find(|title| title.path == path)
            .map(|title| title.id.clone())
            .unwrap()
    }

    /// Delete with the values a fresh preview shows.
    async fn delete(&self, relative: &str) -> Result<super::DeleteOutcome, DeleteError> {
        let (title_id, entry_id) = self.ids(relative);
        let shown = preview(&self.shared.load(), &title_id, &entry_id)
            .await
            .unwrap();
        self.delete_with(title_id, entry_id, shown.size_bytes, &shown.mtime_ns)
            .await
    }

    async fn delete_with(
        &self,
        title_id: String,
        entry_id: String,
        size: u64,
        mtime_ns: &str,
    ) -> Result<super::DeleteOutcome, DeleteError> {
        delete(
            Arc::clone(&self.shared),
            self.storage.clone(),
            Arc::clone(&self.config),
            DeleteRequest {
                admin: "admin".to_string(),
                title_id,
                entry_id,
                size,
                mtime_ns: mtime_ns.parse().unwrap(),
            },
        )
        .await
    }

    async fn count(&self, sql: &str, id: &str) -> i64 {
        sqlx::query_scalar(sql)
            .bind(id)
            .fetch_one(self.storage.pool())
            .await
            .unwrap()
    }
}

#[tokio::test]
async fn deleting_an_entry_removes_its_file_rows_and_metadata_keys() {
    let fixture = Fixture::new(false, &["Series/Vol 1.cbz", "Series/Vol 2.cbz"]).await;
    let (title_id, entry_id) = fixture.ids("Series/Vol 1.cbz");
    let series = fixture.root().join("Series");
    fixture
        .shared
        .load()
        .metadata()
        .update(&series, |info| {
            for user in ["alice", "bob"] {
                info.set_progress(user, "Vol 1", 3);
                info.set_progress(user, "Vol 2", 4);
            }
        })
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO thumbnails (id, data, filename, mime, size) VALUES (?, x'00', 't.jpg', 'image/jpeg', 1)",
    )
    .bind(&entry_id)
    .execute(fixture.storage.pool())
    .await
    .unwrap();
    fixture
        .storage
        .save_dimensions(&entry_id, &[(0, 10, 20)])
        .await
        .unwrap();

    let outcome = fixture.delete("Series/Vol 1.cbz").await.unwrap();

    assert!(outcome.warnings.is_empty(), "{:?}", outcome.warnings);
    assert!(!outcome.removes_title);
    assert_eq!(outcome.redirect, format!("/book/{title_id}"));
    assert!(!series.join("Vol 1.cbz").exists());
    for table in [
        "SELECT COUNT(*) FROM ids WHERE id = ?",
        "SELECT COUNT(*) FROM thumbnails WHERE id = ?",
        "SELECT COUNT(*) FROM dimensions WHERE entry_id = ?",
    ] {
        assert_eq!(fixture.count(table, &entry_id).await, 0, "{table}");
    }
    let library = fixture.shared.load();
    assert!(library.get_entry(&title_id, &entry_id).is_none());
    assert!(fixture
        .storage
        .get_missing_entries()
        .await
        .unwrap()
        .is_empty());
    let info = Library::new(
        fixture.root().to_path_buf(),
        fixture.storage.clone(),
        &fixture.config,
    )
    .metadata()
    .read(&series)
    .await
    .unwrap();
    for user in ["alice", "bob"] {
        assert_eq!(info.get_progress(user, "Vol 1"), None);
        assert_eq!(info.get_progress(user, "Vol 2"), Some(4));
    }
}

#[tokio::test]
async fn the_preview_mtime_survives_a_json_round_trip() {
    let fixture = Fixture::new(false, &["Series/Vol 1.cbz", "Series/Vol 2.cbz"]).await;
    let path = fixture.root().join("Series/Vol 1.cbz");
    std::fs::File::options()
        .write(true)
        .open(&path)
        .unwrap()
        .set_modified(UNIX_EPOCH + Duration::new(1_700_000_000, 123_456_789))
        .unwrap();
    let (title_id, entry_id) = fixture.ids("Series/Vol 1.cbz");
    let shown = preview(&fixture.shared.load(), &title_id, &entry_id)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::to_value(&shown).unwrap();
    let mtime_ns = json["mtime_ns"].as_str().unwrap();
    assert!(mtime_ns.ends_with("123456789"), "{mtime_ns}");
    let size = json["size_bytes"].as_u64().unwrap();
    fixture
        .delete_with(title_id, entry_id, size, mtime_ns)
        .await
        .unwrap();
    assert!(!path.exists());
}

#[tokio::test]
async fn a_file_rewritten_after_the_preview_is_kept() {
    let fixture = Fixture::new(false, &["Series/Vol 1.cbz", "Series/Vol 2.cbz"]).await;
    let path = fixture.root().join("Series/Vol 1.cbz");
    let (title_id, entry_id) = fixture.ids("Series/Vol 1.cbz");
    let shown = preview(&fixture.shared.load(), &title_id, &entry_id)
        .await
        .unwrap();
    // A different size keeps this independent of mtime granularity.
    std::fs::write(&path, b"a longer replacement archive").unwrap();
    let result = fixture
        .delete_with(title_id, entry_id, shown.size_bytes, &shown.mtime_ns)
        .await;
    assert!(matches!(result, Err(DeleteError::Changed)), "{result:?}");
    assert!(path.exists());
}

#[cfg(unix)]
#[tokio::test]
async fn a_symlinked_archive_loses_only_the_link() {
    let fixture = Fixture::new(false, &["Series/Vol 1.cbz"]).await;
    let target = fixture._temp.path().join("outside.cbz");
    std::fs::write(&target, b"archive").unwrap();
    let link = fixture.root().join("Series/Vol 2.cbz");
    std::os::unix::fs::symlink(&target, &link).unwrap();
    fixture.rescan().await;

    fixture.delete("Series/Vol 2.cbz").await.unwrap();

    assert!(std::fs::symlink_metadata(&link).is_err());
    assert!(target.exists());
}

#[cfg(unix)]
#[tokio::test]
async fn a_dangling_symlink_is_not_found_and_left_in_place() {
    let fixture = Fixture::new(false, &["Series/Vol 1.cbz"]).await;
    let target = fixture._temp.path().join("outside.cbz");
    std::fs::write(&target, b"archive").unwrap();
    let link = fixture.root().join("Series/Vol 2.cbz");
    std::os::unix::fs::symlink(&target, &link).unwrap();
    fixture.rescan().await;
    std::fs::remove_file(&target).unwrap();

    let (title_id, entry_id) = fixture.ids("Series/Vol 2.cbz");
    let result = preview(&fixture.shared.load(), &title_id, &entry_id).await;
    assert!(matches!(result, Err(DeleteError::NotFound)), "{result:?}");
    assert!(std::fs::symlink_metadata(&link).is_ok());
}

#[tokio::test]
async fn files_removed_after_the_scan_and_repeated_deletes_are_not_found() {
    let fixture = Fixture::new(false, &["Series/Vol 1.cbz", "Series/Vol 2.cbz"]).await;
    let (title_id, entry_id) = fixture.ids("Series/Vol 2.cbz");
    std::fs::remove_file(fixture.root().join("Series/Vol 2.cbz")).unwrap();
    let result = preview(&fixture.shared.load(), &title_id, &entry_id).await;
    assert!(matches!(result, Err(DeleteError::NotFound)), "{result:?}");

    let (title_id, entry_id) = fixture.ids("Series/Vol 1.cbz");
    let shown = preview(&fixture.shared.load(), &title_id, &entry_id)
        .await
        .unwrap();
    fixture
        .delete_with(
            title_id.clone(),
            entry_id.clone(),
            shown.size_bytes,
            &shown.mtime_ns,
        )
        .await
        .unwrap();
    let again = fixture
        .delete_with(title_id, entry_id, shown.size_bytes, &shown.mtime_ns)
        .await;
    assert!(matches!(again, Err(DeleteError::NotFound)), "{again:?}");
}

#[tokio::test]
async fn loose_image_entries_are_refused() {
    let fixture = Fixture::new(false, &["Series/Vol 1.cbz", "Series/Vol 2/001.jpg"]).await;
    let (title_id, entry_id) = fixture.ids("Series/Vol 2");
    let result = preview(&fixture.shared.load(), &title_id, &entry_id).await;
    assert!(matches!(result, Err(DeleteError::Refused)), "{result:?}");
}

#[tokio::test]
async fn deleting_the_last_entry_keeps_the_title_as_a_missing_record() {
    let fixture = Fixture::new(false, &["Series/Vol 1.cbz", "Other/Vol 1.cbz"]).await;
    let title_id = fixture.title_id("Series");
    fixture.storage.add_tag(&title_id, "keep").await.unwrap();

    let outcome = fixture.delete("Series/Vol 1.cbz").await.unwrap();

    assert!(outcome.removes_title);
    assert_eq!(outcome.redirect, "/library");
    let missing = fixture.storage.get_missing_titles().await.unwrap();
    assert_eq!(missing.len(), 1);
    assert_eq!(missing[0].id, title_id);
    assert_eq!(
        fixture.storage.get_title_tags(&title_id).await.unwrap(),
        ["keep"]
    );
}

#[tokio::test]
async fn emptying_a_nested_title_returns_to_the_surviving_parent() {
    let fixture = Fixture::new(false, &["Series/Vol 1.cbz", "Series/Arc/Ch 1.cbz"]).await;
    let series = fixture.title_id("Series");
    let outcome = fixture.delete("Series/Arc/Ch 1.cbz").await.unwrap();
    assert!(outcome.removes_title);
    assert_eq!(outcome.redirect, format!("/book/{series}"));
}

#[tokio::test]
async fn emptying_a_root_through_its_only_nested_title_returns_to_the_library() {
    let fixture = Fixture::new(false, &["Series/Arc/Ch 1.cbz", "Other/Vol 1.cbz"]).await;
    let outcome = fixture.delete("Series/Arc/Ch 1.cbz").await.unwrap();
    assert!(outcome.removes_title);
    assert_eq!(outcome.redirect, "/library");
}

#[tokio::test]
async fn a_malformed_parent_info_json_is_set_aside_and_the_delete_succeeds() {
    let fixture = Fixture::new(false, &["Series/Vol 1.cbz", "Series/Vol 2.cbz"]).await;
    let series = fixture.root().join("Series");
    std::fs::write(series.join("info.json"), "not json").unwrap();
    let fresh = Library::new(
        fixture.root().to_path_buf(),
        fixture.storage.clone(),
        &fixture.config,
    );
    // Drop cached metadata so the delete reloads the file from disk.
    fixture.shared.store(Arc::new(fresh));
    fixture.rescan().await;

    let outcome = fixture.delete("Series/Vol 1.cbz").await.unwrap();

    assert!(outcome.warnings.is_empty(), "{:?}", outcome.warnings);
    let backups = std::fs::read_dir(&series)
        .unwrap()
        .filter(|entry| {
            entry
                .as_ref()
                .unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with("info.json.malformed-")
        })
        .count();
    assert_eq!(backups, 1);
}

#[tokio::test]
async fn a_cache_file_that_cannot_be_removed_stops_the_delete() {
    let fixture = Fixture::new(false, &["Series/Vol 1.cbz", "Series/Vol 2.cbz"]).await;
    std::fs::create_dir(&fixture.config.library_cache_path).unwrap();
    std::fs::write(fixture.config.library_cache_path.join("blocker"), b"").unwrap();

    let result = fixture.delete("Series/Vol 1.cbz").await;

    assert!(matches!(result, Err(DeleteError::Failed(_))), "{result:?}");
    assert!(fixture.root().join("Series/Vol 1.cbz").exists());
}

#[tokio::test]
async fn a_failed_rescan_leaves_the_keys_and_row_for_later_cleanup() {
    let fixture = Fixture::new(false, &["Series/Vol 1.cbz", "Series/Vol 2.cbz"]).await;
    let (title_id, entry_id) = fixture.ids("Series/Vol 1.cbz");
    let series = fixture.root().join("Series");
    let library = fixture.shared.load_full();
    library
        .metadata()
        .update(&series, |info| info.set_progress("alice", "Vol 1", 3))
        .await
        .unwrap();
    let title = library.get_title(&title_id).unwrap();
    let entry = library.get_entry(&title_id, &entry_id).unwrap();

    let warnings = cleanup_after_rescan(
        &fixture.shared,
        &fixture.storage,
        title,
        entry,
        Err(crate::error::Error::Internal("scan failed".to_string())),
    )
    .await;

    assert_eq!(warnings.len(), 1);
    assert!(warnings[0].contains("rescan failed"), "{warnings:?}");
    let info = library.metadata().read(&series).await.unwrap();
    assert_eq!(info.get_progress("alice", "Vol 1"), Some(3));
    assert_eq!(
        fixture
            .count("SELECT COUNT(*) FROM ids WHERE id = ?", &entry_id)
            .await,
        1
    );
}

#[tokio::test]
async fn a_restart_after_a_delete_does_not_restore_the_entry_from_the_cache() {
    let fixture = Fixture::new(true, &["Series/Vol 1.cbz", "Series/Vol 2.cbz"]).await;
    let (title_id, entry_id) = fixture.ids("Series/Vol 1.cbz");
    fixture.delete("Series/Vol 1.cbz").await.unwrap();

    let mut restarted = Library::new(
        fixture.root().to_path_buf(),
        fixture.storage.clone(),
        &fixture.config,
    );
    assert!(restarted.try_load_from_cache().await.unwrap());
    assert!(restarted.get_title(&title_id).is_some());
    assert!(restarted.get_entry(&title_id, &entry_id).is_none());
}
