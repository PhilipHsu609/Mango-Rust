use axum::{
    extract::{rejection::JsonRejection, Path, Query, State},
    http::{header, StatusCode},
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};

use super::recently_added::{group_recent_entries, RecentEntry, RECENT_ITEMS_LIMIT};

use crate::{
    error::Result,
    library::{
        chapter_sort::{compare_numerically, ChapterSorter},
        Entry, SortMethod,
    },
    AppState,
};

/// API route: GET /api/library
/// Returns Mango's library object with title and entry JSON.
#[utoipa::path(get, path = "/api/library", tag = "library", summary = "Get the library", params(("depth" = Option<i32>, Query, description = "Maximum nested-title depth"), ("percentage" = Option<String>, Query, description = "Include reading percentages"), ("slim" = Option<String>, Query, description = "Use the slim response format")), responses((status = 200, description = "Library returned")))]
pub async fn get_library(
    State(state): State<AppState>,
    crate::auth::Username(username): crate::auth::Username,
    Query(params): Query<CatalogQuery>,
) -> axum::response::Response {
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
    let depth = catalog_depth(params.depth.as_deref());
    let mut titles = Vec::new();
    let mut ordered_titles = Vec::with_capacity(lib.get_titles().len());
    for title in lib.get_titles() {
        let sort_title = match state.storage.get_title_sort_title(&title.id).await {
            Ok(sort_title) => sort_title.unwrap_or_else(|| title.title.clone()),
            Err(error) => {
                return Json(serde_json::json!({
                    "success": false,
                    "error": error.to_string()
                }))
                .into_response();
            }
        };
        let info = cache.get_title_info(&title.id).unwrap_or_default();
        let percentage = title_progress_percentage(title, cache, &username);
        ordered_titles.push((title, sort_title, percentage, info));
    }
    ordered_titles.sort_by(
        |(left, left_sort, left_progress, _), (right, right_sort, right_progress, _)| {
            match sort_method {
                SortMethod::TimeModified => left
                    .mtime
                    .cmp(&right.mtime)
                    .then_with(|| compare_numerically(left_sort, right_sort)),
                SortMethod::Progress => left_progress
                    .total_cmp(right_progress)
                    .then_with(|| compare_numerically(left_sort, right_sort)),
                SortMethod::Name | SortMethod::TimeAdded | SortMethod::Auto => {
                    compare_numerically(left_sort, right_sort)
                }
            }
        },
    );
    if !ascending {
        ordered_titles.reverse();
    }
    let mut title_percentages = Vec::with_capacity(ordered_titles.len());

    for (title, _, percentage, info) in ordered_titles {
        title_percentages.push(percentage);
        match mango_title_response(
            &state,
            title,
            &info,
            cache,
            &username,
            Vec::new(),
            depth,
            params.percentage.is_some(),
            params.slim.is_some(),
            Some((sort_method, ascending)),
        )
        .await
        {
            Ok(response) => titles.push(response),
            Err(error) => {
                return Json(serde_json::json!({
                    "success": false,
                    "error": error.to_string()
                }))
                .into_response();
            }
        }
    }

    Json(MangoLibraryResponse {
        dir: state.config.library_path.to_string_lossy().into_owned(),
        titles,
        title_percentages: params.percentage.map(|_| title_percentages),
    })
    .into_response()
}

/// API route: GET /api/book/:tid
/// Returns Mango's title JSON contract.
#[utoipa::path(get, path = "/api/book/{tid}", tag = "library", summary = "Get a title", params(("tid" = String, Path, description = "Title ID"), ("depth" = Option<i32>, Query, description = "Maximum nested-title depth"), ("percentage" = Option<String>, Query, description = "Include reading percentages"), ("slim" = Option<String>, Query, description = "Use the slim response format")), responses((status = 200, description = "Title returned")))]
pub async fn get_title(
    State(state): State<AppState>,
    Path(title_id): Path<String>,
    crate::auth::Username(username): crate::auth::Username,
    Query(params): Query<CatalogQuery>,
) -> Result<axum::response::Response> {
    let lib = state.library.load_full();
    let Some(title) = lib.get_title(&title_id) else {
        return Ok((
            StatusCode::NOT_FOUND,
            format!("Title ID `{title_id}` not found"),
        )
            .into_response());
    };
    let info = lib
        .progress_cache()
        .get_title_info(&title.id)
        .unwrap_or_default();
    let response = match mango_title_response(
        &state,
        title,
        &info,
        lib.progress_cache(),
        &username,
        title_parent_summaries(&lib, title),
        catalog_depth(params.depth.as_deref()),
        params.percentage.is_some(),
        params.slim.is_some(),
        None,
    )
    .await
    {
        Ok(response) => response,
        Err(error) => {
            return Ok((StatusCode::NOT_FOUND, error.to_string()).into_response());
        }
    };

    Ok(Json(response).into_response())
}

