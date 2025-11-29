/// Utility functions shared across the codebase
use crate::error::{Error, Result};
use serde::Deserialize;
use std::path::Path;

/// Calculate file signature (inode on Unix, CRC32 hash on Windows)
/// Returns as String for Mango database compatibility
#[cfg(unix)]
pub fn file_signature(path: &Path) -> Result<String> {
    use std::os::unix::fs::MetadataExt;
    let metadata = std::fs::metadata(path)?;
    Ok(metadata.ino().to_string())
}

/// Calculate file signature using CRC32 hash of path + file size
/// Used on Windows and other non-Unix systems
/// Returns as String for Mango database compatibility
#[cfg(not(unix))]
pub fn file_signature(path: &Path) -> Result<String> {
    use crc32fast::Hasher;

    let metadata = std::fs::metadata(path)?;
    let mut hasher = Hasher::new();

    // Hash path + file size as signature
    hasher.update(path.to_string_lossy().as_bytes());
    hasher.update(&metadata.len().to_le_bytes());

    Ok((hasher.finalize() as u64).to_string())
}

/// Query parameters for sorting
#[derive(Deserialize)]
pub struct SortParams {
    /// Optional sort method (title, modified, auto, progress)
    pub sort: Option<String>,
    /// Optional ascend flag (1 for ascending, 0 for descending)
    pub ascend: Option<String>,
}

/// Navigation state for templates
/// Tracks which page is currently active in the navigation menu
#[derive(Debug, Clone)]
pub struct NavigationState {
    pub home_active: bool,
    pub library_active: bool,
    pub admin_active: bool,
}

impl NavigationState {
    /// Create navigation state with home page active
    pub fn home() -> Self {
        Self {
            home_active: true,
            library_active: false,
            admin_active: false,
        }
    }

    /// Create navigation state with library page active
    pub fn library() -> Self {
        Self {
            home_active: false,
            library_active: true,
            admin_active: false,
        }
    }

    /// Create navigation state with admin page active
    pub fn admin() -> Self {
        Self {
            home_active: false,
            library_active: false,
            admin_active: true,
        }
    }
}

/// Helper function to convert template render errors to Error::Internal
/// Use this instead of duplicating error handling across route handlers
pub fn render_error<E: std::fmt::Display>(e: E) -> Error {
    Error::Internal(format!("Template render error: {}", e))
}

/// Get sort preferences for a user from info.json
/// If query params are provided, saves them and returns them
/// Otherwise, returns saved preferences or defaults
///
/// Returns (sort_method, ascending) tuple
pub async fn get_and_save_sort(
    dir: &Path,
    username: &str,
    params: &SortParams,
) -> Result<(String, bool)> {
    use crate::library::progress::TitleInfo;

    let mut info = TitleInfo::load(dir).await?;

    // If query params exist, use them and save to info.json
    if let Some(method) = &params.sort {
        let ascending = params
            .ascend
            .as_ref()
            .and_then(|s| s.parse::<i32>().ok())
            .map(|v| v != 0)
            .unwrap_or(true);

        info.set_sort_by(username, method, ascending);
        info.save(dir).await?;

        return Ok((method.clone(), ascending));
    }

    // Otherwise, load saved preferences or use defaults
    if let Some((method, ascending)) = info.get_sort_by(username) {
        Ok((method, ascending))
    } else {
        // Default: sort by title ascending
        Ok(("title".to_string(), true))
    }
}
