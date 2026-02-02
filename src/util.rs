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

/// Get directory inode (Unix only)
#[cfg(unix)]
fn dir_inode(path: &Path) -> Result<String> {
    use std::os::unix::fs::MetadataExt;
    let metadata = std::fs::metadata(path)?;
    Ok(metadata.ino().to_string())
}

/// Get directory signature using CRC32 (Windows fallback)
#[cfg(not(unix))]
fn dir_inode(path: &Path) -> Result<String> {
    use crc32fast::Hasher;
    let mut hasher = Hasher::new();
    hasher.update(path.to_string_lossy().as_bytes());
    Ok((hasher.finalize() as u64).to_string())
}

/// Calculate directory signature recursively (matches original Mango behavior)
/// Includes:
/// - Directory's own inode
/// - All supported file inodes
/// - All nested directory signatures (recursive)
///
/// Returns CRC32 checksum as String
pub fn dir_signature(path: &Path) -> Result<String> {
    let mut signatures = Vec::new();

    // Include directory's own inode
    signatures.push(dir_inode(path)?);

    // Recursively collect all signatures
    let entries = std::fs::read_dir(path)?;
    for entry in entries {
        let entry = entry?;
        let entry_path = entry.path();
        let name = entry.file_name();
        let name_str = name.to_string_lossy();

        // Skip hidden files
        if name_str.starts_with('.') {
            continue;
        }

        if entry_path.is_dir() {
            // Recursively get subdirectory signature
            signatures.push(dir_signature(&entry_path)?);
        } else if is_supported_file(&entry_path) {
            // Get file signature
            let sig = file_signature(&entry_path)?;
            // Only add if non-zero (original Mango behavior)
            if sig != "0" {
                signatures.push(sig);
            }
        }
    }

    // Sort signatures
    signatures.sort();

    // Join and calculate CRC32 (matching original: Digest::CRC32.checksum(signatures.sort.join))
    let joined = signatures.join("");
    let checksum = crc32fast::hash(joined.as_bytes());

    Ok((checksum as u64).to_string())
}

// ============================================================================
// File Type Detection Constants
// ============================================================================

/// Archive formats that can be extracted by the ZIP library (what we can actually READ)
/// When adding support for new formats (e.g., RAR), update the extraction code in
/// entry.rs first, then move the extensions here from ALL_ARCHIVE_EXTENSIONS
pub const EXTRACTABLE_ARCHIVE_EXTENSIONS: &[&str] = &["zip", "cbz", "rar", "cbr", "7z", "cb7"];

/// All archive formats we recognize (may not all be extractable yet)
/// Used for file signature calculation and future format support
pub const ALL_ARCHIVE_EXTENSIONS: &[&str] =
    &["zip", "cbz", "rar", "cbr", "7z", "cb7", "tar", "cbt"];

/// Image formats we can display
pub const IMAGE_EXTENSIONS: &[&str] = &["jpg", "jpeg", "png", "gif", "webp", "bmp"];

/// Check if file is a supported archive or image file
/// Used for directory signature calculation - recognizes all media types
fn is_supported_file(path: &Path) -> bool {
    if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
        let ext_lower = ext.to_lowercase();
        ALL_ARCHIVE_EXTENSIONS.contains(&ext_lower.as_str())
            || IMAGE_EXTENSIONS.contains(&ext_lower.as_str())
    } else {
        false
    }
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
/// and user permission level for conditional UI rendering
#[derive(Debug, Clone, serde::Serialize)]
pub struct NavigationState {
    pub home_active: bool,
    pub library_active: bool,
    pub tags_active: bool,
    pub admin_active: bool,
    pub is_admin: bool,
}

impl NavigationState {
    /// Create navigation state with home page active
    pub fn home() -> Self {
        Self {
            home_active: true,
            library_active: false,
            tags_active: false,
            admin_active: false,
            is_admin: false,
        }
    }

    /// Create navigation state with library page active
    pub fn library() -> Self {
        Self {
            home_active: false,
            library_active: true,
            tags_active: false,
            admin_active: false,
            is_admin: false,
        }
    }

    /// Create navigation state with tags page active
    pub fn tags() -> Self {
        Self {
            home_active: false,
            library_active: false,
            tags_active: true,
            admin_active: false,
            is_admin: false,
        }
    }

    /// Create navigation state with admin page active
    pub fn admin() -> Self {
        Self {
            home_active: false,
            library_active: false,
            tags_active: false,
            admin_active: true,
            is_admin: false,
        }
    }

    /// Builder method to set admin permission status
    /// Use this to indicate whether the current user has admin privileges
    pub fn with_admin(mut self, is_admin: bool) -> Self {
        self.is_admin = is_admin;
        self
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_navigation_state_home() {
        let nav = NavigationState::home();
        assert!(nav.home_active);
        assert!(!nav.library_active);
        assert!(!nav.tags_active);
        assert!(!nav.admin_active);
        assert!(!nav.is_admin);
    }

    #[test]
    fn test_navigation_state_library() {
        let nav = NavigationState::library();
        assert!(!nav.home_active);
        assert!(nav.library_active);
        assert!(!nav.tags_active);
        assert!(!nav.admin_active);
        assert!(!nav.is_admin);
    }

    #[test]
    fn test_navigation_state_tags() {
        let nav = NavigationState::tags();
        assert!(!nav.home_active);
        assert!(!nav.library_active);
        assert!(nav.tags_active);
        assert!(!nav.admin_active);
        assert!(!nav.is_admin);
    }

    #[test]
    fn test_navigation_state_admin() {
        let nav = NavigationState::admin();
        assert!(!nav.home_active);
        assert!(!nav.library_active);
        assert!(!nav.tags_active);
        assert!(nav.admin_active);
        assert!(!nav.is_admin);
    }

    #[test]
    fn test_navigation_state_with_admin() {
        let nav = NavigationState::library().with_admin(true);
        assert!(!nav.home_active);
        assert!(nav.library_active);
        assert!(!nav.tags_active);
        assert!(!nav.admin_active);
        assert!(nav.is_admin);
    }

    #[test]
    fn test_navigation_state_builder_chain() {
        // Test that builder pattern works
        let nav_admin = NavigationState::home().with_admin(true);
        assert!(nav_admin.is_admin);

        let nav_regular = NavigationState::home().with_admin(false);
        assert!(!nav_regular.is_admin);
    }
}
