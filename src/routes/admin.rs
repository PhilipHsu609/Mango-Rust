use askama::Template;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Html,
    Json,
};
use serde::{Deserialize, Serialize};
use std::time::Instant;

use crate::{auth::AdminOnly, error::Result, util::render_error, AppState};

/// Admin dashboard template
#[derive(Template)]
#[template(path = "admin.html")]
struct AdminTemplate {
    home_active: bool,
    library_active: bool,
    tags_active: bool,
    admin_active: bool,
    is_admin: bool,
    missing_count: usize,
}

/// GET /admin - Admin dashboard
/// Shows links to:
/// - User Management
/// - Missing Items
/// - Scan Library
/// - Generate Thumbnails
pub async fn admin_dashboard(
    State(state): State<AppState>,
    AdminOnly(_username): AdminOnly,
) -> Result<Html<String>> {
    // Get actual missing count from database
    let missing_count = state.storage.get_missing_count().await?;

    let template = AdminTemplate {
        home_active: false,
        library_active: false,
        tags_active: false,
        admin_active: true,
        is_admin: true,
        missing_count,
    };

    Ok(Html(template.render().map_err(render_error)?))
}

/// Response for library scan endpoint
#[derive(Serialize)]
pub struct ScanResponse {
    pub titles: usize,
    pub milliseconds: u128,
}

/// POST /api/admin/scan - Trigger library rescan
/// Returns number of titles found and time taken in milliseconds
pub async fn scan_library(
    State(state): State<AppState>,
    AdminOnly(_username): AdminOnly,
) -> Result<Json<ScanResponse>> {
    let start = Instant::now();

    // Trigger library scan
    let mut library = state.library.write().await;
    library.scan().await?;
    let stats = library.stats();

    let elapsed = start.elapsed().as_millis();

    tracing::info!(
        "Library scan completed: {} titles in {}ms",
        stats.titles,
        elapsed
    );

    Ok(Json(ScanResponse {
        titles: stats.titles,
        milliseconds: elapsed,
    }))
}

/// GET /api/admin/entries/missing - Get all missing entries
/// Returns list of entries marked as unavailable in the database
pub async fn get_missing_entries(
    State(state): State<AppState>,
    AdminOnly(_username): AdminOnly,
) -> Result<Json<Vec<crate::storage::MissingEntry>>> {
    let entries = state.storage.get_missing_entries().await?;
    Ok(Json(entries))
}

