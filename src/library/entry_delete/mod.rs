//! Permanently delete one archive entry from the library folder.
//!
//! Crystal Mango never deletes library files; this is an intended divergence.
//! See `docs/archive-entry-delete-design.md`.

use std::path::Path;
use std::sync::Arc;
use std::time::UNIX_EPOCH;

use serde::Serialize;

use crate::error::Error;
use crate::library::scan::{rescan_and_publish, SCAN_LOCK};
use crate::library::{Entry, Library, LibraryStats, SharedLibrary, Title};
use crate::{Config, Storage};

/// What the confirmation dialog shows, and the values the delete rechecks.
#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct DeletePreview {
    /// Archive file name, with its extension.
    pub entry_name: String,
    /// Path relative to the library root.
    pub relative_path: String,
    pub size_bytes: u64,
    pub size_label: String,
    /// Modification time in nanoseconds since the Unix epoch, as a decimal
    /// string because JavaScript numbers cannot hold it exactly.
    pub mtime_ns: String,
    /// Whether the delete leaves the entry's title empty, so the scan drops it.
    pub removes_title: bool,
    /// Page to show after the delete.
    pub redirect: String,
}

#[derive(Debug)]
pub struct DeleteOutcome {
    pub removes_title: bool,
    pub redirect: String,
    /// Cleanup steps that failed after the file was removed.
    pub warnings: Vec<String>,
}

#[derive(Debug)]
pub enum DeleteError {
    /// The entry or its file is gone.
    NotFound,
    /// The file's size or modification time differs from the confirmation.
    Changed,
    /// Not an archive file (a loose-image folder or another file type).
    Refused,
    Failed(Error),
}

/// A confirmed delete, with the values the admin saw.
#[derive(Debug, Clone)]
pub struct DeleteRequest {
    pub admin: String,
    pub title_id: String,
    pub entry_id: String,
    pub size: u64,
    pub mtime_ns: u128,
}

/// Describe the entry without changing anything on disk.
pub async fn preview(
    library: &Library,
    title_id: &str,
    entry_id: &str,
) -> Result<DeletePreview, DeleteError> {
    let (title, entry) = lookup(library, title_id, entry_id)?;
    inspect(library, title, entry).await
}

/// Delete the entry's file, rescan, then remove its metadata keys and row.
/// Callers run this in a spawned task so a dropped request cannot stop it halfway.
pub async fn delete(
    shared: SharedLibrary,
    storage: Storage,
    config: Arc<Config>,
    request: DeleteRequest,
) -> Result<DeleteOutcome, DeleteError> {
    let mut checked = None;
    let result = delete_locked(&shared, &storage, &config, &request, &mut checked).await;
    let (relative_path, size_bytes) = checked
        .map(|preview| (preview.relative_path, preview.size_bytes.to_string()))
        .unwrap_or_default();
    let (outcome, warnings): (&str, &[String]) = match &result {
        Ok(outcome) => ("deleted", &outcome.warnings),
        Err(DeleteError::NotFound) => ("not_found", &[]),
        Err(DeleteError::Changed) => ("changed", &[]),
        Err(DeleteError::Refused) => ("refused", &[]),
        Err(DeleteError::Failed(_)) => ("failed", &[]),
    };
    let error = match &result {
        Err(DeleteError::Failed(error)) => error.to_string(),
        _ => String::new(),
    };
    tracing::info!(
        admin = %request.admin,
        title_id = %request.title_id,
        entry_id = %request.entry_id,
        relative_path = %relative_path,
        size_bytes = %size_bytes,
        outcome,
        ?warnings,
        error = %error,
        "Archive entry delete"
    );
    result
}

async fn delete_locked(
    shared: &SharedLibrary,
    storage: &Storage,
    config: &Config,
    request: &DeleteRequest,
    checked: &mut Option<DeletePreview>,
) -> Result<DeleteOutcome, DeleteError> {
    let guard = SCAN_LOCK.lock().await;
    let library = shared.load_full();
    let (title, entry) = lookup(&library, &request.title_id, &request.entry_id)?;
    let preview = inspect(&library, title, entry).await?;
    *checked = Some(preview.clone());
    if preview.size_bytes != request.size || preview.mtime_ns != request.mtime_ns.to_string() {
        return Err(DeleteError::Changed);
    }

    // Without a cache file, a crash after the unlink makes the next boot scan
    // instead of restoring a cached tree that still lists the entry.
    let file_manager = library.cache().lock().await.file_manager();
    file_manager.delete().await.map_err(DeleteError::Failed)?;
    tokio::fs::remove_file(&entry.path)
        .await
        .map_err(io_error)?;

    let rescan = rescan_and_publish(shared, storage, config, &guard).await;
    let warnings = cleanup_after_rescan(shared, storage, title, entry, rescan).await;
    Ok(DeleteOutcome {
        removes_title: preview.removes_title,
        redirect: preview.redirect,
        warnings,
    })
}

