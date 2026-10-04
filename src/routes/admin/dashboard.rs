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
    version: &'static str,
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
        nav: crate::routes::presentation::NavigationState::admin().with_admin(true), // Admin pages are always accessed by admins
        missing_count,
        version: VERSION,
    };

    Ok(Html(template.render().map_err(render_error)?))
}
