use std::path::{Path, PathBuf};
use uuid::Uuid;

use super::entry::Entry;
use super::manager::SortMethod;
use crate::error::Result;

/// Represents a manga series (directory containing chapters/volumes)
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
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
        let mtime = entries.iter().map(|e| e.mtime).max().unwrap_or(0);

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

    /// Get entries sorted by specified method and order
    pub fn get_entries_sorted(&self, method: SortMethod, ascending: bool) -> Vec<&Entry> {
        let mut entries: Vec<&Entry> = self.entries.iter().collect();

        use super::{sort_by_mtime, sort_by_name};

        match method {
            SortMethod::Name | SortMethod::Progress | SortMethod::Auto => {
                // Progress sorting doesn't apply to entries (only at route level with username context)
                // Auto uses name sorting (future: smart chapter detection)
                sort_by_name(&mut entries, ascending);
            }
            SortMethod::TimeModified => {
                sort_by_mtime(&mut entries, ascending);
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

    /// Save reading progress for an entry
    pub async fn save_entry_progress(
        &self,
        username: &str,
        entry_id: &str,
        page: usize,
    ) -> Result<()> {
        use super::progress::TitleInfo;

        let mut info = TitleInfo::load(&self.path).await?;

        // If page is 0, remove the progress (mark as unread)
        if page == 0 {
            info.remove_progress(username, entry_id);
        } else {
            info.set_progress(username, entry_id, page);
        }

        info.save(&self.path).await?;
        Ok(())
    }

    /// Load reading progress for an entry
    pub async fn load_entry_progress(&self, username: &str, entry_id: &str) -> Result<usize> {
        use super::progress::TitleInfo;

        let info = TitleInfo::load(&self.path).await?;
        Ok(info.get_progress(username, entry_id).unwrap_or(0))
    }

    /// Get progress information for an entry (percentage and page number)
    pub async fn get_entry_progress(&self, username: &str, entry_id: &str) -> Result<(f32, usize)> {
        // Find the entry to get its page count
        let entry = self
            .entries
            .iter()
            .find(|e| e.id == entry_id)
            .ok_or_else(|| {
                crate::error::Error::NotFound(format!("Entry not found: {}", entry_id))
            })?;

        let page = self.load_entry_progress(username, entry_id).await?;
        let percentage = if entry.pages > 0 {
            (page as f32 / entry.pages as f32) * 100.0
        } else {
            0.0
        };

        Ok((percentage, page))
    }

    /// Mark all entries as read
    pub async fn read_all(&self, username: &str) -> Result<()> {
        use super::progress::TitleInfo;

        let mut info = TitleInfo::load(&self.path).await?;

        // Set progress to last page for all entries
        for entry in &self.entries {
            info.set_progress(username, &entry.id, entry.pages);
        }

        info.save(&self.path).await?;
        Ok(())
    }

    /// Mark all entries as unread
    pub async fn unread_all(&self, username: &str) -> Result<()> {
        use super::progress::TitleInfo;

        let mut info = TitleInfo::load(&self.path).await?;

        // Remove progress for all entries
        for entry in &self.entries {
            info.remove_progress(username, &entry.id);
        }

        info.save(&self.path).await?;
        Ok(())
    }

    /// Get overall title progress (average across all entries)
    pub async fn get_title_progress(&self, username: &str) -> Result<f32> {
        if self.entries.is_empty() {
            return Ok(0.0);
        }

        use super::progress::TitleInfo;
        let info = TitleInfo::load(&self.path).await?;

        let mut total_progress = 0.0;
        let mut entry_count = 0;

        for entry in &self.entries {
            let page = info.get_progress(username, &entry.id).unwrap_or(0);
            let percentage = if entry.pages > 0 {
                (page as f32 / entry.pages as f32) * 100.0
            } else {
                0.0
            };
            total_progress += percentage;
            entry_count += 1;
        }

        Ok(total_progress / entry_count as f32)
    }

    /// Populate date_added timestamps for newly discovered entries
    /// Should be called after scanning to track when entries were first discovered
    pub async fn populate_date_added(&self) -> Result<()> {
        use super::progress::TitleInfo;

        let mut info = TitleInfo::load(&self.path).await?;
        let now = chrono::Utc::now().timestamp();

        for entry in &self.entries {
            // Only set if not already set (preserve original date for existing entries)
            info.set_date_added_if_new(&entry.id, now);
        }

        info.save(&self.path).await?;
        Ok(())
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
            let sig = crate::util::file_signature(&entry_path)?;
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
    use sha1::{Digest, Sha1};
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

impl super::Sortable for Title {
    fn sort_name(&self) -> &str {
        &self.title
    }

    fn sort_mtime(&self) -> i64 {
        self.mtime
    }
}

impl super::Sortable for &Title {
    fn sort_name(&self) -> &str {
        &self.title
    }

    fn sort_mtime(&self) -> i64 {
        self.mtime
    }
}
