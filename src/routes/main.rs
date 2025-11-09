use axum::{
    extract::{Query, Request, State},
    response::Html,
};
use askama::Template;
use serde::Deserialize;

use crate::{auth::get_username, error::Result, library::SortMethod, AppState, error::Error};

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
    request: Request,
) -> Result<Html<String>> {
    // Get username from request extensions (injected by auth middleware)
    let _username = get_username(&request).unwrap_or_else(|| "Unknown".to_string());

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
    request: Request,
) -> Result<Html<String>> {
    // Get username from request extensions (injected by auth middleware)
    let _username = get_username(&request).unwrap_or_else(|| "Unknown".to_string());

    // Parse sort method and ascend flag
    let (sort_method, ascending) = SortMethod::from_params(
        params.sort.as_deref(),
        params.ascend.as_deref(),
    );

    // Get library statistics and title data
    let (title_count, titles) = {
        let lib = state.library.read().await;
        let stats = lib.stats();
        let titles: Vec<TitleData> = lib.get_titles_sorted(sort_method, ascending)
            .iter()
            .map(|t| TitleData {
                id: t.id.clone(),
                name: t.title.clone(),
                entry_count: t.entries.len(),
            })
            .collect();
        (stats.titles, titles)
    }; // Lock is released here

    // Determine which sort option is selected
    let sort_name_asc = matches!(sort_method, SortMethod::Name) && ascending;
    let sort_name_desc = matches!(sort_method, SortMethod::Name) && !ascending;
    let sort_time_asc = matches!(sort_method, SortMethod::TimeModified) && ascending;
    let sort_time_desc = matches!(sort_method, SortMethod::TimeModified) && !ascending;

    let template = LibraryTemplate {
        home_active: false,
        library_active: true,
        title_count,
        sort_name_asc,
        sort_name_desc,
        sort_time_asc,
        sort_time_desc,
        titles,
    };

    Ok(Html(template.render().map_err(|e| {
        Error::Internal(format!("Template render error: {}", e))
    })?))
}
