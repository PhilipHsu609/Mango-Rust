/// Utility functions shared across the codebase
use crate::error::{Error, Result};
use serde::Deserialize;
use std::path::Path;

/// Calculate file signature (inode on Unix, CRC32 hash on Windows)
/// Matches original Mango's file signature behavior
#[cfg(unix)]
pub fn file_signature(path: &Path) -> Result<u64> {
    use std::os::unix::fs::MetadataExt;
    let metadata = std::fs::metadata(path)?;
    Ok(metadata.ino())
}

/// Calculate file signature using CRC32 hash of path + file size
/// Used on Windows and other non-Unix systems
#[cfg(not(unix))]
pub fn file_signature(path: &Path) -> Result<u64> {
    use crc32fast::Hasher;

    let metadata = std::fs::metadata(path)?;
    let mut hasher = Hasher::new();

    // Hash path + file size as signature
    hasher.update(path.to_string_lossy().as_bytes());
    hasher.update(&metadata.len().to_le_bytes());

    Ok(hasher.finalize() as u64)
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
