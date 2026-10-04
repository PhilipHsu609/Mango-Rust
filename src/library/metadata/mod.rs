mod file;
mod record;

pub use record::TitleInfo;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

use tokio::sync::Mutex;

use crate::error::{Error, Result};

use super::Title;

#[derive(Default)]
struct PathRecord {
    io: Mutex<()>,
    snapshot: RwLock<Option<Arc<TitleInfo>>>,
}

impl PathRecord {
    fn cached(&self) -> Option<Arc<TitleInfo>> {
        self.snapshot.read().ok()?.as_ref().map(Arc::clone)
    }

    fn publish(&self, info: TitleInfo) -> Result<Arc<TitleInfo>> {
        let info = Arc::new(info);
        *self.snapshot.write().map_err(|_| lock_error())? = Some(Arc::clone(&info));
        Ok(info)
    }
}

fn lock_error() -> Error {
    Error::Internal("Metadata store lock poisoned".to_string())
}

/// Owns info.json persistence and immutable read snapshots for directory paths.
/// Each path serializes its own reads, refreshes and updates; filesystem I/O
/// never holds the registry lock or blocks operations on unrelated paths.
#[derive(Default)]
pub struct MetadataStore {
    records: RwLock<HashMap<PathBuf, Arc<PathRecord>>>,
}

impl MetadataStore {
    pub fn new() -> Self {
        Self::default()
    }

    fn record(&self, path: &Path) -> Result<Arc<PathRecord>> {
        if let Some(record) = self.records.read().map_err(|_| lock_error())?.get(path) {
            return Ok(Arc::clone(record));
        }
        let mut records = self.records.write().map_err(|_| lock_error())?;
        Ok(Arc::clone(records.entry(path.to_path_buf()).or_default()))
    }

    /// Returns the last successfully loaded or saved snapshot, without I/O.
    pub fn cached(&self, path: &Path) -> Option<Arc<TitleInfo>> {
        self.records.read().ok()?.get(path)?.cached()
    }

    /// Loads only when there is no cached snapshot.
    pub async fn read(&self, path: &Path) -> Result<Arc<TitleInfo>> {
        let record = self.record(path)?;
        if let Some(info) = record.cached() {
            return Ok(info);
        }
        let _io = record.io.lock().await;
        if let Some(info) = record.cached() {
            return Ok(info);
        }
        record.publish(file::load(path).await?)
    }

    /// Reloads disk metadata, including edits made outside this store.
    pub async fn refresh(&self, path: &Path) -> Result<Arc<TitleInfo>> {
        let record = self.record(path)?;
        let _io = record.io.lock().await;
        record.publish(file::load(path).await?)
    }

    /// Reloads, mutates and saves while holding this path's I/O lock. Readers
    /// retain the old snapshot until persistence succeeds; failed saves never
    /// publish mutated state. The closure runs on a private, owned record.
    pub async fn update(
        &self,
        path: &Path,
        mutate: impl FnOnce(&mut TitleInfo),
    ) -> Result<Arc<TitleInfo>> {
        let record = self.record(path)?;
        let _io = record.io.lock().await;
        let mut info = file::load(path).await?;
        mutate(&mut info);
        file::save(path, &info).await?;
        record.publish(info)
    }

    /// Individual progress updates record last_read, including page zero.
    pub async fn save_progress(
        &self,
        path: &Path,
        username: &str,
        entry_title: &str,
        page: i32,
    ) -> Result<()> {
        self.update(path, |info| info.set_progress(username, entry_title, page))
            .await?;
        Ok(())
    }

    /// Bulk updates preserve last_read and retain explicit zero progress.
    pub async fn save_bulk_progress(
        &self,
        path: &Path,
        username: &str,
        updates: &[(String, i32)],
    ) -> Result<()> {
        self.update(path, |info| {
            let progress = info.progress.entry(username.to_string()).or_default();
            for (entry_title, page) in updates {
                progress.insert(entry_title.clone(), *page);
            }
        })
        .await?;
        Ok(())
    }

    pub async fn read_all(&self, title: &Title, username: &str) -> Result<()> {
        for title in std::iter::once(title).chain(title.deep_titles()) {
            self.update(&title.path, |info| {
                for entry in &title.entries {
                    info.set_progress(username, &entry.title, entry.pages as i32);
                }
            })
            .await?;
        }
        Ok(())
    }

    pub async fn unread_all(&self, title: &Title, username: &str) -> Result<()> {
        for title in std::iter::once(title).chain(title.deep_titles()) {
            self.update(&title.path, |info| {
                for entry in &title.entries {
                    info.remove_progress(username, &entry.title);
                }
            })
            .await?;
        }
        Ok(())
    }

    /// Aligns a scanned title and descendants with Mango's entry-title keys,
    /// preserving existing dates while replacing legacy ID dates with ctime.
    pub async fn populate_date_added(&self, title: &Title) -> Result<()> {
        for title in std::iter::once(title).chain(title.deep_titles()) {
            let record = self.record(&title.path)?;
            let _io = record.io.lock().await;
            let mut info = file::load(&title.path).await?;
            for entry in &title.entries {
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
            let entry_titles = title
                .entries
                .iter()
                .map(|entry| entry.title.clone())
                .collect();
            info.remove_orphaned_entry_ids(&entry_titles);
            info.normalize_entry_timestamps();
            file::save(&title.path, &info).await?;
            record.publish(info)?;
        }
        Ok(())
    }

    /// Optional method and ascending inputs use the web's established parsing:
    /// absent/invalid ascending means true, and ascending alone does not save.
    pub async fn get_and_save_sort(
        &self,
        path: &Path,
        username: &str,
        method: Option<&str>,
        ascending: Option<&str>,
    ) -> Result<(String, bool)> {
        if let Some(method) = method {
            let ascending = ascending
                .and_then(|value| value.parse::<i32>().ok())
                .map(|value| value != 0)
                .unwrap_or(true);
            self.update(path, |info| info.set_sort_by(username, method, ascending))
                .await?;
            return Ok((method.to_string(), ascending));
        }
        Ok(self
            .read(path)
            .await?
            .get_sort_by(username)
            .unwrap_or_else(|| ("auto".to_string(), true)))
    }
}

#[cfg(test)]
mod tests;
