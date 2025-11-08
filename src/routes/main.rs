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

    // Get library statistics and title data
    let (stats, title_data) = {
        let lib = state.library.read().await;
        let stats = lib.stats();
        let title_data: Vec<_> = lib.get_titles().iter().map(|t| {
            (t.id.clone(), t.title.clone(), t.entries.len(), t.total_pages(),
             t.entries.first().map(|e| e.id.clone()), t.path.clone())
        }).collect();
        (stats, title_data)
    }; // Lock is released here

    // Build title list HTML
    let mut titles_html = String::new();
    for (title_id, title_name, entry_count, pages, first_entry_id, title_path) in title_data {
        // Get first entry ID for the "Read" link, and check for saved progress
        let read_link = if let Some(entry_id) = first_entry_id {
            // Try to load progress for this entry from info.json directly
            let progress_page = {
                let info_path = title_path.join("info.json");
                if info_path.exists() {
                    if let Ok(content) = tokio::fs::read_to_string(&info_path).await {
                        if let Ok(info) = serde_json::from_str::<serde_json::Value>(&content) {
                            info.get("progress")
                                .and_then(|p| p.get(&username))
                                .and_then(|u| u.get(&entry_id))
                                .and_then(|page| page.as_u64())
                                .map(|p| p as usize)
                                .unwrap_or(1)
                        } else {
                            1
                        }
                    } else {
                        1
                    }
                } else {
                    1
                }
            };

            format!("/reader/{}/{}/{}", title_id, entry_id, progress_page)
        } else {
            format!("/api/title/{}", title_id)  // Fallback to details if no entries
        };

        titles_html.push_str(&format!(
            r#"<div class="title-card">
                <h3>{}</h3>
                <p>{} entries • {} pages</p>
                <a href="{}">Read</a>
              </div>"#,
            title_name,
            entry_count,
            pages,
            read_link
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
