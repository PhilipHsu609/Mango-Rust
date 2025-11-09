use axum::{
    extract::{Path, Query, State},
    response::Html,
};
use askama::Template;
use serde::Deserialize;

use crate::{auth::Username, error::{Error, Result}, library::SortMethod, AppState};

/// Query parameters for book page
#[derive(Deserialize)]
pub struct BookParams {
    pub sort: Option<String>,
    pub ascend: Option<String>,
    pub search: Option<String>,
}

/// Entry data for book template
#[derive(serde::Serialize)]
struct EntryData {
    entry_id: String,
    entry_name: String,
    pages: usize,
    progress: String,  // Formatted with 1 decimal place
    saved_page: usize,
    path: String,
}

/// Book page template
#[derive(Template)]
#[template(path = "book.html")]
struct BookTemplate {
    home_active: bool,
    library_active: bool,
    title_id: String,
    title_name: String,
    entry_count: usize,
    sort_title_asc: bool,
    sort_title_desc: bool,
    sort_modified_asc: bool,
    sort_modified_desc: bool,
    sort_progress_asc: bool,
    sort_progress_desc: bool,
    entries: Vec<EntryData>,
}


pub async fn get_book(
    State(state): State<AppState>,
    Path(title_id): Path<String>,
    Query(params): Query<BookParams>,
    Username(username): Username,
) -> Result<Html<String>> {

    // Parse sort method and ascend flag
    let (sort_method, ascending) = SortMethod::from_params(
        params.sort.as_deref(),
        params.ascend.as_deref(),
    );

    // Get title and its entries
    let (title_name, mut entries) = {
        let lib = state.library.read().await;

        // Get the title
        let title = lib
            .get_title(&title_id)
            .ok_or_else(|| Error::NotFound(format!("Title not found: {}", title_id)))?;

        let title_name = title.title.clone();

        // For progress sorting, we need to calculate progress first, then sort
        // For other methods, use the title's built-in sorting
        let all_entries = if matches!(sort_method, SortMethod::Progress) {
            title.get_entries_sorted(SortMethod::Name, true)  // Get name-sorted as base
        } else {
            title.get_entries_sorted(sort_method, ascending)
        };

        // Build entry data
        let mut entries = Vec::new();
        for entry in all_entries {
            // Load progress for this entry using Title's method
            let (progress_percentage, saved_page) = title
                .get_entry_progress(&username, &entry.id)
                .await
                .unwrap_or((0.0, 0));

            // Apply search filter if provided
            if let Some(ref search) = params.search {
                if !entry
                    .title
                    .to_lowercase()
                    .contains(&search.to_lowercase())
                {
                    continue;
                }
            }

            entries.push(EntryData {
                entry_id: entry.id.clone(),
                entry_name: entry.title.clone(),
                pages: entry.pages,
                progress: format!("{:.1}", progress_percentage),
                saved_page,
                path: entry.path.to_string_lossy().to_string(),
            });
        }

        (title_name, entries)
    }; // Lock is released here

    // Sort by progress if requested (after calculating progress)
    if matches!(sort_method, SortMethod::Progress) {
        entries.sort_by(|a, b| {
            let a_progress: f32 = a.progress.parse().unwrap_or(0.0);
            let b_progress: f32 = b.progress.parse().unwrap_or(0.0);
            if ascending {
                a_progress.partial_cmp(&b_progress).unwrap_or(std::cmp::Ordering::Equal)
            } else {
                b_progress.partial_cmp(&a_progress).unwrap_or(std::cmp::Ordering::Equal)
            }
        });
    }

    let entry_count = entries.len();

    // Determine which sort option is selected
    let sort_title_asc = matches!(sort_method, SortMethod::Name) && ascending;
    let sort_title_desc = matches!(sort_method, SortMethod::Name) && !ascending;
    let sort_modified_asc = matches!(sort_method, SortMethod::TimeModified) && ascending;
    let sort_modified_desc = matches!(sort_method, SortMethod::TimeModified) && !ascending;
    let sort_progress_asc = matches!(sort_method, SortMethod::Progress) && ascending;
    let sort_progress_desc = matches!(sort_method, SortMethod::Progress) && !ascending;

    let template = BookTemplate {
        home_active: false,
        library_active: true,
        title_id,
        title_name,
        entry_count,
        sort_title_asc,
        sort_title_desc,
        sort_modified_asc,
        sort_modified_desc,
        sort_progress_asc,
        sort_progress_desc,
        entries,
    };

    Ok(Html(template.render().map_err(|e| {
        Error::Internal(format!("Template render error: {}", e))
    })?))
}
