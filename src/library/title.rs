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

    /// Directory signature (CRC32 of file inodes) - stored as TEXT for Mango compatibility
    pub signature: String,

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

        let nested_titles = Vec::new();

        // Collect all archive paths first
        let mut archive_paths = Vec::new();
        let mut dir_entries = tokio::fs::read_dir(&path).await?;

        while let Some(entry) = dir_entries.next_entry().await? {
            let entry_path = entry.path();

            if entry_path.is_dir() {
                // For Week 2: treat subdirectories as nested titles (simplified)
                // TODO Week 5: Add proper nested title support
                continue;
            } else if is_archive(&entry_path) {
                archive_paths.push(entry_path);
            }
        }

        // Process all entries in parallel for better performance
        let entry_tasks: Vec<_> = archive_paths
            .into_iter()
            .map(|entry_path| {
                tokio::spawn(async move {
                    let mut manga_entry = Entry::from_archive(entry_path).await?;
                    manga_entry.calculate_signature()?;
                    Ok::<Entry, crate::error::Error>(manga_entry)
                })
            })
            .collect();

        // Collect all results
        let mut entries = Vec::new();
        for task in entry_tasks {
            match task.await {
                Ok(Ok(entry)) => entries.push(entry),
                Ok(Err(e)) => {
                    tracing::warn!("Failed to process entry: {}", e);
                }
                Err(e) => {
                    tracing::warn!("Entry processing task failed: {}", e);
                }
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

    /// Find the single continuation entry Mango would show for this title.
    pub fn get_continue_reading_entry<'a>(
        &'a self,
        username: &str,
        info: &super::progress::TitleInfo,
    ) -> Option<(&'a Entry, Option<&'a Entry>)> {
        let (method, ascending) = info
            .get_sort_by(username)
            .map(|(method, ascending)| (SortMethod::parse(&method), ascending))
            .unwrap_or((SortMethod::Auto, true));
        let entries = self.get_entries_sorted(method, ascending);
        let mut index = entries.iter().rposition(|entry| {
            info.get_progress(username, &entry.title)
                .unwrap_or(0)
                .min(entry.pages as i32)
                > 0
        })?;

        let last_read_entry = entries[index];
        let progress = info
            .get_progress(username, &last_read_entry.title)
            .unwrap_or(0)
            .min(last_read_entry.pages as i32);
        if progress >= last_read_entry.pages as i32 {
            if index + 1 < entries.len() {
                index += 1;
            } else {
                index = entries.iter().position(|entry| {
                    info.get_progress(username, &entry.title)
                        .unwrap_or(0)
                        .min(entry.pages as i32)
                        < entry.pages as i32
                })?;
            }
        }

        let previous = index
            .checked_sub(1)
            .and_then(|previous| entries.get(previous).copied());
        Some((entries[index], previous))
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

    /// Save reading progress for an entry.
    pub async fn save_entry_progress(
        &self,
        username: &str,
        entry_id: &str,
        page: i32,
    ) -> Result<()> {
        use super::progress::TitleInfo;

        let entry = self
            .entries
            .iter()
            .find(|entry| entry.id == entry_id)
            .ok_or_else(|| {
                crate::error::Error::NotFound(format!("Entry not found: {}", entry_id))
            })?;
        let mut info = TitleInfo::load(&self.path).await?;

        if page == 0 {
            info.remove_progress(username, &entry.title);
        } else {
            info.set_progress(username, &entry.title, page);
        }

        info.save(&self.path).await?;
        Ok(())
    }

    /// Load reading progress for an entry.
    pub async fn load_entry_progress(&self, username: &str, entry_id: &str) -> Result<i32> {
        use super::progress::TitleInfo;

        let entry = self
            .entries
            .iter()
            .find(|entry| entry.id == entry_id)
            .ok_or_else(|| {
                crate::error::Error::NotFound(format!("Entry not found: {}", entry_id))
            })?;
        let info = TitleInfo::load(&self.path).await?;
        Ok(info.get_progress(username, &entry.title).unwrap_or(0))
    }

    /// Get progress information for an entry (percentage and page number).
    pub async fn get_entry_progress(&self, username: &str, entry_id: &str) -> Result<(f32, i32)> {
        let entry = self
            .entries
            .iter()
            .find(|entry| entry.id == entry_id)
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
            info.set_progress(username, &entry.title, entry.pages as i32);
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
            info.remove_progress(username, &entry.title);
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
            let page = info.get_progress(username, &entry.title).unwrap_or(0);
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

        for entry in &self.entries {
            let has_mango_date = info.date_added.contains_key(&entry.title);
            let legacy_date = info.migrate_entry_id_to_title(&entry.id, &entry.title);
            if has_mango_date {
                continue;
            }

            match entry.date_added_timestamp().await {
                Ok(timestamp) if legacy_date.is_some() => {
                    info.set_date_added(&entry.title, timestamp);
                }
                Ok(timestamp) => info.set_date_added_if_new(&entry.title, timestamp),
                Err(error) => tracing::warn!(
                    "Failed to read ctime for recently scanned entry {}: {}",
                    entry.path.display(),
                    error
                ),
            }
        }

        let entry_titles = self
            .entries
            .iter()
            .map(|entry| entry.title.clone())
            .collect();
        info.remove_orphaned_entry_ids(&entry_titles);
        info.normalize_entry_timestamps();

        info.save(&self.path).await?;
        Ok(())
    }
}

/// Check if a file is a supported archive format
/// Only returns true for formats we can actually extract (currently ZIP/CBZ only)
/// When adding new format support, update entry.rs extraction code first,
/// then add extensions to util::EXTRACTABLE_ARCHIVE_EXTENSIONS
fn is_archive(path: &Path) -> bool {
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        let ext_lower = ext.to_lowercase();
        crate::util::EXTRACTABLE_ARCHIVE_EXTENSIONS.contains(&ext_lower.as_str())
    } else {
        false
    }
}

