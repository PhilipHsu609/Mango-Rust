use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use tokio::sync::RwLock;

use crate::error::Result;
use crate::Storage;
use super::title::Title;
use super::entry::Entry;

/// Main library manager
/// Handles scanning, ID tracking, and title registry
pub struct Library {
    /// Library root directory
    path: PathBuf,

    /// All titles indexed by ID
    titles: HashMap<String, Title>,

    /// Database storage for ID persistence
    storage: Storage,
}

impl Library {
    /// Create a new Library instance
    pub fn new(path: PathBuf, storage: Storage) -> Self {
        Self {
            path,
            titles: HashMap::new(),
            storage,
        }
    }

    /// Scan the library directory for manga titles
    pub async fn scan(&mut self) -> Result<()> {
        tracing::info!("Starting library scan: {}", self.path.display());

        let mut new_titles = HashMap::new();
        let mut dir_entries = tokio::fs::read_dir(&self.path).await?;

        while let Some(entry) = dir_entries.next_entry().await? {
            let entry_path = entry.path();

            if entry_path.is_dir() {
                // Each top-level directory is a manga title (series)
                match Title::from_directory(entry_path.clone()).await {
                    Ok(mut title) => {
                        // Try to match with existing ID from database
                        if let Some(existing_id) = self.find_existing_id(&title).await? {
                            title.id = existing_id;
                            tracing::debug!("Matched existing title: {} ({})", title.title, title.id);
                        } else {
                            // New title, persist ID to database
                            self.persist_title_id(&title).await?;
                            tracing::info!("Discovered new title: {} ({})", title.title, title.id);
                        }

                        // Persist entry IDs
                        for entry in &mut title.entries {
                            if let Some(existing_id) = self.find_existing_entry_id(entry).await? {
                                entry.id = existing_id;
                            } else {
                                self.persist_entry_id(entry).await?;
                                tracing::debug!("  New entry: {} ({})", entry.title, entry.id);
                            }
                        }

                        new_titles.insert(title.id.clone(), title);
                    }
                    Err(e) => {
                        tracing::warn!("Failed to scan title at {}: {}", entry_path.display(), e);
                    }
                }
            }
        }

        let title_count = new_titles.len();
        let entry_count: usize = new_titles.values().map(|t| t.entries.len()).sum();

        self.titles = new_titles;

        tracing::info!(
            "Library scan complete: {} titles, {} entries",
            title_count,
            entry_count
        );

        Ok(())
    }

    /// Find existing title ID from database (by path or signature)
    async fn find_existing_id(&self, title: &Title) -> Result<Option<String>> {
        // Tier 1: Exact match (path + signature)
        if let Some(id) = sqlx::query_scalar::<_, String>(
            "SELECT id FROM ids WHERE path = ? AND signature = ? AND type = 'title'"
        )
        .bind(title.path.to_string_lossy().as_ref())
        .bind(title.signature as i64)
        .fetch_optional(self.storage.pool())
        .await?
        {
            return Ok(Some(id));
        }

        // Tier 2: Path-only match (directory modified but not moved)
        if let Some(id) = sqlx::query_scalar::<_, String>(
            "SELECT id FROM ids WHERE path = ? AND type = 'title'"
        )
        .bind(title.path.to_string_lossy().as_ref())
        .fetch_optional(self.storage.pool())
        .await?
        {
            // Update signature
            sqlx::query("UPDATE ids SET signature = ? WHERE id = ?")
                .bind(title.signature as i64)
                .bind(&id)
                .execute(self.storage.pool())
                .await?;

            return Ok(Some(id));
        }

        // Tier 3: Signature-only match (directory moved/renamed)
        // For Week 2, we'll skip path similarity matching (add in Week 5)

        Ok(None)
    }

    /// Find existing entry ID from database
    async fn find_existing_entry_id(&self, entry: &Entry) -> Result<Option<String>> {
        // Tier 1: Exact match
        if let Some(id) = sqlx::query_scalar::<_, String>(
            "SELECT id FROM ids WHERE path = ? AND signature = ? AND type = 'entry'"
        )
        .bind(entry.path.to_string_lossy().as_ref())
        .bind(entry.signature as i64)
        .fetch_optional(self.storage.pool())
        .await?
        {
            return Ok(Some(id));
        }

        // Tier 2: Path-only match
        if let Some(id) = sqlx::query_scalar::<_, String>(
            "SELECT id FROM ids WHERE path = ? AND type = 'entry'"
        )
        .bind(entry.path.to_string_lossy().as_ref())
        .fetch_optional(self.storage.pool())
        .await?
        {
            // Update signature
            sqlx::query("UPDATE ids SET signature = ? WHERE id = ?")
                .bind(entry.signature as i64)
                .bind(&id)
                .execute(self.storage.pool())
                .await?;

            return Ok(Some(id));
        }

        Ok(None)
    }

