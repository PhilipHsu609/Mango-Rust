use axum::{
    extract::{Path, State, Request},
    response::Html,
};

use crate::{auth::get_username, AppState, error::{Error, Result}};

/// Reader page HTML template
const READER_HTML: &str = include_str!("../../templates/reader.html");
/// Reader page styles
const READER_STYLES: &str = include_str!("../../templates/reader_styles.css");
/// Reader page scripts
const READER_SCRIPTS: &str = include_str!("../../templates/reader_scripts.js");

/// Reader page - displays manga pages with navigation
pub async fn reader(
    State(state): State<AppState>,
    Path((title_id, entry_id, page)): Path<(String, String, usize)>,
    request: Request,
) -> Result<Html<String>> {
    // Get username from request extensions (injected by auth middleware)
    let username = get_username(&request).unwrap_or_else(|| "Unknown".to_string());

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
    let mut entries_options = String::new();
    for e in &title.entries {
        let selected = if e.id == entry_id { "selected" } else { "" };
        entries_options.push_str(&format!(
            "<option value=\"{}\" {}>{}</option>\n",
            e.id, selected, e.title
        ));
    }

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

    let exit_url = format!("/book/{}", title_id);

    // Build page options for dropdown
    let mut page_options = String::new();
    for p in 1..=total_pages {
        let selected = if p == page { "selected" } else { "" };
        page_options.push_str(&format!(
            "<option value=\"{}\" {}>{}</option>\n",
            p, selected, p
        ));
    }

    // Build prev/next entry buttons
    let prev_entry_button = if let Some(url) = &prev_entry_url {
        format!(r#"<a class="uk-button uk-button-default uk-margin-small-bottom uk-margin-small-right" href="{}">Previous Entry</a>"#, url)
    } else {
        String::new()
    };

    let next_entry_button = if let Some(url) = &next_entry_url {
        format!(r#"<a class="uk-button uk-button-default uk-margin-small-bottom uk-margin-small-right" href="{}">Next Entry</a>"#, url)
    } else {
        String::new()
    };

    // Prepare scripts with template variables replaced
    let scripts = READER_SCRIPTS
        .replace("{{ title_id }}", &title_id)
        .replace("{{ entry_id }}", &entry_id)
        .replace("{{ current_page }}", &page.to_string())
        .replace("{{ total_pages }}", &total_pages.to_string())
        .replace(
            "{% if let Some(url) = next_entry_url %}'{{ url }}'{% else %}null{% endif %}",
            &next_entry_url.as_ref().map(|u| format!("'{}'", u)).unwrap_or_else(|| "null".to_string())
        )
        .replace("{{ exit_url }}", &exit_url);

    // Render the HTML
    let html = READER_HTML
        .replace("{{ entry_name }}", &entry.title)
        .replace("{{ current_page }}", &page.to_string())
        .replace("{{ page_styles }}", READER_STYLES)
        .replace("{{ entry_path }}", &entry.path.display().to_string())
        .replace("{{ total_pages }}", &total_pages.to_string())
        .replace("{{ page_options }}", &page_options)
        .replace("{{ entry_options }}", &entries_options)
        .replace("{{ prev_entry_button }}", &prev_entry_button)
        .replace("{{ next_entry_button }}", &next_entry_button)
        .replace("{{ exit_url }}", &exit_url)
        .replace("{{ page_scripts }}", &scripts);

    Ok(Html(html))
}
