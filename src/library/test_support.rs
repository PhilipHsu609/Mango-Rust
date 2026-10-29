use crate::{Config, Storage};

/// A temporary library folder, database, and config for scanner tests.
/// `cache` turns the persistent library cache on (scans then save it).
pub(crate) async fn scan_config(cache: bool) -> (tempfile::TempDir, Config, Storage) {
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
        cache_enabled: cache,
        cache_size_mbs: if cache { 4 } else { 0 },
        cache_log_enabled: false,
        disable_login: false,
        default_username: String::new(),
        auth_proxy_header_name: String::new(),
        plugin_update_interval_hours: 24,
    };
    (temp, config, storage)
}
