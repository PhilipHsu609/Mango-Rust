use axum::{
    extract::{Request, State},
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Redirect, Response},
};
use tower_sessions::Session;

use crate::Storage;

/// Session key for storing username
pub const SESSION_USERNAME_KEY: &str = "username";

/// Session key for storing user token
pub const SESSION_TOKEN_KEY: &str = "token";

/// Authentication middleware that checks if user is logged in
/// Matches original Mango's AuthHandler
pub async fn require_auth(
    State(storage): State<Storage>,
    session: Session,
    mut request: Request,
    next: Next,
) -> Response {
    // Skip auth for public paths
    let path = request.uri().path();
    if is_public_path(path) {
        return next.run(request).await;
    }

    // Check if user has valid session
    if let Ok(Some(token)) = session.get::<String>(SESSION_TOKEN_KEY).await {
        // Verify token in database
        match storage.verify_token(&token).await {
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
    State(storage): State<Storage>,
    session: Session,
    request: Request,
    next: Next,
) -> Response {
    // First check if authenticated
    if let Ok(Some(token)) = session.get::<String>(SESSION_TOKEN_KEY).await {
        match storage.verify_admin(&token).await {
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

/// Helper to get username from request extensions
/// Injected by require_auth middleware
pub fn get_username(request: &Request) -> Option<String> {
    request.extensions().get::<String>().cloned()
}
