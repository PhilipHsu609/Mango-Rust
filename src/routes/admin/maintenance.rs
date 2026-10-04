use askama::Template;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{Html, IntoResponse},
    Json,
};
use serde::Serialize;
use std::{
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
    time::Instant,
};

use crate::{auth::AdminOnly, error::Result, util::render_error, AppState};

/// Response for library scan endpoint
#[derive(Serialize)]
pub struct ScanResponse {
    pub titles: usize,
    pub milliseconds: f64,
}

/// POST /api/admin/scan - Trigger library rescan
/// Returns number of titles found and time taken in milliseconds
/// Builds a replacement library during the request, then atomically swaps it in
#[utoipa::path(post, path = "/api/admin/scan", tag = "admin", summary = "Scan library", responses((status = 200, description = "Library scan completed")))]
pub async fn scan_library(
    State(state): State<AppState>,
    AdminOnly(_username): AdminOnly,
) -> Result<Json<ScanResponse>> {
    let start = Instant::now();
    let _scan_guard = crate::library::scan::SCAN_LOCK.lock().await;

    // Publish completed roots while building the replacement library.
    let mut new_lib = crate::library::Library::new(
        state.config.library_path.clone(),
        state.storage.clone(),
        &state.config,
    );
    let previous = state.library.load_full();
    crate::library::scan::scan(
        &mut new_lib,
        Some(previous),
        Some(std::sync::Arc::clone(&state.library)),
    )
    .await?;
    let titles = new_lib.get_titles().len();

    // Atomically swap the new library in
    state.library.store(std::sync::Arc::new(new_lib));

    let elapsed = start.elapsed().as_secs_f64() * 1000.0;

    tracing::info!("Library scan completed: {} titles in {}ms", titles, elapsed);

    Ok(Json(ScanResponse {
        titles,
        milliseconds: elapsed,
    }))
}

#[derive(Serialize)]
pub struct MissingTitlesResponse {
    success: bool,
    error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    titles: Option<Vec<crate::storage::MissingItem>>,
}

#[derive(Serialize)]
pub struct MissingEntriesResponse {
    success: bool,
    error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    entries: Option<Vec<crate::storage::MissingItem>>,
}

#[derive(Serialize)]
pub struct MissingItemsMutationResponse {
    success: bool,
    error: Option<String>,
}

/// GET /api/admin/titles/missing - Get unavailable titles.
#[utoipa::path(get, path = "/api/admin/titles/missing", tag = "admin", summary = "Get missing titles", responses((status = 200, description = "Missing titles returned")))]
pub async fn get_missing_titles(
    State(state): State<AppState>,
    AdminOnly(_username): AdminOnly,
) -> Json<MissingTitlesResponse> {
    match state.storage.get_missing_titles().await {
        Ok(titles) => Json(MissingTitlesResponse {
            success: true,
            error: None,
            titles: Some(titles),
        }),
        Err(error) => Json(MissingTitlesResponse {
            success: false,
            error: Some(error.to_string()),
            titles: None,
        }),
    }
}

/// GET /api/admin/entries/missing - Get unavailable entries.
#[utoipa::path(get, path = "/api/admin/entries/missing", tag = "admin", summary = "Get missing entries", responses((status = 200, description = "Missing entries returned")))]
pub async fn get_missing_entries(
    State(state): State<AppState>,
    AdminOnly(_username): AdminOnly,
) -> Json<MissingEntriesResponse> {
    match state.storage.get_missing_entries().await {
        Ok(entries) => Json(MissingEntriesResponse {
            success: true,
            error: None,
            entries: Some(entries),
        }),
        Err(error) => Json(MissingEntriesResponse {
            success: false,
            error: Some(error.to_string()),
            entries: None,
        }),
    }
}

