use axum::{
    extract::{Multipart, Path, State},
    http::{header, StatusCode},
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};

use super::{api_failure, success_response};
use crate::{auth::AdminOnly, error::Result, library::media, AppState};

fn page_index(page: i32) -> Option<usize> {
    usize::try_from(page.checked_sub(1)?).ok()
}

/// API route: GET /api/page/:tid/:eid/:page
/// Serves a specific page image from an entry
#[utoipa::path(get, path = "/api/page/{tid}/{eid}/{page}", tag = "reader", summary = "Get a page image", params(("tid" = String, Path, description = "Title ID"), ("eid" = String, Path, description = "Entry ID"), ("page" = i32, Path, description = "Page number")), responses((status = 200, description = "Page image returned")))]
pub async fn get_page(
    State(state): State<AppState>,
    Path((title_id, entry_id, page)): Path<(String, String, String)>,
    headers: axum::http::HeaderMap,
) -> Result<axum::response::Response> {
    let page = match page.parse::<i32>() {
        Ok(page) => page,
        Err(_) => {
            return Ok((
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Invalid Int32: {page}"),
            )
                .into_response());
        }
    };
    let lib = state.library.load();
    let Some(title) = lib.get_title(&title_id) else {
        return Ok((
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Title ID `{title_id}` not found"),
        )
            .into_response());
    };
    let Some(entry) = title.entries.iter().find(|entry| entry.id == entry_id) else {
        return Ok((
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Entry ID `{entry_id}` of `{}` not found", title.title),
        )
            .into_response());
    };

    let Some(page_idx) = page_index(page) else {
        return Ok((
            StatusCode::INTERNAL_SERVER_ERROR,
            format!(
                "Failed to load page {page} of `{}/{}`",
                title.title, entry.title
            ),
        )
            .into_response());
    };

    let image_data = match media::get_page(entry, page_idx).await {
        Ok(image_data) => image_data,
        Err(error) => {
            return Ok((StatusCode::INTERNAL_SERVER_ERROR, error.to_string()).into_response())
        }
    };
    let mime_type = guess_mime_type(&image_data);
    let previous_etag = headers
        .get(header::IF_NONE_MATCH)
        .and_then(|value| value.to_str().ok());

    let cache_control = if entry.path.is_dir() {
        "no-cache, max-age=86400"
    } else {
        "public, max-age=86400"
    };
    Ok(image_response(image_data, mime_type, previous_etag, Some(cache_control)).into_response())
}

/// GET /api/cover/:tid/:eid - Get manga entry cover/thumbnail
#[utoipa::path(get, path = "/api/cover/{tid}/{eid}", tag = "reader", summary = "Get an entry cover", params(("tid" = String, Path, description = "Title ID"), ("eid" = String, Path, description = "Entry ID")), responses((status = 200, description = "Cover returned")))]
pub async fn get_cover(
    State(state): State<AppState>,
    Path((title_id, entry_id)): Path<(String, String)>,
    headers: axum::http::HeaderMap,
) -> Result<axum::response::Response> {
    let lib = state.library.load();
    let Some(title) = lib.get_title(&title_id) else {
        return Ok((
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Title ID `{title_id}` not found"),
        )
            .into_response());
    };
    let Some(entry) = title.entries.iter().find(|entry| entry.id == entry_id) else {
        return Ok((
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Entry ID `{entry_id}` of `{}` not found", title.title),
        )
            .into_response());
    };
    let db = state.storage.pool();
    let thumbnail = match media::get_thumbnail(&entry_id, db).await {
        Ok(Some(image)) => Some(image),
        Ok(None) => match media::generate_thumbnail(entry, db).await {
            Ok(Some((data, mime, _))) => Some((data, mime)),
            Ok(None) => None,
            Err(error) => {
                tracing::warn!("Thumbnail generation failed for {}: {}", entry_id, error);
                None
            }
        },
        Err(error) => {
            tracing::warn!("Error getting thumbnail for {}: {}", entry_id, error);
            None
        }
    };
    let (data, mime) = match thumbnail {
        Some(image) => image,
        None => {
            let data = match media::get_page(entry, 0).await {
                Ok(data) => data,
                Err(error) => {
                    return Ok(
                        (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()).into_response()
                    );
                }
            };
            let mime = guess_mime_type(&data).to_string();
            (data, mime)
        }
    };
    let previous_etag = headers
        .get(header::IF_NONE_MATCH)
        .and_then(|value| value.to_str().ok());
    Ok(image_response(data, &mime, previous_etag, None).into_response())
}

