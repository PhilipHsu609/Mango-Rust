use std::path::Path;

use crate::error::Result;
use crate::library::{media, Entry};

/// Calculate file signature (inode on Unix, CRC32 hash on Windows)
/// Returns as String for Mango database compatibility
#[cfg(unix)]
pub(super) fn file_signature(path: &Path) -> Result<String> {
    use std::os::unix::fs::MetadataExt;
    let metadata = std::fs::metadata(path)?;
    Ok(metadata.ino().to_string())
}

/// Calculate file signature using CRC32 hash of path + file size
/// Used on Windows and other non-Unix systems
/// Returns as String for Mango database compatibility
#[cfg(not(unix))]
pub(super) fn file_signature(path: &Path) -> Result<String> {
    use crc32fast::Hasher;

    let metadata = std::fs::metadata(path)?;
    let mut hasher = Hasher::new();

    // Hash path + file size as signature
    hasher.update(path.to_string_lossy().as_bytes());
    hasher.update(&metadata.len().to_le_bytes());

    Ok((hasher.finalize() as u64).to_string())
}

/// Get directory inode (Unix only)
#[cfg(unix)]
fn dir_inode(path: &Path) -> Result<String> {
    use std::os::unix::fs::MetadataExt;
    let metadata = std::fs::metadata(path)?;
    Ok(metadata.ino().to_string())
}

/// Get directory signature using CRC32 (Windows fallback)
#[cfg(not(unix))]
fn dir_inode(path: &Path) -> Result<String> {
    use crc32fast::Hasher;
    let mut hasher = Hasher::new();
    hasher.update(path.to_string_lossy().as_bytes());
    Ok((hasher.finalize() as u64).to_string())
}

/// Calculate directory signature recursively (matches original Mango behavior)
/// Includes:
/// - Directory's own inode
/// - All supported file inodes
/// - All nested directory signatures (recursive)
///
/// Returns CRC32 checksum as String
pub(super) fn dir_signature(path: &Path) -> Result<String> {
    let mut signatures = Vec::new();

    // Include directory's own inode
    signatures.push(dir_inode(path)?);

    // Recursively collect all signatures
    let entries = std::fs::read_dir(path)?;
    for entry in entries {
        let entry = entry?;
        let entry_path = entry.path();
        let name = entry.file_name();
        let name_str = name.to_string_lossy();

        // Skip hidden files
        if name_str.starts_with('.') {
            continue;
        }

        if entry_path.is_dir() {
            // Recursively get subdirectory signature
            signatures.push(dir_signature(&entry_path)?);
        } else if media::is_supported_file(&entry_path) {
            // Get file signature
            let sig = file_signature(&entry_path)?;
            // Only add if non-zero (original Mango behavior)
            if sig != "0" {
                signatures.push(sig);
            }
        }
    }

    // Sort signatures
    signatures.sort();

    // Join and calculate CRC32 (matching original: Digest::CRC32.checksum(signatures.sort.join))
    let joined = signatures.join("");
    let checksum = crc32fast::hash(joined.as_bytes());

    Ok((checksum as u64).to_string())
}

/// Fingerprint visible library contents without opening archives. Include the
/// metadata of readable files so replacing an archive or changing loose pages
/// invalidates the cached title even when the filename stays the same.
pub(super) fn calculate_contents_signature(path: &Path) -> Result<String> {
    use sha1::{Digest, Sha1};
    use std::fs;

    fn visit(path: &Path, hasher: &mut Sha1) -> Result<()> {
        let mut children = fs::read_dir(path)?.collect::<std::io::Result<Vec<_>>>()?;
        children.sort_by_key(|entry| entry.file_name());
        for child in children {
            let name = child.file_name();
            if name.to_string_lossy().starts_with('.') {
                continue;
            }
            let child_path = child.path();
            let metadata = fs::metadata(&child_path)?;
            if metadata.is_dir() {
                hasher.update(b"d");
                hasher.update(name.as_encoded_bytes());
                hasher.update([0]);
                visit(&child_path, hasher)?;
                hasher.update(b"e");
            } else if metadata.is_file() && media::is_supported_file(&child_path) {
                hasher.update(b"f");
                hasher.update(name.as_encoded_bytes());
                hasher.update([0]);
                hasher.update(metadata.len().to_le_bytes());
                let modified = metadata
                    .modified()?
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default();
                hasher.update(modified.as_secs().to_le_bytes());
                hasher.update(modified.subsec_nanos().to_le_bytes());
                #[cfg(unix)]
                {
                    use std::os::unix::fs::MetadataExt;
                    hasher.update(metadata.ino().to_le_bytes());
                    hasher.update(metadata.ctime().to_le_bytes());
                    hasher.update(metadata.ctime_nsec().to_le_bytes());
                }
            }
        }
        Ok(())
    }

    let mut hasher = Sha1::new();
    visit(path, &mut hasher)?;
    Ok(format!("v2:{:x}", hasher.finalize()))
}

pub(super) fn filesystem_ctime(metadata: &std::fs::Metadata) -> i64 {
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

/// Get the file ctime, reading metadata for older cached entries.
pub(crate) async fn date_added_timestamp(entry: &Entry) -> Result<i64> {
    if entry.ctime != 0 {
        return Ok(entry.ctime);
    }

    let metadata = tokio::fs::metadata(&entry.path).await?;
    Ok(filesystem_ctime(&metadata))
}
