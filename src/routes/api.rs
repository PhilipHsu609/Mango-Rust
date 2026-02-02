use axum::{
    extract::{Path, Query, State},
    http::{header, StatusCode},
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::{
    error::{Error, Result},
    library::{Entry, SortMethod},
    routes::calculate_progress_percentage,
    util::SortParams,
    AppState,
};

/// API route: GET /api/library?sort=title|modified|auto&ascend=0|1
/// Returns list of all manga titles with optional sorting
pub async fn get_library(
    State(state): State<AppState>,
    Query(params): Query<SortParams>,
) -> Result<impl IntoResponse> {
    let lib = state.library.load();
    let (sort_method, ascending) =
        SortMethod::from_params(params.sort.as_deref(), params.ascend.as_deref());
    let titles = lib.get_titles_sorted(sort_method, ascending);

    let response: Vec<TitleInfo> = titles
        .iter()
        .map(|t| TitleInfo {
            id: t.id.clone(),
            title: t.title.clone(),
            entries: t.entries.len(),
            pages: t.total_pages(),
        })
        .collect();

    Ok(Json(response))
}

/// API route: GET /api/title/:id?sort=title|modified|auto&ascend=0|1
/// Returns details of a specific manga title including all its entries with optional sorting
pub async fn get_title(
    State(state): State<AppState>,
    Path(title_id): Path<String>,
    Query(params): Query<SortParams>,
) -> Result<impl IntoResponse> {
    let lib = state.library.load();

    let title = lib
        .get_title(&title_id)
        .ok_or_else(|| crate::error::Error::NotFound(format!("Title not found: {}", title_id)))?;

    let (sort_method, ascending) =
        SortMethod::from_params(params.sort.as_deref(), params.ascend.as_deref());
    let entries: Vec<EntryInfo> = title
        .get_entries_sorted(sort_method, ascending)
        .iter()
        .map(|e| EntryInfo {
            id: e.id.clone(),
            title: e.title.clone(),
            pages: e.pages,
        })
        .collect();

    let response = TitleDetail {
        id: title.id.clone(),
        title: title.title.clone(),
        entries,
    };

    Ok(Json(response))
}

/// API route: GET /api/page/:tid/:eid/:page
/// Serves a specific page image from an entry
pub async fn get_page(
    State(state): State<AppState>,
    Path((title_id, entry_id, page)): Path<(String, String, usize)>,
) -> Result<impl IntoResponse> {
    let lib = state.library.load();

    let entry = lib.get_entry(&title_id, &entry_id).ok_or_else(|| {
        crate::error::Error::NotFound(format!("Entry not found: {}/{}", title_id, entry_id))
    })?;

    // Pages are 1-indexed in the API, but 0-indexed internally
    let page_idx = page.saturating_sub(1);
    let image_data = entry.get_page(page_idx).await?;

    // Determine MIME type from image data
    let mime_type = guess_mime_type(&image_data);

    Ok((
        StatusCode::OK,
        [(header::CONTENT_TYPE, mime_type)],
        image_data,
    ))
}

/// API route: GET /api/stats
/// Returns library statistics
pub async fn get_stats(State(state): State<AppState>) -> Result<impl IntoResponse> {
    let lib = state.library.load();
    let stats = lib.stats();

    let response = LibraryStats {
        titles: stats.titles,
        entries: stats.entries,
        pages: stats.pages,
    };

    Ok(Json(response))
}

/// GET /api/cover/:tid/:eid - Get manga entry cover/thumbnail
pub async fn get_cover(
    State(state): State<AppState>,
    Path((title_id, entry_id)): Path<(String, String)>,
) -> Result<impl IntoResponse> {
    let lib = state.library.load();

    // Get entry
    let entry = lib
        .get_entry(&title_id, &entry_id)
        .ok_or_else(|| Error::NotFound(format!("Entry not found: {}/{}", title_id, entry_id)))?;

    let db = state.storage.pool();

    // Try to get thumbnail first
    match Entry::get_thumbnail(&entry_id, db).await {
        Ok(Some((data, mime))) => {
            return Ok(([(header::CONTENT_TYPE, mime.as_str())], data).into_response());
        }
        Ok(None) => {
            // No thumbnail exists, try to generate one
            match entry.generate_thumbnail(db).await {
                Ok(Some((data, mime, _size))) => {
                    return Ok(([(header::CONTENT_TYPE, mime.as_str())], data).into_response());
                }
                Ok(None) => {
                    tracing::warn!(
                        "Thumbnail generation returned None for entry {}: no image data produced",
                        entry_id
                    );
                }
                Err(e) => {
                    tracing::warn!(
                        "Thumbnail generation failed for entry {}: {}. Falling back to first page.",
                        entry_id,
                        e
                    );
                }
            }
            // Fall through to return first page
        }
        Err(e) => {
            tracing::warn!("Error getting thumbnail for {}: {}", entry_id, e);
            // Fall through to return first page
        }
    }

    // Fallback: return first page directly
    let data = entry.get_page(0).await?;
    let mime = guess_mime_type(&data);
    Ok(([(header::CONTENT_TYPE, mime)], data).into_response())
}

// Response types

#[derive(Serialize)]
struct TitleInfo {
    id: String,
    title: String,
    entries: usize,
    pages: usize,
}

#[derive(Serialize)]
struct TitleDetail {
    id: String,
    title: String,
    entries: Vec<EntryInfo>,
}

#[derive(Serialize)]
struct EntryInfo {
    id: String,
    title: String,
    pages: usize,
}

#[derive(Serialize)]
struct LibraryStats {
    titles: usize,
    entries: usize,
    pages: usize,
}

/// API route: GET /api/library/continue_reading
/// Returns the last 8 entries the user has read, sorted by last_read timestamp
pub async fn continue_reading(
    State(state): State<AppState>,
    crate::auth::Username(username): crate::auth::Username,
) -> Result<impl IntoResponse> {
    let lib = state.library.load();
    let cache = lib.progress_cache();
    let mut entries_with_progress = Vec::new();

    // Collect all entries with last_read timestamps (O(1) cache lookups instead of O(N) file reads)
    for title in lib.get_titles_sorted(crate::library::SortMethod::Name, true) {
        for entry in &title.entries {
            if let Some(last_read) = cache.get_last_read(&title.id, &username, &entry.id) {
                let progress = cache.get_progress(&title.id, &username, &entry.id).unwrap_or(0);
                let percentage = calculate_progress_percentage(progress, entry.pages);

                entries_with_progress.push(ContinueReadingEntry {
                    title_id: title.id.clone(),
                    title_name: title.title.clone(),
                    entry_id: entry.id.clone(),
                    entry_name: entry.title.clone(),
                    pages: entry.pages,
                    progress,
                    percentage,
                    last_read,
                });
            }
        }
    }

    // Sort by last_read (most recent first) and take top 8
    entries_with_progress.sort_by(|a, b| b.last_read.cmp(&a.last_read));
    entries_with_progress.truncate(8);

    Ok(Json(entries_with_progress))
}

/// API route: GET /api/library/start_reading
/// Returns unread titles (0% progress) for the user
pub async fn start_reading(
    State(state): State<AppState>,
    crate::auth::Username(username): crate::auth::Username,
) -> Result<impl IntoResponse> {
    let lib = state.library.load();
    let cache = lib.progress_cache();
    let mut unread_titles = Vec::new();

    for title in lib.get_titles_sorted(crate::library::SortMethod::Name, true) {
        // Calculate title progress using cache (avoids filesystem reads)
        let progress_pct = if title.entries.is_empty() {
            0.0
        } else {
            let mut total_progress = 0.0;
            for entry in &title.entries {
                let page = cache
                    .get_progress(&title.id, &username, &entry.id)
                    .unwrap_or(0);
                let pct = if entry.pages > 0 {
                    (page as f32 / entry.pages as f32) * 100.0
                } else {
                    0.0
                };
                total_progress += pct;
            }
            total_progress / title.entries.len() as f32
        };

        if progress_pct == 0.0 {
            unread_titles.push(StartReadingTitle {
                id: title.id.clone(),
                title: title.title.clone(),
                entry_count: title.entries.len(),
                first_entry_id: title.entries.first().map(|e| e.id.clone()),
            });
        }
    }

    // Shuffle and take top 8
    use rand::seq::SliceRandom;
    let mut rng = rand::thread_rng();
    unread_titles.shuffle(&mut rng);
    unread_titles.truncate(8);

    Ok(Json(unread_titles))
}

/// Intermediate struct for recently_added sorting (replaces hard-to-read tuple)
struct RecentEntryData {
    title_id: String,
    title_name: String,
    entry_id: String,
    entry_name: String,
    pages: usize,
    percentage: f32,
    date_added: i64,
}

/// API route: GET /api/library/recently_added
/// Returns recently added entries (within last month) with grouping by title
pub async fn recently_added(
    State(state): State<AppState>,
    crate::auth::Username(username): crate::auth::Username,
) -> Result<impl IntoResponse> {
    let lib = state.library.load();
    let cache = lib.progress_cache();
    let mut entries_with_dates = Vec::new();
    let one_month_ago = chrono::Utc::now().timestamp() - (30 * 24 * 60 * 60);

    // Collect all entries with date_added within last month (O(1) cache lookups)
    for title in lib.get_titles_sorted(crate::library::SortMethod::Name, true) {
        for entry in &title.entries {
            if let Some(date_added) = cache.get_date_added(&title.id, &entry.id) {
                if date_added > one_month_ago {
                    let progress = cache.get_progress(&title.id, &username, &entry.id).unwrap_or(0);
                    let percentage = calculate_progress_percentage(progress, entry.pages);

                    entries_with_dates.push(RecentEntryData {
                        title_id: title.id.clone(),
                        title_name: title.title.clone(),
                        entry_id: entry.id.clone(),
                        entry_name: entry.title.clone(),
                        pages: entry.pages,
                        percentage,
                        date_added,
                    });
                }
            }
        }
    }

    // Sort by date_added (most recent first)
    entries_with_dates.sort_by(|a, b| b.date_added.cmp(&a.date_added));

    // Group consecutive entries from same title added on same day
    let mut result: Vec<RecentlyAddedEntry> = Vec::new();
    for entry in entries_with_dates {
        if result.len() >= 8 {
            break;
        }

        // Check if we can group with last entry
        let should_group = if let Some(last) = result.last() {
            last.title_id == entry.title_id && (entry.date_added - last.date_added).abs() < (24 * 60 * 60)
        } else {
            false
        };

        if should_group {
            // Group with previous entry
            if let Some(last) = result.last_mut() {
                last.grouped_count += 1;
                last.percentage = 0.0; // Hide percentage for grouped items
            }
        } else {
            result.push(RecentlyAddedEntry {
                title_id: entry.title_id,
                title_name: entry.title_name,
                entry_id: entry.entry_id,
                entry_name: entry.entry_name,
                pages: entry.pages,
                percentage: entry.percentage,
                grouped_count: 1,
                date_added: entry.date_added,
            });
        }
    }

    Ok(Json(result))
}

// Response types for home page sections

#[derive(Serialize)]
struct ContinueReadingEntry {
    title_id: String,
    title_name: String,
    entry_id: String,
    entry_name: String,
    pages: usize,
    progress: i32,
    percentage: f32, // Progress percentage (0.0 - 100.0)
    last_read: i64,
}

#[derive(Serialize)]
struct StartReadingTitle {
    id: String,
    title: String,
    entry_count: usize,
    first_entry_id: Option<String>,
}

#[derive(Serialize)]
struct RecentlyAddedEntry {
    title_id: String,
    title_name: String,
    entry_id: String,
    entry_name: String,
    pages: usize,
    percentage: f32, // Progress percentage (0.0 - 100.0)
    grouped_count: usize,
    date_added: i64,
}

// ========== Tags API Endpoints ==========

/// Standard API response wrapper for frontend compatibility
#[derive(Serialize)]
struct ApiResponse<T: Serialize> {
    success: bool,
    #[serde(flatten)]
    data: T,
}

/// Success response helper
fn success_response<T: Serialize>(data: T) -> Json<ApiResponse<T>> {
    Json(ApiResponse {
        success: true,
        data,
    })
}

#[derive(Serialize)]
struct TagsListResponse {
    tags: Vec<String>,
}

/// API route: GET /api/tags
/// Returns all tags with their usage counts, sorted by count desc then name asc
pub async fn list_tags(
    State(state): State<AppState>,
    _username: crate::auth::Username,
) -> Result<impl IntoResponse> {
    let storage = &state.storage;
    let tags = storage.list_tags().await?;

    // Count titles for each tag
    let mut tag_counts: HashMap<String, usize> = HashMap::new();
    for tag in tags {
        let title_ids = storage.get_tag_titles(&tag).await?;
        tag_counts.insert(tag, title_ids.len());
    }

    // Sort by count desc, then by tag name asc
    let mut tags_with_counts: Vec<(String, usize)> = tag_counts.into_iter().collect();
    tags_with_counts.sort_by(|a, b| {
        b.1.cmp(&a.1)
            .then_with(|| a.0.to_lowercase().cmp(&b.0.to_lowercase()))
    });

    // Return just the tag names in sorted order (frontend expects this format)
    let sorted_tags: Vec<String> = tags_with_counts.into_iter().map(|(tag, _)| tag).collect();

    Ok(success_response(TagsListResponse { tags: sorted_tags }))
}

/// API route: GET /api/tags/:tid
/// Returns all tags for a specific title
pub async fn get_title_tags(
    State(state): State<AppState>,
    Path(title_id): Path<String>,
    _username: crate::auth::Username,
) -> Result<impl IntoResponse> {
    let storage = &state.storage;
    let tags = storage.get_title_tags(&title_id).await?;
    Ok(success_response(TagsListResponse { tags }))
}

#[derive(Serialize)]
struct SuccessOnly {
    // Empty struct - just for the success wrapper
}

/// API route: PUT /api/admin/tags/:tid/:tag
/// Add a tag to a title (admin only)
pub async fn add_tag(
    State(state): State<AppState>,
    Path((title_id, tag)): Path<(String, String)>,
    _admin: crate::auth::AdminOnly,
) -> Result<impl IntoResponse> {
    let storage = &state.storage;
    storage.add_tag(&title_id, &tag).await?;
    Ok(success_response(SuccessOnly {}))
}

/// API route: DELETE /api/admin/tags/:tid/:tag
/// Remove a tag from a title (admin only)
pub async fn delete_tag(
    State(state): State<AppState>,
    Path((title_id, tag)): Path<(String, String)>,
    _admin: crate::auth::AdminOnly,
) -> Result<impl IntoResponse> {
    let storage = &state.storage;
    storage.delete_tag(&title_id, &tag).await?;
    Ok(success_response(SuccessOnly {}))
}

/// API route: GET /api/download/:tid/:eid
/// Download the original archive file for an entry (used by OPDS clients)
pub async fn download_entry(
    State(state): State<AppState>,
    Path((title_id, entry_id)): Path<(String, String)>,
    _username: crate::auth::Username,
) -> Result<impl IntoResponse> {
    let lib = state.library.load();

    // Get entry
    let entry = lib
        .get_entry(&title_id, &entry_id)
        .ok_or_else(|| Error::NotFound(format!("Entry not found: {}/{}", title_id, entry_id)))?;

    // Read the archive file
    let file_data = tokio::fs::read(&entry.path).await.map_err(|e| {
        Error::Internal(format!(
            "Failed to read file {}: {}",
            entry.path.display(),
            e
        ))
    })?;

    // Determine MIME type from file extension
    let mime_type = match entry.path.extension().and_then(|e| e.to_str()) {
        Some("cbz") | Some("zip") => "application/zip",
        Some("cbr") | Some("rar") => "application/x-rar-compressed",
        _ => "application/octet-stream",
    };

    // Get filename
    let filename = entry
        .path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("download");

    // Set headers for file download
    let content_disposition = format!("attachment; filename=\"{}\"", filename);

    Ok((
        [
            (header::CONTENT_TYPE, mime_type),
            (header::CONTENT_DISPOSITION, content_disposition.as_str()),
        ],
        file_data,
    )
        .into_response())
}

/// Guess MIME type from image data magic bytes
fn guess_mime_type(data: &[u8]) -> &'static str {
    if data.len() < 4 {
        return "application/octet-stream";
    }

    // Check magic bytes
    match &data[0..4] {
        [0xFF, 0xD8, 0xFF, ..] => "image/jpeg",
        [0x89, 0x50, 0x4E, 0x47] => "image/png",
        [0x47, 0x49, 0x46, 0x38] => "image/gif",
        [0x52, 0x49, 0x46, 0x46] => "image/webp", // RIFF header (WebP)
        [0x42, 0x4D, ..] => "image/bmp",
        _ => "application/octet-stream",
    }
}