/// DELETE /api/admin/titles/missing/:id - Delete an unavailable title.
#[utoipa::path(delete, path = "/api/admin/titles/missing/{id}", tag = "admin", summary = "Delete missing title", params(("id" = String, Path, description = "Missing title identifier")), responses((status = 200, description = "Missing title deleted")))]
pub async fn delete_missing_title(
    State(state): State<AppState>,
    AdminOnly(_username): AdminOnly,
    Path(id): Path<String>,
) -> Json<MissingItemsMutationResponse> {
    match state.storage.delete_missing_title(&id).await {
        Ok(()) => Json(MissingItemsMutationResponse {
            success: true,
            error: None,
        }),
        Err(error) => Json(MissingItemsMutationResponse {
            success: false,
            error: Some(error.to_string()),
        }),
    }
}

/// DELETE /api/admin/entries/missing/:id - Delete an unavailable entry.
#[utoipa::path(delete, path = "/api/admin/entries/missing/{id}", tag = "admin", summary = "Delete missing entry", params(("id" = String, Path, description = "Missing entry identifier")), responses((status = 200, description = "Missing entry deleted")))]
pub async fn delete_missing_entry(
    State(state): State<AppState>,
    AdminOnly(_username): AdminOnly,
    Path(id): Path<String>,
) -> Json<MissingItemsMutationResponse> {
    match state.storage.delete_missing_entry(&id).await {
        Ok(()) => Json(MissingItemsMutationResponse {
            success: true,
            error: None,
        }),
        Err(error) => Json(MissingItemsMutationResponse {
            success: false,
            error: Some(error.to_string()),
        }),
    }
}

/// DELETE /api/admin/titles/missing - Delete all unavailable titles.
#[utoipa::path(delete, path = "/api/admin/titles/missing", tag = "admin", summary = "Delete all missing titles", responses((status = 200, description = "Missing titles deleted")))]
pub async fn delete_all_missing_titles(
    State(state): State<AppState>,
    AdminOnly(_username): AdminOnly,
) -> Json<MissingItemsMutationResponse> {
    match state.storage.delete_all_missing_titles().await {
        Ok(_) => Json(MissingItemsMutationResponse {
            success: true,
            error: None,
        }),
        Err(error) => Json(MissingItemsMutationResponse {
            success: false,
            error: Some(error.to_string()),
        }),
    }
}

/// DELETE /api/admin/entries/missing - Delete all unavailable entries.
#[utoipa::path(delete, path = "/api/admin/entries/missing", tag = "admin", summary = "Delete all missing entries", responses((status = 200, description = "Missing entries deleted")))]
pub async fn delete_all_missing_entries(
    State(state): State<AppState>,
    AdminOnly(_username): AdminOnly,
) -> Json<MissingItemsMutationResponse> {
    match state.storage.delete_all_missing_entries().await {
        Ok(_) => Json(MissingItemsMutationResponse {
            success: true,
            error: None,
        }),
        Err(error) => Json(MissingItemsMutationResponse {
            success: false,
            error: Some(error.to_string()),
        }),
    }
}

/// Missing Items template
#[derive(Template)]
#[template(path = "missing-items.html")]
struct MissingItemsTemplate {
    nav: crate::util::NavigationState,
}

/// GET /admin/missing - Missing items management page.
pub async fn missing_items_page(AdminOnly(_username): AdminOnly) -> Result<Html<String>> {
    let template = MissingItemsTemplate {
        nav: crate::util::NavigationState::admin().with_admin(true),
    };

    Ok(Html(template.render().map_err(render_error)?))
}

/// Global thumbnail generation state
static THUMBNAIL_GENERATING: AtomicBool = AtomicBool::new(false);
static THUMBNAIL_CURRENT: AtomicUsize = AtomicUsize::new(0);
static THUMBNAIL_TOTAL: AtomicUsize = AtomicUsize::new(0);

