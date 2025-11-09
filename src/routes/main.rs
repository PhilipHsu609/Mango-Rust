use axum::{
    extract::{Query, Request, State},
    response::Html,
};
use serde::Deserialize;

use crate::{auth::get_username, library::SortMethod, AppState};

/// Query parameters for sorting
#[derive(Deserialize)]
pub struct SortParams {
    pub sort: Option<String>,
    pub ascend: Option<String>,
}

/// Layout template
const LAYOUT: &str = include_str!("../../templates/layout.html");
/// Home page content
const HOME_CONTENT: &str = include_str!("../../templates/home_content.html");
/// Home page styles
const HOME_STYLES: &str = include_str!("../../templates/home_styles.css");
/// Library page content
const LIBRARY_CONTENT: &str = include_str!("../../templates/library_content.html");
/// Library page styles
const LIBRARY_STYLES: &str = include_str!("../../templates/library_styles.css");
/// Library page scripts
const LIBRARY_SCRIPTS: &str = include_str!("../../templates/library_scripts.js");

/// GET / - Home page with Continue Reading, Start Reading, Recently Added (requires authentication)
pub async fn home(
    State(_state): State<AppState>,
    request: Request,
) -> Html<String> {
    // Get username from request extensions (injected by auth middleware)
    let _username = get_username(&request).unwrap_or_else(|| "Unknown".to_string());

    // TODO: Implement Continue Reading, Start Reading, Recently Added logic
    // For now, show empty states

    let continue_reading = r#"<div class="empty-state">
        <div class="empty-state-icon">📚</div>
        <p>No manga in progress yet. Start reading from the Library!</p>
    </div>"#;

    let start_reading = r#"<div class="empty-state">
        <div class="empty-state-icon">✨</div>
        <p>All caught up! Check the Library for new titles.</p>
    </div>"#;

    let recently_added = r#"<div class="empty-state">
        <div class="empty-state-icon">🆕</div>
        <p>No recently added titles.</p>
    </div>"#;

    // Render page content
    let content = HOME_CONTENT
        .replace("{{ continue_reading }}", continue_reading)
        .replace("{{ start_reading }}", start_reading)
        .replace("{{ recently_added }}", recently_added);

    // Render with layout
    let html = LAYOUT
        .replace("{{ page_title }}", "Home")
        .replace("{{ home_active }}", " class=\"uk-active\"")
        .replace("{{ library_active }}", "")
        .replace("{{ page_styles }}", HOME_STYLES)
        .replace("{{ content }}", &content)
        .replace("{{ page_scripts }}", "");

    Html(html)
}

/// GET /library - Library page with all titles (requires authentication)
pub async fn library(
    State(state): State<AppState>,
    Query(params): Query<SortParams>,
    request: Request,
) -> Html<String> {
    // Get username from request extensions (injected by auth middleware)
    let username = get_username(&request).unwrap_or_else(|| "Unknown".to_string());

    // Parse sort method and ascend flag
    let (sort_method, ascending) = SortMethod::from_params(
        params.sort.as_deref(),
        params.ascend.as_deref(),
    );

    // Get library statistics and title data
    let (stats, title_data) = {
        let lib = state.library.read().await;
        let stats = lib.stats();
        let title_data: Vec<_> = lib.get_titles_sorted(sort_method, ascending).iter().map(|t| {
            (t.id.clone(), t.title.clone(), t.entries.len(), t.total_pages(),
             t.entries.first().map(|e| e.id.clone()), t.path.clone())
        }).collect();
        (stats, title_data)
    }; // Lock is released here

    // Build title list HTML
    let mut titles_html = String::new();
    for (title_id, title_name, entry_count, _pages, _first_entry_id, _title_path) in title_data {
        // Link to book page to show all entries
        let book_link = format!("/book/{}", title_id);

        titles_html.push_str(&format!(
            r#"<a href="{}" class="title-card">
                <div class="title-thumbnail">
                    <div class="placeholder-icon">📖</div>
                    <div class="progress-badge">0.0%</div>
                </div>
                <div class="title-info">
                    <div class="title-name">{}</div>
                    <div class="title-stats">{} {}</div>
                </div>
              </a>"#,
            book_link,
            title_name,
            entry_count,
            if entry_count == 1 { "entry" } else { "entries" }
        ));
    }

    if titles_html.is_empty() {
        titles_html = "<p>No manga found. Add manga files to your library directory.</p>".to_string();
    }

    // Determine which sort option is selected
    let sort_name_asc_selected = if matches!(sort_method, SortMethod::Name) && ascending { "selected" } else { "" };
    let sort_name_desc_selected = if matches!(sort_method, SortMethod::Name) && !ascending { "selected" } else { "" };
    let sort_time_asc_selected = if matches!(sort_method, SortMethod::TimeModified) && ascending { "selected" } else { "" };
    let sort_time_desc_selected = if matches!(sort_method, SortMethod::TimeModified) && !ascending { "selected" } else { "" };

    // Render page content
    let content = LIBRARY_CONTENT
        .replace("{{ title_count }}", &stats.titles.to_string())
        .replace("{{ sort_name_asc_selected }}", sort_name_asc_selected)
        .replace("{{ sort_name_desc_selected }}", sort_name_desc_selected)
        .replace("{{ sort_time_asc_selected }}", sort_time_asc_selected)
        .replace("{{ sort_time_desc_selected }}", sort_time_desc_selected)
        .replace("{{ titles }}", &titles_html);

    // Render with layout
    let html = LAYOUT
        .replace("{{ page_title }}", "Library")
        .replace("{{ home_active }}", "")
        .replace("{{ library_active }}", " class=\"uk-active\"")
        .replace("{{ page_styles }}", LIBRARY_STYLES)
        .replace("{{ content }}", &content)
        .replace("{{ page_scripts }}", &format!("<script>{}</script>", LIBRARY_SCRIPTS));

    Html(html)
}
