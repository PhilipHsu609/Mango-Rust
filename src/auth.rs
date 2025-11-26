use axum::{
    async_trait,
    extract::{FromRequestParts, Request, State},
    http::{request::Parts, StatusCode},
    middleware::Next,
    response::{IntoResponse, Redirect, Response},
};
use tower_sessions::Session;

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
    // Skip auth for public paths
    let path = request.uri().path();
    if is_public_path(path) {
        return next.run(request).await;
    }

    // For OPDS paths, try Basic Auth first (for e-reader support)
    if path.starts_with("/opds") || path.starts_with("/api/download") {
        tracing::debug!("OPDS path detected: {}", path);
        if let Some(auth_header) = request.headers().get("authorization") {
            tracing::debug!("Authorization header found");
            if let Ok(auth_str) = auth_header.to_str() {
                if auth_str.starts_with("Basic ") {
                    tracing::debug!("Basic auth detected");
                    if let Some(username) = verify_basic_auth(&state, &auth_str[6..]).await {
                        tracing::debug!("Basic auth successful for user: {}", username);
                        request.extensions_mut().insert(username.clone());
                        return next.run(request).await;
                    } else {
                        tracing::debug!("Basic auth failed");
                    }
                }
            }
        } else {
            tracing::debug!("No authorization header found");
        }
    }

    // Check if user has valid session
    if let Ok(Some(token)) = session.get::<String>(SESSION_TOKEN_KEY).await {
        // Verify token in database
        match state.storage.verify_token(&token).await {
            Ok(Some(username)) => {
                // Add username to request extensions for handlers to use
                request.extensions_mut().insert(username.clone());
                return next.run(request).await;
            }
            Ok(None) => {
                // Token invalid, clear session
                let _ = session.delete().await;
            }
            Err(e) => {
                tracing::error!("Error verifying token: {}", e);
            }
        }
    }

    // Not authenticated, redirect to login
    Redirect::to("/login").into_response()
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
        || path.starts_with("/api/login")
        || path.starts_with("/static/")
        || path.starts_with("/img/")
        || path.starts_with("/css/")
        || path.starts_with("/js/")
}

/// Verify HTTP Basic Auth credentials
/// Returns username if credentials are valid
async fn verify_basic_auth(state: &AppState, base64_credentials: &str) -> Option<String> {
    use base64::{engine::general_purpose, Engine as _};

    tracing::debug!("Verifying basic auth credentials");

    // Decode base64
    let decoded = general_purpose::STANDARD.decode(base64_credentials).ok()?;
    tracing::debug!("Base64 decoded successfully");

    let credentials = String::from_utf8(decoded).ok()?;
    tracing::debug!("Credentials string: {}", credentials);

    // Split into username:password
    let mut parts = credentials.splitn(2, ':');
    let username = parts.next()?;
    let password = parts.next()?;

    tracing::debug!("Attempting to verify user: {}", username);

    // Verify credentials against database
    match state.storage.verify_user(username, password).await {
        Ok(Some(_token)) => {
            tracing::debug!("User verified successfully: {}", username);
            Some(username.to_string())
        }
        Ok(None) => {
            tracing::debug!("User verification failed - invalid credentials");
            None
        }
        Err(e) => {
            tracing::error!("Error verifying user: {}", e);
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
