use std::sync::Arc;

use askama::Template;
use axum::{extract::State, response::Html, Json};

use crate::{
    auth::AdminOnly,
    error::Result,
    library::storage::StorageReport,
    routes::presentation::{render_error, NavigationState},
    AppState,
};

#[derive(Template)]
#[template(path = "storage.html")]
struct StorageTemplate {
    nav: NavigationState,
}

/// GET /admin/storage - Supported indexed entry storage information.
pub async fn storage_page(AdminOnly(_username): AdminOnly) -> Result<Html<String>> {
    let template = StorageTemplate {
        nav: NavigationState::admin().with_admin(true),
    };
    Ok(Html(template.render().map_err(render_error)?))
}

/// Returns the last completed measurement without scanning or accessing payload files.
/// Null means no report has been completed. Cached reports are stale and may have
/// unknown sizes. Refresh with POST /api/admin/scan, then request this endpoint again.
#[utoipa::path(
    get, path = "/api/admin/storage", tag = "admin",
    summary = "Get indexed library storage information",
    description = "Last completed report of supported indexed entry payloads, excluding metadata, artwork, hidden and unsupported files. Recursive title totals overlap; total_bytes counts unique payloads. Null before the first report. Partial sizes are lower bounds; cached reports are stale. Refresh through POST /api/admin/scan.",
    responses(
        (status = 200, description = "Completed report, or null when not measured", body = Option<StorageReport>),
        (status = 401, description = "Authentication required"),
        (status = 403, description = "Administrator access required")
    )
)]
pub async fn get_storage(
    State(state): State<AppState>,
    AdminOnly(_username): AdminOnly,
) -> Json<Option<Arc<StorageReport>>> {
    Json(state.library.load().storage_report())
}
