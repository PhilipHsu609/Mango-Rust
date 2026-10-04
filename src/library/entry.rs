use std::path::PathBuf;

/// Represents a single readable entry (chapter/volume)
/// Can be a ZIP/CBZ archive or a directory containing images
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Entry {
    /// Unique identifier (persisted in database)
    pub id: String,

    /// Absolute path to the archive file or directory
    pub path: PathBuf,

    /// Display name (filename without extension)
    pub title: String,

    /// File signature (inode on Unix, CRC32 on Windows) - stored as TEXT for Mango compatibility
    pub signature: String,

    /// File metadata change time, matching Mango's `ctime` date_added behavior
    #[serde(default)]
    pub ctime: i64,

    /// Modification time (for sorting)
    pub mtime: i64,

    /// Number of pages (images) in this entry
    pub pages: usize,

    /// List of image filenames (sorted)
    pub image_files: Vec<String>,

    /// Total bytes in the archive or loose-image directory.
    #[serde(default)]
    pub size_bytes: u64,

    /// Archive validation failure, if the entry cannot be read.
    #[serde(default)]
    pub err_msg: Option<String>,
}

impl super::Sortable for Entry {
    fn sort_name(&self) -> &str {
        &self.title
    }

    fn sort_mtime(&self) -> i64 {
        self.mtime
    }
}

impl super::Sortable for &Entry {
    fn sort_name(&self) -> &str {
        &self.title
    }

    fn sort_mtime(&self) -> i64 {
        self.mtime
    }
}