#[derive(Deserialize)]
pub struct CatalogQuery {
    depth: Option<String>,
    percentage: Option<String>,
    slim: Option<String>,
}

fn catalog_depth(depth: Option<&str>) -> i32 {
    depth.and_then(|value| value.parse().ok()).unwrap_or(-1)
}

#[derive(Deserialize, utoipa::ToSchema)]
pub struct SortOptionUpdate {
    tid: Option<String>,
    sort: String,
    ascend: bool,
}

#[utoipa::path(get, path = "/api/sort_opt", tag = "library", summary = "Get sort options", params(("tid" = Option<String>, Query, description = "Title ID; omit for library defaults")), responses((status = 200, description = "Sort options returned")))]
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
                "error": "Nil assertion failed"
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

#[utoipa::path(put, path = "/api/sort_opt", tag = "library", summary = "Update sort options", request_body = SortOptionUpdate, responses((status = 200, description = "Sort options updated")))]
pub async fn update_sort_opt(
    State(state): State<AppState>,
    crate::auth::Username(username): crate::auth::Username,
    request: std::result::Result<Json<SortOptionUpdate>, JsonRejection>,
) -> Json<serde_json::Value> {
    let Json(request) = match request {
        Ok(request) => request,
        Err(error) => return api_failure(error.body_text()),
    };
    let dir = if let Some(title_id) = request.tid.as_deref() {
        let lib = state.library.load();
        let Some(title) = lib.get_title(title_id) else {
            return Json(serde_json::json!({
                "success": false,
                "error": "Nil assertion failed"
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
        Ok(()) => {
            if let Some(title_id) = request.tid.as_deref() {
                let lib = state.library.load_full();
                if let Err(error) = lib.progress_cache().load_title(title_id, &dir).await {
                    return Json(serde_json::json!({
                        "success": false,
                        "error": error.to_string()
                    }));
                }
            }
            Json(serde_json::json!({ "success": true }))
        }
        Err(error) => Json(serde_json::json!({
            "success": false,
            "error": error.to_string()
        })),
    }
}

fn page_index(page: i32) -> Option<usize> {
    usize::try_from(page.checked_sub(1)?).ok()
}

fn order_continue_candidates<T>(entries: &mut Vec<(Option<i64>, T, f64)>) {
    entries.truncate(8);
    entries.sort_by(|a, b| b.0.cmp(&a.0));
}

/// API route: GET /api/page/:tid/:eid/:page
/// Serves a specific page image from an entry
#[utoipa::path(get, path = "/api/page/{tid}/{eid}/{page}", tag = "reader", summary = "Get a page image", params(("tid" = String, Path, description = "Title ID"), ("eid" = String, Path, description = "Entry ID"), ("page" = i32, Path, description = "Page number")), responses((status = 200, description = "Page image returned")))]
pub async fn get_page(
    State(state): State<AppState>,
    Path((title_id, entry_id, page)): Path<(String, String, String)>,
    headers: axum::http::HeaderMap,
) -> Result<axum::response::Response> {
    let page = match page.parse::<i32>() {
        Ok(page) => page,
        Err(_) => {
            return Ok((
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Invalid Int32: {page}"),
            )
                .into_response());
        }
    };
    let lib = state.library.load();
    let Some(title) = lib.get_title(&title_id) else {
        return Ok((
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Title ID `{title_id}` not found"),
        )
            .into_response());
    };
    let Some(entry) = title.entries.iter().find(|entry| entry.id == entry_id) else {
        return Ok((
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Entry ID `{entry_id}` of `{}` not found", title.title),
        )
            .into_response());
    };

    let Some(page_idx) = page_index(page) else {
        return Ok((
            StatusCode::INTERNAL_SERVER_ERROR,
            format!(
                "Failed to load page {page} of `{}/{}`",
                title.title, entry.title
            ),
        )
            .into_response());
    };

    let image_data = match entry.get_page(page_idx).await {
        Ok(image_data) => image_data,
        Err(error) => {
            return Ok((StatusCode::INTERNAL_SERVER_ERROR, error.to_string()).into_response())
        }
    };
    let mime_type = guess_mime_type(&image_data);
    let previous_etag = headers
        .get(header::IF_NONE_MATCH)
        .and_then(|value| value.to_str().ok());

    let cache_control = if entry.path.is_dir() {
        "no-cache, max-age=86400"
    } else {
        "public, max-age=86400"
    };
    Ok(image_response(image_data, mime_type, previous_etag, Some(cache_control)).into_response())
}

/// GET /api/cover/:tid/:eid - Get manga entry cover/thumbnail
#[utoipa::path(get, path = "/api/cover/{tid}/{eid}", tag = "reader", summary = "Get an entry cover", params(("tid" = String, Path, description = "Title ID"), ("eid" = String, Path, description = "Entry ID")), responses((status = 200, description = "Cover returned")))]
pub async fn get_cover(
    State(state): State<AppState>,
    Path((title_id, entry_id)): Path<(String, String)>,
    headers: axum::http::HeaderMap,
) -> Result<axum::response::Response> {
    let lib = state.library.load();
    let Some(title) = lib.get_title(&title_id) else {
        return Ok((
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Title ID `{title_id}` not found"),
        )
            .into_response());
    };
    let Some(entry) = title.entries.iter().find(|entry| entry.id == entry_id) else {
        return Ok((
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Entry ID `{entry_id}` of `{}` not found", title.title),
        )
            .into_response());
    };
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
            let data = match entry.get_page(0).await {
                Ok(data) => data,
                Err(error) => {
                    return Ok(
                        (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()).into_response()
                    );
                }
            };
            let mime = guess_mime_type(&data).to_string();
            (data, mime)
        }
    };
    let previous_etag = headers
        .get(header::IF_NONE_MATCH)
        .and_then(|value| value.to_str().ok());
    Ok(image_response(data, &mime, previous_etag, None).into_response())
}

// Response types

/// API route: GET /api/library/continue_reading
/// Returns the last 8 entries the user has read, sorted by last_read timestamp
#[utoipa::path(get, path = "/api/library/continue_reading", tag = "library", summary = "Get continue-reading entries", responses((status = 200, description = "Entries returned")))]
pub async fn continue_reading(
    State(state): State<AppState>,
    crate::auth::Username(username): crate::auth::Username,
) -> axum::response::Response {
    let lib = state.library.load_full();
    let cache = lib.progress_cache();
    let mut entries_with_progress = Vec::new();

    for title in lib.all_titles() {
        let info = cache.get_title_info(&title.id).unwrap_or_default();
        let mut sort_title_overrides = std::collections::HashMap::new();
        for entry in &title.entries {
            match state.storage.get_entry_sort_title(&entry.id).await {
                Ok(Some(sort_title)) => {
                    sort_title_overrides.insert(entry.id.clone(), sort_title);
                }
                Ok(None) => {}
                Err(error) => {
                    return Json(serde_json::json!({
                        "success": false,
                        "error": error.to_string()
                    }))
                    .into_response();
                }
            }
        }
        if let Some((entry, previous)) =
            title.get_continue_reading_entry(&username, &info, &sort_title_overrides)
        {
            let last_read = info
                .get_last_read(&username, &entry.title)
                .or_else(|| previous.and_then(|entry| info.get_last_read(&username, &entry.title)));
            let progress = info.get_progress(&username, &entry.title).unwrap_or(0);
            let percentage = entry_progress_percentage(progress, entry.pages);
            let entry_json =
                match mango_entry_response(&state, title, entry, &info, None, false).await {
                    Ok(entry) => entry,
                    Err(error) => {
                        return Json(serde_json::json!({
                            "success": false,
                            "error": error.to_string()
                        }))
                        .into_response();
                    }
                };
            entries_with_progress.push((last_read, entry_json, percentage));
        }
    }

    order_continue_candidates(&mut entries_with_progress);
    let (entries, entry_percentages): (Vec<_>, Vec<_>) = entries_with_progress
        .into_iter()
        .map(|(_, entry, percentage)| (entry, percentage))
        .unzip();

    success_response(ContinueReadingResponse {
        entries,
        entry_percentages,
    })
    .into_response()
}

/// API route: GET /api/library/start_reading
/// Returns unread titles (0% progress) for the user
#[utoipa::path(get, path = "/api/library/start_reading", tag = "library", summary = "Get unread titles", responses((status = 200, description = "Titles returned")))]
pub async fn start_reading(
    State(state): State<AppState>,
    crate::auth::Username(username): crate::auth::Username,
) -> axum::response::Response {
    let lib = state.library.load_full();
    let cache = lib.progress_cache();
    let mut unread_titles = Vec::new();

    for title in lib.get_titles_sorted(crate::library::SortMethod::Name, true) {
        if title.total_pages() > 0 && title_progress_percentage(title, cache, &username) == 0.0 {
            unread_titles.push(title);
        }
    }

    use rand::seq::SliceRandom;
    unread_titles.shuffle(&mut rand::thread_rng());
    unread_titles.truncate(8);

    let mut titles = Vec::with_capacity(unread_titles.len());
    for title in unread_titles {
        let info = cache.get_title_info(&title.id).unwrap_or_default();
        match mango_title_response(
            &state,
            title,
            &info,
            cache,
            &username,
            title_parent_summaries(&lib, title),
            1,
            false,
            false,
            None,
        )
        .await
        {
            Ok(title) => titles.push(title),
            Err(error) => {
                return Json(serde_json::json!({
                    "success": false,
                    "error": error.to_string()
                }))
                .into_response();
            }
        }
    }
    success_response(StartReadingResponse { titles }).into_response()
}

/// Data retained while recent entries are sorted and grouped.
struct RecentEntryData {
    entry_id: String,
}

/// API route: GET /api/library/recently_added
/// Returns Mango's `{success, items}` response.
#[utoipa::path(get, path = "/api/library/recently_added", tag = "library", summary = "Get recently added entries", responses((status = 200, description = "Entries returned")))]
pub async fn recently_added(
    State(state): State<AppState>,
    crate::auth::Username(username): crate::auth::Username,
) -> axum::response::Response {
    let lib = state.library.load_full();
    let cache = lib.progress_cache();
    let mut entries_with_dates = Vec::new();
    let one_month_ago = chrono::Utc::now()
        .checked_sub_months(chrono::Months::new(1))
        .expect("current date can be shifted back one month")
        .timestamp();

    for title in lib.all_titles() {
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
        let Some(title) = lib.get_title(&group.title_id) else {
            return Json(serde_json::json!({
                "success": false,
                "error": format!("Title not found: {}", group.title_id)
            }))
            .into_response();
        };
        let info = cache.get_title_info(&title.id).unwrap_or_default();
        let item = if group.grouped_count == 1 {
            let Some(entry) = lib.get_entry(&title.id, &group.item.entry_id) else {
                return Json(serde_json::json!({
                    "success": false,
                    "error": format!("Entry not found: {}", group.item.entry_id)
                }))
                .into_response();
            };
            match mango_entry_response(&state, title, entry, &info, None, false).await {
                Ok(entry) => RecentItem::Entry(entry),
                Err(error) => {
                    return Json(serde_json::json!({
                        "success": false,
                        "error": error.to_string()
                    }))
                    .into_response();
                }
            }
        } else {
            match mango_title_summary(
                &state,
                title,
                &info,
                title_parent_summaries(&lib, title),
                false,
            )
            .await
            {
                Ok(title) => RecentItem::Title(title),
                Err(error) => {
                    return Json(serde_json::json!({
                        "success": false,
                        "error": error.to_string()
                    }))
                    .into_response();
                }
            }
        };

        items.push(RecentlyAddedItem {
            item,
            percentage: group.percentage,
            count: group.grouped_count,
        });
    }

    success_response(RecentlyAddedResponse { items }).into_response()
}

// Response types for home page sections

#[derive(Serialize)]
struct ContinueReadingResponse {
    entries: Vec<MangoEntry>,
    entry_percentages: Vec<f64>,
}

#[derive(Serialize)]
struct MangoEntry {
    path: String,
    title: String,
    size: String,
    id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    err_msg: Option<String>,
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
    title_percentages: Option<Vec<f64>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    entry_percentages: Option<Vec<f64>>,
}

#[derive(Serialize)]
struct MangoLibraryResponse {
    dir: String,
    titles: Vec<MangoTitleResponse>,
    #[serde(skip_serializing_if = "Option::is_none")]
    title_percentages: Option<Vec<f64>>,
}

#[derive(Clone, Serialize)]
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
    percentage: f64,
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
    sort_title_override: Option<&str>,
    slim: bool,
) -> Result<MangoEntry> {
    let path = entry.path.to_string_lossy().into_owned();
    let size = if entry.size_bytes == 0 {
        tokio::fs::metadata(&entry.path).await?.len()
    } else {
        entry.size_bytes
    };
    let sort_title = if let Some(sort_title) = sort_title_override {
        sort_title.to_string()
    } else {
        state
            .storage
            .get_entry_sort_title(&entry.id)
            .await?
            .unwrap_or_else(|| entry.title.clone())
    };
    let display_name = info
        .entry_display_name
        .get(&entry.title)
        .filter(|name| !name.is_empty())
        .cloned()
        .unwrap_or_else(|| entry.title.clone());
    let cover_url = if entry.err_msg.is_some() {
        format!("{}static/img/icons/icon_x192.png", state.config.base_url)
    } else {
        info.entry_cover_url
            .get(&entry.title)
            .filter(|url| !url.is_empty())
            .map(|url| join_base_url(&state.config.base_url, url))
            .unwrap_or_else(|| {
                format!(
                    "{}api/cover/{}/{}",
                    state.config.base_url, title.id, entry.id
                )
            })
    };

    Ok(MangoEntry {
        path: path.clone(),
        title: entry.title.clone(),
        size: humansize::format_size(size, humansize::BINARY),
        id: entry.id.clone(),
        err_msg: entry.err_msg.clone(),
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
    parents: Vec<MangoTitleParent>,
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
        mango_entry_response(state, title, entry, info, None, false)
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
        parents,
    })
}

fn title_parent_summaries(
    library: &crate::library::Library,
    title: &crate::library::Title,
) -> Vec<MangoTitleParent> {
    library
        .parent_titles(title)
        .into_iter()
        .map(|parent| MangoTitleParent {
            title: parent.title.clone(),
            id: parent.id.clone(),
        })
        .collect()
}

async fn mango_title_response(
    state: &AppState,
    title: &crate::library::Title,
    info: &crate::library::progress::TitleInfo,
    cache: &crate::library::ProgressCache,
    username: &str,
    parents: Vec<MangoTitleParent>,
    depth: i32,
    include_percentages: bool,
    slim: bool,
    sort_context: Option<(SortMethod, bool)>,
) -> Result<MangoTitleResponse> {
    let summary = mango_title_summary(state, title, info, parents.clone(), slim).await?;
    if depth == 0 {
        return Ok(MangoTitleResponse {
            title: summary,
            titles: None,
            entries: None,
            title_percentages: None,
            entry_percentages: None,
        });
    }

    let (sort_method, ascending) = sort_context.unwrap_or_else(|| {
        info.get_sort_by(username)
            .map(|(method, ascending)| (SortMethod::parse(&method), ascending))
            .unwrap_or((SortMethod::Auto, true))
    });
    let mut nested_titles = Vec::with_capacity(title.nested_titles.len());
    let mut nested_order = Vec::with_capacity(title.nested_titles.len());
    for nested in &title.nested_titles {
        let sort_title = state
            .storage
            .get_title_sort_title(&nested.id)
            .await?
            .unwrap_or_else(|| nested.title.clone());
        let percentage = title_progress_percentage(nested, cache, username);
        nested_order.push((nested, sort_title, percentage));
    }
    nested_order.sort_by(
        |(left, left_sort, left_progress), (right, right_sort, right_progress)| match sort_method {
            SortMethod::TimeModified => left
                .mtime
                .cmp(&right.mtime)
                .then_with(|| compare_numerically(left_sort, right_sort)),
            SortMethod::Progress => left_progress
                .total_cmp(right_progress)
                .then_with(|| compare_numerically(left_sort, right_sort)),
            SortMethod::Name | SortMethod::TimeAdded | SortMethod::Auto => {
                compare_numerically(left_sort, right_sort)
            }
        },
    );
    if !ascending {
        nested_order.reverse();
    }
    let mut title_percentages = Vec::with_capacity(title.nested_titles.len());
    let mut child_parents = parents;
    child_parents.push(MangoTitleParent {
        title: title.title.clone(),
        id: title.id.clone(),
    });
    for (nested, _, percentage) in nested_order {
        let nested_info = cache.get_title_info(&nested.id).unwrap_or_default();
        title_percentages.push(percentage);
        nested_titles.push(
            Box::pin(mango_title_response(
                state,
                nested,
                &nested_info,
                cache,
                username,
                child_parents.clone(),
                if depth > 0 { depth - 1 } else { depth },
                include_percentages,
                slim,
                Some((sort_method, ascending)),
            ))
            .await?,
        );
    }

    let mut entries_with_sort_title = Vec::with_capacity(title.entries.len());
    for entry in &title.entries {
        let sort_title = state
            .storage
            .get_entry_sort_title(&entry.id)
            .await?
            .unwrap_or_else(|| entry.title.clone());
        entries_with_sort_title.push((entry, sort_title));
    }
    let chapter_sorter = if matches!(sort_method, SortMethod::Auto) {
        let sort_titles = entries_with_sort_title
            .iter()
            .map(|(_, sort_title)| sort_title.as_str())
            .collect::<Vec<_>>();
        Some(ChapterSorter::new(&sort_titles))
    } else {
        None
    };
    entries_with_sort_title.sort_by(|(left, left_sort), (right, right_sort)| match sort_method {
        SortMethod::TimeModified => left
            .mtime
            .cmp(&right.mtime)
            .then_with(|| compare_numerically(left_sort, right_sort)),
        SortMethod::TimeAdded => info
            .get_date_added(&left.title)
            .unwrap_or_default()
            .cmp(&info.get_date_added(&right.title).unwrap_or_default())
            .then_with(|| compare_numerically(left_sort, right_sort)),
        SortMethod::Progress => {
            let left_progress = entry_progress_percentage(
                info.get_progress(username, &left.title).unwrap_or(0),
                left.pages,
            );
            let right_progress = entry_progress_percentage(
                info.get_progress(username, &right.title).unwrap_or(0),
                right.pages,
            );
            left_progress
                .total_cmp(&right_progress)
                .then_with(|| compare_numerically(left_sort, right_sort))
        }
        SortMethod::Name => compare_numerically(left_sort, right_sort),
        SortMethod::Auto => chapter_sorter
            .as_ref()
            .expect("auto sorting builds a chapter sorter")
            .compare(left_sort, right_sort)
            .then_with(|| compare_numerically(left_sort, right_sort)),
    });
    if !ascending {
        entries_with_sort_title.reverse();
    }
    let ordered_entries = entries_with_sort_title
        .into_iter()
        .map(|(entry, sort_title)| (entry, Some(sort_title)))
        .collect::<Vec<_>>();

    let mut entries = Vec::with_capacity(title.entries.len());
    let mut entry_percentages = Vec::with_capacity(title.entries.len());
    for (entry, sort_title) in ordered_entries {
        let progress = info.get_progress(username, &entry.title).unwrap_or(0);
        entry_percentages.push(entry_progress_percentage(progress, entry.pages));
        entries.push(
            mango_entry_response(state, title, entry, info, sort_title.as_deref(), slim).await?,
        );
    }

    Ok(MangoTitleResponse {
        title: summary,
        titles: Some(nested_titles),
        entries: Some(entries),
        title_percentages: include_percentages.then_some(title_percentages),
        entry_percentages: include_percentages.then_some(entry_percentages),
    })
}

fn title_progress_percentage(
    title: &crate::library::Title,
    cache: &crate::library::ProgressCache,
    username: &str,
) -> f64 {
    let mut total_pages = 0usize;
    let mut read_pages = 0f64;
    for nested in std::iter::once(title).chain(title.deep_titles()) {
        let info = cache.get_title_info(&nested.id).unwrap_or_default();
        for entry in &nested.entries {
            total_pages += entry.pages;
            read_pages += info
                .get_progress(username, &entry.title)
                .unwrap_or(0)
                .clamp(0, entry.pages as i32) as f64;
        }
    }
    if total_pages == 0 {
        0.0
    } else {
        read_pages / total_pages as f64
    }
}

fn entry_progress_percentage(progress: i32, pages: usize) -> f64 {
    if pages == 0 {
        return 0.0;
    }
    progress.clamp(0, pages as i32) as f64 / pages as f64
}

pub(super) fn join_base_url(base_url: &str, path: &str) -> String {
    if path.starts_with("http://") || path.starts_with("https://") {
        path.to_string()
    } else {
        format!("{}{}", base_url, path.trim_start_matches('/'))
    }
}

/// API route: GET /api/tags
#[utoipa::path(get, path = "/api/tags", tag = "library", summary = "List tags", responses((status = 200, description = "Tags returned")))]
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
#[utoipa::path(get, path = "/api/tags/{tid}", tag = "library", summary = "Get title tags", params(("tid" = String, Path, description = "Title ID")), responses((status = 200, description = "Tags returned")))]
pub async fn get_title_tags(
    State(state): State<AppState>,
    Path(title_id): Path<String>,
    _username: crate::auth::Username,
) -> Json<serde_json::Value> {
    let lib = state.library.load();
    if lib.get_title(&title_id).is_none() {
        return api_failure("Nil assertion failed".to_string());
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
#[utoipa::path(put, path = "/api/admin/tags/{tid}/{tag}", tag = "library", summary = "Add a tag to a title", params(("tid" = String, Path, description = "Title ID"), ("tag" = String, Path, description = "Tag")), responses((status = 200, description = "Tag added")))]
pub async fn add_tag(
    State(state): State<AppState>,
    Path((title_id, tag)): Path<(String, String)>,
    _admin: crate::auth::AdminOnly,
) -> Json<serde_json::Value> {
    if state.library.load().get_title(&title_id).is_none() {
        return api_failure("Nil assertion failed".to_string());
    }
    match state.storage.add_tag(&title_id, &tag).await {
        Ok(()) => Json(serde_json::json!({"success": true, "error": null})),
        Err(error) => api_failure(error.to_string()),
    }
}

/// API route: DELETE /api/admin/tags/:tid/:tag
#[utoipa::path(delete, path = "/api/admin/tags/{tid}/{tag}", tag = "library", summary = "Delete a tag from a title", params(("tid" = String, Path, description = "Title ID"), ("tag" = String, Path, description = "Tag")), responses((status = 200, description = "Tag deleted")))]
pub async fn delete_tag(
    State(state): State<AppState>,
    Path((title_id, tag)): Path<(String, String)>,
    _admin: crate::auth::AdminOnly,
) -> Json<serde_json::Value> {
    if state.library.load().get_title(&title_id).is_none() {
        return api_failure("Nil assertion failed".to_string());
    }
    match state.storage.delete_tag(&title_id, &tag).await {
        Ok(()) => Json(serde_json::json!({"success": true, "error": null})),
        Err(error) => api_failure(error.to_string()),
    }
}

/// API route: GET /api/download/:tid/:eid
/// Download the original archive file for an entry (used by OPDS clients)
#[utoipa::path(get, path = "/api/download/{tid}/{eid}", tag = "reader", summary = "Download an entry", params(("tid" = String, Path, description = "Title ID"), ("eid" = String, Path, description = "Entry ID")), responses((status = 200, description = "Entry downloaded")))]
pub async fn download_entry(
    State(state): State<AppState>,
    Path((title_id, entry_id)): Path<(String, String)>,
    _username: crate::auth::Username,
) -> axum::response::Response {
    let lib = state.library.load();
    let Some(entry) = lib.get_entry(&title_id, &entry_id) else {
        return (StatusCode::NOT_FOUND, "Nil assertion failed").into_response();
    };
    let file_data = match tokio::fs::read(&entry.path).await {
        Ok(data) => data,
        Err(error) => return (StatusCode::NOT_FOUND, error.to_string()).into_response(),
    };

    let mime_type = match entry.path.extension().and_then(|e| e.to_str()) {
        Some("cbz") | Some("zip") => "application/zip",
        Some("cbr") | Some("rar") => "application/x-rar-compressed",
        _ => "application/octet-stream",
    };
    let filename = entry
        .path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("download");
    let content_disposition = format!("attachment; filename=\"{}\"", filename);

    (
        [
            (header::CONTENT_TYPE, mime_type),
            (header::CONTENT_DISPOSITION, content_disposition.as_str()),
        ],
        file_data,
    )
        .into_response()
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

fn dimensions_response(
    dimensions: Vec<PageDimension>,
    etag: &str,
    cache_control: &str,
) -> axum::response::Response {
    let mut response = success_response(DimensionsResponse { dimensions }).into_response();
    response.headers_mut().insert(
        header::ETAG,
        etag.parse().expect("SHA-1 ETag is a valid header value"),
    );
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        cache_control.parse().expect("static cache-control header"),
    );
    response
}

/// API route: GET /api/dimensions/:tid/:eid
/// Returns the image dimensions of all pages in an entry (used by reader for layout)
#[utoipa::path(get, path = "/api/dimensions/{tid}/{eid}", tag = "reader", summary = "Get entry page dimensions", params(("tid" = String, Path, description = "Title ID"), ("eid" = String, Path, description = "Entry ID")), responses((status = 200, description = "Dimensions returned")))]
pub async fn get_dimensions(
    State(state): State<AppState>,
    Path((title_id, entry_id)): Path<(String, String)>,
    headers: axum::http::HeaderMap,
) -> Result<impl IntoResponse> {
    let lib = state.library.load();

    let Some(title) = lib.get_title(&title_id) else {
        return Ok(api_failure(format!("Title ID `{title_id}` not found")).into_response());
    };
    let Some(entry) = title.entries.iter().find(|entry| entry.id == entry_id) else {
        return Ok(api_failure(format!(
            "Entry ID `{entry_id}` of `{}` not found",
            title.title
        ))
        .into_response());
    };
    let entry_pages = entry.pages;
    let is_directory = entry.path.is_dir();
    let mut etag_source = format!("{}{}", entry.path.display(), entry.mtime);
    if is_directory {
        etag_source.push_str(&humansize::format_size(entry.size_bytes, humansize::BINARY));
    }
    let etag = format!("W/{:x}", {
        use sha1::Digest;
        sha1::Sha1::digest(etag_source.as_bytes())
    });
    let cache_control = if is_directory {
        "no-cache, max-age=86400"
    } else {
        "public, max-age=86400"
    };
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
            return Ok(dimensions_response(dimensions, &etag, cache_control));
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

    Ok(dimensions_response(dimensions, &etag, cache_control))
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
#[utoipa::path(put, path = "/api/progress/{tid}/{page}", tag = "progress", summary = "Update reading progress", params(("tid" = String, Path, description = "Title ID"), ("page" = i32, Path, description = "Page number"), ("eid" = Option<String>, Query, description = "Entry ID; omit to update all entries")), responses((status = 200, description = "Progress updated")))]
pub async fn update_progress(
    State(state): State<AppState>,
    Path((title_id, page)): Path<(String, String)>,
    Query(query): Query<ProgressQuery>,
    crate::auth::Username(username): crate::auth::Username,
) -> Json<serde_json::Value> {
    let lib = state.library.load();
    let Some(title) = lib.get_title(&title_id) else {
        return Json(serde_json::json!({
            "success": false,
            "error": "Nil assertion failed"
        }));
    };
    let page = match page.parse::<i32>() {
        Ok(page) => page,
        Err(_) => {
            return Json(serde_json::json!({
                "success": false,
                "error": format!("Invalid Int32: {page}")
            }));
        }
    };

    if let Some(entry_id) = query.eid {
        let Some(entry) = lib.get_entry(&title_id, &entry_id) else {
            return Json(serde_json::json!({
                "success": false,
                "error": "Nil assertion failed"
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
        let result = if page == 0 {
            title.unread_all(&username).await
        } else {
            title.read_all(&username).await
        };
        if let Err(error) = result {
            return Json(serde_json::json!({
                "success": false,
                "error": error.to_string()
            }));
        }
    }

    lib.invalidate_cache_for_progress(&username).await;
    Json(serde_json::json!({ "success": true }))
}
#[cfg(test)]
mod parity_contract_tests {
    use super::{entry_progress_percentage, order_continue_candidates, page_index};

    #[test]
    fn reader_pages_are_one_based() {
        assert_eq!(page_index(0), None);
        assert_eq!(page_index(1), Some(0));
        assert_eq!(page_index(12), Some(11));
    }

    #[test]
    fn continue_reading_limits_candidates_before_sorting() {
        let mut candidates: Vec<(Option<i64>, usize, f64)> = (0..10)
            .map(|timestamp| (Some(timestamp), timestamp as usize, 0.0))
            .collect();

        order_continue_candidates(&mut candidates);

        assert_eq!(
            candidates
                .iter()
                .map(|(_, candidate_id, _)| *candidate_id)
                .collect::<Vec<_>>(),
            (0..8).rev().map(|id| id as usize).collect::<Vec<_>>()
        );
    }

    #[test]
    fn progress_percentage_preserves_float64_precision() {
        assert_eq!(entry_progress_percentage(1, 3), 1.0_f64 / 3.0);
    }
}
