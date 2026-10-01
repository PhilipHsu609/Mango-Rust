use askama::Template;
use axum::{
    extract::{Path, State},
    http::{header, StatusCode},
    response::IntoResponse,
};

use crate::{error::Result, AppState};

/// Template for OPDS main catalog feed
#[derive(Template)]
#[template(path = "opds_index.xml", escape = "xml")]
struct OPDSIndexTemplate {
    base_url: String,
    titles: Vec<OPDSTitleEntry>,
}

/// Simplified title entry for OPDS
struct OPDSTitleEntry {
    id: String,
    name: String,
}

/// Title information for OPDS
struct OPDSTitleInfo {
    id: String,
    name: String,
    titles: Vec<OPDSTitleEntry>,
}

/// Template for OPDS title detail feed
#[derive(Template)]
#[template(path = "opds_title.xml", escape = "xml")]
struct OPDSTitleTemplate {
    base_url: String,
    title: OPDSTitleInfo,
    entries: Vec<OPDSEntryInfo>,
}

/// Entry information for OPDS
struct OPDSEntryInfo {
    id: String,
    title: String,
    mime_type: String,
}

/// OPDS route: GET /opds
/// Returns the main catalog feed listing all titles
pub async fn opds_index(
    State(state): State<AppState>,
    _username: crate::auth::Username,
) -> Result<impl IntoResponse> {
    let lib = state.library.load();
    let titles = lib.get_titles();

    let mut opds_titles = Vec::with_capacity(titles.len());
    for title in titles {
        let info = crate::library::TitleInfo::load(&title.path).await?;
        opds_titles.push(OPDSTitleEntry {
            id: title.id.clone(),
            name: if info.display_name.is_empty() {
                title.title.clone()
            } else {
                info.display_name
            },
        });
    }

    let template = OPDSIndexTemplate {
        base_url: get_base_url(&state),
        titles: opds_titles,
    };

    let xml = template.render().map_err(|e| {
        crate::error::Error::Internal(format!("Failed to render OPDS index: {}", e))
    })?;

    Ok((
        StatusCode::OK,
        [(
            header::CONTENT_TYPE,
            "application/atom+xml;profile=opds-catalog;kind=navigation",
        )],
        xml,
    ))
}

/// OPDS route: GET /opds/book/:title_id
/// Returns a feed for a specific title showing all its entries
pub async fn opds_title(
    State(state): State<AppState>,
    Path(title_id): Path<String>,
    _username: crate::auth::Username,
) -> Result<impl IntoResponse> {
    let lib = state.library.load();

    // Get the title
    let title = lib
        .get_title(&title_id)
        .ok_or_else(|| crate::error::Error::NotFound(format!("Title not found: {}", title_id)))?;

    let info = crate::library::TitleInfo::load(&title.path).await?;
    let opds_title = OPDSTitleInfo {
        id: title.id.clone(),
        name: if info.display_name.is_empty() {
            title.title.clone()
        } else {
            info.display_name
        },
        titles: {
            let mut nested_titles = Vec::with_capacity(title.nested_titles.len());
            for nested in &title.nested_titles {
                let nested_info = crate::library::TitleInfo::load(&nested.path).await?;
                nested_titles.push(OPDSTitleEntry {
                    id: nested.id.clone(),
                    name: if nested_info.display_name.is_empty() {
                        nested.title.clone()
                    } else {
                        nested_info.display_name
                    },
                });
            }
            nested_titles
        },
    };

    let opds_entries: Vec<OPDSEntryInfo> = title
        .entries
        .iter()
        .map(|entry| OPDSEntryInfo {
            id: entry.id.clone(),
            title: info
                .entry_display_name
                .get(&entry.title)
                .filter(|name| !name.is_empty())
                .cloned()
                .unwrap_or_else(|| entry.title.clone()),
            mime_type: get_mime_type(&entry.path),
        })
        .collect();

    let template = OPDSTitleTemplate {
        base_url: get_base_url(&state),
        title: opds_title,
        entries: opds_entries,
    };

    let xml = template.render().map_err(|e| {
        crate::error::Error::Internal(format!("Failed to render OPDS title feed: {}", e))
    })?;

    Ok((
        StatusCode::OK,
        [(
            header::CONTENT_TYPE,
            "application/atom+xml;profile=opds-catalog;kind=navigation",
        )],
        xml,
    ))
}

fn get_base_url(state: &AppState) -> String {
    state.config.base_url.clone()
}

/// Determine MIME type from file path
fn get_mime_type(path: &std::path::Path) -> String {
    match path.extension().and_then(|extension| extension.to_str()) {
        Some(extension)
            if extension.eq_ignore_ascii_case("cbz") || extension.eq_ignore_ascii_case("zip") =>
        {
            "application/zip".to_string()
        }
        Some(extension)
            if extension.eq_ignore_ascii_case("cbr") || extension.eq_ignore_ascii_case("rar") =>
        {
            "application/x-rar-compressed".to_string()
        }
        _ => "application/octet-stream".to_string(),
    }
}
