use axum::{
    extract::{Path, Query, State},
    http::{header, StatusCode},
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};

use super::recently_added::{group_recent_entries, RecentEntry, RECENT_ITEMS_LIMIT};

use crate::{
    error::{Error, Result},
    library::{Entry, SortMethod},
    AppState,
};

/// API route: GET /api/library
/// Returns Mango's library object with title and entry JSON.
pub async fn get_library(
    State(state): State<AppState>,
    crate::auth::Username(username): crate::auth::Username,
    Query(params): Query<CatalogQuery>,
) -> Result<impl IntoResponse> {
    let lib = state.library.load_full();
    let cache = lib.progress_cache();
    let library_info = crate::library::progress::TitleInfo::load(&state.config.library_path)
        .await
        .unwrap_or_default();
    let (library_sort, library_ascending) = library_info
        .get_sort_by(&username)
        .unwrap_or_else(|| ("auto".to_string(), true));
    let (sort_method, ascending) = SortMethod::from_params(
        Some(&library_sort),
        Some(if library_ascending { "1" } else { "0" }),
    );
    let depth = params.depth.unwrap_or(-1);
    let mut titles = Vec::new();
    let mut title_percentages = Vec::new();

    for title in lib.get_titles_sorted(sort_method, ascending) {
        let info = cache.get_title_info(&title.id).unwrap_or_default();
        let percentage = title_progress_percentage(title, &info, &username);
        title_percentages.push(percentage);
        titles.push(
            mango_title_response(
                &state,
                title,
                &info,
                &username,
                depth,
                params.percentage.is_some(),
                params.slim.is_some(),
            )
            .await?,
        );
    }

    Ok(Json(MangoLibraryResponse {
        dir: state.config.library_path.to_string_lossy().into_owned(),
        titles,
        title_percentages: params.percentage.map(|_| title_percentages),
    }))
}

/// API route: GET /api/book/:tid
/// Returns Mango's title JSON contract.
pub async fn get_title(
    State(state): State<AppState>,
    Path(title_id): Path<String>,
    crate::auth::Username(username): crate::auth::Username,
    Query(params): Query<CatalogQuery>,
) -> Result<impl IntoResponse> {
    let lib = state.library.load_full();
    let title = lib
        .get_title(&title_id)
        .ok_or_else(|| Error::NotFound(format!("Title not found: {}", title_id)))?;
    let info = lib
        .progress_cache()
        .get_title_info(&title.id)
        .unwrap_or_default();
    let response = mango_title_response(
        &state,
        title,
        &info,
        &username,
        params.depth.unwrap_or(-1),
        params.percentage.is_some(),
        params.slim.is_some(),
    )
    .await?;

    Ok(Json(response))
}

#[derive(Deserialize)]
pub struct CatalogQuery {
    depth: Option<i32>,
    percentage: Option<String>,
    slim: Option<String>,
}

#[derive(Deserialize)]
pub struct SortOptionUpdate {
    tid: Option<String>,
    sort: String,
    ascend: bool,
}

pub async fn get_sort_opt(
    State(state): State<AppState>,
    Query(query): Query<SortOptionQuery>,
    crate::auth::Username(username): crate::auth::Username,
) -> Json<serde_json::Value> {
    let dir = if let Some(title_id) = query.tid {
        let lib = state.library.load();
        let Some(title) = lib.get_title(&title_id) else {
            return Json(serde_json::json!({
                "success": false,
                "error": format!("Title not found: {title_id}")
            }));
        };
        title.path.clone()
    } else {
        state.config.library_path.clone()
    };

    match crate::library::progress::TitleInfo::load(&dir).await {
        Ok(info) => {
            let (method, ascend) = info
                .get_sort_by(&username)
                .unwrap_or_else(|| ("auto".to_string(), true));
            Json(serde_json::json!({
                "method": method,
                "ascend": ascend
            }))
        }
        Err(error) => Json(serde_json::json!({
            "success": false,
            "error": error.to_string()
        })),
    }
}

