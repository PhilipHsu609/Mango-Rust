use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Deserialize;

use crate::{
    auth::AdminOnly,
    library::entry_delete::{self, DeleteError, DeletePreview, DeleteRequest},
    AppState,
};

/// The confirmation values the admin saw in the preview.
#[derive(Deserialize)]
pub struct DeleteQuery {
    size: u64,
    /// Decimal nanoseconds since the Unix epoch, exactly as the preview returned it.
    mtime_ns: String,
}

/// GET /api/admin/entry-delete/:tid/:eid - Describe an archive entry before deleting it.
#[utoipa::path(get, path = "/api/admin/entry-delete/{tid}/{eid}", tag = "admin", summary = "Preview archive entry delete", params(("tid" = String, Path, description = "Title identifier"), ("eid" = String, Path, description = "Entry identifier")), responses((status = 200, description = "Delete preview", body = DeletePreview), (status = 404, description = "Entry not found"), (status = 422, description = "Not an archive file")))]
pub async fn delete_preview(
    State(state): State<AppState>,
    AdminOnly(_username): AdminOnly,
    Path((tid, eid)): Path<(String, String)>,
) -> Response {
    let library = state.library.load_full();
    match entry_delete::preview(&library, &tid, &eid).await {
        Ok(preview) => Json(preview).into_response(),
        Err(error) => error_response(error),
    }
}

/// DELETE /api/admin/entry-delete/:tid/:eid - Permanently delete an archive entry's file.
/// Runs in a spawned task, so closing the browser cannot stop it halfway.
#[utoipa::path(delete, path = "/api/admin/entry-delete/{tid}/{eid}", tag = "admin", summary = "Delete archive entry", params(("tid" = String, Path, description = "Title identifier"), ("eid" = String, Path, description = "Entry identifier"), ("size" = u64, Query, description = "File size shown in the preview"), ("mtime_ns" = String, Query, description = "Modification time shown in the preview")), responses((status = 200, description = "Entry deleted, possibly with cleanup warnings"), (status = 400, description = "Invalid confirmation values"), (status = 404, description = "Entry not found"), (status = 409, description = "File changed since the preview"), (status = 422, description = "Not an archive file")))]
pub async fn delete_entry(
    State(state): State<AppState>,
    AdminOnly(username): AdminOnly,
    Path((tid, eid)): Path<(String, String)>,
    Query(query): Query<DeleteQuery>,
) -> Response {
    let Ok(mtime_ns) = query.mtime_ns.parse::<u128>() else {
        return failure(StatusCode::BAD_REQUEST, "Invalid confirmation values");
    };
    let request = DeleteRequest {
        admin: username.clone(),
        title_id: tid.clone(),
        entry_id: eid.clone(),
        size: query.size,
        mtime_ns,
    };
    let task = tokio::spawn(entry_delete::delete(
        state.library.clone(),
        state.storage.clone(),
        state.config.clone(),
        request,
    ));
    match task.await {
        Ok(Ok(outcome)) => Json(serde_json::json!({
            "success": true,
            "removes_title": outcome.removes_title,
            "redirect": outcome.redirect,
            "warnings": outcome.warnings,
        }))
        .into_response(),
        Ok(Err(error)) => error_response(error),
        Err(error) => {
            tracing::error!(
                admin = %username,
                title_id = %tid,
                entry_id = %eid,
                outcome = "failed",
                error = %error,
                "Archive entry delete task failed"
            );
            failure(StatusCode::INTERNAL_SERVER_ERROR, &error.to_string())
        }
    }
}

fn error_response(error: DeleteError) -> Response {
    match error {
        DeleteError::NotFound => failure(
            StatusCode::NOT_FOUND,
            "Entry not found; it may already have been deleted",
        ),
        DeleteError::Changed => failure(
            StatusCode::CONFLICT,
            "This file changed since you opened this dialog",
        ),
        DeleteError::Refused => failure(
            StatusCode::UNPROCESSABLE_ENTITY,
            "Only archive files can be deleted",
        ),
        DeleteError::Failed(error) => {
            failure(StatusCode::INTERNAL_SERVER_ERROR, &error.to_string())
        }
    }
}

fn failure(status: StatusCode, error: &str) -> Response {
    (
        status,
        Json(serde_json::json!({ "success": false, "error": error })),
    )
        .into_response()
}
