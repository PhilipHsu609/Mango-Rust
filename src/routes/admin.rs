use askama::Template;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{Html, IntoResponse},
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
    let titles = new_lib.get_titles().len();

    // Atomically swap the new library in
    state.library.store(std::sync::Arc::new(new_lib));

    let elapsed = start.elapsed().as_millis();

    tracing::info!("Library scan completed: {} titles in {}ms", titles, elapsed);

    Ok(Json(ScanResponse {
        titles,
        milliseconds: elapsed,
    }))
}

#[derive(Serialize)]
pub struct MissingTitlesResponse {
    success: bool,
    error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    titles: Option<Vec<crate::storage::MissingItem>>,
}

#[derive(Serialize)]
pub struct MissingEntriesResponse {
    success: bool,
    error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    entries: Option<Vec<crate::storage::MissingItem>>,
}

#[derive(Serialize)]
pub struct MissingItemsMutationResponse {
    success: bool,
    error: Option<String>,
}

/// GET /api/admin/titles/missing - Get unavailable titles.
pub async fn get_missing_titles(
    State(state): State<AppState>,
    AdminOnly(_username): AdminOnly,
) -> Json<MissingTitlesResponse> {
    match state.storage.get_missing_titles().await {
        Ok(titles) => Json(MissingTitlesResponse {
            success: true,
            error: None,
            titles: Some(titles),
        }),
        Err(error) => Json(MissingTitlesResponse {
            success: false,
            error: Some(error.to_string()),
            titles: None,
        }),
    }
}

/// GET /api/admin/entries/missing - Get unavailable entries.
pub async fn get_missing_entries(
    State(state): State<AppState>,
    AdminOnly(_username): AdminOnly,
) -> Json<MissingEntriesResponse> {
    match state.storage.get_missing_entries().await {
        Ok(entries) => Json(MissingEntriesResponse {
            success: true,
            error: None,
            entries: Some(entries),
        }),
        Err(error) => Json(MissingEntriesResponse {
            success: false,
            error: Some(error.to_string()),
            entries: None,
        }),
    }
}

/// DELETE /api/admin/titles/missing/:id - Delete an unavailable title.
pub async fn delete_missing_title(
    State(state): State<AppState>,
    AdminOnly(_username): AdminOnly,
    Path(id): Path<String>,
) -> Json<MissingItemsMutationResponse> {
    match state.storage.delete_missing_title(&id).await {
        Ok(()) => Json(MissingItemsMutationResponse {
            success: true,
            error: None,
        }),
        Err(error) => Json(MissingItemsMutationResponse {
            success: false,
            error: Some(error.to_string()),
        }),
    }
}

/// DELETE /api/admin/entries/missing/:id - Delete an unavailable entry.
pub async fn delete_missing_entry(
    State(state): State<AppState>,
    AdminOnly(_username): AdminOnly,
    Path(id): Path<String>,
) -> Json<MissingItemsMutationResponse> {
    match state.storage.delete_missing_entry(&id).await {
        Ok(()) => Json(MissingItemsMutationResponse {
            success: true,
            error: None,
        }),
        Err(error) => Json(MissingItemsMutationResponse {
            success: false,
            error: Some(error.to_string()),
        }),
    }
}

/// DELETE /api/admin/titles/missing - Delete all unavailable titles.
pub async fn delete_all_missing_titles(
    State(state): State<AppState>,
    AdminOnly(_username): AdminOnly,
) -> Json<MissingItemsMutationResponse> {
    match state.storage.delete_all_missing_titles().await {
        Ok(_) => Json(MissingItemsMutationResponse {
            success: true,
            error: None,
        }),
        Err(error) => Json(MissingItemsMutationResponse {
            success: false,
            error: Some(error.to_string()),
        }),
    }
}

/// DELETE /api/admin/entries/missing - Delete all unavailable entries.
pub async fn delete_all_missing_entries(
    State(state): State<AppState>,
    AdminOnly(_username): AdminOnly,
) -> Json<MissingItemsMutationResponse> {
    match state.storage.delete_all_missing_entries().await {
        Ok(_) => Json(MissingItemsMutationResponse {
            success: true,
            error: None,
        }),
        Err(error) => Json(MissingItemsMutationResponse {
            success: false,
            error: Some(error.to_string()),
        }),
    }
}

/// Missing Items template
#[derive(Template)]
#[template(path = "missing-items.html")]
struct MissingItemsTemplate {
    nav: crate::util::NavigationState,
}

/// GET /admin/missing - Missing items management page.
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
    users: Vec<UserResponse>,
}

/// User edit template
#[derive(Template)]
#[template(path = "user-edit.html")]
struct UserEditTemplate {
    nav: crate::util::NavigationState,
    new_user: bool,
    edit_username: String,
    is_admin: bool,
    error: String,
}

