use std::path::{Path, PathBuf};
use uuid::Uuid;

use crate::error::Result;

/// Represents a single readable entry (chapter/volume)
/// Can be a ZIP/CBZ archive or a directory containing images
#[derive(Debug, Clone)]
pub struct Entry {
    /// Unique identifier (persisted in database)
    pub id: String,

    /// Absolute path to the archive file or directory
    pub path: PathBuf,

    /// Display name (filename without extension)
    pub title: String,

    /// File signature (inode on Unix, CRC32 on Windows)
    pub signature: u64,

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
        let mtime = metadata.modified()?
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        // Extract image list from archive
        let image_files = extract_image_list(&path)?;
        let pages = image_files.len();

        Ok(Self {
            id: Uuid::new_v4().to_string(),
            path,
            title,
            signature: 0, // Will be set later
            mtime,
            pages,
            image_files,
        })
    }

    /// Get page image data from archive
    pub async fn get_page(&self, page: usize) -> Result<Vec<u8>> {
        if page >= self.pages {
            return Err(crate::error::Error::NotFound);
        }

        let image_name = &self.image_files[page];
        extract_image_from_archive(&self.path, image_name)
    }

    /// Generate file signature for change detection
    pub fn calculate_signature(&mut self) -> Result<()> {
        self.signature = file_signature(&self.path)?;
        Ok(())
    }
}

/// Extract list of image filenames from a ZIP archive
fn extract_image_list(archive_path: &Path) -> Result<Vec<String>> {
    let file = std::fs::File::open(archive_path)?;
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
}

/// Extract a single image from ZIP archive
fn extract_image_from_archive(archive_path: &Path, image_name: &str) -> Result<Vec<u8>> {
    use std::io::Read;

    let file = std::fs::File::open(archive_path)?;
    let mut archive = zip::ZipArchive::new(file)?;

    let mut image_file = archive.by_name(image_name)?;
    let mut buffer = Vec::new();
    image_file.read_to_end(&mut buffer)?;

    Ok(buffer)
}

/// Check if filename has an image extension
fn is_image_file(filename: &str) -> bool {
    let lower = filename.to_lowercase();
    lower.ends_with(".jpg")
        || lower.ends_with(".jpeg")
        || lower.ends_with(".png")
        || lower.ends_with(".gif")
        || lower.ends_with(".webp")
        || lower.ends_with(".bmp")
}

/// Calculate file signature (inode on Unix, CRC32 hash on Windows)
/// Matches original Mango's file signature behavior
#[cfg(unix)]
fn file_signature(path: &Path) -> Result<u64> {
    use std::os::unix::fs::MetadataExt;
    let metadata = std::fs::metadata(path)?;
    Ok(metadata.ino())
}

#[cfg(not(unix))]
fn file_signature(path: &Path) -> Result<u64> {
    use crc32fast::Hasher;

    let metadata = std::fs::metadata(path)?;
    let mut hasher = Hasher::new();

    // Hash path + file size as signature
    hasher.update(path.to_string_lossy().as_bytes());
    hasher.update(&metadata.len().to_le_bytes());

    Ok(hasher.finalize() as u64)
}
