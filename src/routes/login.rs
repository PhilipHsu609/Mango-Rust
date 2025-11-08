use axum::{
    extract::State,
    response::{Html, IntoResponse, Redirect},
    Form,
};
use serde::Deserialize;
use tower_sessions::Session;

use crate::{
    auth::{SESSION_TOKEN_KEY, SESSION_USERNAME_KEY},
    error::{Error, Result},
    AppState,
};

/// Login page template
const LOGIN_PAGE: &str = include_str!("../../templates/login.html");

/// Login form data
#[derive(Deserialize)]
pub struct LoginForm {
    username: String,
    password: String,
}

/// GET /login - Show login page
pub async fn get_login() -> Html<String> {
    // Render template without error - remove error block entirely
    let html = LOGIN_PAGE
        .replace("{% if error %}", "<!--")
        .replace("{% endif %}", "-->");
    Html(html)
}

/// POST /login - Process login
pub async fn post_login(
    State(state): State<AppState>,
    session: Session,
    Form(form): Form<LoginForm>,
) -> Result<impl IntoResponse> {
    // Verify credentials
    match state.storage.verify_user(&form.username, &form.password).await? {
        Some(token) => {
            // Store token and username in session
            session
                .insert(SESSION_TOKEN_KEY, token)
                .await
                .map_err(|e| Error::Internal(format!("Failed to save session: {}", e)))?;
            session
                .insert(SESSION_USERNAME_KEY, form.username.clone())
                .await
                .map_err(|e| Error::Internal(format!("Failed to save session: {}", e)))?;

            tracing::info!("User {} logged in successfully", form.username);
            Ok(Redirect::to("/"))
        }
        None => {
            // Invalid credentials, show error
            tracing::warn!(
                "Failed login attempt for username: {}",
                form.username
            );
            let _html = LOGIN_PAGE
                .replace("{% if error %}", "{% if true %}")
                .replace("{{ error }}", "Invalid username or password");
            // TODO: Implement proper error display with Askama templates
            Ok(Redirect::to("/login"))
        }
    }
}

/// GET /logout - Clear session and redirect to login
pub async fn logout(session: Session) -> Redirect {
    // Clear session
    let _ = session.delete().await;
    tracing::info!("User logged out");
    Redirect::to("/login")
}
