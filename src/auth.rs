use axum::{
    async_trait,
    extract::{FromRequestParts, Request, State},
    http::{request::Parts, StatusCode},
    middleware::Next,
    response::{IntoResponse, Redirect, Response},
};
use std::str::FromStr;
use tower_sessions::{session::Id, Session, SessionStore};

use crate::AppState;

/// Session key for storing username
pub const SESSION_USERNAME_KEY: &str = "username";

/// Session key for storing user token
pub const SESSION_TOKEN_KEY: &str = "token";

/// Authentication middleware that checks if user is logged in
/// Matches original Mango's AuthHandler
pub async fn require_auth(
    State(state): State<AppState>,
    session: Session,
    mut request: Request,
    next: Next,
) -> Response {
    let path = request.uri().path();
    if request.method() == axum::http::Method::OPTIONS || is_public_path(path) {
        return next.run(request).await;
    }

    let is_opds_path = path.starts_with("/opds") || path.starts_with("/api/download");
    let authorization = request
        .headers()
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|header| header.to_str().ok())
        .map(str::to_owned);

    // Cookie sessions take precedence over Authorization credentials and configured identities.
    if let Ok(Some(token)) = session.get::<String>(SESSION_TOKEN_KEY).await {
        match state.storage.verify_token(&token).await {
            Ok(Some(username)) => {
                request.extensions_mut().insert(username);
                return next.run(request).await;
            }
            Ok(None) => {
                let _ = session.delete().await;
            }
            Err(error) => tracing::error!("Error verifying token: {}", error),
        }
    }

    // Mango accepts Basic credentials on any protected path and stores the token in the session.
    if let Some(credentials) = authorization
        .as_deref()
        .and_then(|value| value.strip_prefix("Basic "))
    {
        if let Some((username, token)) = verify_basic_auth(&state, credentials).await {
            if let Err(error) = session.insert(SESSION_TOKEN_KEY, token).await {
                tracing::error!("Error saving Basic-auth session token: {}", error);
            }
            request.extensions_mut().insert(username);
            return next.run(request).await;
        }
    }

    if let Some(session_id) = authorization
        .as_deref()
        .and_then(|value| value.strip_prefix("Bearer "))
    {
        match Id::from_str(session_id) {
            Ok(id) => match state.session_store.load(&id).await {
                Ok(Some(record)) => {
                    if let Some(token) = record
                        .data
                        .get(SESSION_TOKEN_KEY)
                        .and_then(serde_json::Value::as_str)
                    {
                        match state.storage.verify_token(token).await {
                            Ok(Some(username)) => {
                                request.extensions_mut().insert(username);
                                return next.run(request).await;
                            }
                            Ok(None) => {}
                            Err(error) => {
                                tracing::error!("Error verifying bearer session token: {}", error)
                            }
                        }
                    }
                }
                Ok(None) => {}
                Err(error) => tracing::error!("Error loading bearer session: {}", error),
            },
            Err(error) => tracing::debug!("Invalid bearer session ID: {}", error),
        }
    }

    if state.config.disable_login {
        if let Some(username) =
            authenticated_configured_user(&state, &state.config.default_username).await
        {
            request.extensions_mut().insert(username);
            return next.run(request).await;
        }
    } else if !state.config.auth_proxy_header_name.is_empty() {
        if let Some(username) = request
            .headers()
            .get(&state.config.auth_proxy_header_name)
            .and_then(|header| header.to_str().ok())
        {
            if let Some(username) = authenticated_configured_user(&state, username).await {
                request.extensions_mut().insert(username);
                return next.run(request).await;
            }
        }
    }

    if is_opds_path {
        return (
            StatusCode::UNAUTHORIZED,
            [(
                axum::http::header::WWW_AUTHENTICATE,
                "Basic realm=\"Mango\"",
            )],
        )
            .into_response();
    }

    if path.starts_with("/api") {
        return (StatusCode::UNAUTHORIZED, "Unauthorized").into_response();
    }

    Redirect::to("/login").into_response()
}

async fn authenticated_configured_user(state: &AppState, username: &str) -> Option<String> {
    match state.storage.username_exists(username).await {
        Ok(true) => Some(username.to_owned()),
        Ok(false) => None,
        Err(error) => {
            tracing::error!("Error checking configured auth user: {}", error);
            None
        }
    }
}

