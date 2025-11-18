use crate::error::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

/// Structure for storing title metadata and progress in info.json
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct TitleInfo {
    /// Progress tracking: username -> entry_id -> page_number
    #[serde(default)]
    pub progress: HashMap<String, HashMap<String, usize>>,

    /// Last read timestamp: username -> entry_id -> unix_timestamp
    #[serde(default)]
    pub last_read: HashMap<String, HashMap<String, i64>>,

    /// Date added timestamp: entry_id -> unix_timestamp
    #[serde(default)]
    pub date_added: HashMap<String, i64>,
}

impl TitleInfo {
    /// Load TitleInfo from a directory's info.json file
    pub async fn load(dir: &Path) -> Result<Self> {
        let info_path = dir.join("info.json");

        if !info_path.exists() {
            return Ok(TitleInfo::default());
        }

        let content = tokio::fs::read_to_string(&info_path).await?;
        let info: TitleInfo = serde_json::from_str(&content).unwrap_or_default();

        Ok(info)
    }

    /// Save TitleInfo to a directory's info.json file
    pub async fn save(&self, dir: &Path) -> Result<()> {
        let info_path = dir.join("info.json");

        // If there's no progress data, delete the file instead
        if self.progress.is_empty() {
            if info_path.exists() {
                tokio::fs::remove_file(&info_path).await?;
            }
            return Ok(());
        }

        let json = serde_json::to_string_pretty(self)?;
        tokio::fs::write(&info_path, json).await?;

        Ok(())
    }

    /// Get progress for a specific user and entry
    pub fn get_progress(&self, username: &str, entry_id: &str) -> Option<usize> {
        self.progress
            .get(username)
            .and_then(|user_progress| user_progress.get(entry_id))
            .copied()
    }

    /// Set progress for a specific user and entry
    pub fn set_progress(&mut self, username: &str, entry_id: &str, page: usize) {
        self.progress
            .entry(username.to_string())
            .or_default()
            .insert(entry_id.to_string(), page);

        // Update last_read timestamp
        self.set_last_read(username, entry_id, chrono::Utc::now().timestamp());
    }

    /// Remove progress for a specific user and entry
    pub fn remove_progress(&mut self, username: &str, entry_id: &str) {
        if let Some(user_progress) = self.progress.get_mut(username) {
            user_progress.remove(entry_id);
            // If user has no more progress entries, remove the user
            if user_progress.is_empty() {
                self.progress.remove(username);
            }
        }
    }

    /// Get last read timestamp for a specific user and entry
    pub fn get_last_read(&self, username: &str, entry_id: &str) -> Option<i64> {
        self.last_read
            .get(username)
            .and_then(|user_last_read| user_last_read.get(entry_id))
            .copied()
    }

    /// Set last read timestamp for a specific user and entry
    pub fn set_last_read(&mut self, username: &str, entry_id: &str, timestamp: i64) {
        self.last_read
            .entry(username.to_string())
            .or_default()
            .insert(entry_id.to_string(), timestamp);
    }

    /// Get date added timestamp for an entry
    pub fn get_date_added(&self, entry_id: &str) -> Option<i64> {
        self.date_added.get(entry_id).copied()
    }

    /// Set date added timestamp for an entry
    pub fn set_date_added(&mut self, entry_id: &str, timestamp: i64) {
        self.date_added.insert(entry_id.to_string(), timestamp);
    }

    /// Set date added for an entry if not already set
    pub fn set_date_added_if_new(&mut self, entry_id: &str, timestamp: i64) {
        self.date_added.entry(entry_id.to_string()).or_insert(timestamp);
    }
}
