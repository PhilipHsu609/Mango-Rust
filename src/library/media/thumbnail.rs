use crate::error::Result;
use crate::library::Entry;

use super::get_page;

/// Generate thumbnail from first page
/// Returns (thumbnail_data, mime_type, size)
pub async fn generate_thumbnail(
    entry: &Entry,
    db: &sqlx::SqlitePool,
) -> Result<Option<(Vec<u8>, String, usize)>> {
    if entry.err_msg.is_some() {
        return Ok(None);
    }

    // Get first page
    let page_data = match get_page(entry, 0).await {
        Ok(data) => data,
        Err(e) => {
            tracing::warn!(
                "Failed to get first page for thumbnail of {}: {}",
                entry.title,
                e
            );
            return Ok(None);
        }
    };

    // Load image
    let img = match image::load_from_memory(&page_data) {
        Ok(img) => img,
        Err(e) => {
            tracing::warn!(
                "Failed to load image for thumbnail of {}: {}",
                entry.title,
                e
            );
            return Ok(None);
        }
    };

    // Resize based on aspect ratio (matching original Mango logic)
    let (width, height) = (img.width(), img.height());
    let thumbnail = if height > width {
        // Portrait: resize to width 200
        img.resize(200, u32::MAX, image::imageops::FilterType::Lanczos3)
    } else {
        // Landscape: resize to height 300
        img.resize(u32::MAX, 300, image::imageops::FilterType::Lanczos3)
    };

    // Encode to JPEG
    let mut buffer = Vec::new();
    let mut cursor = std::io::Cursor::new(&mut buffer);

    match thumbnail.write_to(&mut cursor, image::ImageFormat::Jpeg) {
        Ok(_) => {}
        Err(e) => {
            tracing::warn!("Failed to encode thumbnail for {}: {}", entry.title, e);
            return Ok(None);
        }
    }

    let size = buffer.len() as i64;
    let mime = "image/jpeg".to_string();

    // Get filename from first image
    let filename = entry
        .image_files
        .first()
        .map(|s| s.as_str())
        .unwrap_or("thumbnail.jpg")
        .to_string();

    // Save to database
    sqlx::query!(
        "INSERT OR REPLACE INTO thumbnails (id, data, filename, mime, size) VALUES (?, ?, ?, ?, ?)",
        entry.id,
        buffer,
        filename,
        mime,
        size
    )
    .execute(db)
    .await?;

    Ok(Some((buffer, mime, size as usize)))
}

/// Get thumbnail from database
pub async fn get_thumbnail(
    entry_id: &str,
    db: &sqlx::SqlitePool,
) -> Result<Option<(Vec<u8>, String)>> {
    let result = sqlx::query!("SELECT data, mime FROM thumbnails WHERE id = ?", entry_id)
        .fetch_optional(db)
        .await?;

    Ok(result.map(|row| (row.data, row.mime)))
}
