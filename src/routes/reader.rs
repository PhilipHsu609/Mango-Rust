use askama::Template;
use axum::{
    extract::{Path, State},
    response::{Html, Redirect},
};

use crate::{
    auth::Username,
    error::{Error, Result},
    util::render_error,
    AppState,
};

/// Entry option data for reader template
#[derive(serde::Serialize)]
struct EntryOption {
    id: String,
    name: String,
}

/// Reader page template
#[derive(Template)]
#[template(path = "reader.html")]
struct ReaderTemplate {
    title_id: String,
    entry_id: String,
    entry_name: String,
    entry_path: String,
    current_page: usize,
    total_pages: usize,
    entries: Vec<EntryOption>,
    prev_entry_url: Option<String>,
    next_entry_url: Option<String>,
    exit_url: String,
}

/// GET /reader/{title_id}/{entry_id}/{page} - Display reader for an entry page
/// Returns: HTML page with reader interface, entry content, and navigation
pub async fn reader(
    State(state): State<AppState>,
    Path((title_id, entry_id, page)): Path<(String, String, usize)>,
    Username(_username): Username,
) -> Result<Html<String>> {
    // Get library read lock
    let lib = state.library.load();

    // Find the title
    let title = lib
        .get_title(&title_id)
        .ok_or_else(|| Error::NotFound(format!("Title not found: {}", title_id)))?;

    // Find the entry within the title
    let entry = lib
        .get_entry(&title_id, &entry_id)
        .ok_or_else(|| Error::NotFound(format!("Entry not found: {}", entry_id)))?;

    let total_pages = entry.pages;

    // Validate page number (1-indexed)
    if page < 1 || page > total_pages {
        return Err(Error::NotFound(format!(
            "Page {} not found (valid: 1-{})",
            page, total_pages
        )));
    }

    // Get all entries in this title for jump functionality
    let entries: Vec<EntryOption> = title
        .entries
        .iter()
        .map(|e| EntryOption {
            id: e.id.clone(),
            name: e.title.clone(),
        })
        .collect();

    // Find current entry index to determine prev/next entry
    let current_entry_idx = title.entries.iter().position(|e| e.id == entry_id);

    let (prev_entry_url, next_entry_url) = if let Some(idx) = current_entry_idx {
        let prev_url = if idx > 0 {
            let prev_entry = &title.entries[idx - 1];
            Some(format!("/reader/{}/{}/1", title_id, prev_entry.id))
        } else {
            None
        };

        let next_url = if idx < title.entries.len() - 1 {
            let next_entry = &title.entries[idx + 1];
            Some(format!("/reader/{}/{}/1", title_id, next_entry.id))
        } else {
            None
        };

        (prev_url, next_url)
    } else {
        (None, None)
    };

    let template = ReaderTemplate {
        title_id,
        entry_id,
        entry_name: entry.title.clone(),
        entry_path: entry.path.display().to_string(),
        current_page: page,
        total_pages,
        entries,
        prev_entry_url,
        next_entry_url,
        exit_url: format!("/book/{}", title.id),
    };

    Ok(Html(template.render().map_err(render_error)?))
}

/// GET /reader/{title_id}/{entry_id} - Continue reading from saved progress
/// Redirects to the reader page at the user's saved progress, or page 1 if finished/not started
pub async fn reader_continue(
    State(state): State<AppState>,
    Path((title_id, entry_id)): Path<(String, String)>,
    Username(username): Username,
) -> Result<Redirect> {
    // Get library read lock
    let lib = state.library.load();

    // Find the title
    let title = lib
        .get_title(&title_id)
        .ok_or_else(|| Error::NotFound(format!("Title not found: {}", title_id)))?;

    // Find the entry within the title
    let entry = lib
        .get_entry(&title_id, &entry_id)
        .ok_or_else(|| Error::NotFound(format!("Entry not found: {}", entry_id)))?;

    let total_pages = entry.pages;

    // Load the user's progress
    let progress_page = match title.load_entry_progress(&username, &entry_id).await {
        Ok(page) => page,
        Err(e) => {
            tracing::error!(
                "Failed to load progress for user '{}' entry '{}': {}. Starting from beginning.",
                username,
                entry_id,
                e
            );
            0
        }
    };

    // If not started (0) or finished (>= total_pages), start from page 1
    // Otherwise, continue from saved progress (clamped to at least 1)
    let page = if progress_page == 0 || progress_page >= total_pages as i32 {
        1
    } else {
        progress_page.max(1)
    };

    Ok(Redirect::to(&format!("/reader/{}/{}/{}", title_id, entry_id, page)))
}
