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

/// Application version from Cargo.toml
const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Admin dashboard template
#[derive(Template)]
#[template(path = "admin.html")]
struct AdminTemplate {
    nav: crate::util::NavigationState,
    missing_count: usize,
    version: &'static str,
}

/// Cache debug template
#[derive(Template)]
#[template(path = "cache_debug.html")]
struct CacheDebugTemplate {
    nav: crate::util::NavigationState,
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
        nav: crate::util::NavigationState::admin().with_admin(true), // Admin pages are always accessed by admins
        missing_count,
        version: VERSION,
    };

    Ok(Html(template.render().map_err(render_error)?))
}

/// GET /debug/cache - Cache debug page
/// Shows cache statistics, entries, and control buttons
pub async fn cache_debug_page(
    State(state): State<AppState>,
    AdminOnly(_username): AdminOnly,
) -> Result<Html<String>> {
    let lib = state.library.load();

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
        nav: crate::util::NavigationState::admin().with_admin(true),
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
/// Uses double-buffer approach: builds new library in background, then atomically swaps
pub async fn scan_library(
    State(state): State<AppState>,
    AdminOnly(_username): AdminOnly,
) -> Result<Json<ScanResponse>> {
    let start = Instant::now();

    // Build new library instance and scan (double-buffer approach)
    let mut new_lib = crate::library::Library::new(
        state.config.library_path.clone(),
        state.storage.clone(),
        &state.config,
    );
    new_lib.scan().await?;
    let stats = new_lib.stats();

    // Atomically swap the new library in
    state.library.store(std::sync::Arc::new(new_lib));

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
    nav: crate::util::NavigationState,
}

/// GET /admin/missing-items - Missing items management page
/// Shows list of items in database whose files no longer exist
pub async fn missing_items_page(AdminOnly(_username): AdminOnly) -> Result<Html<String>> {
    let template = MissingItemsTemplate {
        nav: crate::util::NavigationState::admin().with_admin(true),
    };

    Ok(Html(template.render().map_err(render_error)?))
}

/// Users template
#[derive(Template)]
#[template(path = "users.html")]
struct UsersTemplate {
    nav: crate::util::NavigationState,
    username: String,
}

/// GET /admin/users - User management page
/// Shows list of users and allows creating/deleting users
pub async fn users_page(AdminOnly(username): AdminOnly) -> Result<Html<String>> {
    let template = UsersTemplate {
        nav: crate::util::NavigationState::admin().with_admin(true),
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
        return Err(crate::error::Error::Conflict(format!(
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
        return Err(crate::error::Error::Forbidden(
            "Cannot demote yourself from admin".to_string(),
        ));
    }

    // Check if user exists
    if !state.storage.username_exists(&username).await? {
        return Err(crate::error::Error::NotFound(format!(
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
        return Err(crate::error::Error::Forbidden(
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
    let lib = state.library.load();
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
    let lib = state.library.load();

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
/// Uses double-buffer approach: creates new library, loads from cache, swaps
pub async fn cache_load_library_api(
    State(state): State<AppState>,
    AdminOnly(_username): AdminOnly,
) -> Result<Json<serde_json::Value>> {
    // Build new library instance and try to load from cache
    let mut new_lib = crate::library::Library::new(
        state.config.library_path.clone(),
        state.storage.clone(),
        &state.config,
    );

    let loaded = new_lib.try_load_from_cache().await?;

    if loaded {
        let stats = new_lib.stats();

        // Atomically swap the new library in
        state.library.store(std::sync::Arc::new(new_lib));

        tracing::info!("Library cache loaded by admin");

        Ok(Json(serde_json::json!({
            "success": true,
            "message": "Library loaded from cache successfully",
            "titles": stats.titles,
            "entries": stats.entries
        })))
    } else {
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
    let lib = state.library.load();
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

// ========== Title/Entry Metadata API Endpoints ==========

#[derive(Deserialize)]
pub struct DisplayNameQuery {
    eid: Option<String>,
}

/// PUT /api/admin/display_name/:tid/:name - Update display name for title or entry
pub async fn update_display_name(
    State(state): State<AppState>,
    AdminOnly(_username): AdminOnly,
    Path((title_id, name)): Path<(String, String)>,
    axum::extract::Query(query): axum::extract::Query<DisplayNameQuery>,
) -> Result<Json<serde_json::Value>> {
    let decoded_name = percent_encoding::percent_decode_str(&name)
        .decode_utf8()
        .map_err(|e| crate::error::Error::BadRequest(format!("Invalid UTF-8 in name: {}", e)))?
        .to_string();

    // Update display name in database
    if let Some(entry_id) = query.eid {
        state
            .storage
            .update_entry_display_name(&entry_id, &decoded_name)
            .await?;
        tracing::info!("Updated entry {} display name to '{}'", entry_id, decoded_name);
    } else {
        state
            .storage
            .update_title_display_name(&title_id, &decoded_name)
            .await?;
        tracing::info!("Updated title {} display name to '{}'", title_id, decoded_name);
    }

    Ok(Json(serde_json::json!({
        "success": true
    })))
}

#[derive(Deserialize)]
pub struct SortTitleQuery {
    eid: Option<String>,
    name: Option<String>,
}

/// PUT /api/admin/sort_title/:tid - Update sort title for title or entry
pub async fn update_sort_title(
    State(state): State<AppState>,
    AdminOnly(_username): AdminOnly,
    Path(title_id): Path<String>,
    axum::extract::Query(query): axum::extract::Query<SortTitleQuery>,
) -> Result<Json<serde_json::Value>> {
    let sort_title = query.name.as_deref();

    if let Some(entry_id) = &query.eid {
        state
            .storage
            .update_entry_sort_title(entry_id, sort_title)
            .await?;
        tracing::info!("Updated entry {} sort title to {:?}", entry_id, sort_title);
    } else {
        state
            .storage
            .update_title_sort_title(&title_id, sort_title)
            .await?;
        tracing::info!("Updated title {} sort title to {:?}", title_id, sort_title);
    }

    Ok(Json(serde_json::json!({
        "success": true
    })))
}

// ========== Bulk Progress API ==========

#[derive(Deserialize)]
pub struct BulkProgressRequest {
    ids: Vec<String>,
}

/// PUT /api/bulk_progress/:action/:tid - Bulk update progress for multiple entries
/// action: "read" (100%) or "unread" (0%)
pub async fn bulk_progress(
    State(state): State<AppState>,
    crate::auth::Username(username): crate::auth::Username,
    Path((action, title_id)): Path<(String, String)>,
    Json(request): Json<BulkProgressRequest>,
) -> Result<Json<serde_json::Value>> {
    let lib = state.library.load();

    let title = lib
        .get_title(&title_id)
        .ok_or_else(|| crate::error::Error::NotFound(format!("Title not found: {}", title_id)))?;

    let cache = lib.progress_cache();
    for entry_id in &request.ids {
        // Get entry to find page count
        if let Some(entry) = lib.get_entry(&title_id, entry_id) {
            let page = match action.as_str() {
                "read" => entry.pages as i32,
                "unread" => 0i32,
                _ => {
                    return Err(crate::error::Error::BadRequest(format!(
                        "Invalid action: {}. Use 'read' or 'unread'",
                        action
                    )))
                }
            };

            cache
                .save_progress(&title_id, &title.path, &username, entry_id, page)
                .await?;
        }
    }

    // Invalidate cache
    lib.invalidate_cache_for_progress(&title_id, &username).await;

    tracing::info!(
        "Bulk progress update: {} entries marked as {} for title {}",
        request.ids.len(),
        action,
        title_id
    );

    Ok(Json(serde_json::json!({
        "success": true
    })))
}

// ========== Thumbnail Generation API ==========

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

/// Global thumbnail generation state
static THUMBNAIL_GENERATING: AtomicBool = AtomicBool::new(false);
static THUMBNAIL_CURRENT: AtomicUsize = AtomicUsize::new(0);
static THUMBNAIL_TOTAL: AtomicUsize = AtomicUsize::new(0);

/// GET /api/admin/thumbnail_progress - Get thumbnail generation progress
pub async fn thumbnail_progress(
    AdminOnly(_username): AdminOnly,
) -> Result<Json<serde_json::Value>> {
    let generating = THUMBNAIL_GENERATING.load(Ordering::SeqCst);
    let current = THUMBNAIL_CURRENT.load(Ordering::SeqCst);
    let total = THUMBNAIL_TOTAL.load(Ordering::SeqCst);

    Ok(Json(serde_json::json!({
        "success": true,
        "generating": generating,
        "current": current,
        "total": total
    })))
}

/// POST /api/admin/generate_thumbnails - Start thumbnail generation
pub async fn generate_thumbnails(
    State(state): State<AppState>,
    AdminOnly(_username): AdminOnly,
) -> Result<Json<serde_json::Value>> {
    // Atomically check and set to avoid race condition
    if THUMBNAIL_GENERATING
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return Ok(Json(serde_json::json!({
            "success": false,
            "error": "Thumbnail generation already in progress"
        })));
    }
    THUMBNAIL_CURRENT.store(0, Ordering::SeqCst);

    // Get all entries that need thumbnails
    let lib = state.library.load();
    let mut entries_to_process: Vec<(String, String)> = Vec::new();

    for title in lib.get_titles() {
        for entry in &title.entries {
            entries_to_process.push((title.id.clone(), entry.id.clone()));
        }
    }

    THUMBNAIL_TOTAL.store(entries_to_process.len(), Ordering::SeqCst);
    drop(lib);

    // Spawn background task
    let state_clone = state.clone();
    tokio::spawn(async move {
        let lib = state_clone.library.load();
        let db = state_clone.storage.pool();

        for (i, (title_id, entry_id)) in entries_to_process.iter().enumerate() {
            THUMBNAIL_CURRENT.store(i + 1, Ordering::SeqCst);

            if let Some(entry) = lib.get_entry(title_id, entry_id) {
                // Check if thumbnail already exists
                match crate::library::Entry::get_thumbnail(entry_id, db).await {
                    Ok(Some(_)) => continue, // Already has thumbnail
                    _ => {}
                }

                // Generate thumbnail
                if let Err(e) = entry.generate_thumbnail(db).await {
                    tracing::warn!("Failed to generate thumbnail for {}: {}", entry_id, e);
                }
            }
        }

        THUMBNAIL_GENERATING.store(false, Ordering::SeqCst);
        tracing::info!("Thumbnail generation completed");
    });

    Ok(Json(serde_json::json!({
        "success": true,
        "message": "Thumbnail generation started"
    })))
}

// ========== Cover Upload API ==========

use axum::extract::Multipart;

#[derive(Deserialize)]
pub struct CoverUploadQuery {
    tid: String,
    eid: Option<String>,
}

/// POST /api/admin/upload/cover - Upload custom cover image
pub async fn upload_cover(
    State(state): State<AppState>,
    AdminOnly(_username): AdminOnly,
    axum::extract::Query(query): axum::extract::Query<CoverUploadQuery>,
    mut multipart: Multipart,
) -> Result<Json<serde_json::Value>> {
    // Get the file from multipart
    let mut file_data: Option<Vec<u8>> = None;
    let mut content_type: Option<String> = None;

    while let Some(field) = multipart.next_field().await.map_err(|e| {
        crate::error::Error::BadRequest(format!("Failed to parse multipart: {}", e))
    })? {
        if field.name() == Some("file") {
            content_type = field.content_type().map(|s| s.to_string());
            file_data = Some(field.bytes().await.map_err(|e| {
                crate::error::Error::BadRequest(format!("Failed to read file: {}", e))
            })?.to_vec());
            break;
        }
    }

    let data = file_data.ok_or_else(|| {
        crate::error::Error::BadRequest("No file provided".to_string())
    })?;

    // Validate file size (max 10MB)
    const MAX_COVER_SIZE: usize = 10 * 1024 * 1024;
    if data.len() > MAX_COVER_SIZE {
        return Err(crate::error::Error::BadRequest(format!(
            "File too large. Maximum size is {} bytes",
            MAX_COVER_SIZE
        )));
    }

    // Determine entry ID (either specific entry or first entry of title)
    let entry_id = if let Some(eid) = query.eid {
        eid
    } else {
        // Get first entry of title
        let lib = state.library.load();
        let title = lib.get_title(&query.tid).ok_or_else(|| {
            crate::error::Error::NotFound(format!("Title not found: {}", query.tid))
        })?;
        title.entries.first().map(|e| e.id.clone()).ok_or_else(|| {
            crate::error::Error::NotFound("Title has no entries".to_string())
        })?
    };

    // Determine MIME type
    let mime = content_type.unwrap_or_else(|| {
        // Guess from data
        if data.starts_with(&[0xFF, 0xD8, 0xFF]) {
            "image/jpeg".to_string()
        } else if data.starts_with(&[0x89, 0x50, 0x4E, 0x47]) {
            "image/png".to_string()
        } else {
            "image/jpeg".to_string()
        }
    });

    // Save thumbnail to database
    let db = state.storage.pool();
    crate::library::Entry::save_thumbnail(&entry_id, &data, &mime, db).await?;

    tracing::info!("Uploaded custom cover for entry {}", entry_id);

    Ok(Json(serde_json::json!({
        "success": true
    })))
}
