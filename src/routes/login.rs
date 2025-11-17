use askama::Template;
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
    util::render_error,
    AppState,
};

/// Login page template
#[derive(Template)]
#[template(path = "login.html")]
struct LoginTemplate {
    error: Option<String>,
}

/// Login form data
#[derive(Deserialize)]
pub struct LoginForm {
    username: String,
    password: String,
}

/// GET /login - Show login page
pub async fn get_login() -> Result<Html<String>> {
    let template = LoginTemplate { error: None };
    Ok(Html(template.render().map_err(render_error)?))
}

/// POST /login - Process login
pub async fn post_login(
    State(state): State<AppState>,
    session: Session,
    Form(form): Form<LoginForm>,
) -> Result<impl IntoResponse> {
    // Verify credentials
    match state
        .storage
        .verify_user(&form.username, &form.password)
        .await?
    {
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
            tracing::warn!("Failed login attempt for username: {}", form.username);
            let _template = LoginTemplate {
                error: Some("Invalid username or password".to_string()),
            };
            Ok(Redirect::to("/login")) // TODO: Return HTML with error instead of redirect
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
