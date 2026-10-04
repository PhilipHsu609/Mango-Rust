use axum::{
    extract::{rejection::JsonRejection, Path, Query, State},
    Json,
};
use serde::Deserialize;

use super::api_failure;
use crate::{auth::AdminOnly, error::Result, AppState};

#[derive(Deserialize, utoipa::ToSchema)]
pub struct SortOptionUpdate {
    tid: Option<String>,
    sort: String,
    ascend: bool,
}

#[utoipa::path(get, path = "/api/sort_opt", tag = "library", summary = "Get sort options", params(("tid" = Option<String>, Query, description = "Title ID; omit for library defaults")), responses((status = 200, description = "Sort options returned")))]
pub async fn get_sort_opt(
    State(state): State<AppState>,
    Query(query): Query<SortOptionQuery>,
    crate::auth::Username(username): crate::auth::Username,
) -> Json<serde_json::Value> {
    let dir = if let Some(title_id) = query.tid {
        let lib = state.library.load();
        let Some(title) = lib.get_title(&title_id) else {
            return Json(serde_json::json!({
                "success": false,
                "error": "Nil assertion failed"
            }));
        };
        title.path.clone()
    } else {
        state.config.library_path.clone()
    };

    match state.library.load_full().metadata().read(&dir).await {
        Ok(info) => {
            let (method, ascend) = info
                .get_sort_by(&username)
                .unwrap_or_else(|| ("auto".to_string(), true));
            Json(serde_json::json!({
                "method": method,
                "ascend": ascend
            }))
        }
        Err(error) => Json(serde_json::json!({
            "success": false,
            "error": error.to_string()
        })),
    }
}

#[derive(Deserialize)]
pub struct SortOptionQuery {
    tid: Option<String>,
}

#[utoipa::path(put, path = "/api/sort_opt", tag = "library", summary = "Update sort options", request_body = SortOptionUpdate, responses((status = 200, description = "Sort options updated")))]
pub async fn update_sort_opt(
    State(state): State<AppState>,
    crate::auth::Username(username): crate::auth::Username,
    request: std::result::Result<Json<SortOptionUpdate>, JsonRejection>,
) -> Json<serde_json::Value> {
    let Json(request) = match request {
        Ok(request) => request,
        Err(error) => return api_failure(error.body_text()),
    };
    let dir = if let Some(title_id) = request.tid.as_deref() {
        let lib = state.library.load();
        let Some(title) = lib.get_title(title_id) else {
            return Json(serde_json::json!({
                "success": false,
                "error": "Nil assertion failed"
            }));
        };
        title.path.clone()
    } else {
        state.config.library_path.clone()
    };
    let lib = state.library.load_full();
    match lib
        .metadata()
        .update(&dir, |info| {
            info.set_sort_by(&username, &request.sort, request.ascend);
        })
        .await
    {
        Ok(_) => Json(serde_json::json!({ "success": true })),
        Err(error) => Json(serde_json::json!({
            "success": false,
            "error": error.to_string()
        })),
    }
}

#[derive(Deserialize)]
pub struct DisplayNameQuery {
    eid: Option<String>,
}

/// PUT /api/admin/display_name/:tid/:name - Update display name for title or entry
#[utoipa::path(put, path = "/api/admin/display_name/{tid}/{name}", tag = "admin", summary = "Update display name", params(("tid" = String, Path, description = "Title identifier"), ("name" = String, Path, description = "Display name"), ("eid" = Option<String>, Query, description = "Entry ID; omit to change the title")), responses((status = 200, description = "Display name updated")))]
pub async fn update_display_name(
    State(state): State<AppState>,
    AdminOnly(_username): AdminOnly,
    Path((title_id, name)): Path<(String, String)>,
    axum::extract::Query(query): axum::extract::Query<DisplayNameQuery>,
) -> Json<serde_json::Value> {
    let result: Result<()> = async {
        let (title_path, entry_title) = {
            let lib = state.library.load();
            let title = lib.get_title(&title_id).ok_or_else(|| {
                crate::error::Error::BadRequest("Nil assertion failed".to_string())
            })?;
            let entry_title = if let Some(entry_id) = query.eid.as_deref() {
                Some(
                    lib.get_entry(&title_id, entry_id)
                        .ok_or_else(|| {
                            crate::error::Error::BadRequest("Nil assertion failed".to_string())
                        })?
                        .title
                        .clone(),
                )
            } else {
                None
            };
            (title.path.clone(), entry_title)
        };

        state
            .library
            .load_full()
            .metadata()
            .update(&title_path, |info| {
                if let Some(entry_title) = entry_title {
                    info.entry_display_name.insert(entry_title, name);
                } else {
                    info.display_name = name;
                }
            })
            .await?;
        Ok(())
    }
    .await;

    match result {
        Ok(()) => {
            tracing::info!("Updated display name for title {}", title_id);
            Json(serde_json::json!({ "success": true }))
        }
        Err(error) => Json(serde_json::json!({
            "success": false,
            "error": error.to_string()
        })),
    }
}

#[derive(Deserialize)]
pub struct SortTitleQuery {
    eid: Option<String>,
    name: Option<String>,
}

/// PUT /api/admin/sort_title/:tid - Update sort title for title or entry
#[utoipa::path(put, path = "/api/admin/sort_title/{tid}", tag = "admin", summary = "Update sort title", params(("tid" = String, Path, description = "Title identifier"), ("eid" = Option<String>, Query, description = "Entry ID; omit to change the title"), ("name" = Option<String>, Query, description = "Sort title")), responses((status = 200, description = "Sort title updated")))]
pub async fn update_sort_title(
    State(state): State<AppState>,
    AdminOnly(_username): AdminOnly,
    Path(title_id): Path<String>,
    axum::extract::Query(query): axum::extract::Query<SortTitleQuery>,
) -> Json<serde_json::Value> {
    let result: Result<()> = async {
        let entry_belongs_to_title = {
            let library = state.library.load();
            let Some(title) = library.get_title(&title_id) else {
                return Err(crate::error::Error::BadRequest(
                    "Nil assertion failed".to_string(),
                ));
            };
            query
                .eid
                .as_deref()
                .is_some_and(|entry_id| title.entries.iter().any(|entry| entry.id == entry_id))
        };
        let sort_title = query.name.as_deref();

        if let Some(entry_id) = &query.eid {
            if entry_belongs_to_title {
                state
                    .storage
                    .update_entry_sort_title(entry_id, sort_title)
                    .await?;
                tracing::info!("Updated entry {} sort title to {:?}", entry_id, sort_title);
            }
        } else {
            state
                .storage
                .update_title_sort_title(&title_id, sort_title)
                .await?;
            tracing::info!("Updated title {} sort title to {:?}", title_id, sort_title);
        }
        Ok(())
    }
    .await;

    match result {
        Ok(()) => Json(serde_json::json!({ "success": true })),
        Err(error) => Json(serde_json::json!({
            "success": false,
            "error": error.to_string()
        })),
    }
}