/// GET /admin/user - User management page
/// Shows list of users and allows creating/deleting users
pub async fn users_page(
    State(state): State<AppState>,
    AdminOnly(username): AdminOnly,
) -> Result<Html<String>> {
    let users = state.storage.list_users().await?;
    let users = users
        .into_iter()
        .map(|(username, is_admin)| UserResponse { username, is_admin })
        .collect();

    let template = UsersTemplate {
        nav: crate::util::NavigationState::admin().with_admin(true),
        username,
        users,
    };

    Ok(Html(template.render().map_err(render_error)?))
}

/// User response for API endpoints
#[derive(Serialize)]
pub struct UserResponse {
    pub username: String,
    pub is_admin: bool,
}

/// GET /api/admin/user - Get all users
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

/// POST /api/admin/user - Create a new user
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
    pub password: Option<String>,
}

/// PATCH /api/admin/user/:username - Update user's admin status
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

    // Update user using existing update_user method
    state
        .storage
        .update_user(
            &username,
            &username,
            request.password.as_deref(),
            request.is_admin,
        )
        .await?;

    tracing::info!(
        "User '{}' updated (admin: {}, password changed: {})",
        username,
        request.is_admin,
        request.password.is_some()
    );

    Ok(StatusCode::NO_CONTENT)
}

/// DELETE /api/admin/user/:username - Delete a user
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
    let (title_path, entry_title) = {
        let lib = state.library.load();
        let title = lib
            .get_title(&title_id)
            .ok_or_else(|| crate::error::Error::NotFound(format!("Title not found: {title_id}")))?;
        let entry_title = if let Some(entry_id) = query.eid.as_deref() {
            Some(
                lib.get_entry(&title_id, entry_id)
                    .ok_or_else(|| {
                        crate::error::Error::NotFound(format!("Entry not found: {entry_id}"))
                    })?
                    .title
                    .clone(),
            )
        } else {
            None
        };
        (title.path.clone(), entry_title)
    };

    let mut info = crate::library::progress::TitleInfo::load(&title_path).await?;
    if let Some(entry_title) = entry_title {
        info.entry_display_name.insert(entry_title, name);
    } else {
        info.display_name = name;
    }
    info.save(&title_path).await?;
    state
        .library
        .load()
        .progress_cache()
        .load_title(&title_id, &title_path)
        .await?;

    tracing::info!("Updated display name for title {}", title_id);
    Ok(Json(serde_json::json!({ "success": true })))
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

    Ok(Json(serde_json::json!({ "success": true })))
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

    let Some(title) = lib.get_title(&title_id) else {
        return Ok(Json(serde_json::json!({
            "success": false,
            "error": format!("Title not found: {}", title_id)
        })));
    };

    if action != "read" && action != "unread" {
        return Ok(Json(serde_json::json!({
            "success": false,
            "error": format!("Unknown action {}", action)
        })));
    }

    let updates: Vec<(String, i32)> = request
        .ids
        .iter()
        .filter_map(|entry_id| {
            lib.get_entry(&title_id, entry_id).map(|entry| {
                let page = if action == "read" {
                    entry.pages as i32
                } else {
                    0
                };
                (entry.title.clone(), page)
            })
        })
        .collect();
    if let Err(error) = lib
        .progress_cache()
        .save_bulk_progress(&title_id, &title.path, &username, &updates)
        .await
    {
        return Ok(Json(serde_json::json!({
            "success": false,
            "error": error.to_string()
        })));
    }

    lib.invalidate_cache_for_progress(&username).await;

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
    let total = THUMBNAIL_TOTAL.load(Ordering::SeqCst);
    let current = THUMBNAIL_CURRENT.load(Ordering::SeqCst);
    let progress = if total == 0 {
        0.0
    } else {
        (current as f64 / total as f64).min(1.0)
    };

    Ok(Json(serde_json::json!({
        "progress": progress
    })))
}

