use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

use crate::error::Result;

/// Application configuration matching original Mango's config.yml structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Host to bind to (default: 0.0.0.0)
    pub host: String,

    /// Port to bind to (default: 9000)
    pub port: u16,

    /// Base URL path (default: /)
    pub base_url: String,

    /// Session secret for cookie signing
    pub session_secret: String,

    /// Path to manga library directory
    pub library_path: PathBuf,

    /// Path to SQLite database
    pub db_path: PathBuf,

    /// Path to queue database (for downloads - Tier 3)
    pub queue_db_path: PathBuf,

    /// Library scan interval in minutes (0 = manual only)
    pub scan_interval_minutes: u32,

    /// Thumbnail generation interval in hours (0 = manual only)
    pub thumbnail_generation_interval_hours: u32,

    /// Log level (trace, debug, info, warn, error)
    pub log_level: String,

    /// Path for uploaded files
    pub upload_path: PathBuf,

    /// Path to plugins directory (Tier 3)
    pub plugin_path: PathBuf,

    /// Download timeout in seconds
    pub download_timeout_seconds: u64,

    /// Path to library cache file (Tier 3 performance)
    pub library_cache_path: PathBuf,

    /// Enable library caching (Tier 3)
    pub cache_enabled: bool,

    /// Cache size in megabytes
    pub cache_size_mbs: usize,

    /// Enable cache logging
    pub cache_log_enabled: bool,

    /// Disable login requirement (use with default_username)
    pub disable_login: bool,

    /// Default username when login is disabled
    pub default_username: String,

    /// Header name for auth proxy support
    pub auth_proxy_header_name: String,

    /// Plugin update interval in hours (Tier 3)
    pub plugin_update_interval_hours: u32,
}

impl Config {
    /// Load configuration with file > environment > defaults precedence.
    pub fn load(path: Option<&str>) -> Result<Self> {
        let config_path = path
            .map(str::to_owned)
            .or_else(|| std::env::var("CONFIG_PATH").ok())
            .unwrap_or_else(|| "~/.config/mango/config.yml".to_string());
        let expanded_path = expand_home(&config_path);

        let defaults: [(&str, ::config::Value); 21] = [
            ("host", "0.0.0.0".into()),
            ("port", 9000_u16.into()),
            ("base_url", "/".into()),
            ("session_secret", "mango-session-secret".into()),
            ("library_path", "~/mango/library".into()),
            ("db_path", "~/mango.db".into()),
            ("queue_db_path", "~/mango/queue.db".into()),
            ("scan_interval_minutes", 5_u32.into()),
            ("thumbnail_generation_interval_hours", 24_u32.into()),
            ("log_level", "info".into()),
            ("upload_path", "~/mango/uploads".into()),
            ("plugin_path", "~/mango/plugins".into()),
            ("download_timeout_seconds", 30_u64.into()),
            ("library_cache_path", "~/mango/library.yml.gz".into()),
            ("cache_enabled", true.into()),
            ("cache_size_mbs", 50_u64.into()),
            ("cache_log_enabled", true.into()),
            ("disable_login", false.into()),
            ("default_username", "".into()),
            ("auth_proxy_header_name", "".into()),
            ("plugin_update_interval_hours", 24_u32.into()),
        ];
        let builder = defaults
            .into_iter()
            .try_fold(::config::Config::builder(), |builder, (key, value)| {
                builder.set_default(key, value)
            })
            .map_err(|error| crate::error::Error::Config(error.to_string()))?;

        let settings = builder
            .add_source(
                ::config::Environment::default()
                    .convert_case(::config::Case::Snake)
                    .try_parsing(true),
            )
            .add_source(
                ::config::File::from(expanded_path.clone())
                    .format(::config::FileFormat::Yaml)
                    .required(false),
            )
            .build()
            .map_err(|error| crate::error::Error::Config(error.to_string()))?;
        let mut config: Config = settings
            .try_deserialize()
            .map_err(|error| crate::error::Error::Config(error.to_string()))?;

        config.expand_paths();
        config.validate()?;

        if !expanded_path.exists() {
            config.save_default(&expanded_path)?;
        }

        Ok(config)
    }
    /// Expand ~ in all path fields
    fn expand_paths(&mut self) {
        self.library_path = expand_home(&self.library_path);
        self.db_path = expand_home(&self.db_path);
        self.queue_db_path = expand_home(&self.queue_db_path);
        self.upload_path = expand_home(&self.upload_path);
        self.plugin_path = expand_home(&self.plugin_path);
        self.library_cache_path = expand_home(&self.library_cache_path);
    }

    /// Validate configuration
    fn validate(&mut self) -> Result<()> {
        if !self.base_url.starts_with('/') {
            return Err(crate::error::Error::Config(format!(
                "base_url must start with '/', got: {}",
                self.base_url
            )));
        }
        if !self.base_url.ends_with('/') {
            self.base_url.push('/');
        }

        if self.disable_login && self.default_username.is_empty() {
            return Err(crate::error::Error::Config(
                "disable_login is true but default_username is not set".to_string(),
            ));
        }

        Ok(())
    }

    /// Save default configuration to file
    fn save_default(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let yaml = serde_yaml::to_string(self).map_err(|e| {
            crate::error::Error::Config(format!("Failed to serialize config: {}", e))
        })?;

        fs::write(path, yaml)?;
        tracing::info!("Created default config at: {}", path.display());

        Ok(())
    }

    /// Get the SQLite URL used to create or open the configured database.
    pub fn database_url(&self) -> String {
        format!("sqlite://{}?mode=rwc", self.db_path.display())
    }
}

