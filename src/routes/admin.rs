use axum::{
    response::Html,
    extract::State,
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
    State(_state): State<AppState>,
    AdminOnly(_username): AdminOnly,
) -> Result<Html<String>> {
    let template = AdminTemplate {
        home_active: false,
        library_active: false,
        admin_active: true,
        missing_count: 0, // TODO: Implement missing items tracking
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
