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
    page: i32,
}

#[derive(Debug, Serialize)]
pub struct ProgressResponse {
    page: i32,
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
    let lib = state.library.load();
    let title = lib
        .get_title(&title_id)
        .ok_or_else(|| Error::NotFound(format!("Title not found: {}", title_id)))?;

    // Verify entry exists
    let _entry = lib
        .get_entry(&title_id, &entry_id)
        .ok_or_else(|| Error::NotFound(format!("Entry not found: {}", entry_id)))?;

    // Save progress via cache (updates cache and persists to disk)
    lib.progress_cache()
        .save_progress(&title_id, &title.path, &username, &entry_id, request.page)
        .await?;

    // Invalidate response cache after progress update
    lib.invalidate_cache_for_progress(&title_id, &username)
        .await;
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
    // Get library read lock
    let lib = state.library.load();

    // Verify title exists
    let _ = lib
        .get_title(&title_id)
        .ok_or_else(|| Error::NotFound(format!("Title not found: {}", title_id)))?;

    // Get progress from cache
    let page = lib
        .progress_cache()
        .get_progress(&title_id, &username, &entry_id)
        .unwrap_or(0);
    drop(lib);

    Ok(Json(ProgressResponse { page: page.max(1) })) // Default to page 1
}

/// GET /api/progress - Get all progress for a user across all titles
/// Returns: JSON object mapping "title_id:entry_id" to page numbers
pub async fn get_all_progress(
    State(state): State<AppState>,
    Username(username): Username,
) -> Result<impl IntoResponse> {
    let lib = state.library.load();
    let cache = lib.progress_cache();
    let mut all_progress = HashMap::new();

    // Iterate through all titles using cache
    for title in lib.get_titles() {
        for entry in &title.entries {
            if let Some(page) = cache.get_progress(&title.id, &username, &entry.id) {
                if page > 0 {
                    all_progress.insert(format!("{}:{}", title.id, entry.id), page);
                }
            }
        }
    }

    Ok(Json(all_progress))
}
