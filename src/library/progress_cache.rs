use std::collections::HashMap;
use std::path::Path;
use std::sync::RwLock;

use crate::error::{Error, Result};
use crate::library::progress::TitleInfo;

/// In-memory cache for progress data from info.json files
/// Eliminates O(N) filesystem reads when loading progress
pub struct ProgressCache {
    /// title_id -> TitleInfo
    data: RwLock<HashMap<String, TitleInfo>>,
}

impl ProgressCache {
    pub fn new() -> Self {
        Self {
            data: RwLock::new(HashMap::new()),
        }
    }

    /// Helper to acquire read lock with logging on poison
    fn read_data(&self) -> Option<std::sync::RwLockReadGuard<'_, HashMap<String, TitleInfo>>> {
        match self.data.read() {
            Ok(guard) => Some(guard),
            Err(e) => {
                tracing::error!(
                    "Progress cache RwLock poisoned during read: {}. Cache state may be corrupted.",
                    e
                );
                None
            }
        }
    }

    /// Load progress for a title from cache
    pub fn get_progress(&self, title_id: &str, username: &str, entry_id: &str) -> Option<i32> {
        let data = self.read_data()?;
        data.get(title_id)?.get_progress(username, entry_id)
    }

    /// Get last read timestamp from cache
    pub fn get_last_read(&self, title_id: &str, username: &str, entry_id: &str) -> Option<i64> {
        let data = self.read_data()?;
        data.get(title_id)?.get_last_read(username, entry_id)
    }

    /// Get date added from cache
    pub fn get_date_added(&self, title_id: &str, entry_id: &str) -> Option<i64> {
        let data = self.read_data()?;
        data.get(title_id)?.get_date_added(entry_id)
    }

    /// Get display name from cache
    pub fn get_display_name(&self, title_id: &str) -> Option<String> {
        let data = self.read_data()?;
        let info = data.get(title_id)?;
        if info.display_name.is_empty() {
            None
        } else {
            Some(info.display_name.clone())
        }
    }

    /// Get full TitleInfo for a title (for operations needing full access)
    pub fn get_title_info(&self, title_id: &str) -> Option<TitleInfo> {
        let data = self.read_data()?;
        data.get(title_id).cloned()
    }

    /// Load a title's info.json into cache
    pub async fn load_title(&self, title_id: &str, title_path: &Path) -> Result<()> {
        let info = TitleInfo::load(title_path).await?;
        let mut data = self.data.write().map_err(|e| {
            tracing::error!("Progress cache lock poisoned during load_title: {}", e);
            Error::Internal("Progress cache lock poisoned".to_string())
        })?;
        data.insert(title_id.to_string(), info);
        Ok(())
    }

    /// Save progress and persist to info.json
    pub async fn save_progress(
        &self,
        title_id: &str,
        title_path: &Path,
        username: &str,
        entry_id: &str,
        page: i32,
    ) -> Result<()> {
        // Update cache and clone for saving in one lock acquisition
        let info_to_save = {
            let mut data = self.data.write().map_err(|e| {
                tracing::error!("Progress cache lock poisoned during save_progress: {}", e);
                Error::Internal("Progress cache lock poisoned".to_string())
            })?;
            let info = data
                .entry(title_id.to_string())
                .or_insert_with(TitleInfo::default);
            info.set_progress(username, entry_id, page);
            info.clone()
        };

        // Persist to file (outside of lock)
        info_to_save.save(title_path).await?;

        Ok(())
    }

    /// Clear cache (for rescans)
    pub fn clear(&self) {
        match self.data.write() {
            Ok(mut data) => {
                data.clear();
            }
            Err(e) => {
                tracing::error!(
                    "Progress cache lock poisoned during clear: {}. Cache may contain stale data.",
                    e
                );
            }
        }
    }

    /// Check if a title is in the cache
    pub fn contains(&self, title_id: &str) -> bool {
        match self.data.read() {
            Ok(data) => data.contains_key(title_id),
            Err(e) => {
                tracing::error!("Progress cache lock poisoned during contains check: {}", e);
                false
            }
        }
    }

    /// Get the number of cached titles
    pub fn len(&self) -> usize {
        match self.data.read() {
            Ok(data) => data.len(),
            Err(e) => {
                tracing::error!("Progress cache lock poisoned during len: {}", e);
                0
            }
        }
    }

    /// Check if cache is empty
    pub fn is_empty(&self) -> bool {
        match self.data.read() {
            Ok(data) => data.is_empty(),
            Err(e) => {
                tracing::error!("Progress cache lock poisoned during is_empty: {}", e);
                true
            }
        }
    }
}

impl Default for ProgressCache {
    fn default() -> Self {
        Self::new()
    }
}
