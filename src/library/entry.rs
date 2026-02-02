use std::path::{Path, PathBuf};
use uuid::Uuid;

use crate::error::Result;

/// Represents a single readable entry (chapter/volume)
/// Can be a ZIP/CBZ archive or a directory containing images
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Entry {
    /// Unique identifier (persisted in database)
    pub id: String,

    /// Absolute path to the archive file or directory
    pub path: PathBuf,

    /// Display name (filename without extension)
    pub title: String,

    /// File signature (inode on Unix, CRC32 on Windows) - stored as TEXT for Mango compatibility
    pub signature: String,

    /// Modification time (for sorting)
    pub mtime: i64,

    /// Number of pages (images) in this entry
    pub pages: usize,

    /// List of image filenames (sorted)
    pub image_files: Vec<String>,
}

impl Entry {
    /// Create a new Entry from a file path (ZIP/CBZ archive)
    pub async fn from_archive(path: PathBuf) -> Result<Self> {
        let title = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Unknown")
            .to_string();

        let metadata = tokio::fs::metadata(&path).await?;
        let mtime = metadata
            .modified()?
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        // Extract image list from archive (moved to blocking task to avoid blocking async runtime)
        let image_files = extract_image_list(&path).await?;
        let pages = image_files.len();

        Ok(Self {
            id: Uuid::new_v4().to_string(),
            path,
            title,
            signature: String::new(), // Will be set later
            mtime,
            pages,
            image_files,
        })
    }

    /// Get page image data from archive
    pub async fn get_page(&self, page: usize) -> Result<Vec<u8>> {
        if page >= self.pages {
            return Err(crate::error::Error::NotFound(format!(
                "Page {} out of range (0-{})",
                page,
                self.pages - 1
            )));
        }

        let image_name = &self.image_files[page];
        extract_image_from_archive(&self.path, image_name).await
    }

    /// Generate file signature for change detection
    pub fn calculate_signature(&mut self) -> Result<()> {
        self.signature = crate::util::file_signature(&self.path)?;
        Ok(())
    }

    /// Generate thumbnail from first page
    /// Returns (thumbnail_data, mime_type, size)
    pub async fn generate_thumbnail(
        &self,
        db: &sqlx::SqlitePool,
    ) -> Result<Option<(Vec<u8>, String, usize)>> {
        // Get first page
        let page_data = match self.get_page(0).await {
            Ok(data) => data,
            Err(e) => {
                tracing::warn!(
                    "Failed to get first page for thumbnail of {}: {}",
                    self.title,
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
                    self.title,
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
                tracing::warn!("Failed to encode thumbnail for {}: {}", self.title, e);
                return Ok(None);
            }
        }

        let size = buffer.len() as i64;
        let mime = "image/jpeg".to_string();

        // Get filename from first image
        let filename = self
            .image_files
            .first()
            .map(|s| s.as_str())
            .unwrap_or("thumbnail.jpg")
            .to_string();

        // Save to database
        sqlx::query!(
            "INSERT OR REPLACE INTO thumbnails (id, data, filename, mime, size) VALUES (?, ?, ?, ?, ?)",
            self.id,
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

    /// Save custom thumbnail to database (for uploaded covers)
    pub async fn save_thumbnail(
        entry_id: &str,
        data: &[u8],
        mime: &str,
        db: &sqlx::SqlitePool,
    ) -> Result<()> {
        let size = data.len() as i64;

        // Insert or replace thumbnail
        sqlx::query!(
            "INSERT OR REPLACE INTO thumbnails (id, data, mime, size) VALUES (?, ?, ?, ?)",
            entry_id,
            data,
            mime,
            size
        )
        .execute(db)
        .await?;

        Ok(())
    }
}

/// Extract list of image filenames from a ZIP archive
/// Uses spawn_blocking to avoid blocking the async runtime
async fn extract_image_list(archive_path: &Path) -> Result<Vec<String>> {
    let path = archive_path.to_path_buf();

    tokio::task::spawn_blocking(move || {
        let file = std::fs::File::open(&path)?;
        let mut archive = zip::ZipArchive::new(file)?;

        let mut images = Vec::new();

        for i in 0..archive.len() {
            let file = archive.by_index(i)?;
            let name = file.name().to_string();

            if is_image_file(&name) {
                images.push(name);
            }
        }

        // Sort naturally (Chapter 2 before Chapter 10)
        images.sort_by(|a, b| natord::compare(a, b));

        Ok(images)
    })
    .await
    .map_err(|e| crate::error::Error::Internal(format!("Task join error: {}", e)))?
}

/// Extract a single image from ZIP archive
/// Uses spawn_blocking to avoid blocking the async runtime
async fn extract_image_from_archive(archive_path: &Path, image_name: &str) -> Result<Vec<u8>> {
    use std::io::Read;

    let path = archive_path.to_path_buf();
    let name = image_name.to_string();

    tokio::task::spawn_blocking(move || {
        let file = std::fs::File::open(&path)?;
        let mut archive = zip::ZipArchive::new(file)?;

        let mut image_file = archive.by_name(&name)?;
        let mut buffer = Vec::new();
        image_file.read_to_end(&mut buffer)?;

        Ok(buffer)
    })
    .await
    .map_err(|e| crate::error::Error::Internal(format!("Task join error: {}", e)))?
}

/// Check if filename has an image extension
/// Takes &str because it's used for filenames from inside ZIP archives
fn is_image_file(filename: &str) -> bool {
    if let Some(ext) = filename.rsplit('.').next() {
        let ext_lower = ext.to_lowercase();
        crate::util::IMAGE_EXTENSIONS.contains(&ext_lower.as_str())
    } else {
        false
    }
}

impl super::Sortable for Entry {
    fn sort_name(&self) -> &str {
        &self.title
    }

    fn sort_mtime(&self) -> i64 {
        self.mtime
    }
}

impl super::Sortable for &Entry {
    fn sort_name(&self) -> &str {
        &self.title
    }

    fn sort_mtime(&self) -> i64 {
        self.mtime
    }
}
