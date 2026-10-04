mod archive;
mod thumbnail;

pub(super) use archive::image_list;
pub use thumbnail::{generate_thumbnail, get_thumbnail};

use std::path::Path;

use crate::error::Result;
use crate::library::Entry;

/// Get page image data from this archive or directory entry.
pub async fn get_page(entry: &Entry, page: usize) -> Result<Vec<u8>> {
    if page >= entry.pages {
        return Err(crate::error::Error::NotFound(format!(
            "Page {} out of range (0-{})",
            page,
            entry.pages.saturating_sub(1)
        )));
    }

    let image_name = &entry.image_files[page];
    if entry.path.is_dir() {
        Ok(tokio::fs::read(entry.path.join(image_name)).await?)
    } else {
        archive::extract_image_from_archive(&entry.path, image_name).await
    }
}

/// Archive formats supported by the archive decoder.
const EXTRACTABLE_ARCHIVE_EXTENSIONS: &[&str] = &["zip", "cbz", "rar", "cbr", "7z", "cb7"];

/// All recognized archives, including formats used only in filesystem signatures.
const ALL_ARCHIVE_EXTENSIONS: &[&str] = &["zip", "cbz", "rar", "cbr", "7z", "cb7", "tar", "cbt"];

/// Image formats available as reader pages.
const IMAGE_EXTENSIONS: &[&str] = &["jpg", "jpeg", "png", "gif", "webp", "bmp"];

/// Check whether an archive can be decoded.
pub(super) fn is_archive(path: &Path) -> bool {
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        let ext_lower = ext.to_lowercase();
        EXTRACTABLE_ARCHIVE_EXTENSIONS.contains(&ext_lower.as_str())
    } else {
        false
    }
}

/// Check if filename has an image extension
/// Takes &str because it's used for filenames from inside ZIP archives
pub(super) fn is_image_file(filename: &str) -> bool {
    if let Some(ext) = filename.rsplit('.').next() {
        let ext_lower = ext.to_lowercase();
        IMAGE_EXTENSIONS.contains(&ext_lower.as_str())
    } else {
        false
    }
}

/// Check if file is a supported archive or image file
/// Used for directory signature calculation - recognizes all media types
pub(super) fn is_supported_file(path: &Path) -> bool {
    if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
        let ext_lower = ext.to_lowercase();
        ALL_ARCHIVE_EXTENSIONS.contains(&ext_lower.as_str())
            || IMAGE_EXTENSIONS.contains(&ext_lower.as_str())
    } else {
        false
    }
}
