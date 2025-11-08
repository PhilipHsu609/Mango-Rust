use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::fs;

use crate::error::Result;

/// Application configuration matching original Mango's config.yml structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Host to bind to (default: 0.0.0.0)
    #[serde(default = "default_host")]
    pub host: String,

    /// Port to bind to (default: 9000)
    #[serde(default = "default_port")]
    pub port: u16,

    /// Base URL path (default: /)
    #[serde(default = "default_base_url")]
    pub base_url: String,

    /// Session secret for cookie signing
    #[serde(default = "default_session_secret")]
    pub session_secret: String,

    /// Path to manga library directory
    #[serde(default = "default_library_path")]
    pub library_path: PathBuf,

    /// Path to SQLite database
    #[serde(default = "default_db_path")]
    pub db_path: PathBuf,

    /// Path to queue database (for downloads - Tier 3)
    #[serde(default = "default_queue_db_path")]
    pub queue_db_path: PathBuf,

    /// Library scan interval in minutes (0 = manual only)
    #[serde(default = "default_scan_interval")]
    pub scan_interval_minutes: u32,

    /// Thumbnail generation interval in hours (0 = manual only)
    #[serde(default = "default_thumbnail_interval")]
    pub thumbnail_generation_interval_hours: u32,

    /// Log level (trace, debug, info, warn, error)
    #[serde(default = "default_log_level")]
    pub log_level: String,

    /// Path for uploaded files
    #[serde(default = "default_upload_path")]
    pub upload_path: PathBuf,

    /// Path to plugins directory (Tier 3)
    #[serde(default = "default_plugin_path")]
    pub plugin_path: PathBuf,

    /// Download timeout in seconds
    #[serde(default = "default_download_timeout")]
    pub download_timeout_seconds: u64,

    /// Path to library cache file (Tier 3 performance)
    #[serde(default = "default_library_cache_path")]
    pub library_cache_path: PathBuf,

    /// Enable library caching (Tier 3)
    #[serde(default = "default_true")]
    pub cache_enabled: bool,

    /// Cache size in megabytes
    #[serde(default = "default_cache_size")]
    pub cache_size_mbs: usize,

    /// Enable cache logging
    #[serde(default = "default_true")]
    pub cache_log_enabled: bool,

    /// Disable login requirement (use with default_username)
    #[serde(default)]
    pub disable_login: bool,

    /// Default username when login is disabled
    #[serde(default)]
    pub default_username: Option<String>,

    /// Header name for auth proxy support
    #[serde(default)]
    pub auth_proxy_header_name: Option<String>,

    /// Plugin update interval in hours (Tier 3)
    #[serde(default = "default_plugin_update_interval")]
    pub plugin_update_interval_hours: u32,
}

// Default value functions
fn default_host() -> String { "0.0.0.0".to_string() }
fn default_port() -> u16 { 9000 }
fn default_base_url() -> String { "/".to_string() }
fn default_session_secret() -> String { "mango-session-secret".to_string() }
fn default_library_path() -> PathBuf { expand_home("~/mango/library") }
fn default_db_path() -> PathBuf { expand_home("~/mango/mango.db") }
fn default_queue_db_path() -> PathBuf { expand_home("~/mango/queue.db") }
fn default_scan_interval() -> u32 { 5 }
fn default_thumbnail_interval() -> u32 { 24 }
fn default_log_level() -> String { "info".to_string() }
fn default_upload_path() -> PathBuf { expand_home("~/mango/uploads") }
fn default_plugin_path() -> PathBuf { expand_home("~/mango/plugins") }
fn default_download_timeout() -> u64 { 30 }
fn default_library_cache_path() -> PathBuf { expand_home("~/mango/library.yml.gz") }
fn default_true() -> bool { true }
fn default_cache_size() -> usize { 50 }
fn default_plugin_update_interval() -> u32 { 24 }

