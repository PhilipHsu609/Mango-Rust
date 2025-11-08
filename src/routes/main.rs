use axum::{
    extract::{Request, State},
    response::Html,
};

use crate::{auth::get_username, AppState};

/// Home page template
const HOME_PAGE: &str = include_str!("../../templates/home.html");

/// GET / - Home page (requires authentication)
pub async fn home(
    State(state): State<AppState>,
    request: Request,
) -> Html<String> {
    // Get username from request extensions (injected by auth middleware)
    let username = get_username(&request).unwrap_or_else(|| "Unknown".to_string());

    // Get library statistics
    let lib = state.library.read().await;
    let stats = lib.stats();
    let titles = lib.get_titles();

    // Build title list HTML
    let mut titles_html = String::new();
    for title in titles {
        titles_html.push_str(&format!(
            r#"<div class="title-card">
                <h3>{}</h3>
                <p>{} entries • {} pages</p>
                <a href="/api/title/{}">View</a>
              </div>"#,
            title.title,
            title.entries.len(),
            title.total_pages(),
            title.id
        ));
    }

    if titles_html.is_empty() {
        titles_html = "<p>No manga found. Add manga files to your library directory.</p>".to_string();
    }

    // Render template with data
    let html = HOME_PAGE
        .replace("{{ username }}", &username)
        .replace("{{ title_count }}", &stats.titles.to_string())
        .replace("{{ entry_count }}", &stats.entries.to_string())
        .replace("{{ page_count }}", &stats.pages.to_string())
        .replace("{{ titles }}", &titles_html);

    Html(html)
}
