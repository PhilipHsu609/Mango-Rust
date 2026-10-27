use askama::Template;
use axum::{extract::State, response::Html};

use crate::{auth::AdminOnly, error::Result, routes::presentation::render_error, AppState};

/// Application version from Cargo.toml
const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Admin dashboard template
#[derive(Template)]
#[template(path = "admin.html")]
struct AdminTemplate {
    nav: crate::routes::presentation::NavigationState,
    missing_count: usize,
    storage_total: Option<String>,
    version: &'static str,
    base_url: String,
}

/// GET /admin - Admin dashboard
/// Shows links to:
/// - User Management
/// - Missing Items
/// - Storage Information
/// - Scan Library
/// - Generate Thumbnails
pub async fn admin_dashboard(
    State(state): State<AppState>,
    AdminOnly(_username): AdminOnly,
) -> Result<Html<String>> {
    // Get actual missing count from database
    let missing_count = state.storage.get_missing_count().await?;
    let storage_total = state
        .library
        .load()
        .storage_report()
        .and_then(|report| report.total_label());

    let template = AdminTemplate {
        nav: crate::routes::presentation::NavigationState::admin().with_admin(true), // Admin pages are always accessed by admins
        missing_count,
        storage_total,
        version: VERSION,
        base_url: state.config.base_url.clone(),
    };

    Ok(Html(template.render().map_err(render_error)?))
}