#[derive(Deserialize)]
pub struct SortOptionQuery {
    tid: Option<String>,
}

pub async fn update_sort_opt(
    State(state): State<AppState>,
    crate::auth::Username(username): crate::auth::Username,
    Json(request): Json<SortOptionUpdate>,
) -> Json<serde_json::Value> {
    let dir = if let Some(title_id) = request.tid.as_deref() {
        let lib = state.library.load();
        let Some(title) = lib.get_title(title_id) else {
            return Json(serde_json::json!({
                "success": false,
                "error": format!("Title not found: {title_id}")
            }));
        };
        title.path.clone()
    } else {
        state.config.library_path.clone()
    };
    let mut info = match crate::library::progress::TitleInfo::load(&dir).await {
        Ok(info) => info,
        Err(error) => {
            return Json(serde_json::json!({
                "success": false,
                "error": error.to_string()
            }));
        }
    };
    info.set_sort_by(&username, &request.sort, request.ascend);
    match info.save(&dir).await {
        Ok(()) => Json(serde_json::json!({ "success": true })),
        Err(error) => Json(serde_json::json!({
            "success": false,
            "error": error.to_string()
        })),
    }
}

/// API route: GET /api/page/:tid/:eid/:page
/// Serves a specific page image from an entry
pub async fn get_page(
    State(state): State<AppState>,
    Path((title_id, entry_id, page)): Path<(String, String, usize)>,
    headers: axum::http::HeaderMap,
) -> Result<impl IntoResponse> {
    let lib = state.library.load();

    let entry = lib.get_entry(&title_id, &entry_id).ok_or_else(|| {
        crate::error::Error::NotFound(format!("Entry not found: {}/{}", title_id, entry_id))
    })?;

    let page_idx = page.saturating_sub(1);
    let image_data = entry.get_page(page_idx).await?;
    let mime_type = guess_mime_type(&image_data);
    let previous_etag = headers
        .get(header::IF_NONE_MATCH)
        .and_then(|value| value.to_str().ok());

    Ok(image_response(
        image_data,
        mime_type,
        previous_etag,
        Some("public, max-age=86400"),
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
    headers: axum::http::HeaderMap,
) -> Result<impl IntoResponse> {
    let lib = state.library.load();
    let entry = lib
        .get_entry(&title_id, &entry_id)
        .ok_or_else(|| Error::NotFound(format!("Entry not found: {}/{}", title_id, entry_id)))?;
    let db = state.storage.pool();
    let thumbnail = match Entry::get_thumbnail(&entry_id, db).await {
        Ok(Some(image)) => Some(image),
        Ok(None) => match entry.generate_thumbnail(db).await {
            Ok(Some((data, mime, _))) => Some((data, mime)),
            Ok(None) => None,
            Err(error) => {
                tracing::warn!("Thumbnail generation failed for {}: {}", entry_id, error);
                None
            }
        },
        Err(error) => {
            tracing::warn!("Error getting thumbnail for {}: {}", entry_id, error);
            None
        }
    };
    let (data, mime) = match thumbnail {
        Some(image) => image,
        None => {
            let data = entry.get_page(0).await?;
            let mime = guess_mime_type(&data).to_string();
            (data, mime)
        }
    };
    let previous_etag = headers
        .get(header::IF_NONE_MATCH)
        .and_then(|value| value.to_str().ok());
    Ok(image_response(data, &mime, previous_etag, None))
}

// Response types

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
    let lib = state.library.load_full();
    let cache = lib.progress_cache();
    let mut entries_with_progress = Vec::new();

    for title in lib.get_titles_sorted(crate::library::SortMethod::Name, true) {
        let info = cache.get_title_info(&title.id).unwrap_or_default();
        if let Some((entry, previous)) = title.get_continue_reading_entry(&username, &info) {
            let last_read = info
                .get_last_read(&username, &entry.title)
                .or_else(|| previous.and_then(|entry| info.get_last_read(&username, &entry.title)));
            let progress = info.get_progress(&username, &entry.title).unwrap_or(0);
            let percentage = entry_progress_percentage(progress, entry.pages);
            let entry_json = mango_entry_response(&state, title, entry, &info, false).await?;
            entries_with_progress.push((last_read, entry_json, percentage));
        }
    }

    entries_with_progress.sort_by(|a, b| b.0.cmp(&a.0));
    entries_with_progress.truncate(8);
    let (entries, entry_percentages): (Vec<_>, Vec<_>) = entries_with_progress
        .into_iter()
        .map(|(_, entry, percentage)| (entry, percentage))
        .unzip();

    Ok(success_response(ContinueReadingResponse {
        entries,
        entry_percentages,
    }))
}

/// API route: GET /api/library/start_reading
/// Returns unread titles (0% progress) for the user
pub async fn start_reading(
    State(state): State<AppState>,
    crate::auth::Username(username): crate::auth::Username,
) -> Result<impl IntoResponse> {
    let lib = state.library.load_full();
    let cache = lib.progress_cache();
    let mut unread_titles = Vec::new();

    for title in lib.get_titles_sorted(crate::library::SortMethod::Name, true) {
        let info = cache.get_title_info(&title.id).unwrap_or_default();
        if !title.entries.is_empty() && title_progress_percentage(title, &info, &username) == 0.0 {
            unread_titles.push(title);
        }
    }

    use rand::seq::SliceRandom;
    unread_titles.shuffle(&mut rand::thread_rng());
    unread_titles.truncate(8);

    let mut titles = Vec::with_capacity(unread_titles.len());
    for title in unread_titles {
        let info = cache.get_title_info(&title.id).unwrap_or_default();
        titles.push(mango_title_response(&state, title, &info, &username, 1, false, false).await?);
    }
    Ok(success_response(StartReadingResponse { titles }))
}

/// Data retained while recent entries are sorted and grouped.
struct RecentEntryData {
    entry_id: String,
}

/// API route: GET /api/library/recently_added
/// Returns Mango's `{success, items}` response.
pub async fn recently_added(
    State(state): State<AppState>,
    crate::auth::Username(username): crate::auth::Username,
) -> Result<impl IntoResponse> {
    let lib = state.library.load_full();
    let cache = lib.progress_cache();
    let mut entries_with_dates = Vec::new();
    let one_month_ago = chrono::Utc::now()
        .checked_sub_months(chrono::Months::new(1))
        .expect("current date can be shifted back one month")
        .timestamp();

    for title in lib.get_titles_sorted(crate::library::SortMethod::Name, true) {
        let info = cache.get_title_info(&title.id).unwrap_or_default();
        for entry in &title.entries {
            if let Some(date_added) = info.get_date_added(&entry.title) {
                if date_added > one_month_ago {
                    let progress = info.get_progress(&username, &entry.title).unwrap_or(0);
                    entries_with_dates.push(RecentEntry {
                        title_id: title.id.clone(),
                        date_added,
                        percentage: entry_progress_percentage(progress, entry.pages),
                        item: RecentEntryData {
                            entry_id: entry.id.clone(),
                        },
                    });
                }
            }
        }
    }

    let mut items = Vec::new();
    for group in group_recent_entries(entries_with_dates, RECENT_ITEMS_LIMIT) {
        let title = lib
            .get_title(&group.title_id)
            .ok_or_else(|| Error::NotFound(format!("Title not found: {}", group.title_id)))?;
        let info = cache.get_title_info(&title.id).unwrap_or_default();
        let item = if group.grouped_count == 1 {
            let entry = lib
                .get_entry(&title.id, &group.item.entry_id)
                .ok_or_else(|| {
                    Error::NotFound(format!("Entry not found: {}", group.item.entry_id))
                })?;
            RecentItem::Entry(mango_entry_response(&state, title, entry, &info, false).await?)
        } else {
            RecentItem::Title(mango_title_summary(&state, title, &info, false).await?)
        };

        items.push(RecentlyAddedItem {
            item,
            percentage: group.percentage,
            count: group.grouped_count,
        });
    }

    Ok(success_response(RecentlyAddedResponse { items }))
}

// Response types for home page sections

#[derive(Serialize)]
struct ContinueReadingResponse {
    entries: Vec<MangoEntry>,
    entry_percentages: Vec<f32>,
}

#[derive(Serialize)]
struct MangoEntry {
    path: String,
    title: String,
    size: String,
    id: String,
    zip_path: String,
    title_id: String,
    title_title: String,
    sort_title: String,
    pages: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    display_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cover_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    mtime: Option<i64>,
}

#[derive(Serialize)]
struct MangoTitleSummary {
    dir: String,
    title: String,
    id: String,
    signature: u64,
    sort_title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    display_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cover_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    mtime: Option<i64>,
    parents: Vec<MangoTitleParent>,
}

#[derive(Serialize)]
struct MangoTitleResponse {
    #[serde(flatten)]
    title: MangoTitleSummary,
    #[serde(skip_serializing_if = "Option::is_none")]
    titles: Option<Vec<MangoTitleResponse>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    entries: Option<Vec<MangoEntry>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    title_percentages: Option<Vec<f32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    entry_percentages: Option<Vec<f32>>,
}

#[derive(Serialize)]
struct MangoLibraryResponse {
    dir: String,
    titles: Vec<MangoTitleResponse>,
    #[serde(skip_serializing_if = "Option::is_none")]
    title_percentages: Option<Vec<f32>>,
}

#[derive(Serialize)]
struct MangoTitleParent {
    title: String,
    id: String,
}

#[derive(Serialize)]
#[serde(untagged)]
enum RecentItem {
    Entry(MangoEntry),
    Title(MangoTitleSummary),
}

#[derive(Serialize)]
struct RecentlyAddedItem {
    item: RecentItem,
    percentage: f32,
    count: usize,
}

#[derive(Serialize)]
struct RecentlyAddedResponse {
    items: Vec<RecentlyAddedItem>,
}

#[derive(Serialize)]
struct StartReadingResponse {
    titles: Vec<MangoTitleResponse>,
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
async fn mango_entry_response(
    state: &AppState,
    title: &crate::library::Title,
    entry: &Entry,
    info: &crate::library::progress::TitleInfo,
    slim: bool,
) -> Result<MangoEntry> {
    let path = entry.path.to_string_lossy().into_owned();
    let size = tokio::fs::metadata(&entry.path).await?.len();
    let sort_title = state
        .storage
        .get_entry_sort_title(&entry.id)
        .await?
        .unwrap_or_else(|| entry.title.clone());
    let display_name = info
        .entry_display_name
        .get(&entry.title)
        .filter(|name| !name.is_empty())
        .cloned()
        .unwrap_or_else(|| entry.title.clone());
    let cover_url = info
        .entry_cover_url
        .get(&entry.title)
        .filter(|url| !url.is_empty())
        .map(|url| join_base_url(&state.config.base_url, url))
        .unwrap_or_else(|| {
            format!(
                "{}api/cover/{}/{}",
                state.config.base_url, title.id, entry.id
            )
        });

    Ok(MangoEntry {
        path: path.clone(),
        title: entry.title.clone(),
        size: humanize_bytes(size),
        id: entry.id.clone(),
        zip_path: path,
        title_id: title.id.clone(),
        title_title: title.title.clone(),
        sort_title,
        pages: entry.pages,
        display_name: (!slim).then_some(display_name),
        cover_url: (!slim).then_some(cover_url),
        mtime: (!slim).then_some(entry.mtime),
    })
}

async fn mango_title_summary(
    state: &AppState,
    title: &crate::library::Title,
    info: &crate::library::progress::TitleInfo,
    slim: bool,
) -> Result<MangoTitleSummary> {
    let sort_title = state
        .storage
        .get_title_sort_title(&title.id)
        .await?
        .unwrap_or_else(|| title.title.clone());
    let display_name = if info.display_name.is_empty() {
        title.title.clone()
    } else {
        info.display_name.clone()
    };
    let cover_url = if !info.cover_url.is_empty() {
        join_base_url(&state.config.base_url, &info.cover_url)
    } else if let Some(entry) = title.entries.first() {
        mango_entry_response(state, title, entry, info, false)
            .await?
            .cover_url
            .unwrap_or_default()
    } else {
        format!("{}img/icons/icon_x192.png", state.config.base_url)
    };

    Ok(MangoTitleSummary {
        dir: title.path.to_string_lossy().into_owned(),
        title: title.title.clone(),
        id: title.id.clone(),
        signature: title.signature.parse().unwrap_or_default(),
        sort_title,
        display_name: (!slim).then_some(display_name),
        cover_url: (!slim).then_some(cover_url),
        mtime: (!slim).then_some(title.mtime),
        parents: Vec::new(),
    })
}

async fn mango_title_response(
    state: &AppState,
    title: &crate::library::Title,
    info: &crate::library::progress::TitleInfo,
    username: &str,
    depth: i32,
    include_percentages: bool,
    slim: bool,
) -> Result<MangoTitleResponse> {
    let summary = mango_title_summary(state, title, info, slim).await?;
    if depth == 0 {
        return Ok(MangoTitleResponse {
            title: summary,
            titles: None,
            entries: None,
            title_percentages: None,
            entry_percentages: None,
        });
    }

    let (entry_sort, entry_ascending) = info
        .get_sort_by(username)
        .unwrap_or_else(|| ("auto".to_string(), true));
    let (entry_sort, entry_ascending) = SortMethod::from_params(
        Some(&entry_sort),
        Some(if entry_ascending { "1" } else { "0" }),
    );
    let mut entries = Vec::with_capacity(title.entries.len());
    let mut entry_percentages = Vec::with_capacity(title.entries.len());
    for entry in title.get_entries_sorted(entry_sort, entry_ascending) {
        let progress = info.get_progress(username, &entry.title).unwrap_or(0);
        entry_percentages.push(if entry.pages == 0 {
            0.0
        } else {
            progress.min(entry.pages as i32).max(0) as f32 / entry.pages as f32
        });
        entries.push(mango_entry_response(state, title, entry, info, slim).await?);
    }

    Ok(MangoTitleResponse {
        title: summary,
        titles: Some(Vec::new()),
        entries: Some(entries),
        title_percentages: include_percentages.then(Vec::new),
        entry_percentages: include_percentages.then_some(entry_percentages),
    })
}

fn title_progress_percentage(
    title: &crate::library::Title,
    info: &crate::library::progress::TitleInfo,
    username: &str,
) -> f32 {
    let total_pages: usize = title.entries.iter().map(|entry| entry.pages).sum();
    if total_pages == 0 {
        return 0.0;
    }
    let read_pages: f64 = title
        .entries
        .iter()
        .map(|entry| {
            info.get_progress(username, &entry.title)
                .unwrap_or(0)
                .clamp(0, entry.pages as i32) as f64
        })
        .sum();
    (read_pages / total_pages as f64) as f32
}

fn entry_progress_percentage(progress: i32, pages: usize) -> f32 {
    if pages == 0 {
        return 0.0;
    }
    progress.clamp(0, pages as i32) as f32 / pages as f32
}

fn join_base_url(base_url: &str, path: &str) -> String {
    if path.starts_with("http://") || path.starts_with("https://") {
        path.to_string()
    } else {
        format!("{}{}", base_url, path.trim_start_matches('/'))
    }
}

fn humanize_bytes(bytes: u64) -> String {
    const UNITS: [&str; 6] = ["B", "KiB", "MiB", "GiB", "TiB", "PiB"];
    if bytes < 1024 {
        return format!("{bytes}B");
    }

    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit + 1 < UNITS.len() {
        value /= 1024.0;
        unit += 1;
    }
    format!("{value:.1}{}", UNITS[unit])
}

/// API route: GET /api/tags
pub async fn list_tags(
    State(state): State<AppState>,
    _username: crate::auth::Username,
) -> Json<serde_json::Value> {
    match state.storage.list_tags().await {
        Ok(tags) => Json(serde_json::json!({"success": true, "tags": tags})),
        Err(error) => api_failure(error.to_string()),
    }
}

/// API route: GET /api/tags/:tid
pub async fn get_title_tags(
    State(state): State<AppState>,
    Path(title_id): Path<String>,
    _username: crate::auth::Username,
) -> Json<serde_json::Value> {
    let lib = state.library.load();
    if lib.get_title(&title_id).is_none() {
        return api_failure(format!("Title not found: {title_id}"));
    }
    match state.storage.get_title_tags(&title_id).await {
        Ok(tags) => Json(serde_json::json!({"success": true, "tags": tags})),
        Err(error) => api_failure(error.to_string()),
    }
}

fn api_failure(error: String) -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "success": false,
        "error": error
    }))
}

