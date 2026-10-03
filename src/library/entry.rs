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

    /// File metadata change time, matching Mango's `ctime` date_added behavior
    #[serde(default)]
    pub ctime: i64,

    /// Modification time (for sorting)
    pub mtime: i64,

    /// Number of pages (images) in this entry
    pub pages: usize,

    /// List of image filenames (sorted)
    pub image_files: Vec<String>,

    /// Total bytes in the archive or loose-image directory.
    #[serde(default)]
    pub size_bytes: u64,

    /// Archive validation failure, if the entry cannot be read.
    #[serde(default)]
    pub err_msg: Option<String>,
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
        let ctime = filesystem_ctime(&metadata);
        let mtime = metadata
            .modified()?
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        // Archive validation errors are represented by a visible, unreadable entry.
        // Metadata and identity still belong to the file, even when it has no pages.
        let (image_files, err_msg) = match extract_image_list(&path).await {
            Ok(images) => (images, None),
            Err(crate::error::Error::Archive(error)) => {
                let message = format!("Archive error: {error}");
                tracing::warn!("Unable to extract archive {}. {}", path.display(), message);
                (Vec::new(), Some(message))
            }
            Err(crate::error::Error::Io(error))
                if error.kind() == std::io::ErrorKind::PermissionDenied =>
            {
                let message = format!("File {} is not readable.", path.display());
                tracing::warn!(
                    "{message} Please make sure the file permission is configured correctly."
                );
                (Vec::new(), Some(message))
            }
            Err(error) => return Err(error),
        };
        let pages = image_files.len();

        Ok(Self {
            id: Uuid::new_v4().to_string(),
            path,
            title,
            signature: String::new(), // Will be set later
            ctime,
            mtime,
            pages,
            image_files,
            size_bytes: metadata.len(),
            err_msg,
        })
    }

    /// Create an entry from a directory of loose image files.
    pub async fn from_directory(path: PathBuf) -> Result<Option<Self>> {
        let mut dir_entries = tokio::fs::read_dir(&path).await?;
        let mut image_paths = Vec::new();
        while let Some(entry) = dir_entries.next_entry().await? {
            let image_path = entry.path();
            if image_path.is_file()
                && image_path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| !name.starts_with('.') && is_image_file(name))
            {
                image_paths.push(image_path);
            }
        }
        image_paths.sort_by(|left, right| {
            natord::compare(&left.to_string_lossy(), &right.to_string_lossy())
        });
        if image_paths.is_empty() {
            return Ok(None);
        }

        let mut size_bytes = 0;
        let mut mtime = 0;
        let mut image_files = Vec::with_capacity(image_paths.len());
        for image_path in &image_paths {
            let metadata = tokio::fs::metadata(image_path).await?;
            size_bytes += metadata.len();
            mtime = mtime.max(
                metadata
                    .modified()?
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs() as i64,
            );
            image_files.push(
                image_path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or_default()
                    .to_string(),
            );
        }
        let metadata = tokio::fs::metadata(&path).await?;

        Ok(Some(Self {
            id: Uuid::new_v4().to_string(),
            title: path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("Unknown")
                .to_string(),
            path,
            signature: String::new(),
            ctime: filesystem_ctime(&metadata),
            mtime,
            pages: image_files.len(),
            image_files,
            size_bytes,
            err_msg: None,
        }))
    }

    /// Get the file ctime, reading metadata for older cached entries.
    pub(crate) async fn date_added_timestamp(&self) -> Result<i64> {
        if self.ctime != 0 {
            return Ok(self.ctime);
        }

        let metadata = tokio::fs::metadata(&self.path).await?;
        Ok(filesystem_ctime(&metadata))
    }

    /// Get page image data from this archive or directory entry.
    pub async fn get_page(&self, page: usize) -> Result<Vec<u8>> {
        if page >= self.pages {
            return Err(crate::error::Error::NotFound(format!(
                "Page {} out of range (0-{})",
                page,
                self.pages.saturating_sub(1)
            )));
        }

        let image_name = &self.image_files[page];
        if self.path.is_dir() {
            Ok(tokio::fs::read(self.path.join(image_name)).await?)
        } else {
            extract_image_from_archive(&self.path, image_name).await
        }
    }

    /// Generate file or directory-entry signature for change detection.
    pub fn calculate_signature(&mut self) -> Result<()> {
        if self.path.is_dir() {
            use sha1::{Digest, Sha1};

            let mut hasher = Sha1::new();
            for image in &self.image_files {
                let signature = crate::util::file_signature(&self.path.join(image))?;
                hasher.update(signature.as_bytes());
            }
            self.signature = format!("{:x}", hasher.finalize());
        } else {
            self.signature = crate::util::file_signature(&self.path)?;
        }
        Ok(())
    }

    /// Generate thumbnail from first page
    /// Returns (thumbnail_data, mime_type, size)
    pub async fn generate_thumbnail(
        &self,
        db: &sqlx::SqlitePool,
    ) -> Result<Option<(Vec<u8>, String, usize)>> {
        if self.err_msg.is_some() {
            return Ok(None);
        }

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

fn filesystem_ctime(metadata: &std::fs::Metadata) -> i64 {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        metadata.ctime()
    }

    #[cfg(not(unix))]
    {
        metadata
            .created()
            .or_else(|_| metadata.modified())
            .ok()
            .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|duration| duration.as_secs() as i64)
            .unwrap_or_default()
    }
}