pub(in crate::routes) fn join_base_url(base_url: &str, path: &str) -> String {
    if path.starts_with("http://") || path.starts_with("https://") {
        path.to_string()
    } else {
        format!("{}{}", base_url, path.trim_start_matches('/'))
    }
}

/// API route: GET /api/download/:tid/:eid
/// Download the original archive file for an entry (used by OPDS clients)
#[utoipa::path(get, path = "/api/download/{tid}/{eid}", tag = "reader", summary = "Download an entry", params(("tid" = String, Path, description = "Title ID"), ("eid" = String, Path, description = "Entry ID")), responses((status = 200, description = "Entry downloaded")))]
pub async fn download_entry(
    State(state): State<AppState>,
    Path((title_id, entry_id)): Path<(String, String)>,
    _username: crate::auth::Username,
) -> axum::response::Response {
    let lib = state.library.load();
    let Some(entry) = lib.get_entry(&title_id, &entry_id) else {
        return (StatusCode::NOT_FOUND, "Nil assertion failed").into_response();
    };
    let file_data = match tokio::fs::read(&entry.path).await {
        Ok(data) => data,
        Err(error) => return (StatusCode::NOT_FOUND, error.to_string()).into_response(),
    };

    let mime_type = match entry.path.extension().and_then(|e| e.to_str()) {
        Some("cbz") | Some("zip") => "application/zip",
        Some("cbr") | Some("rar") => "application/x-rar-compressed",
        _ => "application/octet-stream",
    };
    let filename = entry
        .path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("download");
    let content_disposition = format!("attachment; filename=\"{}\"", filename);

    (
        [
            (header::CONTENT_TYPE, mime_type),
            (header::CONTENT_DISPOSITION, content_disposition.as_str()),
        ],
        file_data,
    )
        .into_response()
}

/// Guess MIME type from image data magic bytes
fn guess_mime_type(data: &[u8]) -> &'static str {
    if data.len() < 4 {
        return "application/octet-stream";
    }

    // Check magic bytes
    match &data[0..4] {
        [0xFF, 0xD8, 0xFF, ..] => "image/jpeg",
        [0x89, 0x50, 0x4E, 0x47] => "image/png",
        [0x47, 0x49, 0x46, 0x38] => "image/gif",
        [0x52, 0x49, 0x46, 0x46] => "image/webp", // RIFF header (WebP)
        [0x42, 0x4D, ..] => "image/bmp",
        _ => "application/octet-stream",
    }
}

fn image_response(
    data: Vec<u8>,
    mime: &str,
    previous_etag: Option<&str>,
    cache_control: Option<&str>,
) -> axum::response::Response {
    use sha1::Digest;

    let etag = format!("{:x}", sha1::Sha1::digest(&data));
    if previous_etag == Some(etag.as_str()) {
        return StatusCode::NOT_MODIFIED.into_response();
    }

    let mut headers = axum::http::HeaderMap::new();
    headers.insert(
        header::ETAG,
        etag.parse()
            .expect("SHA-1 hex digest is a valid header value"),
    );
    headers.insert(
        header::CONTENT_TYPE,
        mime.parse()
            .unwrap_or_else(|_| axum::http::HeaderValue::from_static("application/octet-stream")),
    );
    if let Some(cache_control) = cache_control {
        headers.insert(
            header::CACHE_CONTROL,
            cache_control.parse().expect("static cache-control header"),
        );
    }
    (StatusCode::OK, headers, data).into_response()
}

// ========== Dimensions API (for reader) ==========

#[derive(Serialize)]
struct PageDimension {
    width: u32,
    height: u32,
}

#[derive(Serialize)]
struct DimensionsResponse {
    dimensions: Vec<PageDimension>,
}

fn dimensions_response(
    dimensions: Vec<PageDimension>,
    etag: &str,
    cache_control: &str,
) -> axum::response::Response {
    let mut response = success_response(DimensionsResponse { dimensions }).into_response();
    response.headers_mut().insert(
        header::ETAG,
        etag.parse().expect("SHA-1 ETag is a valid header value"),
    );
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        cache_control.parse().expect("static cache-control header"),
    );
    response
}

