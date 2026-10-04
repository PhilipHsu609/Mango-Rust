use std::path::PathBuf;
use uuid::Uuid;

use super::filesystem::{dir_signature, file_signature, filesystem_ctime};
use crate::error::Result;
use crate::library::{media, Entry, Title};

/// Create a new Title by scanning a directory
pub(super) async fn title_from_directory(path: PathBuf) -> Result<Title> {
    let title = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("Unknown")
        .to_string();
    let id = Uuid::new_v4().to_string();
    let mut mtime = tokio::fs::metadata(&path)
        .await?
        .modified()?
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

    let mut archive_paths = Vec::new();
    let mut child_paths = Vec::new();
    let mut dir_entries = tokio::fs::read_dir(&path).await?;
    while let Some(entry) = dir_entries.next_entry().await? {
        let entry_path = entry.path();
        if entry_path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with('.'))
        {
            continue;
        }
        if entry_path.is_dir() {
            child_paths.push(entry_path);
        } else if media::is_archive(&entry_path) {
            archive_paths.push(entry_path);
        }
    }

    let entry_tasks: Vec<_> = archive_paths
        .into_iter()
        .map(|entry_path| {
            tokio::spawn(async move {
                let mut entry = entry_from_archive(entry_path).await?;
                calculate_entry_signature(&mut entry)?;
                Ok::<Entry, crate::error::Error>(entry)
            })
        })
        .collect();

    let mut entries = Vec::new();
    for task in entry_tasks {
        match task.await {
            Ok(Ok(entry)) => entries.push(entry),
            Ok(Err(error)) => tracing::warn!("Failed to process entry: {}", error),
            Err(error) => tracing::warn!("Entry processing task failed: {}", error),
        }
    }

    let mut nested_titles = Vec::new();
    for child_path in child_paths {
        let mut child_title = Box::pin(title_from_directory(child_path.clone())).await?;
        if !child_title.entries.is_empty() || !child_title.nested_titles.is_empty() {
            child_title.parent_id = Some(id.clone());
            mtime = mtime.max(child_title.mtime);
            nested_titles.push(child_title);
        }

        if let Some(mut entry) = entry_from_directory(child_path).await? {
            calculate_entry_signature(&mut entry)?;
            mtime = mtime.max(entry.mtime);
            entries.push(entry);
        }
    }

    entries.sort_by(|a, b| natord::compare(&a.title, &b.title));
    nested_titles.sort_by(|a, b| natord::compare(&a.title, &b.title));
    mtime = mtime.max(entries.iter().map(|entry| entry.mtime).max().unwrap_or(0));

    let signature = dir_signature(&path)?;
    let contents_signature = String::new();

    Ok(Title {
        id,
        path,
        title,
        signature,
        contents_signature,
        mtime,
        entries,
        parent_id: None,
        nested_titles,
    })
}

/// Create a new Entry from a file path (ZIP/CBZ archive)
pub(super) async fn entry_from_archive(path: PathBuf) -> Result<Entry> {
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
    let (image_files, err_msg) = match media::image_list(&path).await {
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

    Ok(Entry {
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
pub(super) async fn entry_from_directory(path: PathBuf) -> Result<Option<Entry>> {
    let mut dir_entries = tokio::fs::read_dir(&path).await?;
    let mut image_paths = Vec::new();
    while let Some(entry) = dir_entries.next_entry().await? {
        let image_path = entry.path();
        if image_path.is_file()
            && image_path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| !name.starts_with('.') && media::is_image_file(name))
        {
            image_paths.push(image_path);
        }
    }
    image_paths
        .sort_by(|left, right| natord::compare(&left.to_string_lossy(), &right.to_string_lossy()));
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

    Ok(Some(Entry {
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

/// Generate file or directory-entry signature for change detection.
fn calculate_entry_signature(entry: &mut Entry) -> Result<()> {
    if entry.path.is_dir() {
        use sha1::{Digest, Sha1};

        let mut hasher = Sha1::new();
        for image in &entry.image_files {
            let signature = file_signature(&entry.path.join(image))?;
            hasher.update(signature.as_bytes());
        }
        entry.signature = format!("{:x}", hasher.finalize());
    } else {
        entry.signature = file_signature(&entry.path)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::title_from_directory;

    #[tokio::test]
    async fn scans_nested_titles_and_loose_image_entries() {
        let library = tempfile::tempdir().unwrap();
        let root = library.path().join("Series");
        let chapter = root.join("Volume 1/Chapters/Chapter 1");
        std::fs::create_dir_all(&chapter).unwrap();
        std::fs::write(chapter.join("001.png"), b"page").unwrap();

        let title = title_from_directory(root).await.unwrap();
        assert_eq!(title.nested_titles.len(), 1);
        let volume = &title.nested_titles[0];
        assert_eq!(volume.parent_id.as_deref(), Some(title.id.as_str()));
        assert_eq!(volume.nested_titles.len(), 1);
        let chapters = &volume.nested_titles[0];
        assert_eq!(chapters.parent_id.as_deref(), Some(volume.id.as_str()));
        assert_eq!(chapters.entries.len(), 1);

        let entry = &chapters.entries[0];
        assert_eq!(entry.title, "Chapter 1");
        assert_eq!(entry.pages, 1);
        assert_eq!(entry.size_bytes, 4);
        assert_eq!(
            crate::library::media::get_page(entry, 0).await.unwrap(),
            b"page"
        );
    }
}

#[cfg(all(test, unix))]
mod ctime_tests {
    use super::entry_from_archive;
    use std::os::unix::fs::MetadataExt;

    #[tokio::test]
    async fn archive_entries_record_filesystem_ctime() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("empty.cbz");
        let mut empty_zip = vec![0; 22];
        empty_zip[..4].copy_from_slice(b"PK\x05\x06");
        std::fs::write(&path, empty_zip).unwrap();

        let expected_ctime = std::fs::metadata(&path).unwrap().ctime();
        let entry = entry_from_archive(path).await.unwrap();

        assert_eq!(entry.ctime, expected_ctime);
        assert_eq!(entry.pages, 0);
        assert_eq!(entry.err_msg, None);
    }
}