// ========== Dimensions API (for reader) ==========

#[derive(Serialize)]
struct PageDimension {
    width: u32,
    height: u32,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    estimated: bool,
}

#[derive(Serialize)]
struct DimensionsResponse {
    dimensions: Vec<PageDimension>,
}

/// API route: GET /api/dimensions/:tid/:eid
/// Returns the image dimensions of all pages in an entry (used by reader for layout)
pub async fn get_dimensions(
    State(state): State<AppState>,
    Path((title_id, entry_id)): Path<(String, String)>,
    _username: crate::auth::Username,
) -> Result<impl IntoResponse> {
    let lib = state.library.load();

    let entry = lib.get_entry(&title_id, &entry_id).ok_or_else(|| {
        Error::NotFound(format!("Entry not found: {}/{}", title_id, entry_id))
    })?;
    let entry_pages = entry.pages;
    let entry_clone = entry.clone();
    drop(lib); // Release library lock early

    // Check database cache first
    match state.storage.get_dimensions(&entry_id).await {
        Ok(Some(cached)) if cached.len() == entry_pages => {
            // Cache hit with correct page count
            let dimensions = cached
                .into_iter()
                .map(|d| PageDimension {
                    width: d.width,
                    height: d.height,
                    estimated: false,
                })
                .collect();
            return Ok(success_response(DimensionsResponse { dimensions }));
        }
        Ok(Some(cached)) => {
            // Cache is stale, will re-extract below
            tracing::debug!(
                "Dimensions cache stale for entry {} (cached: {}, actual: {})",
                entry_id,
                cached.len(),
                entry_pages
            );
        }
        Ok(None) => {
            // Cache miss - normal case
            tracing::debug!("Dimensions cache miss for entry {}", entry_id);
        }
        Err(e) => {
            // Database error - log and fall back to extraction
            tracing::error!(
                "Database error reading dimensions cache for entry {}: {}. Falling back to extraction.",
                entry_id,
                e
            );
        }
    }

    // Extract dimensions from archive (cache miss or stale)
    let mut dimensions = Vec::with_capacity(entry_pages);
    let mut dims_to_cache = Vec::with_capacity(entry_pages);

    for page_idx in 0..entry_pages {
        match entry_clone.get_page(page_idx).await {
            Ok(data) => {
                let (width, height, estimated) = match get_image_dimensions(&data) {
                    Some((w, h)) => (w, h, false),
                    None => {
                        tracing::warn!(
                            "Could not determine dimensions for page {} of entry {}, using defaults",
                            page_idx,
                            entry_id
                        );
                        (1000, 1000, true)
                    }
                };
                dimensions.push(PageDimension { width, height, estimated });
                // Only cache actual dimensions, not estimated ones
                if !estimated {
                    dims_to_cache.push((page_idx, width, height));
                }
            }
            Err(e) => {
                tracing::error!(
                    "Failed to read page {} of entry {}: {}. Using estimated dimensions.",
                    page_idx,
                    entry_id,
                    e
                );
                dimensions.push(PageDimension {
                    width: 1000,
                    height: 1000,
                    estimated: true,
                });
            }
        }
    }

    // Save to cache if we got all dimensions successfully
    if dims_to_cache.len() == entry_pages {
        if let Err(e) = state.storage.save_dimensions(&entry_id, &dims_to_cache).await {
            tracing::warn!("Failed to cache dimensions for entry {}: {}", entry_id, e);
        }
    }

    Ok(success_response(DimensionsResponse { dimensions }))
}

