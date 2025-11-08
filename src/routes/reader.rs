use axum::{
    extract::{Path, State},
    response::{Html, IntoResponse, Response},
    http::StatusCode,
};
use askama::Template;

use crate::{AppState, error::{Error, Result}};

#[derive(Template)]
#[template(path = "reader.html")]
struct ReaderTemplate {
    username: String,
    title_id: String,
    entry_id: String,
    title_name: String,
    entry_name: String,
    current_page: usize,
    total_pages: usize,
    prev_page: Option<usize>,
    next_page: Option<usize>,
}

/// Reader page - displays manga pages with navigation
pub async fn reader(
    State(state): State<AppState>,
    Path((title_id, entry_id, page)): Path<(String, String, usize)>,
) -> Result<impl IntoResponse> {
    // Get username from session (middleware ensures we're authenticated)
    let username = "admin".to_string(); // TODO: Extract from session

    // Get library read lock
    let lib = state.library.read().await;

    // Find the title
    let title = lib.get_title(&title_id)
        .ok_or_else(|| Error::NotFound(format!("Title not found: {}", title_id)))?;

    // Find the entry within the title
    let entry = lib.get_entry(&title_id, &entry_id)
        .ok_or_else(|| Error::NotFound(format!("Entry not found: {}", entry_id)))?;

    let total_pages = entry.pages;

    // Validate page number (1-indexed)
    if page < 1 || page > total_pages {
        return Err(Error::NotFound(format!(
            "Page {} not found (valid: 1-{})",
            page, total_pages
        )));
    }

    // Calculate prev/next pages
    let prev_page = if page > 1 { Some(page - 1) } else { None };
    let next_page = if page < total_pages { Some(page + 1) } else { None };

    let template = ReaderTemplate {
        username,
        title_id: title_id.clone(),
        entry_id: entry_id.clone(),
        title_name: title.title.clone(),
        entry_name: entry.title.clone(),
        current_page: page,
        total_pages,
        prev_page,
        next_page,
    };

    Ok(Html(template.render().map_err(|e| {
        Error::Internal(format!("Template render error: {}", e))
    })?))
}
