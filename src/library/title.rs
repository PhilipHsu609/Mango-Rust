use std::path::{Path, PathBuf};
use uuid::Uuid;

use crate::error::Result;
use super::entry::Entry;
use super::library::SortMethod;

/// Represents a manga series (directory containing chapters/volumes)
#[derive(Debug, Clone)]
pub struct Title {
    /// Unique identifier (persisted in database)
    pub id: String,

    /// Absolute path to the title directory
    pub path: PathBuf,

    /// Display name (directory name by default)
    pub title: String,

    /// Directory signature (CRC32 of file inodes)
    pub signature: u64,

    /// Contents signature (SHA1 of filenames) for change detection
    pub contents_signature: String,

    /// Modification time (latest mtime of all entries)
    pub mtime: i64,

    /// List of entries (chapters/volumes) in this title
    pub entries: Vec<Entry>,

    /// Parent title ID (empty for top-level titles)
    pub parent_id: Option<String>,

    /// Nested titles (for multi-level organization like "Series > Volume > Chapters")
    pub nested_titles: Vec<Title>,
}

impl Title {
    /// Create a new Title by scanning a directory
    pub async fn from_directory(path: PathBuf) -> Result<Self> {
        let title = path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("Unknown")
            .to_string();

        let mut entries = Vec::new();
        let nested_titles = Vec::new();

        // Scan directory contents
        let mut dir_entries = tokio::fs::read_dir(&path).await?;

        while let Some(entry) = dir_entries.next_entry().await? {
            let entry_path = entry.path();

            if entry_path.is_dir() {
                // For Week 2: treat subdirectories as nested titles (simplified)
                // TODO Week 5: Add proper nested title support
                continue;
            } else if is_archive(&entry_path) {
                // It's a manga chapter/volume archive
                let mut manga_entry = Entry::from_archive(entry_path).await?;
                manga_entry.calculate_signature()?;
                entries.push(manga_entry);
            }
        }

        // Sort entries by title (natural ordering)
        entries.sort_by(|a, b| natord::compare(&a.title, &b.title));

        // Calculate latest mtime
        let mtime = entries
            .iter()
            .map(|e| e.mtime)
            .max()
            .unwrap_or(0);

        // Calculate signatures
        let signature = calculate_dir_signature(&path)?;
        let contents_signature = calculate_contents_signature(&path)?;

        Ok(Self {
            id: Uuid::new_v4().to_string(),
            path,
            title,
            signature,
            contents_signature,
            mtime,
            entries,
            parent_id: None,
            nested_titles,
        })
    }

    /// Get total number of pages across all entries
    pub fn total_pages(&self) -> usize {
        self.entries.iter().map(|e| e.pages).sum()
    }

    /// Get entries sorted by specified method
    pub fn get_entries_sorted(&self, method: SortMethod) -> Vec<&Entry> {
        let mut entries: Vec<&Entry> = self.entries.iter().collect();

        match method {
            SortMethod::Name => {
                entries.sort_by(|a, b| natord::compare(&a.title, &b.title));
            }
            SortMethod::NameReverse => {
                entries.sort_by(|a, b| natord::compare(&b.title, &a.title));
            }
            SortMethod::TimeModified => {
                // Newest first
                entries.sort_by(|a, b| b.mtime.cmp(&a.mtime));
            }
            SortMethod::TimeModifiedReverse => {
                // Oldest first
                entries.sort_by(|a, b| a.mtime.cmp(&b.mtime));
            }
            SortMethod::Auto => {
                // For now, use name sorting
                // Future: smart chapter detection
                entries.sort_by(|a, b| natord::compare(&a.title, &b.title));
            }
        }

        entries
    }

    /// Get all entries recursively (including nested titles)
    pub fn deep_entries(&self) -> Vec<&Entry> {
        let mut all_entries = Vec::new();

        // Add own entries
        for entry in &self.entries {
            all_entries.push(entry);
        }

        // Add nested title entries
        for nested in &self.nested_titles {
            all_entries.extend(nested.deep_entries());
        }

        all_entries
    }
}

/// Check if a file is a supported archive format
fn is_archive(path: &Path) -> bool {
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        let ext_lower = ext.to_lowercase();
        ext_lower == "zip" || ext_lower == "cbz"
        // Week 4 will add: || ext_lower == "rar" || ext_lower == "cbr"
    } else {
        false
    }
}

/// Calculate directory signature (CRC32 of all file inodes, sorted)
/// Matches original Mango's Dir.signature behavior
fn calculate_dir_signature(path: &Path) -> Result<u64> {
    use crc32fast::Hasher;
    use std::fs;

    let mut signatures = Vec::new();

    // Collect signatures of all archive files
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let entry_path = entry.path();

        if entry_path.is_file() && is_archive(&entry_path) {
            let sig = file_signature(&entry_path)?;
            signatures.push(sig);
        }
    }

    // Sort signatures
    signatures.sort_unstable();

    // CRC32 of all signatures
    let mut hasher = Hasher::new();
    for sig in signatures {
        hasher.update(&sig.to_le_bytes());
    }

    Ok(hasher.finalize() as u64)
}

/// Calculate contents signature (SHA1 of all filenames, sorted)
/// Used for detecting when directory contents changed
fn calculate_contents_signature(path: &Path) -> Result<String> {
    use sha1::{Sha1, Digest};
    use std::fs;

    let mut filenames = Vec::new();

    // Collect all archive filenames
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let entry_path = entry.path();

        if entry_path.is_file() && is_archive(&entry_path) {
            if let Some(name) = entry_path.file_name().and_then(|n| n.to_str()) {
                filenames.push(name.to_string());
            }
        }
    }

    // Sort filenames
    filenames.sort();

    // SHA1 of concatenated names
    let mut hasher = Sha1::new();
    for name in filenames {
        hasher.update(name.as_bytes());
    }

    Ok(format!("{:x}", hasher.finalize()))
}

/// Calculate file signature (inode on Unix, CRC32 hash on Windows)
#[cfg(unix)]
fn file_signature(path: &Path) -> Result<u64> {
    use std::os::unix::fs::MetadataExt;
    let metadata = std::fs::metadata(path)?;
    Ok(metadata.ino())
}

#[cfg(not(unix))]
fn file_signature(path: &Path) -> Result<u64> {
    use crc32fast::Hasher;

    let metadata = std::fs::metadata(path)?;
    let mut hasher = Hasher::new();

    hasher.update(path.to_string_lossy().as_bytes());
    hasher.update(&metadata.len().to_le_bytes());

    Ok(hasher.finalize() as u64)
}
