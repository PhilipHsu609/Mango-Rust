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

/// Simplified title entry for OPDS index
struct OPDSTitleEntry {
    id: String,
    name: String,
}

/// Template for OPDS title detail feed
#[derive(Template)]
#[template(path = "opds_title.xml", escape = "xml")]
struct OPDSTitleTemplate {
    base_url: String,
    title: OPDSTitleInfo,
    entries: Vec<OPDSEntryInfo>,
}

/// Title information for OPDS
struct OPDSTitleInfo {
    id: String,
    name: String,
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
    let lib = state.library.read().await;
    let titles = lib.get_titles();

    let opds_titles: Vec<OPDSTitleEntry> = titles
        .iter()
        .map(|t| OPDSTitleEntry {
            id: t.id.clone(),
            name: t.title.clone(),
        })
        .collect();

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
    let lib = state.library.read().await;

    // Get the title
    let title = lib
        .get_title(&title_id)
        .ok_or_else(|| crate::error::Error::NotFound(format!("Title not found: {}", title_id)))?;

    let opds_title = OPDSTitleInfo {
        id: title.id.clone(),
        name: title.title.clone(),
    };

    let opds_entries: Vec<OPDSEntryInfo> = title
        .entries
        .iter()
        .map(|e| OPDSEntryInfo {
            id: e.id.clone(),
            title: e.title.clone(),
            mime_type: get_mime_type(&e.path),
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

/// Get base URL from config or default to "/"
fn get_base_url(_state: &AppState) -> String {
    // For now, return root path - can be made configurable later
    "/".to_string()
}

/// Determine MIME type from file path
fn get_mime_type(path: &std::path::Path) -> String {
    match path.extension().and_then(|e| e.to_str()) {
        Some("cbz") | Some("zip") => "application/zip".to_string(),
        Some("cbr") | Some("rar") => "application/x-rar-compressed".to_string(),
        _ => "application/octet-stream".to_string(),
    }
}