/// POST /api/admin/generate_thumbnails - Start thumbnail generation
pub async fn generate_thumbnails(
    State(state): State<AppState>,
    AdminOnly(_username): AdminOnly,
) -> axum::response::Response {
    if THUMBNAIL_GENERATING
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return StatusCode::OK.into_response();
    }
    THUMBNAIL_CURRENT.store(0, Ordering::SeqCst);

    let lib = state.library.load();
    let mut entries_to_process: Vec<(String, String)> = Vec::new();

    for title in lib.all_titles() {
        for entry in &title.entries {
            entries_to_process.push((title.id.clone(), entry.id.clone()));
        }
    }

    THUMBNAIL_TOTAL.store(entries_to_process.len(), Ordering::SeqCst);
    drop(lib);

    let state_clone = state.clone();
    tokio::spawn(async move {
        let lib = state_clone.library.load();
        let db = state_clone.storage.pool();

        for (i, (title_id, entry_id)) in entries_to_process.iter().enumerate() {
            THUMBNAIL_CURRENT.store(i + 1, Ordering::SeqCst);

            if let Some(entry) = lib.get_entry(title_id, entry_id) {
                match crate::library::Entry::get_thumbnail(entry_id, db).await {
                    Ok(Some(_)) => continue,
                    _ => {}
                }

                if let Err(e) = entry.generate_thumbnail(db).await {
                    tracing::warn!("Failed to generate thumbnail for {}: {}", entry_id, e);
                }
            }
        }

        THUMBNAIL_GENERATING.store(false, Ordering::SeqCst);
        tracing::info!("Thumbnail generation completed");
    });

    StatusCode::OK.into_response()
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
) -> axum::response::Response {
    let result: Result<()> = async {
        use tokio::io::AsyncWriteExt;

        let mut field = loop {
            match multipart.next_field().await.map_err(|error| {
                crate::error::Error::BadRequest(format!("Failed to parse multipart: {error}"))
            })? {
                Some(field) if field.name() == Some("file") => break field,
                Some(_) => {}
                None => {
                    return Err(crate::error::Error::BadRequest(
                        "No part with name `file` found".to_string(),
                    ))
                }
            }
        };
        let file_name = field
            .file_name()
            .ok_or_else(|| crate::error::Error::BadRequest("No file uploaded".to_string()))?;
        let extension = std::path::Path::new(file_name)
            .extension()
            .and_then(|extension| extension.to_str())
            .map(str::to_ascii_lowercase)
            .ok_or_else(|| {
                crate::error::Error::BadRequest(
                    "The uploaded image must be either JPEG or PNG".to_string(),
                )
            })?;
        let _mime = match extension.as_str() {
            "jpg" | "jpeg" | "jpe" | "jfif" => "image/jpeg",
            "png" => "image/png",
            "webp" => "image/webp",
            "apng" => "image/apng",
            "avif" => "image/avif",
            "gif" => "image/gif",
            "svg" => "image/svg+xml",
            "jxl" => "image/jxl",
            _ => {
                return Err(crate::error::Error::BadRequest(
                    "The uploaded image must be either JPEG or PNG".to_string(),
                ))
            }
        };

        let (title_path, entry_title) = {
            let lib = state.library.load();
            let title = lib.get_title(&query.tid).ok_or_else(|| {
                crate::error::Error::NotFound(format!("Title not found: {}", query.tid))
            })?;
            let entry_title = query
                .eid
                .as_deref()
                .map(|entry_id| {
                    lib.get_entry(&query.tid, entry_id)
                        .map(|entry| entry.title.clone())
                        .ok_or_else(|| {
                            crate::error::Error::NotFound(format!("Entry not found: {entry_id}"))
                        })
                })
                .transpose()?;
            (title.path.clone(), entry_title)
        };

        let upload_dir = state.config.upload_path.join("img");
        tokio::fs::create_dir_all(&upload_dir).await?;
        let stored_name = format!("{}.{}", uuid::Uuid::new_v4().simple(), extension);
        let destination = upload_dir.join(&stored_name);
        let mut output = tokio::fs::File::create(&destination).await?;
        let write_result: Result<()> = async {
            while let Some(chunk) = field.chunk().await.map_err(|error| {
                crate::error::Error::BadRequest(format!("Failed to read file: {error}"))
            })? {
                output.write_all(&chunk).await?;
            }
            Ok(())
        }
        .await;
        drop(output);
        if let Err(error) = write_result {
            let _ = tokio::fs::remove_file(&destination).await;
            return Err(error);
        }

        let url = format!("/uploads/img/{stored_name}");
        let mut info = crate::library::progress::TitleInfo::load(&title_path).await?;
        if let Some(entry_title) = entry_title {
            info.entry_cover_url.insert(entry_title, url);
        } else {
            info.cover_url = url;
        }
        info.save(&title_path).await?;
        state
            .library
            .load()
            .progress_cache()
            .load_title(&query.tid, &title_path)
            .await?;
        Ok(())
    }
    .await;

    match result {
        Ok(()) => Json(serde_json::json!({ "success": true })).into_response(),
        Err(error) => Json(serde_json::json!({
            "success": false,
            "error": error.to_string()
        }))
        .into_response(),
    }
}

/// Query params for user edit page
#[derive(Deserialize)]
pub struct UserEditQuery {
    pub username: Option<String>,
    pub admin: Option<bool>,
}