    /// Persist title ID to database
    async fn persist_title_id(&self, title: &Title) -> Result<()> {
        sqlx::query(
            "INSERT INTO ids (id, path, signature, type) VALUES (?, ?, ?, 'title')
             ON CONFLICT(id) DO UPDATE SET path = ?, signature = ?"
        )
        .bind(&title.id)
        .bind(title.path.to_string_lossy().as_ref())
        .bind(title.signature as i64)
        .bind(title.path.to_string_lossy().as_ref())
        .bind(title.signature as i64)
        .execute(self.storage.pool())
        .await?;

        Ok(())
    }

    /// Persist entry ID to database
    async fn persist_entry_id(&self, entry: &Entry) -> Result<()> {
        sqlx::query(
            "INSERT INTO ids (id, path, signature, type) VALUES (?, ?, ?, 'entry')
             ON CONFLICT(id) DO UPDATE SET path = ?, signature = ?"
        )
        .bind(&entry.id)
        .bind(entry.path.to_string_lossy().as_ref())
        .bind(entry.signature as i64)
        .bind(entry.path.to_string_lossy().as_ref())
        .bind(entry.signature as i64)
        .execute(self.storage.pool())
        .await?;

        Ok(())
    }

    /// Get all titles (sorted by name)
    pub fn get_titles(&self) -> Vec<&Title> {
        self.get_titles_sorted(SortMethod::default(), true)
    }

    /// Get all titles sorted by specified method
    pub fn get_titles_sorted(&self, method: SortMethod, ascending: bool) -> Vec<&Title> {
        let mut titles: Vec<&Title> = self.titles.values().collect();

        match method {
            SortMethod::Name => {
                if ascending {
                    titles.sort_by(|a, b| natord::compare(&a.title, &b.title));
                } else {
                    titles.sort_by(|a, b| natord::compare(&b.title, &a.title));
                }
            }
            SortMethod::TimeModified => {
                if ascending {
                    // Oldest first
                    titles.sort_by(|a, b| a.mtime.cmp(&b.mtime));
                } else {
                    // Newest first
                    titles.sort_by(|a, b| b.mtime.cmp(&a.mtime));
                }
            }
            SortMethod::Auto => {
                // For now, use name sorting with natural ordering
                // Future: smart chapter detection
                if ascending {
                    titles.sort_by(|a, b| natord::compare(&a.title, &b.title));
                } else {
                    titles.sort_by(|a, b| natord::compare(&b.title, &a.title));
                }
            }
        }

        titles
    }

    /// Get a specific title by ID
    pub fn get_title(&self, id: &str) -> Option<&Title> {
        self.titles.get(id)
    }

    /// Get a specific entry by title ID and entry ID
    pub fn get_entry(&self, title_id: &str, entry_id: &str) -> Option<&Entry> {
        self.titles
            .get(title_id)?
            .entries
            .iter()
            .find(|e| e.id == entry_id)
    }

    /// Get total library statistics
    pub fn stats(&self) -> LibraryStats {
        let title_count = self.titles.len();
        let entry_count: usize = self.titles.values().map(|t| t.entries.len()).sum();
        let page_count: usize = self.titles.values().map(|t| t.total_pages()).sum();

        LibraryStats {
            titles: title_count,
            entries: entry_count,
            pages: page_count,
        }
    }
}

/// Sorting methods for titles and entries
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortMethod {
    /// Sort alphabetically by name/title
    Name,
    /// Sort by modification time
    TimeModified,
    /// Smart chapter detection (future enhancement)
    Auto,
}

impl Default for SortMethod {
    fn default() -> Self {
        SortMethod::Name
    }
}

impl SortMethod {
    /// Parse from string parameter (for API routes)
    /// Matches original Mango API: "title", "modified", "auto"
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "title" | "name" => SortMethod::Name,
            "modified" | "time" => SortMethod::TimeModified,
            "auto" => SortMethod::Auto,
            _ => SortMethod::default(),
        }
    }

    /// Parse sort method and ascend flag from query parameters
    /// Returns (SortMethod, bool) where bool is true for ascending
    pub fn from_params(sort: Option<&str>, ascend: Option<&str>) -> (Self, bool) {
        let method = sort.map(Self::from_str).unwrap_or_default();
        let ascending = ascend
            .and_then(|s| s.parse::<i32>().ok())
            .map(|v| v != 0)
            .unwrap_or(true); // Default to ascending
        (method, ascending)
    }
}

/// Library statistics
#[derive(Debug, Clone)]
pub struct LibraryStats {
    pub titles: usize,
    pub entries: usize,
    pub pages: usize,
}

/// Create a shared Library instance that can be used across async tasks
pub type SharedLibrary = Arc<RwLock<Library>>;

/// Spawn a background task that periodically scans the library
pub fn spawn_periodic_scanner(
    library: SharedLibrary,
    interval_minutes: u64,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(
            std::time::Duration::from_secs(interval_minutes * 60)
        );

        loop {
            interval.tick().await;

            tracing::info!("Starting periodic library scan");
            let mut lib = library.write().await;
            if let Err(e) = lib.scan().await {
                tracing::error!("Periodic scan failed: {}", e);
            }
        }
    })
}