/// Expand a leading `~/` against the current user's home directory.
fn expand_home(path: impl AsRef<Path>) -> PathBuf {
    let path = path.as_ref();
    if let Some(path_str) = path.to_str() {
        if let Some(stripped) = path_str.strip_prefix("~/") {
            if let Some(home) = dirs::home_dir() {
                return home.join(stripped);
            }
        }
    }
    path.to_path_buf()
}

#[cfg(test)]
mod tests {
    use super::*;
    use parking_lot::Mutex;
    use std::ffi::OsString;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    struct EnvRestore(Vec<(&'static str, Option<OsString>)>);

    impl EnvRestore {
        fn capture(keys: &[&'static str]) -> Self {
            Self(
                keys.iter()
                    .map(|key| (*key, std::env::var_os(key)))
                    .collect(),
            )
        }
    }

    impl Drop for EnvRestore {
        fn drop(&mut self) {
            for (key, value) in &self.0 {
                if let Some(value) = value {
                    std::env::set_var(key, value);
                } else {
                    std::env::remove_var(key);
                }
            }
        }
    }

    #[test]
    fn config_uses_config_path_environment_and_file_precedence() {
        let _lock = ENV_LOCK.lock();
        let keys = [
            "CONFIG_PATH",
            "HOST",
            "PORT",
            "BASE_URL",
            "SESSION_SECRET",
            "LIBRARY_PATH",
            "LIBRARY_CACHE_PATH",
            "DB_PATH",
            "QUEUE_DB_PATH",
            "SCAN_INTERVAL_MINUTES",
            "THUMBNAIL_GENERATION_INTERVAL_HOURS",
            "LOG_LEVEL",
            "UPLOAD_PATH",
            "PLUGIN_PATH",
            "DOWNLOAD_TIMEOUT_SECONDS",
            "CACHE_ENABLED",
            "CACHE_SIZE_MBS",
            "cache_log_enabled",
            "DISABLE_LOGIN",
            "DEFAULT_USERNAME",
            "AUTH_PROXY_HEADER_NAME",
            "PLUGIN_UPDATE_INTERVAL_HOURS",
        ];
        let _restore = EnvRestore::capture(&keys);
        let directory = tempfile::tempdir().unwrap();
        let config_path = directory.path().join("config.yml");
        std::fs::write(
            &config_path,
            "port: 4100\nbase_url: /reader\nlog_level: file\n",
        )
        .unwrap();

        for (key, value) in [
            ("CONFIG_PATH", config_path.to_str().unwrap()),
            ("HOST", "127.0.0.1"),
            ("PORT", "5100"),
            ("BASE_URL", "/environment"),
            ("SESSION_SECRET", "123"),
            ("LIBRARY_PATH", "/environment/library"),
            ("LIBRARY_CACHE_PATH", "/environment/library-cache"),
            ("DB_PATH", "/environment/mango.db"),
            ("QUEUE_DB_PATH", "/environment/queue.db"),
            ("SCAN_INTERVAL_MINUTES", "17"),
            ("THUMBNAIL_GENERATION_INTERVAL_HOURS", "23"),
            ("LOG_LEVEL", "environment"),
            ("UPLOAD_PATH", "/environment/uploads"),
            ("PLUGIN_PATH", "/environment/plugins"),
            ("DOWNLOAD_TIMEOUT_SECONDS", "31"),
            ("CACHE_ENABLED", "FALSE"),
            ("CACHE_SIZE_MBS", "77"),
            ("cache_log_enabled", "TRUE"),
            ("DISABLE_LOGIN", "1"),
            ("DEFAULT_USERNAME", "testuser"),
            ("AUTH_PROXY_HEADER_NAME", "X-Remote-User"),
            ("PLUGIN_UPDATE_INTERVAL_HOURS", "13"),
        ] {
            std::env::set_var(key, value);
        }

        let config = Config::load(None).unwrap();

        assert_eq!(config.host, "127.0.0.1");
        assert_eq!(config.port, 4100);
        assert_eq!(config.base_url, "/reader/");
        assert_eq!(config.session_secret, "123");
        assert_eq!(config.library_path, PathBuf::from("/environment/library"));
        assert_eq!(
            config.library_cache_path,
            PathBuf::from("/environment/library-cache")
        );
        assert_eq!(config.db_path, PathBuf::from("/environment/mango.db"));
        assert_eq!(config.queue_db_path, PathBuf::from("/environment/queue.db"));
        assert_eq!(config.scan_interval_minutes, 17);
        assert_eq!(config.thumbnail_generation_interval_hours, 23);
        assert_eq!(config.log_level, "file");
        assert_eq!(config.upload_path, PathBuf::from("/environment/uploads"));
        assert_eq!(config.plugin_path, PathBuf::from("/environment/plugins"));
        assert_eq!(config.download_timeout_seconds, 31);
        assert!(!config.cache_enabled);
        assert_eq!(config.cache_size_mbs, 77);
        assert!(config.cache_log_enabled);
        assert!(config.disable_login);
        assert_eq!(config.default_username, "testuser");
        assert_eq!(config.auth_proxy_header_name, "X-Remote-User");
        assert_eq!(config.plugin_update_interval_hours, 13);
        let expected_db_path = expand_home("~/mango.db");
        for key in keys {
            std::env::remove_var(key);
        }
        let defaults = Config::load(Some(config_path.to_str().unwrap())).unwrap();
        assert_eq!(defaults.host, "0.0.0.0");
        assert_eq!(defaults.port, 4100);
        assert_eq!(defaults.db_path, expected_db_path);
        assert!(defaults.cache_enabled);
        assert!(defaults.cache_log_enabled);
        assert!(!defaults.disable_login);
    }
}