/// GET /api/admin/thumbnail_progress - Get thumbnail generation progress
#[utoipa::path(get, path = "/api/admin/thumbnail_progress", tag = "admin", summary = "Get thumbnail progress", responses((status = 200, description = "Thumbnail progress returned")))]
pub async fn thumbnail_progress(
    AdminOnly(_username): AdminOnly,
) -> Result<Json<serde_json::Value>> {
    let total = THUMBNAIL_TOTAL.load(Ordering::SeqCst);
    let current = THUMBNAIL_CURRENT.load(Ordering::SeqCst);
    let progress = if total == 0 {
        0.0
    } else {
        (current as f64 / total as f64).min(1.0)
    };

    Ok(Json(serde_json::json!({
        "progress": progress
    })))
}

/// POST /api/admin/generate_thumbnails - Start thumbnail generation
#[utoipa::path(post, path = "/api/admin/generate_thumbnails", tag = "admin", summary = "Generate thumbnails", responses((status = 200, description = "Thumbnail generation started")))]
pub async fn generate_thumbnails(
    State(state): State<AppState>,
    AdminOnly(_username): AdminOnly,
) -> axum::response::Response {
    if THUMBNAIL_GENERATING
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return StatusCode::OK.into_response();
    }
    THUMBNAIL_CURRENT.store(0, Ordering::SeqCst);

    let lib = state.library.load();
    let mut entries_to_process: Vec<(String, String)> = Vec::new();

    for title in lib.all_titles() {
        for entry in &title.entries {
            entries_to_process.push((title.id.clone(), entry.id.clone()));
        }
    }

    THUMBNAIL_TOTAL.store(entries_to_process.len(), Ordering::SeqCst);
    drop(lib);

    let state_clone = state.clone();
    tokio::spawn(async move {
        let lib = state_clone.library.load();
        let db = state_clone.storage.pool();

        for (i, (title_id, entry_id)) in entries_to_process.iter().enumerate() {
            THUMBNAIL_CURRENT.store(i + 1, Ordering::SeqCst);

            if let Some(entry) = lib.get_entry(title_id, entry_id) {
                if let Ok(Some(_)) = crate::library::media::get_thumbnail(entry_id, db).await {
                    continue;
                }

                if let Err(e) = crate::library::media::generate_thumbnail(entry, db).await {
                    tracing::warn!("Failed to generate thumbnail for {}: {}", entry_id, e);
                }
            }
        }

        THUMBNAIL_GENERATING.store(false, Ordering::SeqCst);
        tracing::info!("Thumbnail generation completed");
    });

    StatusCode::OK.into_response()
}

#[cfg(test)]
mod missing_items_api_tests {
    use super::{MissingEntriesResponse, MissingItemsMutationResponse, MissingTitlesResponse};
    use crate::storage::MissingItem;

    #[test]
    fn missing_item_api_responses_match_mango_contract() {
        let item = MissingItem {
            id: "item-id".to_string(),
            path: "Series/Volume.cbz".to_string(),
            signature: Some("signature".to_string()),
        };

        assert_eq!(
            serde_json::to_value(MissingTitlesResponse {
                success: true,
                error: None,
                titles: Some(vec![item.clone()]),
            })
            .unwrap(),
            serde_json::json!({
                "success": true,
                "error": null,
                "titles": [{
                    "id": "item-id",
                    "path": "Series/Volume.cbz",
                    "signature": "signature"
                }]
            })
        );
        assert_eq!(
            serde_json::to_value(MissingEntriesResponse {
                success: true,
                error: None,
                entries: Some(vec![item]),
            })
            .unwrap(),
            serde_json::json!({
                "success": true,
                "error": null,
                "entries": [{
                    "id": "item-id",
                    "path": "Series/Volume.cbz",
                    "signature": "signature"
                }]
            })
        );
        assert_eq!(
            serde_json::to_value(MissingItemsMutationResponse {
                success: true,
                error: None,
            })
            .unwrap(),
            serde_json::json!({"success": true, "error": null})
        );
        assert_eq!(
            serde_json::to_value(MissingTitlesResponse {
                success: false,
                error: Some("failed".to_string()),
                titles: None,
            })
            .unwrap(),
            serde_json::json!({"success": false, "error": "failed"})
        );
    }
}