impl Config {
    /// Load configuration from file, with fallback to defaults
    /// Precedence: config file > environment variables > defaults
    pub fn load(path: Option<&str>) -> Result<Self> {
        let config_path = path.unwrap_or("~/.config/mango/config.yml");
        let expanded_path = expand_home(config_path);

        let mut config = if expanded_path.exists() {
            tracing::info!("Loading config from: {}", expanded_path.display());
            let content = fs::read_to_string(&expanded_path)?;
            serde_yaml::from_str::<Config>(&content)
                .map_err(|e| crate::error::Error::Config(format!("Failed to parse config: {}", e)))?
        } else {
            tracing::warn!("Config file not found at {}, using defaults", expanded_path.display());
            Self::default_config()
        };

        // Apply environment variable overrides
        config.apply_env_overrides();

        // Expand all path fields
        config.expand_paths();

        // Validate configuration
        config.validate()?;

        // Create config file if it doesn't exist
        if !expanded_path.exists() {
            config.save_default(&expanded_path)?;
        }

        Ok(config)
    }

    /// Create default configuration
    fn default_config() -> Self {
        Config {
            host: default_host(),
            port: default_port(),
            base_url: default_base_url(),
            session_secret: default_session_secret(),
            library_path: default_library_path(),
            db_path: default_db_path(),
            queue_db_path: default_queue_db_path(),
            scan_interval_minutes: default_scan_interval(),
            thumbnail_generation_interval_hours: default_thumbnail_interval(),
            log_level: default_log_level(),
            upload_path: default_upload_path(),
            plugin_path: default_plugin_path(),
            download_timeout_seconds: default_download_timeout(),
            library_cache_path: default_library_cache_path(),
            cache_enabled: default_true(),
            cache_size_mbs: default_cache_size(),
            cache_log_enabled: default_true(),
            disable_login: false,
            default_username: None,
            auth_proxy_header_name: None,
            plugin_update_interval_hours: default_plugin_update_interval(),
        }
    }

    /// Apply environment variable overrides (matching Crystal's precedence)
    fn apply_env_overrides(&mut self) {
        if let Ok(val) = std::env::var("MANGO_HOST") {
            self.host = val;
        }
        if let Ok(val) = std::env::var("MANGO_PORT") {
            if let Ok(port) = val.parse() {
                self.port = port;
            }
        }
        if let Ok(val) = std::env::var("MANGO_BASE_URL") {
            self.base_url = val;
        }
        if let Ok(val) = std::env::var("MANGO_LIBRARY_PATH") {
            self.library_path = PathBuf::from(val);
        }
        if let Ok(val) = std::env::var("MANGO_DB_PATH") {
            self.db_path = PathBuf::from(val);
        }
        if let Ok(val) = std::env::var("MANGO_LOG_LEVEL") {
            self.log_level = val;
        }
    }

    /// Expand ~ in all path fields
    fn expand_paths(&mut self) {
        self.library_path = expand_home_path(&self.library_path);
        self.db_path = expand_home_path(&self.db_path);
        self.queue_db_path = expand_home_path(&self.queue_db_path);
        self.upload_path = expand_home_path(&self.upload_path);
        self.plugin_path = expand_home_path(&self.plugin_path);
        self.library_cache_path = expand_home_path(&self.library_cache_path);
    }

    /// Validate configuration
    fn validate(&self) -> Result<()> {
        // base_url must start and end with /
        if !self.base_url.starts_with('/') {
            return Err(crate::error::Error::Config(
                format!("base_url must start with '/', got: {}", self.base_url)
            ));
        }

        let mut url = self.base_url.clone();
        if !url.ends_with('/') {
            url.push('/');
        }

        // If login is disabled, default_username must be set
        if self.disable_login && self.default_username.is_none() {
            return Err(crate::error::Error::Config(
                "disable_login is true but default_username is not set".to_string()
            ));
        }

        Ok(())
    }

    /// Save default configuration to file
    fn save_default(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let yaml = serde_yaml::to_string(self)
            .map_err(|e| crate::error::Error::Config(format!("Failed to serialize config: {}", e)))?;

        fs::write(path, yaml)?;
        tracing::info!("Created default config at: {}", path.display());

        Ok(())
    }

    /// Get the database URL for SQLx
    pub fn database_url(&self) -> String {
        format!("sqlite://{}", self.db_path.display())
    }
}

/// Expand ~ to home directory in a string path
fn expand_home(path: &str) -> PathBuf {
    if path.starts_with("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(&path[2..]);
        }
    }
    PathBuf::from(path)
}

/// Expand ~ in a PathBuf
fn expand_home_path(path: &PathBuf) -> PathBuf {
    if let Some(path_str) = path.to_str() {
        expand_home(path_str)
    } else {
        path.clone()
    }
}

// Add dirs crate for home directory expansion
// This needs to be added to Cargo.toml
