pub mod catalog;
pub mod media;
pub mod metadata;
pub mod reading;
pub mod tags;

use axum::Json;
use serde::Serialize;

/// Standard API response wrapper for frontend compatibility
#[derive(Serialize)]
struct ApiResponse<T: Serialize> {
    success: bool,
    #[serde(flatten)]
    data: T,
}

/// Success response helper
fn success_response<T: Serialize>(data: T) -> Json<ApiResponse<T>> {
    Json(ApiResponse {
        success: true,
        data,
    })
}

fn api_failure(error: String) -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "success": false,
        "error": error
    }))
}
