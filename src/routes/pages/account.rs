use askama::Template;
use axum::{extract::State, response::Html};

use crate::{
    auth::User,
    error::Result,
    routes::presentation::{render_error, NavigationState},
    AppState,
};

/// Change Password page template
#[derive(Template)]
#[template(path = "change-password.html")]
struct ChangePasswordTemplate {
    nav: NavigationState,
}

/// GET /change-password - Change password page (requires authentication)
pub async fn change_password_page(user: User) -> Result<Html<String>> {
    let template = ChangePasswordTemplate {
        nav: NavigationState::home().with_admin(user.is_admin), // No specific page active for change password
    };

    Ok(Html(template.render().map_err(render_error)?))
}

/// Request body for change password API endpoint
#[derive(serde::Deserialize, utoipa::ToSchema)]
pub struct ChangePasswordRequest {
    pub current_password: String,
    pub new_password: String,
}

/// POST /api/user/change-password - Change user's password (requires authentication)
#[utoipa::path(post, path = "/api/user/change-password", tag = "users", summary = "Change password", request_body = ChangePasswordRequest, responses((status = 200, description = "Password changed")))]
pub async fn change_password_api(
    State(state): State<AppState>,
    user: User,
    axum::Json(request): axum::Json<ChangePasswordRequest>,
) -> Result<axum::http::StatusCode> {
    // Change the password
    state
        .storage
        .change_password(
            &user.username,
            &request.current_password,
            &request.new_password,
        )
        .await?;

    Ok(axum::http::StatusCode::OK)
}
