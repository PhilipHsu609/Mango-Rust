use axum::{
    extract::{Query, State},
    response::Html,
};
use askama::Template;
use serde::Deserialize;

use crate::{auth::Username, error::Result, library::SortMethod, AppState, error::Error};
use super::{HasProgress, sort_by_progress};

/// Query parameters for sorting
#[derive(Deserialize)]
pub struct SortParams {
    pub sort: Option<String>,
    pub ascend: Option<String>,
}

/// Title data for library template
#[derive(serde::Serialize)]
struct TitleData {
    id: String,
    name: String,
    entry_count: usize,
    progress: String, // Formatted with 1 decimal place
    first_entry_id: Option<String>, // For cover thumbnail URL
}

impl HasProgress for TitleData {
    fn progress(&self) -> &str {
        &self.progress
    }
}

/// Library page template
#[derive(Template)]
#[template(path = "library.html")]
struct LibraryTemplate {
    home_active: bool,
    library_active: bool,
    title_count: usize,
    sort_name_asc: bool,
    sort_name_desc: bool,
    sort_time_asc: bool,
    sort_time_desc: bool,
    sort_progress_asc: bool,
    sort_progress_desc: bool,
    titles: Vec<TitleData>,
}


/// Home page template
#[derive(Template)]
#[template(path = "home.html")]
struct HomeTemplate {
    home_active: bool,
    library_active: bool,
}

/// GET / - Home page with Continue Reading, Start Reading, Recently Added (requires authentication)
pub async fn home(
    State(_state): State<AppState>,
    Username(_username): Username,
) -> Result<Html<String>> {

    // TODO: Implement Continue Reading, Start Reading, Recently Added logic
    let template = HomeTemplate {
        home_active: true,
        library_active: false,
    };
    
    Ok(Html(template.render().map_err(|e| {
        Error::Internal(format!("Template render error: {}", e))
    })?))
}

pub async fn library(
    State(state): State<AppState>,
    Query(params): Query<SortParams>,
    Username(username): Username,
) -> Result<Html<String>> {

    // Parse sort method and ascend flag
    let (sort_method, ascending) = SortMethod::from_params(
        params.sort.as_deref(),
        params.ascend.as_deref(),
    );

    // Get library statistics and title data
    let (title_count, mut titles) = {
        let lib = state.library.read().await;
        let stats = lib.stats();

        // For progress sorting, we need to calculate progress first, then sort
        // For other methods, use the library's built-in sorting
        let sorted_titles = if matches!(sort_method, SortMethod::Progress) {
            lib.get_titles_sorted(SortMethod::Name, true)  // Get unsorted (well, name-sorted as base)
        } else {
            lib.get_titles_sorted(sort_method, ascending)
        };

        // Calculate progress for each title
        let mut titles = Vec::new();
        for t in sorted_titles {
            let progress_pct = t.get_title_progress(&username).await.unwrap_or(0.0);
            titles.push(TitleData {
                id: t.id.clone(),
                name: t.title.clone(),
                entry_count: t.entries.len(),
                progress: format!("{:.1}", progress_pct),
                first_entry_id: t.entries.first().map(|e| e.id.clone()),
            });
        }

        (stats.titles, titles)
    }; // Lock is released here

    // Sort by progress if requested (after calculating progress)
    if matches!(sort_method, SortMethod::Progress) {
        sort_by_progress(&mut titles, ascending);
    }

    // Determine which sort option is selected
    let sort_name_asc = matches!(sort_method, SortMethod::Name) && ascending;
    let sort_name_desc = matches!(sort_method, SortMethod::Name) && !ascending;
    let sort_time_asc = matches!(sort_method, SortMethod::TimeModified) && ascending;
    let sort_time_desc = matches!(sort_method, SortMethod::TimeModified) && !ascending;
    let sort_progress_asc = matches!(sort_method, SortMethod::Progress) && ascending;
    let sort_progress_desc = matches!(sort_method, SortMethod::Progress) && !ascending;

    let template = LibraryTemplate {
        home_active: false,
        library_active: true,
        title_count,
        sort_name_asc,
        sort_name_desc,
        sort_time_asc,
        sort_time_desc,
        sort_progress_asc,
        sort_progress_desc,
        titles,
    };

    Ok(Html(template.render().map_err(|e| {
        Error::Internal(format!("Template render error: {}", e))
    })?))
}