/// Calculate directory signature (matches original Mango's Dir.signature behavior)
/// This is now a simple wrapper around util::dir_signature for consistency
fn calculate_dir_signature(path: &Path) -> Result<String> {
    crate::util::dir_signature(path)
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

#[cfg(test)]
mod tests {
    use super::Title;
    use crate::library::{entry::Entry, progress::TitleInfo};

    #[tokio::test]
    async fn populate_date_added_uses_entry_titles_and_migrates_legacy_ids() {
        let dir = tempfile::tempdir().unwrap();
        let mut info = TitleInfo::load(dir.path()).await.unwrap();
        info.set_date_added("Existing", 1_600_000_000);
        info.set_date_added("legacy-uuid", 1_500_000_000);
        info.save(dir.path()).await.unwrap();

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
                },
            ],
            parent_id: None,
            nested_titles: Vec::new(),
        };

        title.populate_date_added().await.unwrap();

        let info = TitleInfo::load(dir.path()).await.unwrap();
        assert_eq!(info.get_date_added("New"), Some(1_700_000_000));
        assert_eq!(info.get_date_added("Existing"), Some(1_600_000_000));
        assert_eq!(info.get_date_added("Migrated"), Some(1_700_000_002));
        assert!(!info.date_added.contains_key("legacy-uuid"));
    }
    fn continue_reading_title() -> Title {
        let dir = std::env::temp_dir();
        Title {
            id: "title".to_string(),
            path: dir,
            title: "Title".to_string(),
            signature: String::new(),
            contents_signature: String::new(),
            mtime: 0,
            entries: (1..=3)
                .map(|number| Entry {
                    id: format!("entry-{number}"),
                    path: std::path::PathBuf::new(),
                    title: format!("Volume {number}"),
                    signature: String::new(),
                    mtime: 0,
                    ctime: 0,
                    pages: 10,
                    image_files: Vec::new(),
                })
                .collect(),
            parent_id: None,
            nested_titles: Vec::new(),
        }
    }

    #[test]
    fn continue_reading_selects_one_progressed_entry_or_its_next_entry() {
        let title = continue_reading_title();
        let mut info = TitleInfo::default();
        info.set_progress("admin", "Volume 1", 10);
        info.set_progress("admin", "Volume 2", 3);

        let (selected, _) = title.get_continue_reading_entry("admin", &info).unwrap();
        assert_eq!(selected.id, "entry-2");

        info.set_progress("admin", "Volume 2", 10);
        let (selected, previous) = title.get_continue_reading_entry("admin", &info).unwrap();
        assert_eq!(selected.id, "entry-3");
        assert_eq!(previous.unwrap().id, "entry-2");
    }

    #[test]
    fn continue_reading_falls_back_to_first_unfinished_when_latest_is_last() {
        let title = continue_reading_title();
        let mut info = TitleInfo::default();
        info.set_progress("admin", "Volume 3", 10);

        let (selected, previous) = title.get_continue_reading_entry("admin", &info).unwrap();
        assert_eq!(selected.id, "entry-1");
        assert!(previous.is_none());
    }
}