/// API route: PUT /api/admin/tags/:tid/:tag
pub async fn add_tag(
    State(state): State<AppState>,
    Path((title_id, tag)): Path<(String, String)>,
    _admin: crate::auth::AdminOnly,
) -> Json<serde_json::Value> {
    if state.library.load().get_title(&title_id).is_none() {
        return api_failure(format!("Title not found: {title_id}"));
    }
    match state.storage.add_tag(&title_id, &tag).await {
        Ok(()) => Json(serde_json::json!({"success": true, "error": null})),
        Err(error) => api_failure(error.to_string()),
    }
}

/// API route: DELETE /api/admin/tags/:tid/:tag
pub async fn delete_tag(
    State(state): State<AppState>,
    Path((title_id, tag)): Path<(String, String)>,
    _admin: crate::auth::AdminOnly,
) -> Json<serde_json::Value> {
    if state.library.load().get_title(&title_id).is_none() {
        return api_failure(format!("Title not found: {title_id}"));
    }
    match state.storage.delete_tag(&title_id, &tag).await {
        Ok(()) => Json(serde_json::json!({"success": true, "error": null})),
        Err(error) => api_failure(error.to_string()),
    }
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

fn image_response(
    data: Vec<u8>,
    mime: &str,
    previous_etag: Option<&str>,
    cache_control: Option<&str>,
) -> axum::response::Response {
    use sha1::Digest;

    let etag = format!("{:x}", sha1::Sha1::digest(&data));
    if previous_etag == Some(etag.as_str()) {
        return StatusCode::NOT_MODIFIED.into_response();
    }

    let mut headers = axum::http::HeaderMap::new();
    headers.insert(
        header::ETAG,
        etag.parse()
            .expect("SHA-1 hex digest is a valid header value"),
    );
    headers.insert(
        header::CONTENT_TYPE,
        mime.parse()
            .unwrap_or_else(|_| axum::http::HeaderValue::from_static("application/octet-stream")),
    );
    if let Some(cache_control) = cache_control {
        headers.insert(
            header::CACHE_CONTROL,
            cache_control.parse().expect("static cache-control header"),
        );
    }
    (StatusCode::OK, headers, data).into_response()
}

// ========== Dimensions API (for reader) ==========

#[derive(Serialize)]
struct PageDimension {
    width: u32,
    height: u32,
}

#[derive(Serialize)]
struct DimensionsResponse {
    dimensions: Vec<PageDimension>,
}

fn dimensions_response(dimensions: Vec<PageDimension>, etag: &str) -> axum::response::Response {
    let mut response = success_response(DimensionsResponse { dimensions }).into_response();
    response.headers_mut().insert(
        header::ETAG,
        etag.parse().expect("SHA-1 ETag is a valid header value"),
    );
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        "public, max-age=86400"
            .parse()
            .expect("static cache-control header"),
    );
    response
}

