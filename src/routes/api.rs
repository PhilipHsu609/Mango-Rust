use axum::{
    extract::{Path, Query, State},
    http::{header, StatusCode},
    response::IntoResponse,
    Json,
};
use serde::Serialize;

use crate::{
    error::{Error, Result},
    library::{Entry, SortMethod},
    util::SortParams,
    AppState,
};

/// API route: GET /api/library?sort=title|modified|auto&ascend=0|1
/// Returns list of all manga titles with optional sorting
pub async fn get_library(
    State(state): State<AppState>,
    Query(params): Query<SortParams>,
) -> Result<impl IntoResponse> {
    let lib = state.library.read().await;
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
    let lib = state.library.read().await;

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
    let lib = state.library.read().await;

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
    let lib = state.library.read().await;
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
    let lib = state.library.read().await;

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
            if let Ok(Some((data, mime, _size))) = entry.generate_thumbnail(db).await {
                return Ok(([(header::CONTENT_TYPE, mime.as_str())], data).into_response());
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
    use crate::library::progress::TitleInfo;

    let lib = state.library.read().await;
    let mut entries_with_progress = Vec::new();

    // Collect all entries with last_read timestamps
    for title in lib.get_titles_sorted(crate::library::SortMethod::Name, true) {
        let info = TitleInfo::load(&title.path).await?;

        for entry in &title.entries {
            if let Some(last_read) = info.get_last_read(&username, &entry.id) {
                let progress = info.get_progress(&username, &entry.id).unwrap_or(0);
                let percentage = if entry.pages > 0 {
                    (progress as f32 / entry.pages as f32) * 100.0
                } else {
                    0.0
                };

                entries_with_progress.push(ContinueReadingEntry {
                    title_id: title.id.clone(),
                    title_name: title.title.clone(),
                    entry_id: entry.id.clone(),
                    entry_name: entry.title.clone(),
                    pages: entry.pages,
                    progress,
                    percentage: format!("{:.1}", percentage),
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
    let lib = state.library.read().await;
    let mut unread_titles = Vec::new();

    for title in lib.get_titles_sorted(crate::library::SortMethod::Name, true) {
        let progress_pct = title.get_title_progress(&username).await?;

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

/// API route: GET /api/library/recently_added
/// Returns recently added entries (within last month) with grouping by title
pub async fn recently_added(
    State(state): State<AppState>,
    crate::auth::Username(username): crate::auth::Username,
) -> Result<impl IntoResponse> {
    use crate::library::progress::TitleInfo;

    let lib = state.library.read().await;
    let mut entries_with_dates = Vec::new();
    let one_month_ago = chrono::Utc::now().timestamp() - (30 * 24 * 60 * 60);

    // Collect all entries with date_added within last month
    for title in lib.get_titles_sorted(crate::library::SortMethod::Name, true) {
        let info = TitleInfo::load(&title.path).await?;

        for entry in &title.entries {
            if let Some(date_added) = info.get_date_added(&entry.id) {
                if date_added > one_month_ago {
                    let progress = info.get_progress(&username, &entry.id).unwrap_or(0);
                    let percentage = if entry.pages > 0 {
                        (progress as f32 / entry.pages as f32) * 100.0
                    } else {
                        0.0
                    };

                    entries_with_dates.push((
                        title.id.clone(),
                        title.title.clone(),
                        entry.id.clone(),
                        entry.title.clone(),
                        entry.pages,
                        percentage,
                        date_added,
                    ));
                }
            }
        }
    }

    // Sort by date_added (most recent first)
    entries_with_dates.sort_by(|a, b| b.6.cmp(&a.6));

    // Group consecutive entries from same title added on same day
    let mut result: Vec<RecentlyAddedEntry> = Vec::new();
    for (title_id, title_name, entry_id, entry_name, pages, percentage, date_added) in entries_with_dates {
        if result.len() >= 8 {
            break;
        }

        // Check if we can group with last entry
        let should_group = if let Some(last) = result.last() {
            last.title_id == title_id && (date_added - last.date_added).abs() < (24 * 60 * 60)
        } else {
            false
        };

        if should_group {
            // Group with previous entry
            if let Some(last) = result.last_mut() {
                last.grouped_count += 1;
                last.percentage = String::new(); // Hide percentage for grouped items
            }
        } else {
            result.push(RecentlyAddedEntry {
                title_id,
                title_name,
                entry_id,
                entry_name,
                pages,
                percentage: format!("{:.1}", percentage),
                grouped_count: 1,
                date_added,
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
    progress: usize,
    percentage: String,
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
    percentage: String,
    grouped_count: usize,
    date_added: i64,
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