/// Remove the entry's metadata keys and database row once the rescan has
/// published a snapshot without it, so no progress save can write the keys back.
async fn cleanup_after_rescan(
    shared: &SharedLibrary,
    storage: &Storage,
    title: &Title,
    entry: &Entry,
    rescan: crate::error::Result<LibraryStats>,
) -> Vec<String> {
    if let Err(error) = rescan {
        // A failed scan republishes the previous snapshot, which still lists the entry.
        let warning = format!(
            "The file was deleted, but the library rescan failed: {error}. \
             After the next scan, forget the entry on Missing items."
        );
        tracing::warn!("{warning}");
        return vec![warning];
    }
    let mut warnings = Vec::new();
    if let Err(error) = shared
        .load()
        .metadata()
        .remove_entry(&title.path, &entry.title)
        .await
    {
        warnings.push(format!(
            "Reading data for this entry could not be removed: {error}"
        ));
    }
    match storage.delete_missing_entry(&entry.id).await {
        Ok(0) => warnings.push(
            "The entry's database record was not marked missing; \
             check Missing items after the next scan."
                .to_string(),
        ),
        Ok(_) => {}
        Err(error) => warnings.push(format!(
            "The entry's database record could not be removed: {error}. \
             Forget it on Missing items."
        )),
    }
    for warning in &warnings {
        tracing::warn!("{warning}");
    }
    warnings
}

fn lookup<'a>(
    library: &'a Library,
    title_id: &str,
    entry_id: &str,
) -> Result<(&'a Title, &'a Entry), DeleteError> {
    let title = library.get_title(title_id).ok_or(DeleteError::NotFound)?;
    let entry = title
        .entries
        .iter()
        .find(|entry| entry.id == entry_id)
        .ok_or(DeleteError::NotFound)?;
    Ok((title, entry))
}

async fn inspect(
    library: &Library,
    title: &Title,
    entry: &Entry,
) -> Result<DeletePreview, DeleteError> {
    if entry.is_directory {
        return Err(DeleteError::Refused);
    }
    let link = tokio::fs::symlink_metadata(&entry.path)
        .await
        .map_err(io_error)?;
    if !link.is_file() && !link.file_type().is_symlink() {
        return Err(DeleteError::Refused);
    }
    // Follow a link so the size and time describe the archive's contents.
    let metadata = tokio::fs::metadata(&entry.path).await.map_err(io_error)?;
    if !metadata.is_file() {
        return Err(DeleteError::Refused);
    }
    let mtime_ns = metadata
        .modified()
        .map_err(|error| DeleteError::Failed(error.into()))?
        .duration_since(UNIX_EPOCH)
        .map_err(|error| DeleteError::Failed(Error::Internal(error.to_string())))?
        .as_nanos();
    let (removes_title, redirect) = destination(library, title);
    Ok(DeletePreview {
        entry_name: file_name(&entry.path),
        relative_path: entry
            .path
            .strip_prefix(library.path())
            .unwrap_or(&entry.path)
            .to_string_lossy()
            .into_owned(),
        size_bytes: metadata.len(),
        size_label: humansize::format_size(metadata.len(), humansize::BINARY),
        mtime_ns: mtime_ns.to_string(),
        removes_title,
        redirect,
    })
}

/// Where the browser goes after deleting the entry's only remaining content in
/// `title`: titles left with no content disappear from the library on rescan.
fn destination(library: &Library, title: &Title) -> (bool, String) {
    let survives = title.entries.len() > 1 || title.nested_titles.iter().any(has_content);
    if survives {
        return (false, format!("/book/{}", title.id));
    }
    let mut emptied = title.id.as_str();
    for parent in library.parent_titles(title).into_iter().rev() {
        let keeps_content = !parent.entries.is_empty()
            || parent
                .nested_titles
                .iter()
                .any(|nested| nested.id != emptied && has_content(nested));
        if keeps_content {
            return (true, format!("/book/{}", parent.id));
        }
        emptied = parent.id.as_str();
    }
    (true, "/library".to_string())
}

fn has_content(title: &Title) -> bool {
    !title.entries.is_empty() || title.nested_titles.iter().any(has_content)
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn io_error(error: std::io::Error) -> DeleteError {
    if error.kind() == std::io::ErrorKind::NotFound {
        DeleteError::NotFound
    } else {
        DeleteError::Failed(error.into())
    }
}

#[cfg(test)]
mod tests;
