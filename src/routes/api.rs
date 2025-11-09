use axum::{
    extract::{Path, Query, State},
    http::{header, StatusCode},
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};

use crate::{error::Result, library::SortMethod, AppState};

/// API route: GET /api/library?sort=title|modified|auto&ascend=0|1
/// Returns list of all manga titles with optional sorting
pub async fn get_library(
    State(state): State<AppState>,
    Query(params): Query<SortParams>,
) -> Result<impl IntoResponse> {
    let lib = state.library.read().await;
    let (sort_method, ascending) = SortMethod::from_params(
        params.sort.as_deref(),
        params.ascend.as_deref(),
    );
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

    let (sort_method, ascending) = SortMethod::from_params(
        params.sort.as_deref(),
        params.ascend.as_deref(),
    );
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

    let entry = lib
        .get_entry(&title_id, &entry_id)
        .ok_or_else(|| crate::error::Error::NotFound(format!("Entry not found: {}/{}", title_id, entry_id)))?;

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

// Request and response types

/// Query parameters for sorting
#[derive(Deserialize)]
pub struct SortParams {
    /// Optional sort method (title, modified, auto)
    pub sort: Option<String>,
    /// Optional ascend flag (1 for ascending, 0 for descending)
    pub ascend: Option<String>,
}

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
