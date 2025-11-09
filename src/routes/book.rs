use axum::{
    extract::{Path, Query, Request, State},
    response::Html,
};
use serde::Deserialize;

use crate::{auth::get_username, error::Result, library::SortMethod, AppState};

/// Query parameters for book page
#[derive(Deserialize)]
pub struct BookParams {
    pub sort: Option<String>,
    pub ascend: Option<String>,
    pub search: Option<String>,
}

/// Layout template
const LAYOUT: &str = include_str!("../../templates/layout.html");
/// Book page content
const BOOK_CONTENT: &str = include_str!("../../templates/book_content.html");
/// Book page styles
const BOOK_STYLES: &str = include_str!("../../templates/book_styles.css");
/// Book page scripts
const BOOK_SCRIPTS: &str = include_str!("../../templates/book_scripts.js");

/// GET /book/:id - Title/book page showing all entries (requires authentication)
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
    let (title_name, title_path, entry_cards) = {
        let lib = state.library.read().await;

        // Get the title
        let title = lib
            .get_title(&title_id)
            .ok_or_else(|| crate::error::Error::NotFound(format!("Title not found: {}", title_id)))?;

        let title_name = title.title.clone();
        let title_path = title.path.to_string_lossy().to_string();

        // Get all entries, sorted
        let entries = title.get_entries_sorted(sort_method, ascending);

        // Build entry card data
        let mut entry_cards = Vec::new();
        for entry in entries {
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

            entry_cards.push((
                entry.id.clone(),
                entry.title.clone(),
                entry.pages,
                progress_percentage,
                saved_page,
                entry.path.to_string_lossy().to_string(),
            ));
        }

        (title_name, title_path, entry_cards)
    }; // Lock is released here

    // Count entries before building HTML
    let entry_count = entry_cards.len();

    // Build entry cards HTML
    let mut entries_html = String::new();
    for (entry_id, entry_name, pages, progress, saved_page, entry_path) in entry_cards {
        entries_html.push_str(&format!(
            r#"<div class="entry-card" data-entry-id="{}" data-title-id="{}" data-entry-name="{}" data-pages="{}" data-progress="{:.1}" data-saved-page="{}" data-path="{}">
                <div class="entry-thumbnail">
                    <div class="placeholder-icon">📖</div>
                    <div class="progress-badge">{:.1}%</div>
                </div>
                <div class="entry-info">
                    <div class="entry-name">{}</div>
                    <div class="entry-stats">{} pages</div>
                </div>
              </div>"#,
            entry_id,
            title_id,
            entry_name,
            pages,
            progress,
            saved_page,
            entry_path,
            progress,
            entry_name,
            pages
        ));
    }

    if entries_html.is_empty() {
        entries_html = "<p>No entries found.</p>".to_string();
    }

    // Determine which sort option is selected
    let sort_title_asc_selected = if matches!(sort_method, SortMethod::Name) && ascending {
        "selected"
    } else {
        ""
    };
    let sort_title_desc_selected = if matches!(sort_method, SortMethod::Name) && !ascending {
        "selected"
    } else {
        ""
    };
    let sort_modified_asc_selected = if matches!(sort_method, SortMethod::TimeModified) && ascending {
        "selected"
    } else {
        ""
    };
    let sort_modified_desc_selected = if matches!(sort_method, SortMethod::TimeModified) && !ascending {
        "selected"
    } else {
        ""
    };

    // Render page content
    let content = BOOK_CONTENT
        .replace("{{ title_id }}", &title_id)
        .replace("{{ title_name }}", &title_name)
        .replace("{{ title_path }}", &title_path)
        .replace("{{ entry_count }}", &entry_count.to_string())
        .replace("{{ sort_title_asc_selected }}", sort_title_asc_selected)
        .replace("{{ sort_title_desc_selected }}", sort_title_desc_selected)
        .replace("{{ sort_modified_asc_selected }}", sort_modified_asc_selected)
        .replace("{{ sort_modified_desc_selected }}", sort_modified_desc_selected)
        .replace("{{ entries }}", &entries_html);

    // Render with layout
    let html = LAYOUT
        .replace("{{ page_title }}", &format!("{} - Book", title_name))
        .replace("{{ home_active }}", "")
        .replace("{{ library_active }}", " class=\"uk-active\"")
        .replace("{{ page_styles }}", BOOK_STYLES)
        .replace("{{ content }}", &content)
        .replace(
            "{{ page_scripts }}",
            &format!("<script>{}</script>", BOOK_SCRIPTS),
        );

    Ok(Html(html))
}
