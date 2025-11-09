use axum::{
    extract::{Path, State},
    response::Html,
};
use askama::Template;

use crate::{auth::Username, AppState, error::{Error, Result}};

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


pub async fn reader(
    State(state): State<AppState>,
    Path((title_id, entry_id, page)): Path<(String, String, usize)>,
    Username(_username): Username,
) -> Result<Html<String>> {

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

    // Get all entries in this title for jump functionality
    let entries: Vec<EntryOption> = title.entries
        .iter()
        .map(|e| EntryOption {
            id: e.id.clone(),
            name: e.title.clone(),
        })
        .collect();

    // Find current entry index to determine prev/next entry
    let current_entry_idx = title.entries.iter()
        .position(|e| e.id == entry_id);

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

    Ok(Html(template.render().map_err(|e| {
        Error::Internal(format!("Template render error: {}", e))
    })?))
}