/// API route: GET /api/dimensions/:tid/:eid
/// Returns the image dimensions of all pages in an entry (used by reader for layout)
#[utoipa::path(get, path = "/api/dimensions/{tid}/{eid}", tag = "reader", summary = "Get entry page dimensions", params(("tid" = String, Path, description = "Title ID"), ("eid" = String, Path, description = "Entry ID")), responses((status = 200, description = "Dimensions returned")))]
pub async fn get_dimensions(
    State(state): State<AppState>,
    Path((title_id, entry_id)): Path<(String, String)>,
    headers: axum::http::HeaderMap,
) -> Result<impl IntoResponse> {
    let lib = state.library.load();

    let Some(title) = lib.get_title(&title_id) else {
        return Ok(api_failure(format!("Title ID `{title_id}` not found")).into_response());
    };
    let Some(entry) = title.entries.iter().find(|entry| entry.id == entry_id) else {
        return Ok(api_failure(format!(
            "Entry ID `{entry_id}` of `{}` not found",
            title.title
        ))
        .into_response());
    };
    let entry_pages = entry.pages;
    let is_directory = entry.path.is_dir();
    let mut etag_source = format!("{}{}", entry.path.display(), entry.mtime);
    if is_directory {
        etag_source.push_str(&humansize::format_size(entry.size_bytes, humansize::BINARY));
    }
    let etag = format!("W/{:x}", {
        use sha1::Digest;
        sha1::Sha1::digest(etag_source.as_bytes())
    });
    let cache_control = if is_directory {
        "no-cache, max-age=86400"
    } else {
        "public, max-age=86400"
    };
    let previous_etag = headers
        .get(header::IF_NONE_MATCH)
        .and_then(|value| value.to_str().ok());
    if previous_etag == Some(etag.as_str()) {
        return Ok(StatusCode::NOT_MODIFIED.into_response());
    }
    let entry_clone = entry.clone();
    drop(lib); // Release library lock early

    // Check database cache first
    match state.storage.get_dimensions(&entry_id).await {
        Ok(Some(cached)) if cached.len() == entry_pages => {
            // Cache hit with correct page count
            let dimensions = cached
                .into_iter()
                .map(|d| PageDimension {
                    width: d.width,
                    height: d.height,
                })
                .collect();
            return Ok(dimensions_response(dimensions, &etag, cache_control));
        }
        Ok(Some(cached)) => {
            tracing::debug!(
                "Dimensions cache stale for entry {} (cached: {}, actual: {})",
                entry_id,
                cached.len(),
                entry_pages
            );
        }
        Ok(None) => {
            // Cache miss - normal case
            tracing::debug!("Dimensions cache miss for entry {}", entry_id);
        }
        Err(e) => {
            // Database error - log and fall back to extraction
            tracing::error!(
                "Database error reading dimensions cache for entry {}: {}. Falling back to extraction.",
                entry_id,
                e
            );
        }
    }

    // Extract dimensions from archive (cache miss or stale)
    let mut dimensions = Vec::with_capacity(entry_pages);
    let mut dims_to_cache = Vec::with_capacity(entry_pages);

    for page_idx in 0..entry_pages {
        match media::get_page(&entry_clone, page_idx).await {
            Ok(data) => {
                let (width, height, estimated) = match get_image_dimensions(&data) {
                    Some((w, h)) => (w, h, false),
                    None => {
                        tracing::warn!(
                            "Could not determine dimensions for page {} of entry {}, using defaults",
                            page_idx,
                            entry_id
                        );
                        (1000, 1000, true)
                    }
                };
                dimensions.push(PageDimension { width, height });
                // Only cache actual dimensions, not estimated ones
                if !estimated {
                    dims_to_cache.push((page_idx, width, height));
                }
            }
            Err(e) => {
                tracing::error!(
                    "Failed to read page {} of entry {}: {}. Using estimated dimensions.",
                    page_idx,
                    entry_id,
                    e
                );
                dimensions.push(PageDimension {
                    width: 1000,
                    height: 1000,
                });
            }
        }
    }

    // Save to cache if we got all dimensions successfully
    if dims_to_cache.len() == entry_pages {
        if let Err(e) = state
            .storage
            .save_dimensions(&entry_id, &dims_to_cache)
            .await
        {
            tracing::warn!("Failed to cache dimensions for entry {}: {}", entry_id, e);
        }
    }

    Ok(dimensions_response(dimensions, &etag, cache_control))
}

/// Get image dimensions from raw image data
fn get_image_dimensions(data: &[u8]) -> Option<(u32, u32)> {
    // Try to use image crate to get dimensions without full decode
    use std::io::Cursor;

    let reader = image::ImageReader::new(Cursor::new(data))
        .with_guessed_format()
        .ok()?;

    let dims = reader.into_dimensions().ok()?;
    Some(dims)
}

#[derive(Deserialize)]
pub struct CoverUploadQuery {
    tid: String,
    eid: Option<String>,
}

