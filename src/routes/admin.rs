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

/// Cache debug template
#[derive(Template)]
#[template(path = "cache_debug.html")]
struct CacheDebugTemplate {
    home_active: bool,
    library_active: bool,
    tags_active: bool,
    admin_active: bool,
    is_admin: bool,
    stats: crate::library::cache::CacheStats,
    entries: Vec<crate::library::cache::CacheEntryInfo>,
    cache_file_path: String,
    cache_file_exists: bool,
    cache_file_size: u64,
    cache_file_modified: String,
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

/// GET /debug/cache - Cache debug page
/// Shows cache statistics, entries, and control buttons
pub async fn cache_debug_page(
    State(state): State<AppState>,
    AdminOnly(_username): AdminOnly,
) -> Result<Html<String>> {
    let lib = state.library.read().await;

    // Get cache statistics
    let cache = lib.cache().lock().await;
    let stats = cache.stats();

    // Get top 20 cache entries sorted by access count
    let mut entries = cache.entries();
    entries.sort_by(|a, b| b.access_count.cmp(&a.access_count));
    entries.truncate(20);

    drop(cache);

    // Get cache file metadata
    let cache_file_path = state
        .config
        .library_cache_path
        .to_string_lossy()
        .to_string();
    let cache_file_metadata = if let Ok(metadata) =
        tokio::fs::metadata(&state.config.library_cache_path).await
    {
        (
            true,
            metadata.len(),
            metadata
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| {
                    let datetime = chrono::DateTime::<chrono::Utc>::from(std::time::UNIX_EPOCH + d);
                    datetime.format("%Y-%m-%d %H:%M:%S UTC").to_string()
                })
                .unwrap_or_else(|| "Unknown".to_string()),
        )
    } else {
        (false, 0, "N/A".to_string())
    };

    drop(lib);

    let template = CacheDebugTemplate {
        home_active: false,
        library_active: false,
        tags_active: false,
        admin_active: true,
        is_admin: true,
        stats,
        entries,
        cache_file_path,
        cache_file_exists: cache_file_metadata.0,
        cache_file_size: cache_file_metadata.1,
        cache_file_modified: cache_file_metadata.2,
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

/// POST /api/cache/clear - Clear all LRU cache entries
/// Removes all cached sorted lists from memory (library cache file remains)
pub async fn cache_clear_api(
    State(state): State<AppState>,
    AdminOnly(_username): AdminOnly,
) -> Result<Json<serde_json::Value>> {
    let lib = state.library.read().await;
    let mut cache = lib.cache().lock().await;

    cache.clear();
    let stats = cache.stats();

    tracing::info!("Cache cleared by admin");

    Ok(Json(serde_json::json!({
        "success": true,
        "message": "Cache cleared successfully",
        "entries_remaining": stats.entry_count
    })))
}

/// POST /api/cache/save-library - Save library to cache file
/// Saves current library state to persistent cache file
pub async fn cache_save_library_api(
    State(state): State<AppState>,
    AdminOnly(_username): AdminOnly,
) -> Result<Json<serde_json::Value>> {
    let lib = state.library.read().await;

    // Create cached data
    let cached_data = crate::library::cache::CachedLibraryData {
        path: lib.path().to_path_buf(),
        titles: lib.titles().clone(),
    };

    let cache = lib.cache().lock().await;
    cache.save_library_data(cached_data).await?;
    drop(cache);
    drop(lib);

    tracing::info!("Library cache saved by admin");

    Ok(Json(serde_json::json!({
        "success": true,
        "message": "Library cache saved successfully"
    })))
}

/// POST /api/cache/load-library - Load library from cache file
/// Reloads library from persistent cache file
pub async fn cache_load_library_api(
    State(state): State<AppState>,
    AdminOnly(_username): AdminOnly,
) -> Result<Json<serde_json::Value>> {
    let mut lib = state.library.write().await;

    let loaded = lib.try_load_from_cache().await?;

    if loaded {
        let stats = lib.stats();
        drop(lib);

        tracing::info!("Library cache loaded by admin");

        Ok(Json(serde_json::json!({
            "success": true,
            "message": "Library loaded from cache successfully",
            "titles": stats.titles,
            "entries": stats.entries
        })))
    } else {
        drop(lib);
        Ok(Json(serde_json::json!({
            "success": false,
            "message": "No valid cache file found"
        })))
    }
}

/// Request body for cache invalidation endpoint
#[derive(Deserialize)]
pub struct CacheInvalidateRequest {
    /// Pattern to match cache keys (e.g., "sorted_titles:user1:")
    pub pattern: String,
}

/// POST /api/cache/invalidate - Invalidate cache entries by pattern
/// Invalidates all cache entries matching the given pattern prefix
pub async fn cache_invalidate_api(
    State(state): State<AppState>,
    AdminOnly(_username): AdminOnly,
    Json(request): Json<CacheInvalidateRequest>,
) -> Result<Json<serde_json::Value>> {
    let lib = state.library.read().await;
    let mut cache = lib.cache().lock().await;

    // Get all entries and count matches
    let entries = cache.entries();
    let matching_keys: Vec<String> = entries
        .iter()
        .filter(|e| e.key.starts_with(&request.pattern))
        .map(|e| e.key.clone())
        .collect();

    let count = matching_keys.len();

    // Invalidate matching entries
    for key in matching_keys {
        cache.invalidate(&key);
    }

    drop(cache);
    drop(lib);

    tracing::info!(
        "Cache invalidation by admin: {} entries matching '{}'",
        count,
        request.pattern
    );

    Ok(Json(serde_json::json!({
        "success": true,
        "message": format!("Invalidated {} cache entries", count),
        "count": count
    })))
}
