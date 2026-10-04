use std::path::PathBuf;

use super::chapter_sort::{compare_numerically, ChapterSorter};
use super::entry::Entry;
use super::SortMethod;
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

    /// Recursive filesystem fingerprint for reusing unchanged title trees
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
    /// Get total number of pages across this title and nested titles.
    pub fn total_pages(&self) -> usize {
        self.entries.iter().map(|entry| entry.pages).sum::<usize>()
            + self
                .nested_titles
                .iter()
                .map(Title::total_pages)
                .sum::<usize>()
    }

    /// Get nested titles sorted with Mango's title ordering.
    pub fn get_nested_titles_sorted(&self, method: SortMethod, ascending: bool) -> Vec<&Title> {
        let mut titles: Vec<&Title> = self.nested_titles.iter().collect();
        use super::{sort_by_mtime, sort_by_name};
        match method {
            SortMethod::TimeModified => sort_by_mtime(&mut titles, ascending),
            SortMethod::Name | SortMethod::TimeAdded | SortMethod::Progress | SortMethod::Auto => {
                sort_by_name(&mut titles, ascending)
            }
        }
        titles
    }
    /// Get entries sorted by specified method and order.
    pub fn get_entries_sorted(&self, method: SortMethod, ascending: bool) -> Vec<&Entry> {
        let mut entries: Vec<&Entry> = self.entries.iter().collect();
        use super::{sort_by_mtime, sort_by_name};
        match method {
            SortMethod::Name | SortMethod::Progress | SortMethod::Auto => {
                sort_by_name(&mut entries, ascending);
            }
            SortMethod::TimeModified => sort_by_mtime(&mut entries, ascending),
            SortMethod::TimeAdded => {
                entries.sort_by(|a, b| {
                    a.ctime
                        .cmp(&b.ctime)
                        .then_with(|| natord::compare(&a.title, &b.title))
                });
                if !ascending {
                    entries.reverse();
                }
            }
        }
        entries
    }

    /// Find the single continuation entry Mango would show for this title.
    pub fn get_continue_reading_entry<'a>(
        &'a self,
        username: &str,
        info: &super::progress::TitleInfo,
        sort_title_overrides: &std::collections::HashMap<String, String>,
    ) -> Option<(&'a Entry, Option<&'a Entry>)> {
        let (method, ascending) = info
            .get_sort_by(username)
            .map(|(method, ascending)| (SortMethod::parse(&method), ascending))
            .unwrap_or((SortMethod::Auto, true));
        let mut entries: Vec<&Entry> = self.entries.iter().collect();
        let chapter_sorter = if matches!(method, SortMethod::Auto) {
            let sort_titles = self
                .entries
                .iter()
                .map(|entry| {
                    sort_title_overrides
                        .get(&entry.id)
                        .map(String::as_str)
                        .unwrap_or(&entry.title)
                })
                .collect::<Vec<_>>();
            Some(ChapterSorter::new(&sort_titles))
        } else {
            None
        };
        entries.sort_by(|left, right| {
            let left_title = sort_title_overrides
                .get(&left.id)
                .map(String::as_str)
                .unwrap_or(&left.title);
            let right_title = sort_title_overrides
                .get(&right.id)
                .map(String::as_str)
                .unwrap_or(&right.title);
            let name_order = || compare_numerically(left_title, right_title);
            match method {
                SortMethod::TimeModified => left.mtime.cmp(&right.mtime).then_with(name_order),
                SortMethod::TimeAdded => info
                    .get_date_added(&left.title)
                    .unwrap_or(left.ctime)
                    .cmp(&info.get_date_added(&right.title).unwrap_or(right.ctime))
                    .then_with(name_order),
                SortMethod::Progress => {
                    let percentage = |entry: &Entry| {
                        if entry.pages == 0 {
                            0.0
                        } else {
                            info.get_progress(username, &entry.title)
                                .unwrap_or(0)
                                .clamp(0, entry.pages as i32) as f32
                                / entry.pages as f32
                        }
                    };
                    percentage(left)
                        .total_cmp(&percentage(right))
                        .then_with(name_order)
                }
                SortMethod::Name => name_order(),
                SortMethod::Auto => chapter_sorter
                    .as_ref()
                    .expect("auto sorting builds a chapter sorter")
                    .compare(left_title, right_title)
                    .then_with(name_order),
            }
        });
        if !ascending {
            entries.reverse();
        }
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
        self.collect_deep_entries(&mut all_entries);
        all_entries
    }

    fn collect_deep_entries<'a>(&'a self, entries: &mut Vec<&'a Entry>) {
        entries.extend(&self.entries);
        for nested in &self.nested_titles {
            nested.collect_deep_entries(entries);
        }
    }

    /// Get nested titles and all descendants in depth-first order.
    pub fn deep_titles(&self) -> Vec<&Title> {
        let mut all_titles = Vec::new();
        self.collect_deep_titles(&mut all_titles);
        all_titles
    }

    fn collect_deep_titles<'a>(&'a self, titles: &mut Vec<&'a Title>) {
        for nested in &self.nested_titles {
            titles.push(nested);
            nested.collect_deep_titles(titles);
        }
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
        for nested in &self.nested_titles {
            Box::pin(nested.read_all(username)).await?;
        }
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
        for nested in &self.nested_titles {
            Box::pin(nested.unread_all(username)).await?;
        }
        Ok(())
    }

    /// Get title progress as a page-weighted percentage across nested titles.
    pub async fn get_title_progress(&self, username: &str) -> Result<f32> {
        use super::progress::TitleInfo;

        let mut total_pages = 0usize;
        let mut read_pages = 0usize;
        for title in std::iter::once(self).chain(self.deep_titles()) {
            let info = TitleInfo::load(&title.path).await?;
            for entry in &title.entries {
                total_pages += entry.pages;
                read_pages += info
                    .get_progress(username, &entry.title)
                    .unwrap_or(0)
                    .clamp(0, entry.pages as i32) as usize;
            }
        }

        if total_pages == 0 {
            Ok(0.0)
        } else {
            Ok(read_pages as f32 / total_pages as f32 * 100.0)
        }
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

            match super::scan::date_added_timestamp(entry).await {
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
        for nested in &self.nested_titles {
            Box::pin(nested.populate_date_added()).await?;
        }
        Ok(())
    }
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
                    size_bytes: 0,
                    err_msg: None,
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
                    size_bytes: 0,
                    err_msg: None,
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
                    size_bytes: 0,
                    err_msg: None,
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
                    size_bytes: 0,
                    err_msg: None,
                })
                .collect(),
            parent_id: None,
            nested_titles: Vec::new(),
        }
    }

    #[test]
    fn recursive_traversal_preserves_depth_first_order_and_page_totals() {
        let make_title = |id: &str, pages: &[usize]| {
            let mut title = continue_reading_title();
            title.id = id.to_string();
            title.entries.truncate(pages.len());
            for (index, (entry, &pages)) in title.entries.iter_mut().zip(pages).enumerate() {
                entry.id = format!("{id}-{index}");
                entry.pages = pages;
            }
            title
        };

        let mut root = make_title("root", &[1, 2]);
        let mut child = make_title("child", &[3]);
        child.nested_titles.push(make_title("grandchild", &[4]));
        root.nested_titles = vec![child, make_title("empty", &[]), make_title("sibling", &[5])];

        assert_eq!(
            root.deep_entries()
                .iter()
                .map(|entry| entry.id.as_str())
                .collect::<Vec<_>>(),
            ["root-0", "root-1", "child-0", "grandchild-0", "sibling-0"]
        );
        assert_eq!(
            root.deep_titles()
                .iter()
                .map(|title| title.id.as_str())
                .collect::<Vec<_>>(),
            ["child", "grandchild", "empty", "sibling"]
        );
        assert_eq!(root.total_pages(), 15);

        let empty = make_title("empty", &[]);
        assert!(empty.deep_entries().is_empty());
        assert!(empty.deep_titles().is_empty());
        assert_eq!(empty.total_pages(), 0);
    }

    #[test]
    fn continue_reading_selects_one_progressed_entry_or_its_next_entry() {
        let title = continue_reading_title();
        let mut info = TitleInfo::default();
        info.set_progress("admin", "Volume 1", 10);
        info.set_progress("admin", "Volume 2", 3);

        let (selected, _) = title
            .get_continue_reading_entry("admin", &info, &Default::default())
            .unwrap();
        assert_eq!(selected.id, "entry-2");

        info.set_progress("admin", "Volume 2", 10);
        let (selected, previous) = title
            .get_continue_reading_entry("admin", &info, &Default::default())
            .unwrap();
        assert_eq!(selected.id, "entry-3");
        assert_eq!(previous.unwrap().id, "entry-2");
    }
    #[test]
    fn continue_reading_uses_date_added_sort_order() {
        let title = continue_reading_title();
        let mut info = TitleInfo::default();
        info.set_sort_by("admin", "time_added", true);
        info.set_date_added("Volume 2", 1_600_000_000);
        info.set_date_added("Volume 1", 1_700_000_000);
        info.set_progress("admin", "Volume 1", 2);

        let (selected, _) = title
            .get_continue_reading_entry("admin", &info, &Default::default())
            .unwrap();
        assert_eq!(selected.id, "entry-1");
    }
    #[test]
    fn continue_reading_uses_progress_sort_order() {
        let title = continue_reading_title();
        let mut info = TitleInfo::default();
        info.set_sort_by("admin", "progress", true);
        info.set_progress("admin", "Volume 1", 10);
        info.set_progress("admin", "Volume 2", 3);

        let (selected, _) = title
            .get_continue_reading_entry("admin", &info, &Default::default())
            .unwrap();
        assert_eq!(selected.id, "entry-3");
    }

    #[test]
    fn continue_reading_uses_entry_sort_title_overrides() {
        let title = continue_reading_title();
        let mut info = TitleInfo::default();
        info.set_progress("admin", "Volume 1", 2);
        info.set_progress("admin", "Volume 2", 3);
        let sort_titles = std::collections::HashMap::from([
            ("entry-1".to_string(), "Z".to_string()),
            ("entry-2".to_string(), "A".to_string()),
            ("entry-3".to_string(), "M".to_string()),
        ]);

        let (selected, _) = title
            .get_continue_reading_entry("admin", &info, &sort_titles)
            .unwrap();
        assert_eq!(selected.id, "entry-1");
    }

    #[tokio::test]
    async fn whole_title_progress_updates_nested_titles() {
        let dir = tempfile::tempdir().unwrap();
        let root_path = dir.path().join("Series");
        let child_path = root_path.join("Volume 1");
        std::fs::create_dir_all(&child_path).unwrap();
        let child = Title {
            id: "child".to_string(),
            path: child_path.clone(),
            title: "Volume 1".to_string(),
            signature: String::new(),
            contents_signature: String::new(),
            mtime: 0,
            entries: vec![Entry {
                id: "entry".to_string(),
                path: child_path.join("chapter.cbz"),
                title: "Chapter 1".to_string(),
                signature: String::new(),
                mtime: 0,
                ctime: 0,
                pages: 3,
                image_files: Vec::new(),
                size_bytes: 0,
                err_msg: None,
            }],
            parent_id: Some("root".to_string()),
            nested_titles: Vec::new(),
        };
        let root = Title {
            id: "root".to_string(),
            path: root_path,
            title: "Series".to_string(),
            signature: String::new(),
            contents_signature: String::new(),
            mtime: 0,
            entries: Vec::new(),
            parent_id: None,
            nested_titles: vec![child],
        };

        root.read_all("reader").await.unwrap();
        let info = TitleInfo::load(&child_path).await.unwrap();
        assert_eq!(info.get_progress("reader", "Chapter 1"), Some(3));

        root.unread_all("reader").await.unwrap();
        let info = TitleInfo::load(&child_path).await.unwrap();
        assert_eq!(info.get_progress("reader", "Chapter 1"), None);
    }

    #[test]
    fn continue_reading_falls_back_to_first_unfinished_when_latest_is_last() {
        let title = continue_reading_title();
        let mut info = TitleInfo::default();
        info.set_progress("admin", "Volume 3", 10);

        let (selected, previous) = title
            .get_continue_reading_entry("admin", &info, &Default::default())
            .unwrap();
        assert_eq!(selected.id, "entry-1");
        assert!(previous.is_none());
    }
}