/// API route: GET /api/dimensions/:tid/:eid
/// Returns the image dimensions of all pages in an entry (used by reader for layout)
pub async fn get_dimensions(
    State(state): State<AppState>,
    Path((title_id, entry_id)): Path<(String, String)>,
    headers: axum::http::HeaderMap,
) -> Result<impl IntoResponse> {
    let lib = state.library.load();

    let entry = lib
        .get_entry(&title_id, &entry_id)
        .ok_or_else(|| Error::NotFound(format!("Entry not found: {}/{}", title_id, entry_id)))?;
    let entry_pages = entry.pages;
    let etag_source = format!("{}{}", entry.path.display(), entry.mtime);
    let etag = format!("W/{:x}", {
        use sha1::Digest;
        sha1::Sha1::digest(etag_source.as_bytes())
    });
    let previous_etag = headers
        .get(header::IF_NONE_MATCH)
        .and_then(|value| value.to_str().ok());
    if previous_etag == Some(etag.as_str()) {
        return Ok(StatusCode::NOT_MODIFIED.into_response());
    }
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
                })
                .collect();
            return Ok(dimensions_response(dimensions, &etag));
        }
        Ok(Some(cached)) => {
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
                dimensions.push(PageDimension { width, height });
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
                });
            }
        }
    }

    // Save to cache if we got all dimensions successfully
    if dims_to_cache.len() == entry_pages {
        if let Err(e) = state
            .storage
            .save_dimensions(&entry_id, &dims_to_cache)
            .await
        {
            tracing::warn!("Failed to cache dimensions for entry {}: {}", entry_id, e);
        }
    }

    Ok(dimensions_response(dimensions, &etag))
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

