use askama::Template;
use axum::{
    extract::{rejection::JsonRejection, State},
    http::StatusCode,
    response::{Html, IntoResponse, Redirect},
    Form, Json,
};
use serde::Deserialize;
use serde_json::json;
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
            Ok(Redirect::to("/").into_response())
        }
        None => {
            // Invalid credentials, show error
            tracing::warn!("Failed login attempt for username: {}", form.username);
            let template = LoginTemplate {
                error: Some("Invalid username or password".to_string()),
            };
            Ok(Html(template.render().map_err(render_error)?).into_response())
        }
    }
}

/// POST /api/login - Authenticate using Mango's JSON API contract.
pub async fn api_login(
    State(state): State<AppState>,
    session: Session,
    request: std::result::Result<Json<LoginForm>, JsonRejection>,
) -> impl IntoResponse {
    let Json(form) = match request {
        Ok(request) => request,
        Err(error) => {
            return (
                StatusCode::FORBIDDEN,
                Json(json!({"success": false, "error": error.body_text()})),
            )
                .into_response()
        }
    };

    let token = match state
        .storage
        .verify_user(&form.username, &form.password)
        .await
    {
        Ok(Some(token)) => token,
        Ok(None) => {
            return (
                StatusCode::FORBIDDEN,
                Json(json!({"success": false, "error": "Nil assertion failed"})),
            )
                .into_response()
        }
        Err(error) => {
            return (
                StatusCode::FORBIDDEN,
                Json(json!({"success": false, "error": error.to_string()})),
            )
                .into_response()
        }
    };

    if let Err(error) = session.insert(SESSION_TOKEN_KEY, token).await {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({"success": false, "error": error.to_string()})),
        )
            .into_response();
    }

    if let Err(error) = session.save().await {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({"success": false, "error": error.to_string()})),
        )
            .into_response();
    }
    let is_admin = match state.storage.username_is_admin(&form.username).await {
        Ok(is_admin) => is_admin,
        Err(error) => {
            return (
                StatusCode::FORBIDDEN,
                Json(json!({"success": false, "error": error.to_string()})),
            )
                .into_response()
        }
    };
    let Some(session_id) = session.id() else {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({"success": false, "error": "Session ID unavailable"})),
        )
            .into_response();
    };

    (
        StatusCode::OK,
        Json(json!({
            "success": true,
            "session_id": session_id.to_string(),
            "is_admin": is_admin
        })),
    )
        .into_response()
}

/// GET /logout - Clear session and redirect to login
pub async fn logout(session: Session) -> Redirect {
    // Clear session
    let _ = session.delete().await;
    tracing::info!("User logged out");
    Redirect::to("/login")
}
