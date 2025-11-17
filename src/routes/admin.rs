use axum::{
    response::Html,
    extract::{State, Path},
    http::StatusCode,
    Json,
};
use askama::Template;
use serde::Serialize;
use std::time::Instant;

use crate::{auth::AdminOnly, error::Result, AppState};

/// Admin dashboard template
#[derive(Template)]
#[template(path = "admin.html")]
struct AdminTemplate {
    home_active: bool,
    library_active: bool,
    admin_active: bool,
    missing_count: usize,
}

/// GET /admin - Admin dashboard
/// Shows links to:
/// - User Management
/// - Missing Items
/// - Scan Library
/// - Generate Thumbnails
pub async fn admin_dashboard(
    State(state): State<AppState>,
    AdminOnly(_username): AdminOnly,
) -> Result<Html<String>> {
    // Get actual missing count from database
    let missing_count = state.storage.get_missing_count().await?;

    let template = AdminTemplate {
        home_active: false,
        library_active: false,
        admin_active: true,
        missing_count,
    };

    Ok(Html(template.render().map_err(|e| {
        crate::error::Error::Internal(format!("Template render error: {}", e))
    })?))
}

/// Response for library scan endpoint
#[derive(Serialize)]
pub struct ScanResponse {
    pub titles: usize,
    pub milliseconds: u128,
}

/// POST /api/admin/scan - Trigger library rescan
/// Returns number of titles found and time taken in milliseconds
pub async fn scan_library(
    State(state): State<AppState>,
    AdminOnly(_username): AdminOnly,
) -> Result<Json<ScanResponse>> {
    let start = Instant::now();

    // Trigger library scan
    let mut library = state.library.write().await;
    library.scan().await?;
    let stats = library.stats();

    let elapsed = start.elapsed().as_millis();

    tracing::info!("Library scan completed: {} titles in {}ms", stats.titles, elapsed);

    Ok(Json(ScanResponse {
        titles: stats.titles,
        milliseconds: elapsed,
    }))
}

/// GET /api/admin/entries/missing - Get all missing entries
/// Returns list of entries marked as unavailable in the database
pub async fn get_missing_entries(
    State(state): State<AppState>,
    AdminOnly(_username): AdminOnly,
) -> Result<Json<Vec<crate::storage::MissingEntry>>> {
    let entries = state.storage.get_missing_entries().await?;
    Ok(Json(entries))
}

/// DELETE /api/admin/entries/missing/:id - Delete a specific missing entry
/// Removes the entry from the database (cannot be undone)
pub async fn delete_missing_entry(
    State(state): State<AppState>,
    AdminOnly(_username): AdminOnly,
    Path(id): Path<String>,
) -> Result<StatusCode> {
    state.storage.delete_missing_entry(&id).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// DELETE /api/admin/entries/missing - Delete all missing entries
/// Removes all unavailable entries from the database (cannot be undone)
pub async fn delete_all_missing_entries(
    State(state): State<AppState>,
    AdminOnly(_username): AdminOnly,
) -> Result<Json<serde_json::Value>> {
    let count = state.storage.delete_all_missing_entries().await?;
    Ok(Json(serde_json::json!({
        "deleted": count
    })))
}

/// Missing Items template
#[derive(Template)]
#[template(path = "missing-items.html")]
struct MissingItemsTemplate {
    home_active: bool,
    library_active: bool,
    admin_active: bool,
}

/// GET /admin/missing-items - Missing items management page
/// Shows list of items in database whose files no longer exist
pub async fn missing_items_page(
    AdminOnly(_username): AdminOnly,
) -> Result<Html<String>> {
    let template = MissingItemsTemplate {
        home_active: false,
        library_active: false,
        admin_active: true,
    };

    Ok(Html(template.render().map_err(|e| {
        crate::error::Error::Internal(format!("Template render error: {}", e))
    })?))
}
