use askama::Template;
use axum::{
    extract::{
        rejection::{FormRejection, JsonRejection},
        State,
    },
    http::StatusCode,
    response::{Html, IntoResponse, Redirect},
    Form, Json,
};
use serde::Deserialize;
use serde_json::json;
use tower_sessions::Session;

use crate::{
    auth::{SESSION_CALLBACK_KEY, SESSION_TOKEN_KEY, SESSION_USERNAME_KEY},
    error::{Error, Result},
    util::render_error,
    AppState,
};

/// Login page template
#[derive(Template)]
#[template(path = "login.html")]
struct LoginTemplate;

/// Login form data
#[derive(Deserialize, utoipa::ToSchema)]
pub struct LoginForm {
    username: String,
    password: String,
}

/// GET /login - Show login page
pub async fn get_login() -> Result<Html<String>> {
    Ok(Html(LoginTemplate.render().map_err(render_error)?))
}

/// POST /login - Process login
pub async fn post_login(
    State(state): State<AppState>,
    session: Session,
    form: std::result::Result<Form<LoginForm>, FormRejection>,
) -> Result<impl IntoResponse> {
    let Form(form) = match form {
        Ok(form) => form,
        Err(_) => return Ok(Redirect::to("/login").into_response()),
    };

    match state
        .storage
        .verify_user(&form.username, &form.password)
        .await?
    {
        Some(token) => {
            session
                .insert(SESSION_TOKEN_KEY, token)
                .await
                .map_err(|e| Error::Internal(format!("Failed to save session: {}", e)))?;
            session
                .insert(SESSION_USERNAME_KEY, form.username.clone())
                .await
                .map_err(|e| Error::Internal(format!("Failed to save session: {}", e)))?;

            let callback = session
                .remove::<String>(SESSION_CALLBACK_KEY)
                .await
                .map_err(|e| Error::Internal(format!("Failed to consume login callback: {}", e)))?;
            let destination = callback
                .filter(|path| path.starts_with('/') && !path.starts_with("//"))
                .unwrap_or_else(|| "/".to_owned());
            tracing::info!("User {} logged in successfully", form.username);
            Ok(Redirect::to(&destination).into_response())
        }
        None => {
            tracing::warn!("Failed login attempt for username: {}", form.username);
            Ok(Redirect::to("/login").into_response())
        }
    }
}

/// POST /api/login - Authenticate using Mango's JSON API contract.
#[utoipa::path(post, path = "/api/login", tag = "users", summary = "Log in", request_body = LoginForm, responses((status = 200, description = "Login succeeded"), (status = 403, description = "Login failed")))]
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
