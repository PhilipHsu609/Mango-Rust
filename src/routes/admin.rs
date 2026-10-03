use askama::Template;
use axum::{
    extract::{rejection::JsonRejection, Path, State},
    http::StatusCode,
    response::{Html, IntoResponse},
    Json,
};
pub mod users;
use serde::{Deserialize, Serialize};
use std::time::Instant;
pub use users::{
    create_user, delete_user, delete_user_api, get_users, update_user, user_edit_page,
    user_edit_post, user_edit_post_existing, users_page, CreateUserRequest, UpdateUserRequest,
    UserEditForm, UserEditQuery, UserResponse,
};

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
    entries.sort_by_key(|entry| std::cmp::Reverse(entry.access_count));
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
    pub milliseconds: f64,
}

/// POST /api/admin/scan - Trigger library rescan
/// Returns number of titles found and time taken in milliseconds
/// Builds a replacement library during the request, then atomically swaps it in
#[utoipa::path(post, path = "/api/admin/scan", tag = "admin", summary = "Scan library", responses((status = 200, description = "Library scan completed")))]
pub async fn scan_library(
    State(state): State<AppState>,
    AdminOnly(_username): AdminOnly,
) -> Result<Json<ScanResponse>> {
    let start = Instant::now();
    let _scan_guard = crate::library::SCAN_LOCK.lock().await;

    // Publish completed roots while building the replacement library.
    let mut new_lib = crate::library::Library::new(
        state.config.library_path.clone(),
        state.storage.clone(),
        &state.config,
    );
    let previous = state.library.load_full();
    new_lib
        .scan_with_previous_and_publish(Some(previous), std::sync::Arc::clone(&state.library))
        .await?;
    let titles = new_lib.get_titles().len();

    // Atomically swap the new library in
    state.library.store(std::sync::Arc::new(new_lib));

    let elapsed = start.elapsed().as_secs_f64() * 1000.0;

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
#[utoipa::path(get, path = "/api/admin/titles/missing", tag = "admin", summary = "Get missing titles", responses((status = 200, description = "Missing titles returned")))]
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
#[utoipa::path(get, path = "/api/admin/entries/missing", tag = "admin", summary = "Get missing entries", responses((status = 200, description = "Missing entries returned")))]
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
#[utoipa::path(delete, path = "/api/admin/titles/missing/{id}", tag = "admin", summary = "Delete missing title", params(("id" = String, Path, description = "Missing title identifier")), responses((status = 200, description = "Missing title deleted")))]
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
#[utoipa::path(delete, path = "/api/admin/entries/missing/{id}", tag = "admin", summary = "Delete missing entry", params(("id" = String, Path, description = "Missing entry identifier")), responses((status = 200, description = "Missing entry deleted")))]
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
#[utoipa::path(delete, path = "/api/admin/titles/missing", tag = "admin", summary = "Delete all missing titles", responses((status = 200, description = "Missing titles deleted")))]
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
#[utoipa::path(delete, path = "/api/admin/entries/missing", tag = "admin", summary = "Delete all missing entries", responses((status = 200, description = "Missing entries deleted")))]
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

/// POST /api/cache/clear - Clear all LRU cache entries
/// Removes all cached sorted lists from memory (library cache file remains)
#[utoipa::path(post, path = "/api/cache/clear", tag = "admin", summary = "Clear cache", responses((status = 200, description = "Cache cleared")))]
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
#[utoipa::path(post, path = "/api/cache/save-library", tag = "admin", summary = "Save library cache", responses((status = 200, description = "Library cache saved")))]
pub async fn cache_save_library_api(
    State(state): State<AppState>,
    AdminOnly(_username): AdminOnly,
) -> Result<Json<serde_json::Value>> {
    let lib = state.library.load();

    let cache = lib.cache().lock().await;
    cache.save_library(&lib).await?;

    tracing::info!("Library cache saved by admin");

    Ok(Json(serde_json::json!({
        "success": true,
        "message": "Library cache saved successfully"
    })))
}

/// POST /api/cache/load-library - Load library from cache file
/// Reloads library from persistent cache file
/// Uses double-buffer approach: creates new library, loads from cache, swaps
#[utoipa::path(post, path = "/api/cache/load-library", tag = "admin", summary = "Load library cache", responses((status = 200, description = "Library cache loaded")))]
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
#[derive(Deserialize, utoipa::ToSchema)]
pub struct CacheInvalidateRequest {
    /// Pattern to match cache keys (e.g., "sorted_titles:user1:")
    pub pattern: String,
}

/// POST /api/cache/invalidate - Invalidate cache entries by pattern
/// Invalidates all cache entries matching the given pattern prefix
#[utoipa::path(post, path = "/api/cache/invalidate", tag = "admin", summary = "Invalidate cache", request_body = CacheInvalidateRequest, responses((status = 200, description = "Cache invalidated")))]
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
#[utoipa::path(put, path = "/api/admin/display_name/{tid}/{name}", tag = "admin", summary = "Update display name", params(("tid" = String, Path, description = "Title identifier"), ("name" = String, Path, description = "Display name"), ("eid" = Option<String>, Query, description = "Entry ID; omit to change the title")), responses((status = 200, description = "Display name updated")))]
pub async fn update_display_name(
    State(state): State<AppState>,
    AdminOnly(_username): AdminOnly,
    Path((title_id, name)): Path<(String, String)>,
    axum::extract::Query(query): axum::extract::Query<DisplayNameQuery>,
) -> Json<serde_json::Value> {
    let result: Result<()> = async {
        let (title_path, entry_title) = {
            let lib = state.library.load();
            let title = lib.get_title(&title_id).ok_or_else(|| {
                crate::error::Error::BadRequest("Nil assertion failed".to_string())
            })?;
            let entry_title = if let Some(entry_id) = query.eid.as_deref() {
                Some(
                    lib.get_entry(&title_id, entry_id)
                        .ok_or_else(|| {
                            crate::error::Error::BadRequest("Nil assertion failed".to_string())
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
        Ok(())
    }
    .await;

    match result {
        Ok(()) => {
            tracing::info!("Updated display name for title {}", title_id);
            Json(serde_json::json!({ "success": true }))
        }
        Err(error) => Json(serde_json::json!({
            "success": false,
            "error": error.to_string()
        })),
    }
}

#[derive(Deserialize)]
pub struct SortTitleQuery {
    eid: Option<String>,
    name: Option<String>,
}

/// PUT /api/admin/sort_title/:tid - Update sort title for title or entry
#[utoipa::path(put, path = "/api/admin/sort_title/{tid}", tag = "admin", summary = "Update sort title", params(("tid" = String, Path, description = "Title identifier"), ("eid" = Option<String>, Query, description = "Entry ID; omit to change the title"), ("name" = Option<String>, Query, description = "Sort title")), responses((status = 200, description = "Sort title updated")))]
pub async fn update_sort_title(
    State(state): State<AppState>,
    AdminOnly(_username): AdminOnly,
    Path(title_id): Path<String>,
    axum::extract::Query(query): axum::extract::Query<SortTitleQuery>,
) -> Json<serde_json::Value> {
    let result: Result<()> = async {
        let entry_belongs_to_title = {
            let library = state.library.load();
            let Some(title) = library.get_title(&title_id) else {
                return Err(crate::error::Error::BadRequest(
                    "Nil assertion failed".to_string(),
                ));
            };
            query
                .eid
                .as_deref()
                .is_some_and(|entry_id| title.entries.iter().any(|entry| entry.id == entry_id))
        };
        let sort_title = query.name.as_deref();

        if let Some(entry_id) = &query.eid {
            if entry_belongs_to_title {
                state
                    .storage
                    .update_entry_sort_title(entry_id, sort_title)
                    .await?;
                tracing::info!("Updated entry {} sort title to {:?}", entry_id, sort_title);
            }
        } else {
            state
                .storage
                .update_title_sort_title(&title_id, sort_title)
                .await?;
            tracing::info!("Updated title {} sort title to {:?}", title_id, sort_title);
        }
        Ok(())
    }
    .await;

    match result {
        Ok(()) => Json(serde_json::json!({ "success": true })),
        Err(error) => Json(serde_json::json!({
            "success": false,
            "error": error.to_string()
        })),
    }
}

// ========== Bulk Progress API ==========

#[derive(Deserialize, utoipa::ToSchema)]
pub struct BulkProgressRequest {
    ids: Vec<String>,
}

/// PUT /api/bulk_progress/:action/:tid - Bulk update progress for multiple entries
/// action: "read" (100%) or "unread" (0%)
#[utoipa::path(put, path = "/api/bulk_progress/{action}/{tid}", tag = "progress", summary = "Update bulk progress", params(("action" = String, Path, description = "Progress action"), ("tid" = String, Path, description = "Title identifier")), request_body = BulkProgressRequest, responses((status = 200, description = "Progress updated")))]
pub async fn bulk_progress(
    State(state): State<AppState>,
    crate::auth::Username(username): crate::auth::Username,
    Path((action, title_id)): Path<(String, String)>,
    request: std::result::Result<Json<BulkProgressRequest>, JsonRejection>,
) -> Result<Json<serde_json::Value>> {
    let Json(request) = match request {
        Ok(request) => request,
        Err(error) => {
            return Ok(Json(serde_json::json!({
                "success": false,
                "error": error.body_text()
            })));
        }
    };
    let lib = state.library.load();

    let Some(title) = lib.get_title(&title_id) else {
        return Ok(Json(serde_json::json!({
            "success": false,
            "error": "Nil assertion failed"
        })));
    };

    if action != "read" && action != "unread" {
        return Ok(Json(serde_json::json!({
            "success": false,
            "error": format!("Unknow action {}", action)
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
#[utoipa::path(get, path = "/api/admin/thumbnail_progress", tag = "admin", summary = "Get thumbnail progress", responses((status = 200, description = "Thumbnail progress returned")))]
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
#[utoipa::path(post, path = "/api/admin/generate_thumbnails", tag = "admin", summary = "Generate thumbnails", responses((status = 200, description = "Thumbnail generation started")))]
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
                if let Ok(Some(_)) = crate::library::Entry::get_thumbnail(entry_id, db).await {
                    continue;
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
#[utoipa::path(post, path = "/api/admin/upload/cover", tag = "admin", summary = "Upload cover", params(("tid" = String, Query, description = "Title identifier"), ("eid" = Option<String>, Query, description = "Entry identifier; omit to set the title cover")), responses((status = 200, description = "Cover uploaded")))]
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
