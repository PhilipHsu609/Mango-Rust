use axum::{
    extract::{Path, Query, Request, State},
    response::Html,
};
use askama::Template;
use serde::Deserialize;

use crate::{auth::get_username, error::{Error, Result}, library::SortMethod, AppState};

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
    entries: Vec<EntryData>,
}


pub async fn get_book(
    State(state): State<AppState>,
    Path(title_id): Path<String>,
    Query(params): Query<BookParams>,
    request: Request,
) -> Result<Html<String>> {
    // Get username from request extensions (injected by auth middleware)
    let username = get_username(&request).unwrap_or_else(|| "Unknown".to_string());

    // Parse sort method and ascend flag
    let (sort_method, ascending) = SortMethod::from_params(
        params.sort.as_deref(),
        params.ascend.as_deref(),
    );

    // Get title and its entries
    let (title_name, entries) = {
        let lib = state.library.read().await;

        // Get the title
        let title = lib
            .get_title(&title_id)
            .ok_or_else(|| Error::NotFound(format!("Title not found: {}", title_id)))?;

        let title_name = title.title.clone();

        // Get all entries, sorted
        let all_entries = title.get_entries_sorted(sort_method, ascending);

        // Build entry data
        let mut entries = Vec::new();
        for entry in all_entries {
            // Try to load progress for this entry from info.json
            let (progress_percentage, saved_page) = {
                let info_path = title.path.join("info.json");
                if info_path.exists() {
                    if let Ok(content) = tokio::fs::read_to_string(&info_path).await {
                        if let Ok(info) = serde_json::from_str::<serde_json::Value>(&content) {
                            if let Some(page) = info
                                .get("progress")
                                .and_then(|p| p.get(&username))
                                .and_then(|u| u.get(&entry.id))
                                .and_then(|page| page.as_u64())
                            {
                                let page = page as usize;
                                let percentage = (page as f32 / entry.pages as f32) * 100.0;
                                (percentage, page)
                            } else {
                                (0.0, 0)
                            }
                        } else {
                            (0.0, 0)
                        }
                    } else {
                        (0.0, 0)
                    }
                } else {
                    (0.0, 0)
                }
            };

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

    let entry_count = entries.len();

    // Determine which sort option is selected
    let sort_title_asc = matches!(sort_method, SortMethod::Name) && ascending;
    let sort_title_desc = matches!(sort_method, SortMethod::Name) && !ascending;
    let sort_modified_asc = matches!(sort_method, SortMethod::TimeModified) && ascending;
    let sort_modified_desc = matches!(sort_method, SortMethod::TimeModified) && !ascending;

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
        entries,
    };

    Ok(Html(template.render().map_err(|e| {
        Error::Internal(format!("Template render error: {}", e))
    })?))
}