/// GET /admin/user/edit - User edit page
pub async fn user_edit_page(
    AdminOnly(_username): AdminOnly,
    axum::extract::Query(query): axum::extract::Query<UserEditQuery>,
) -> Result<Html<String>> {
    let template = UserEditTemplate {
        nav: crate::util::NavigationState::admin().with_admin(true),
        new_user: query.username.is_none(),
        edit_username: query.username.unwrap_or_default(),
        is_admin: query.admin.unwrap_or(false),
        error: String::new(),
    };

    Ok(Html(template.render().map_err(render_error)?))
}

/// Form data for user edit
#[derive(Deserialize)]
pub struct UserEditForm {
    pub username: String,
    pub password: Option<String>,
    #[serde(default)]
    pub admin: Option<String>,
}

/// POST /admin/user/edit - Create new user
pub async fn user_edit_post(
    State(state): State<AppState>,
    AdminOnly(_username): AdminOnly,
    axum::extract::Form(form): axum::extract::Form<UserEditForm>,
) -> Result<axum::response::Redirect> {
    let is_admin = form.admin.is_some();
    let password = form.password.unwrap_or_default();

    if password.is_empty() {
        return Err(crate::error::Error::BadRequest(
            "Password is required for new users".to_string(),
        ));
    }

    state
        .storage
        .create_user(&form.username, &password, is_admin)
        .await?;

    tracing::info!("Created user '{}' (admin: {})", form.username, is_admin);

    Ok(axum::response::Redirect::to("/admin/user"))
}

/// POST /admin/user/edit/:username - Update existing user
pub async fn user_edit_post_existing(
    State(state): State<AppState>,
    AdminOnly(current_username): AdminOnly,
    Path(username): Path<String>,
    axum::extract::Form(form): axum::extract::Form<UserEditForm>,
) -> Result<axum::response::Redirect> {
    let is_admin = form.admin.is_some();

    // Prevent users from demoting themselves
    if username == current_username && !is_admin {
        return Err(crate::error::Error::Forbidden(
            "Cannot demote yourself from admin".to_string(),
        ));
    }

    let password = form.password.filter(|p| !p.is_empty());

    state
        .storage
        .update_user(&username, &username, password.as_deref(), is_admin)
        .await?;

    tracing::info!(
        "Updated user '{}' (admin: {}, password changed: {})",
        username,
        is_admin,
        password.is_some()
    );

    Ok(axum::response::Redirect::to("/admin/user"))
}

/// DELETE /api/admin/user/delete/:username - Delete user
pub async fn delete_user_api(
    State(state): State<AppState>,
    AdminOnly(current_username): AdminOnly,
    Path(username): Path<String>,
) -> Result<Json<serde_json::Value>> {
    // Prevent self-deletion
    if username == current_username {
        return Ok(Json(serde_json::json!({
            "success": false,
            "error": "Cannot delete yourself"
        })));
    }

    // Check if user exists
    if !state.storage.username_exists(&username).await? {
        return Ok(Json(serde_json::json!({
            "success": false,
            "error": format!("User '{}' not found", username)
        })));
    }

    state.storage.delete_user(&username).await?;

    tracing::info!("Deleted user '{}'", username);

    Ok(Json(serde_json::json!({
        "success": true,
        "error": null
    })))
}

#[cfg(test)]
mod missing_items_api_tests {
    use super::{MissingEntriesResponse, MissingItemsMutationResponse, MissingTitlesResponse};
    use crate::storage::MissingItem;

    #[test]
    fn missing_item_api_responses_match_mango_contract() {
        let item = MissingItem {
            id: "item-id".to_string(),
            path: "Series/Volume.cbz".to_string(),
            signature: Some("signature".to_string()),
        };

        assert_eq!(
            serde_json::to_value(MissingTitlesResponse {
                success: true,
                error: None,
                titles: Some(vec![item.clone()]),
            })
            .unwrap(),
            serde_json::json!({
                "success": true,
                "error": null,
                "titles": [{
                    "id": "item-id",
                    "path": "Series/Volume.cbz",
                    "signature": "signature"
                }]
            })
        );
        assert_eq!(
            serde_json::to_value(MissingEntriesResponse {
                success: true,
                error: None,
                entries: Some(vec![item]),
            })
            .unwrap(),
            serde_json::json!({
                "success": true,
                "error": null,
                "entries": [{
                    "id": "item-id",
                    "path": "Series/Volume.cbz",
                    "signature": "signature"
                }]
            })
        );
        assert_eq!(
            serde_json::to_value(MissingItemsMutationResponse {
                success: true,
                error: None,
            })
            .unwrap(),
            serde_json::json!({"success": true, "error": null})
        );
        assert_eq!(
            serde_json::to_value(MissingTitlesResponse {
                success: false,
                error: Some("failed".to_string()),
                titles: None,
            })
            .unwrap(),
            serde_json::json!({"success": false, "error": "failed"})
        );
    }
}
