use axum::{
    extract::{Path, State},
    response::{IntoResponse, Json},
    http::StatusCode,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

use crate::{AppState, error::{Error, Result}};

#[derive(Debug, Deserialize)]
pub struct SaveProgressRequest {
    page: usize,
}

#[derive(Debug, Serialize)]
pub struct ProgressResponse {
    page: usize,
}

/// info.json structure for storing metadata and progress
#[derive(Debug, Serialize, Deserialize, Default)]
struct TitleInfo {
    #[serde(default)]
    progress: HashMap<String, HashMap<String, usize>>,
}

/// Save reading progress for an entry
pub async fn save_progress(
    State(state): State<AppState>,
    Path((title_id, entry_id)): Path<(String, String)>,
    Json(request): Json<SaveProgressRequest>,
) -> Result<impl IntoResponse> {
    // TODO: Get username from session
    let username = "admin".to_string();

    // Get library read lock to find the title path
    let lib = state.library.read().await;
    let title = lib.get_title(&title_id)
        .ok_or_else(|| Error::NotFound(format!("Title not found: {}", title_id)))?;

    // Verify entry exists
    let _entry = lib.get_entry(&title_id, &entry_id)
        .ok_or_else(|| Error::NotFound(format!("Entry not found: {}", entry_id)))?;

    let title_path = title.path.clone();
    drop(lib); // Release lock before file I/O

    // Load or create info.json
    let info_path = title_path.join("info.json");
    let mut info: TitleInfo = if info_path.exists() {
        let content = tokio::fs::read_to_string(&info_path).await?;
        serde_json::from_str(&content).unwrap_or_default()
    } else {
        TitleInfo::default()
    };

    // Update progress
    info.progress
        .entry(username)
        .or_insert_with(HashMap::new)
        .insert(entry_id.clone(), request.page);

    // Save info.json
    let json = serde_json::to_string_pretty(&info)?;
    tokio::fs::write(&info_path, json).await?;

    tracing::debug!(
        "Saved progress: {} / {} = page {}",
        title_id,
        entry_id,
        request.page
    );

    Ok(StatusCode::OK)
}

/// Get reading progress for an entry
pub async fn get_progress(
    State(state): State<AppState>,
    Path((title_id, entry_id)): Path<(String, String)>,
) -> Result<impl IntoResponse> {
    // TODO: Get username from session
    let username = "admin".to_string();

    // Get library read lock to find the title path
    let lib = state.library.read().await;
    let title = lib.get_title(&title_id)
        .ok_or_else(|| Error::NotFound(format!("Title not found: {}", title_id)))?;

    let title_path = title.path.clone();
    drop(lib); // Release lock before file I/O

    // Load info.json
    let info_path = title_path.join("info.json");
    let info: TitleInfo = if info_path.exists() {
        let content = tokio::fs::read_to_string(&info_path).await?;
        serde_json::from_str(&content).unwrap_or_default()
    } else {
        TitleInfo::default()
    };

    // Get progress for this user and entry
    let page = info
        .progress
        .get(&username)
        .and_then(|user_progress| user_progress.get(&entry_id))
        .copied()
        .unwrap_or(1); // Default to page 1

    Ok(Json(ProgressResponse { page }))
}

/// Get all progress for a user across all titles
pub async fn get_all_progress(
    State(state): State<AppState>,
) -> Result<impl IntoResponse> {
    // TODO: Get username from session
    let username = "admin".to_string();

    let lib = state.library.read().await;
    let mut all_progress = HashMap::new();

    // Iterate through all titles
    for title in lib.get_titles() {
        let info_path = title.path.join("info.json");

        if info_path.exists() {
            if let Ok(content) = tokio::fs::read_to_string(&info_path).await {
                if let Ok(info) = serde_json::from_str::<TitleInfo>(&content) {
                    if let Some(user_progress) = info.progress.get(&username) {
                        for (entry_id, page) in user_progress {
                            all_progress.insert(
                                format!("{}:{}", title.id, entry_id),
                                *page
                            );
                        }
                    }
                }
            }
        }
    }

    Ok(Json(all_progress))
}