/// POST /api/admin/upload/cover - Upload custom cover image
#[utoipa::path(post, path = "/api/admin/upload/cover", tag = "admin", summary = "Upload cover", params(("tid" = String, Query, description = "Title identifier"), ("eid" = Option<String>, Query, description = "Entry identifier; omit to set the title cover")), responses((status = 200, description = "Cover uploaded")))]
pub async fn upload_cover(
    State(state): State<AppState>,
    AdminOnly(_username): AdminOnly,
    axum::extract::Query(query): axum::extract::Query<CoverUploadQuery>,
    mut multipart: Multipart,
) -> axum::response::Response {
    let result: Result<()> = async {
        use tokio::io::AsyncWriteExt;

        let mut field = loop {
            match multipart.next_field().await.map_err(|error| {
                crate::error::Error::BadRequest(format!("Failed to parse multipart: {error}"))
            })? {
                Some(field) if field.name() == Some("file") => break field,
                Some(_) => {}
                None => {
                    return Err(crate::error::Error::BadRequest(
                        "No part with name `file` found".to_string(),
                    ))
                }
            }
        };
        let file_name = field
            .file_name()
            .ok_or_else(|| crate::error::Error::BadRequest("No file uploaded".to_string()))?;
        let extension = std::path::Path::new(file_name)
            .extension()
            .and_then(|extension| extension.to_str())
            .map(str::to_ascii_lowercase)
            .ok_or_else(|| {
                crate::error::Error::BadRequest(
                    "The uploaded image must be either JPEG or PNG".to_string(),
                )
            })?;
        let _mime = match extension.as_str() {
            "jpg" | "jpeg" | "jpe" | "jfif" => "image/jpeg",
            "png" => "image/png",
            "webp" => "image/webp",
            "apng" => "image/apng",
            "avif" => "image/avif",
            "gif" => "image/gif",
            "svg" => "image/svg+xml",
            "jxl" => "image/jxl",
            _ => {
                return Err(crate::error::Error::BadRequest(
                    "The uploaded image must be either JPEG or PNG".to_string(),
                ))
            }
        };

        let (title_path, entry_title) = {
            let lib = state.library.load();
            let title = lib.get_title(&query.tid).ok_or_else(|| {
                crate::error::Error::NotFound(format!("Title not found: {}", query.tid))
            })?;
            let entry_title = query
                .eid
                .as_deref()
                .map(|entry_id| {
                    lib.get_entry(&query.tid, entry_id)
                        .map(|entry| entry.title.clone())
                        .ok_or_else(|| {
                            crate::error::Error::NotFound(format!("Entry not found: {entry_id}"))
                        })
                })
                .transpose()?;
            (title.path.clone(), entry_title)
        };

        let upload_dir = state.config.upload_path.join("img");
        tokio::fs::create_dir_all(&upload_dir).await?;
        let stored_name = format!("{}.{}", uuid::Uuid::new_v4().simple(), extension);
        let destination = upload_dir.join(&stored_name);
        let mut output = tokio::fs::File::create(&destination).await?;
        let write_result: Result<()> = async {
            while let Some(chunk) = field.chunk().await.map_err(|error| {
                crate::error::Error::BadRequest(format!("Failed to read file: {error}"))
            })? {
                output.write_all(&chunk).await?;
            }
            Ok(())
        }
        .await;
        drop(output);
        if let Err(error) = write_result {
            let _ = tokio::fs::remove_file(&destination).await;
            return Err(error);
        }

        let url = format!("/uploads/img/{stored_name}");
        state
            .library
            .load_full()
            .metadata()
            .update(&title_path, |info| {
                if let Some(entry_title) = entry_title {
                    info.entry_cover_url.insert(entry_title, url);
                } else {
                    info.cover_url = url;
                }
            })
            .await?;
        Ok(())
    }
    .await;

    match result {
        Ok(()) => Json(serde_json::json!({ "success": true })).into_response(),
        Err(error) => Json(serde_json::json!({
            "success": false,
            "error": error.to_string()
        }))
        .into_response(),
    }
}

#[cfg(test)]
mod parity_contract_tests {
    use super::page_index;

    #[test]
    fn reader_pages_are_one_based() {
        assert_eq!(page_index(i32::MIN), None);
        assert_eq!(page_index(-1), None);
        assert_eq!(page_index(0), None);
        assert_eq!(page_index(1), Some(0));
        assert_eq!(page_index(12), Some(11));
        assert_eq!(page_index(i32::MAX), Some(i32::MAX as usize - 1));
    }
}
