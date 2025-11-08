use axum::{
    extract::Request,
    response::Html,
};

use crate::auth::get_username;

/// Home page template
const HOME_PAGE: &str = include_str!("../../templates/home.html");

/// GET / - Home page (requires authentication)
pub async fn home(request: Request) -> Html<String> {
    // Get username from request extensions (injected by auth middleware)
    let username = get_username(&request).unwrap_or_else(|| "Unknown".to_string());

    // Render template with username
    let html = HOME_PAGE.replace("{{ username }}", &username);
    Html(html)
}