/// Get image dimensions from raw image data
fn get_image_dimensions(data: &[u8]) -> Option<(u32, u32)> {
    // Try to use image crate to get dimensions without full decode
    use std::io::Cursor;

    let reader = image::ImageReader::new(Cursor::new(data))
        .with_guessed_format()
        .ok()?;

    let dims = reader.into_dimensions().ok()?;
    Some(dims)
}

// ========== Progress API ==========

#[derive(Deserialize)]
pub struct ProgressQuery {
    eid: Option<String>,
}

/// API route: PUT/POST /api/progress/:tid/:page?eid=...
/// Update reading progress for an entry
/// POST is used by sendBeacon when leaving the reader page
pub async fn update_progress(
    State(state): State<AppState>,
    Path((title_id, page)): Path<(String, usize)>,
    Query(query): Query<ProgressQuery>,
    crate::auth::Username(username): crate::auth::Username,
) -> Result<impl IntoResponse> {
    let entry_id = query.eid.ok_or_else(|| {
        Error::BadRequest("Missing 'eid' query parameter".to_string())
    })?;

    let lib = state.library.load();
    let title = lib
        .get_title(&title_id)
        .ok_or_else(|| Error::NotFound(format!("Title not found: {}", title_id)))?;

    // Verify entry exists
    let _entry = lib
        .get_entry(&title_id, &entry_id)
        .ok_or_else(|| Error::NotFound(format!("Entry not found: {}", entry_id)))?;

    // Save progress via cache (updates cache and persists to disk)
    lib.progress_cache()
        .save_progress(&title_id, &title.path, &username, &entry_id, page as i32)
        .await?;

    // Invalidate response cache
    lib.invalidate_cache_for_progress(&title_id, &username).await;
    drop(lib);

    tracing::debug!(
        "Saved progress (legacy): {} / {} = page {}",
        title_id,
        entry_id,
        page
    );

    Ok(success_response(SuccessOnly {}))
}
