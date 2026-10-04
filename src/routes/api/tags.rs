use axum::{
    extract::{Path, State},
    Json,
};

use super::api_failure;
use crate::AppState;

/// API route: GET /api/tags
#[utoipa::path(get, path = "/api/tags", tag = "library", summary = "List tags", responses((status = 200, description = "Tags returned")))]
pub async fn list_tags(
    State(state): State<AppState>,
    _username: crate::auth::Username,
) -> Json<serde_json::Value> {
    match state.storage.list_tags().await {
        Ok(tags) => Json(serde_json::json!({"success": true, "tags": tags})),
        Err(error) => api_failure(error.to_string()),
    }
}

/// API route: GET /api/tags/:tid
#[utoipa::path(get, path = "/api/tags/{tid}", tag = "library", summary = "Get title tags", params(("tid" = String, Path, description = "Title ID")), responses((status = 200, description = "Tags returned")))]
pub async fn get_title_tags(
    State(state): State<AppState>,
    Path(title_id): Path<String>,
    _username: crate::auth::Username,
) -> Json<serde_json::Value> {
    let lib = state.library.load();
    if lib.get_title(&title_id).is_none() {
        return api_failure("Nil assertion failed".to_string());
    }
    match state.storage.get_title_tags(&title_id).await {
        Ok(tags) => Json(serde_json::json!({"success": true, "tags": tags})),
        Err(error) => api_failure(error.to_string()),
    }
}

/// API route: PUT /api/admin/tags/:tid/:tag
#[utoipa::path(put, path = "/api/admin/tags/{tid}/{tag}", tag = "library", summary = "Add a tag to a title", params(("tid" = String, Path, description = "Title ID"), ("tag" = String, Path, description = "Tag")), responses((status = 200, description = "Tag added")))]
pub async fn add_tag(
    State(state): State<AppState>,
    Path((title_id, tag)): Path<(String, String)>,
    _admin: crate::auth::AdminOnly,
) -> Json<serde_json::Value> {
    if state.library.load().get_title(&title_id).is_none() {
        return api_failure("Nil assertion failed".to_string());
    }
    match state.storage.add_tag(&title_id, &tag).await {
        Ok(()) => Json(serde_json::json!({"success": true, "error": null})),
        Err(error) => api_failure(error.to_string()),
    }
}

/// API route: DELETE /api/admin/tags/:tid/:tag
#[utoipa::path(delete, path = "/api/admin/tags/{tid}/{tag}", tag = "library", summary = "Delete a tag from a title", params(("tid" = String, Path, description = "Title ID"), ("tag" = String, Path, description = "Tag")), responses((status = 200, description = "Tag deleted")))]
pub async fn delete_tag(
    State(state): State<AppState>,
    Path((title_id, tag)): Path<(String, String)>,
    _admin: crate::auth::AdminOnly,
) -> Json<serde_json::Value> {
    if state.library.load().get_title(&title_id).is_none() {
        return api_failure("Nil assertion failed".to_string());
    }
    match state.storage.delete_tag(&title_id, &tag).await {
        Ok(()) => Json(serde_json::json!({"success": true, "error": null})),
        Err(error) => api_failure(error.to_string()),
    }
}
