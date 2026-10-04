use askama::Template;
use axum::{
    extract::{Path, State},
    response::{Html, IntoResponse, Redirect, Response},
};

use crate::{
    auth::Username,
    error::{Error, Result},
    library::{
        ordering::{sort_entries, EntryOrdering, SortOptions},
        Entry, SortMethod, Title, TitleInfo,
    },
    routes::presentation::render_error,
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

#[derive(Template)]
#[template(path = "reader-error.html")]
struct ReaderErrorTemplate {
    nav: crate::routes::presentation::NavigationState,
    entry_path: String,
    err_msg: String,
    next_entry_url: Option<String>,
    exit_url: String,
}

async fn ordered_entries<'a>(
    state: &AppState,
    title: &'a Title,
    info: &TitleInfo,
    username: &str,
) -> Result<Vec<(&'a Entry, String)>> {
    let (sort_method, ascending) = info
        .get_sort_by(username)
        .map(|(method, ascending)| (SortMethod::parse(&method), ascending))
        .unwrap_or((SortMethod::Auto, true));
    let mut ordered_entries = Vec::with_capacity(title.entries.len());
    for item in &title.entries {
        let sort_title = state
            .storage
            .get_entry_sort_title(&item.id)
            .await?
            .unwrap_or_else(|| item.title.clone());
        ordered_entries.push((item, sort_title));
    }
    sort_entries(
        &mut ordered_entries,
        info,
        username,
        SortOptions {
            method: sort_method,
            ascending,
        },
        EntryOrdering::Reader,
    );
    Ok(ordered_entries)
}

/// GET /reader/{title_id}/{entry_id}/{page} - Display reader for an entry page
/// Returns: HTML page with reader interface, entry content, and navigation
pub async fn reader(
    State(state): State<AppState>,
    Path((title_id, entry_id, page)): Path<(String, String, usize)>,
    Username(username): Username,
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

    let info = lib.metadata().read(&title.path).await?;
    let ordered_entries = ordered_entries(&state, title, &info, &username).await?;

    let entries: Vec<EntryOption> = ordered_entries
        .iter()
        .map(|(item, _)| EntryOption {
            id: item.id.clone(),
            name: info
                .entry_display_name
                .get(&item.title)
                .filter(|name| !name.is_empty())
                .cloned()
                .unwrap_or_else(|| item.title.clone()),
        })
        .collect();
    let current_entry_idx = ordered_entries
        .iter()
        .position(|(item, _)| item.id == entry_id);
    let (prev_entry_url, next_entry_url) = if let Some(index) = current_entry_idx {
        let previous = index
            .checked_sub(1)
            .and_then(|index| ordered_entries.get(index));
        let next = ordered_entries.get(index + 1);
        (
            previous.map(|(entry, _)| format!("/reader/{}/{}", title_id, entry.id)),
            next.map(|(entry, _)| format!("/reader/{}/{}", title_id, entry.id)),
        )
    } else {
        (None, None)
    };

    let template = ReaderTemplate {
        title_id,
        entry_id,
        entry_name: info
            .entry_display_name
            .get(&entry.title)
            .filter(|name| !name.is_empty())
            .cloned()
            .unwrap_or_else(|| entry.title.clone()),
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
/// Displays an archive error instead of redirecting when the entry cannot be read.
pub async fn reader_continue(
    State(state): State<AppState>,
    Path((title_id, entry_id)): Path<(String, String)>,
    Username(username): Username,
) -> Result<Response> {
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

    if let Some(err_msg) = &entry.err_msg {
        let info = lib.metadata().read(&title.path).await?;
        let ordered = ordered_entries(&state, title, &info, &username).await?;
        let next_entry_url = ordered
            .iter()
            .position(|(item, _)| item.id == entry_id)
            .and_then(|index| ordered.get(index + 1))
            .map(|(item, _)| format!("/reader/{}/{}", title_id, item.id));
        let template = ReaderErrorTemplate {
            nav: crate::routes::presentation::NavigationState {
                home_active: false,
                library_active: false,
                tags_active: false,
                admin_active: false,
                is_admin: state.storage.is_admin(&username).await?,
            },
            entry_path: entry.path.display().to_string(),
            err_msg: err_msg.clone(),
            next_entry_url,
            exit_url: format!("/book/{}", title.id),
        };
        return Ok(Html(template.render().map_err(render_error)?).into_response());
    }

    let total_pages = entry.pages;

    // Load the user's progress
    let progress_page = match lib.metadata().read(&title.path).await {
        Ok(info) => info.get_progress(&username, &entry.title).unwrap_or(0),
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

    Ok(Redirect::to(&format!("/reader/{}/{}/{}", title_id, entry_id, page)).into_response())
}
