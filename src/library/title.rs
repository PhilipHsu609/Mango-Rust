use std::path::PathBuf;

use super::entry::Entry;

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
}

#[cfg(test)]
mod tests {
    use super::Title;
    use crate::library::entry::Entry;

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
}
