use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Json},
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::{
    auth::Username,
    error::{Error, Result},
    AppState,
};

#[derive(Debug, Deserialize)]
pub struct SaveProgressRequest {
    page: usize,
}

#[derive(Debug, Serialize)]
pub struct ProgressResponse {
    page: usize,
}

/// POST /api/progress/{title_id}/{entry_id} - Save reading progress for an entry
/// Returns: 200 OK on success
pub async fn save_progress(
    State(state): State<AppState>,
    Path((title_id, entry_id)): Path<(String, String)>,
    Username(username): Username,
    Json(request): Json<SaveProgressRequest>,
) -> Result<impl IntoResponse> {
    // Get library read lock to find the title
    let lib = state.library.read().await;
    let title = lib
        .get_title(&title_id)
        .ok_or_else(|| Error::NotFound(format!("Title not found: {}", title_id)))?;

    // Verify entry exists
    let _entry = lib
        .get_entry(&title_id, &entry_id)
        .ok_or_else(|| Error::NotFound(format!("Entry not found: {}", entry_id)))?;

    // Save progress using Title's method
    title
        .save_entry_progress(&username, &entry_id, request.page)
        .await?;
    drop(lib); // Release lock

    tracing::debug!(
        "Saved progress: {} / {} = page {}",
        title_id,
        entry_id,
        request.page
    );

    Ok(StatusCode::OK)
}

/// GET /api/progress/{title_id}/{entry_id} - Get reading progress for an entry
/// Returns: JSON with current page number
pub async fn get_progress(
    State(state): State<AppState>,
    Path((title_id, entry_id)): Path<(String, String)>,
    Username(username): Username,
) -> Result<impl IntoResponse> {
    // Get library read lock to find the title
    let lib = state.library.read().await;
    let title = lib
        .get_title(&title_id)
        .ok_or_else(|| Error::NotFound(format!("Title not found: {}", title_id)))?;

    // Load progress using Title's method
    let page = title.load_entry_progress(&username, &entry_id).await?;
    drop(lib);

    Ok(Json(ProgressResponse { page: page.max(1) })) // Default to page 1
}

/// GET /api/progress - Get all progress for a user across all titles
/// Returns: JSON object mapping "title_id:entry_id" to page numbers
pub async fn get_all_progress(
    State(state): State<AppState>,
    Username(username): Username,
) -> Result<impl IntoResponse> {
    let lib = state.library.read().await;
    let mut all_progress = HashMap::new();

    // Iterate through all titles
    for title in lib.get_titles() {
        for entry in &title.entries {
            if let Ok(page) = title.load_entry_progress(&username, &entry.id).await {
                if page > 0 {
                    all_progress.insert(format!("{}:{}", title.id, entry.id), page);
                }
            }
        }
    }

    Ok(Json(all_progress))
}
