use axum::{
    extract::{rejection::JsonRejection, Path, Query, State},
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};

use super::{
    catalog::{
        mango_entry_response, mango_title_response, mango_title_summary, title_parent_summaries,
        MangoEntry, MangoTitleResponse, MangoTitleSummary, TitleResponseOptions,
    },
    success_response,
};
use crate::library::reading::{
    can_start_reading, continue_entry, entry_progress_fraction, group_recent_entries,
    order_continue_candidates, title_progress_fraction, RecentEntry, StartReadingProfile,
    RECENT_ITEMS_LIMIT,
};
use crate::{error::Result, AppState};

/// API route: GET /api/library/continue_reading
/// Returns the last 8 entries the user has read, sorted by last_read timestamp
#[utoipa::path(get, path = "/api/library/continue_reading", tag = "library", summary = "Get continue-reading entries", responses((status = 200, description = "Entries returned")))]
pub async fn continue_reading(
    State(state): State<AppState>,
    crate::auth::Username(username): crate::auth::Username,
) -> axum::response::Response {
    let lib = state.library.load_full();
    let cache = lib.metadata();
    let mut entries_with_progress = Vec::new();

    for title in lib.all_titles() {
        let info = cache.cached(&title.path).unwrap_or_default();
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
            continue_entry(title, &username, &info, &sort_title_overrides)
        {
            let last_read = info
                .get_last_read(&username, &entry.title)
                .or_else(|| previous.and_then(|entry| info.get_last_read(&username, &entry.title)));
            let progress = info.get_progress(&username, &entry.title).unwrap_or(0);
            let percentage = entry_progress_fraction(progress, entry.pages);
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
    let cache = lib.metadata();
    let mut unread_titles = Vec::new();

    for title in lib.get_titles_sorted(crate::library::SortMethod::Name, true) {
        if can_start_reading(
            title,
            title_progress_fraction(title, cache, &username),
            StartReadingProfile::Catalog,
        ) {
            unread_titles.push(title);
        }
    }

    use rand::seq::SliceRandom;
    unread_titles.shuffle(&mut rand::thread_rng());
    unread_titles.truncate(8);

    let mut titles = Vec::with_capacity(unread_titles.len());
    for title in unread_titles {
        let info = cache.cached(&title.path).unwrap_or_default();
        match mango_title_response(
            &state,
            title,
            &info,
            cache,
            &username,
            title_parent_summaries(&lib, title),
            TitleResponseOptions {
                depth: 1,
                include_percentages: false,
                slim: false,
                sort_context: None,
            },
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
    let cache = lib.metadata();
    let mut entries_with_dates = Vec::new();
    let one_month_ago = chrono::Utc::now()
        .checked_sub_months(chrono::Months::new(1))
        .expect("current date can be shifted back one month")
        .timestamp();

    for title in lib.all_titles() {
        let info = cache.cached(&title.path).unwrap_or_default();
        for entry in &title.entries {
            if let Some(date_added) = info.get_date_added(&entry.title) {
                if date_added > one_month_ago {
                    let progress = info.get_progress(&username, &entry.title).unwrap_or(0);
                    entries_with_dates.push(RecentEntry {
                        title_id: title.id.clone(),
                        date_added,
                        percentage: entry_progress_fraction(progress, entry.pages),
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
        let info = cache.cached(&title.path).unwrap_or_default();
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
            .metadata()
            .save_progress(&title.path, &username, &entry.title, page)
            .await
        {
            return Json(serde_json::json!({
                "success": false,
                "error": error.to_string()
            }));
        }
    } else {
        let result = if page == 0 {
            lib.metadata().unread_all(title, &username).await
        } else {
            lib.metadata().read_all(title, &username).await
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
        .metadata()
        .save_bulk_progress(&title.path, &username, &updates)
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