/// Admin authorization middleware - requires authenticated user to be admin
pub async fn require_admin(
    State(state): State<AppState>,
    session: Session,
    request: Request,
    next: Next,
) -> Response {
    // First check if authenticated
    if let Ok(Some(token)) = session.get::<String>(SESSION_TOKEN_KEY).await {
        match state.storage.verify_admin(&token).await {
            Ok(true) => {
                // User is admin, proceed
                return next.run(request).await;
            }
            Ok(false) => {
                // User authenticated but not admin
                return (StatusCode::FORBIDDEN, "Admin access required").into_response();
            }
            Err(e) => {
                tracing::error!("Error verifying admin: {}", e);
            }
        }
    }

    // Not authenticated or not admin
    (StatusCode::FORBIDDEN, "Admin access required").into_response()
}

/// Check if a path should skip authentication
/// Matches original AuthHandler's exclude logic
fn is_public_path(path: &str) -> bool {
    path == "/login"
        || path == "/logout"
        || path.starts_with("/api/login")
        || path.starts_with("/static/")
        || path.starts_with("/img/")
        || path.starts_with("/css/")
        || path.starts_with("/js/")
        || path.starts_with("/uploads/")
}

/// Verify HTTP Basic Auth credentials and return the username and session token.
async fn verify_basic_auth(state: &AppState, base64_credentials: &str) -> Option<(String, String)> {
    use base64::{engine::general_purpose, Engine as _};

    tracing::debug!("Verifying basic auth credentials");

    let decoded = general_purpose::STANDARD.decode(base64_credentials).ok()?;
    let credentials = String::from_utf8(decoded).ok()?;
    let (username, password) = credentials.split_once(':')?;

    tracing::debug!("Attempting to verify user: {}", username);

    match state.storage.verify_user(username, password).await {
        Ok(Some(token)) => {
            tracing::debug!("User verified successfully: {}", username);
            Some((username.to_owned(), token))
        }
        Ok(None) => {
            tracing::debug!("User verification failed - invalid credentials");
            None
        }
        Err(error) => {
            tracing::error!("Error verifying user: {}", error);
            None
        }
    }
}

/// Helper to get username from request extensions
/// Injected by require_auth middleware
pub fn get_username(request: &Request) -> Option<String> {
    request.extensions().get::<String>().cloned()
}

/// Username extractor that can be used as a handler parameter
/// Extracts username from request extensions (set by require_auth middleware)
pub struct Username(pub String);

#[async_trait]
impl<S> FromRequestParts<S> for Username
where
    S: Send + Sync,
{
    type Rejection = StatusCode;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        parts
            .extensions
            .get::<String>()
            .cloned()
            .map(Username)
            .ok_or(StatusCode::UNAUTHORIZED)
    }
}

/// AdminOnly extractor that requires the authenticated user to be an admin
/// Similar to Username but also verifies admin status
pub struct AdminOnly(pub String);

#[async_trait]
impl FromRequestParts<AppState> for AdminOnly {
    type Rejection = (StatusCode, &'static str);

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        // First check if user is authenticated
        let username = parts
            .extensions
            .get::<String>()
            .cloned()
            .ok_or((StatusCode::UNAUTHORIZED, "Not authenticated"))?;

        // Check if user is admin
        let is_admin = state.storage.is_admin(&username).await.map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to verify admin status",
            )
        })?;

        if is_admin {
            Ok(AdminOnly(username))
        } else {
            Err((StatusCode::FORBIDDEN, "Admin access required"))
        }
    }
}

/// User extractor that provides username and admin status
/// Can be used in any authenticated handler
pub struct User {
    pub username: String,
    pub is_admin: bool,
}

#[async_trait]
impl FromRequestParts<AppState> for User {
    type Rejection = StatusCode;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        // Get username from request extensions
        let username = parts
            .extensions
            .get::<String>()
            .cloned()
            .ok_or(StatusCode::UNAUTHORIZED)?;

        // Check if user is admin
        let is_admin = state
            .storage
            .is_admin(&username)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        Ok(User { username, is_admin })
    }
}
