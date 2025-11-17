use axum::{
    response::Html,
    extract::State,
};
use askama::Template;

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
