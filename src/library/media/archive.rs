use std::path::Path;

use super::is_image_file;
use crate::error::Result;

/// Extract list of image filenames from an archive (ZIP, RAR, 7z)
/// Uses spawn_blocking to avoid blocking the async runtime
pub(in crate::library) async fn image_list(archive_path: &Path) -> Result<Vec<String>> {
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
pub(super) async fn extract_image_from_archive(
    archive_path: &Path,
    image_name: &str,
) -> Result<Vec<u8>> {
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