/// Extract list of image filenames from an archive (ZIP, RAR, 7z)
/// Uses spawn_blocking to avoid blocking the async runtime
async fn extract_image_list(archive_path: &Path) -> Result<Vec<String>> {
    let path = archive_path.to_path_buf();

    tokio::task::spawn_blocking(move || {
        use std::io::{Read, Seek};

        let mut file = std::fs::File::open(&path)?;
        // compress-tools enables libarchive's "raw" fallback: arbitrary data can
        // otherwise appear to be a valid archive with no image entries.
        let mut magic = [0; 8];
        if let Err(error) = file.read_exact(&mut magic) {
            if error.kind() == std::io::ErrorKind::UnexpectedEof {
                return Err(crate::error::Error::Archive(
                    compress_tools::Error::Extraction("Unrecognized archive format".to_string()),
                ));
            }
            return Err(error.into());
        }
        let zip = matches!(&magic[..4], b"PK\x03\x04" | b"PK\x05\x06" | b"PK\x07\x08");
        let rar =
            magic.starts_with(b"Rar!\x1a\x07\x00") || magic.starts_with(b"Rar!\x1a\x07\x01\x00");
        let seven_zip = magic.starts_with(b"7z\xbc\xaf\x27\x1c");
        let is_zip_path = path
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| {
                extension.eq_ignore_ascii_case("zip") || extension.eq_ignore_ascii_case("cbz")
            });
        if !(zip || (!is_zip_path && (rar || seven_zip))) {
            return Err(crate::error::Error::Archive(
                compress_tools::Error::Extraction("Unrecognized archive format".to_string()),
            ));
        }
        file.rewind()?;
        let files =
            compress_tools::list_archive_files(file).map_err(crate::error::Error::Archive)?;

        let mut images: Vec<String> = files
            .into_iter()
            .filter(|name| is_image_file(name))
            .collect();

        // Sort naturally (Chapter 2 before Chapter 10)
        images.sort_by(|a, b| natord::compare(a, b));

        Ok(images)
    })
    .await
    .map_err(|e| crate::error::Error::Internal(format!("Task join error: {}", e)))?
}

/// Extract a single image from archive (ZIP, RAR, 7z)
/// Uses spawn_blocking to avoid blocking the async runtime
async fn extract_image_from_archive(archive_path: &Path, image_name: &str) -> Result<Vec<u8>> {
    let path = archive_path.to_path_buf();
    let name = image_name.to_string();

    tokio::task::spawn_blocking(move || {
        let file = std::fs::File::open(&path)?;
        let mut buffer = Vec::new();

        compress_tools::uncompress_archive_file(file, &mut buffer, &name).map_err(|e| {
            crate::error::Error::Internal(format!("Failed to extract {}: {}", name, e))
        })?;

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

#[cfg(all(test, unix))]
mod tests {
    use super::Entry;
    use std::os::unix::fs::MetadataExt;

    #[tokio::test]
    async fn archive_entries_record_filesystem_ctime() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("empty.cbz");
        let mut empty_zip = vec![0; 22];
        empty_zip[..4].copy_from_slice(b"PK\x05\x06");
        std::fs::write(&path, empty_zip).unwrap();

        let expected_ctime = std::fs::metadata(&path).unwrap().ctime();
        let entry = Entry::from_archive(path).await.unwrap();

        assert_eq!(entry.ctime, expected_ctime);
        assert_eq!(entry.pages, 0);
        assert_eq!(entry.err_msg, None);
    }
}