/// DELETE /api/admin/entries/missing/:id - Delete a specific missing entry
/// Removes the entry from the database (cannot be undone)
pub async fn delete_missing_entry(
    State(state): State<AppState>,
    AdminOnly(_username): AdminOnly,
    Path(id): Path<String>,
) -> Result<StatusCode> {
    state.storage.delete_missing_entry(&id).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// DELETE /api/admin/entries/missing - Delete all missing entries
/// Removes all unavailable entries from the database (cannot be undone)
pub async fn delete_all_missing_entries(
    State(state): State<AppState>,
    AdminOnly(_username): AdminOnly,
) -> Result<Json<serde_json::Value>> {
    let count = state.storage.delete_all_missing_entries().await?;
    Ok(Json(serde_json::json!({
        "deleted": count
    })))
}

/// Missing Items template
#[derive(Template)]
#[template(path = "missing-items.html")]
struct MissingItemsTemplate {
    home_active: bool,
    library_active: bool,
    tags_active: bool,
    admin_active: bool,
    is_admin: bool,
}

/// GET /admin/missing-items - Missing items management page
/// Shows list of items in database whose files no longer exist
pub async fn missing_items_page(AdminOnly(_username): AdminOnly) -> Result<Html<String>> {
    let template = MissingItemsTemplate {
        home_active: false,
        library_active: false,
        tags_active: false,
        admin_active: true,
        is_admin: true,
    };

    Ok(Html(template.render().map_err(render_error)?))
}

/// Users template
#[derive(Template)]
#[template(path = "users.html")]
struct UsersTemplate {
    home_active: bool,
    library_active: bool,
    tags_active: bool,
    admin_active: bool,
    is_admin: bool,
    username: String,
}

/// GET /admin/users - User management page
/// Shows list of users and allows creating/deleting users
pub async fn users_page(AdminOnly(username): AdminOnly) -> Result<Html<String>> {
    let template = UsersTemplate {
        home_active: false,
        library_active: false,
        tags_active: false,
        admin_active: true,
        is_admin: true,
        username,
    };

    Ok(Html(template.render().map_err(render_error)?))
}

/// User response for API endpoints
#[derive(Serialize)]
pub struct UserResponse {
    pub username: String,
    pub is_admin: bool,
}

/// GET /api/admin/users - Get all users
/// Returns list of all users with their admin status
pub async fn get_users(
    State(state): State<AppState>,
    AdminOnly(_username): AdminOnly,
) -> Result<Json<Vec<UserResponse>>> {
    let users = state.storage.list_users().await?;
    let response = users
        .into_iter()
        .map(|(username, is_admin)| UserResponse { username, is_admin })
        .collect();
    Ok(Json(response))
}

/// Request body for creating a new user
#[derive(Deserialize)]
pub struct CreateUserRequest {
    pub username: String,
    pub password: String,
    pub is_admin: bool,
}

/// POST /api/admin/users - Create a new user
/// Creates a new user with the given credentials and admin status
pub async fn create_user(
    State(state): State<AppState>,
    AdminOnly(_username): AdminOnly,
    Json(request): Json<CreateUserRequest>,
) -> Result<StatusCode> {
    // Check if username already exists
    if state.storage.username_exists(&request.username).await? {
        return Err(crate::error::Error::Internal(format!(
            "Username '{}' already exists",
            request.username
        )));
    }

    state
        .storage
        .create_user(&request.username, &request.password, request.is_admin)
        .await?;

    tracing::info!(
        "User '{}' created (admin: {})",
        request.username,
        request.is_admin
    );

    Ok(StatusCode::CREATED)
}

/// Request body for updating a user
#[derive(Deserialize)]
pub struct UpdateUserRequest {
    pub is_admin: bool,
}

/// PATCH /api/admin/users/:username - Update user's admin status
/// Changes whether a user is an administrator
pub async fn update_user(
    State(state): State<AppState>,
    AdminOnly(current_username): AdminOnly,
    Path(username): Path<String>,
    Json(request): Json<UpdateUserRequest>,
) -> Result<StatusCode> {
    // Prevent users from demoting themselves
    if username == current_username && !request.is_admin {
        return Err(crate::error::Error::Internal(
            "Cannot demote yourself from admin".to_string(),
        ));
    }

    // Check if user exists
    if !state.storage.username_exists(&username).await? {
        return Err(crate::error::Error::Internal(format!(
            "User '{}' not found",
            username
        )));
    }

    // Update user admin status using existing update_user method
    state
        .storage
        .update_user(&username, &username, None, request.is_admin)
        .await?;

    tracing::info!(
        "User '{}' admin status updated to {}",
        username,
        request.is_admin
    );

    Ok(StatusCode::NO_CONTENT)
}

/// DELETE /api/admin/users/:username - Delete a user
/// Removes a user from the system (cannot be undone)
pub async fn delete_user(
    State(state): State<AppState>,
    AdminOnly(current_username): AdminOnly,
    Path(username): Path<String>,
) -> Result<StatusCode> {
    // Prevent users from deleting themselves
    if username == current_username {
        return Err(crate::error::Error::Internal(
            "Cannot delete yourself".to_string(),
        ));
    }

    state.storage.delete_user(&username).await?;

    tracing::info!("User '{}' deleted", username);

    Ok(StatusCode::NO_CONTENT)
}