/// API route: PUT /api/progress/:tid/:page?eid=...
/// Update one entry or mark all entries in a title read/unread.
pub async fn update_progress(
    State(state): State<AppState>,
    Path((title_id, page)): Path<(String, i32)>,
    Query(query): Query<ProgressQuery>,
    crate::auth::Username(username): crate::auth::Username,
) -> Json<serde_json::Value> {
    let lib = state.library.load();
    let Some(title) = lib.get_title(&title_id) else {
        return Json(serde_json::json!({
            "success": false,
            "error": format!("Title not found: {title_id}")
        }));
    };

    if let Some(entry_id) = query.eid {
        let Some(entry) = lib.get_entry(&title_id, &entry_id) else {
            return Json(serde_json::json!({
                "success": false,
                "error": format!("Entry not found: {entry_id}")
            }));
        };
        if page < 0 || page > entry.pages as i32 {
            return Json(serde_json::json!({
                "success": false,
                "error": "incorrect page value"
            }));
        }
        if let Err(error) = lib
            .progress_cache()
            .save_progress(&title_id, &title.path, &username, &entry.title, page)
            .await
        {
            return Json(serde_json::json!({
                "success": false,
                "error": error.to_string()
            }));
        }
    } else {
        for entry in &title.entries {
            let value = if page == 0 { 0 } else { entry.pages as i32 };
            if let Err(error) = lib
                .progress_cache()
                .save_progress(&title_id, &title.path, &username, &entry.title, value)
                .await
            {
                return Json(serde_json::json!({
                    "success": false,
                    "error": error.to_string()
                }));
            }
        }
    }

    lib.invalidate_cache_for_progress(&title_id, &username)
        .await;
    Json(serde_json::json!({ "success": true }))
}
