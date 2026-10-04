use std::collections::HashMap;
use std::path::Path;

use crate::error::Result;
use crate::library::{Entry, Library, Title};
use crate::Storage;

struct StoredId {
    id: String,
    signature: Option<String>,
    unavailable: i64,
}

/// IDs owned by one scan worker until its completed title reaches the collector.
#[derive(Default)]
pub(super) struct PendingIds {
    pub(super) titles: Vec<(String, String, String)>,
    pub(super) entries: Vec<(String, String, String)>,
}

pub(super) struct IdIndex {
    by_path: HashMap<String, StoredId>,
    by_signature: HashMap<String, Vec<(String, String)>>,
}

impl IdIndex {
    pub(super) async fn load(storage: &Storage, table: &'static str) -> Result<Self> {
        let query = match table {
            "titles" => "SELECT id, path, signature, unavailable FROM titles",
            "ids" => "SELECT id, path, signature, unavailable FROM ids",
            _ => unreachable!("ID table is selected internally"),
        };
        let rows: Vec<(String, String, Option<String>, i64)> =
            sqlx::query_as(query).fetch_all(storage.pool()).await?;
        let mut by_path = HashMap::with_capacity(rows.len());
        let mut by_signature: HashMap<String, Vec<(String, String)>> = HashMap::new();
        for (id, path, signature, unavailable) in rows {
            if let Some(signature) = &signature {
                by_signature
                    .entry(signature.clone())
                    .or_default()
                    .push((id.clone(), path.clone()));
            }
            by_path.insert(
                path,
                StoredId {
                    id,
                    signature,
                    unavailable,
                },
            );
        }
        Ok(Self {
            by_path,
            by_signature,
        })
    }

    fn find(&self, path: &str, signature: &str) -> Option<(&str, bool)> {
        if let Some(stored) = self.by_path.get(path) {
            let unchanged =
                stored.signature.as_deref() == Some(signature) && stored.unavailable == 0;
            return Some((&stored.id, !unchanged));
        }
        self.by_signature
            .get(signature)?
            .iter()
            .max_by(|(_, left), (_, right)| {
                path_component_similarity(left, path)
                    .total_cmp(&path_component_similarity(right, path))
            })
            .map(|(id, _)| (id.as_str(), true))
    }
}

/// Bulk insert title and entry IDs in a single transaction
/// Matches the pattern from original Mango for performance
pub(super) async fn bulk_insert_ids(
    storage: &Storage,
    title_ids: &[(String, String, String)], // (id, path, signature)
    entry_ids: &[(String, String, String)], // (id, path, signature)
) -> Result<()> {
    let mut tx = storage.pool().begin().await?;

    // Insert all title IDs
    for (id, path, signature) in title_ids {
        sqlx::query(
            "INSERT INTO titles (id, path, signature, unavailable) VALUES (?, ?, ?, 0)
             ON CONFLICT(path) DO UPDATE SET id = ?, signature = ?, unavailable = 0",
        )
        .bind(id)
        .bind(path)
        .bind(signature)
        .bind(id)
        .bind(signature)
        .execute(&mut *tx)
        .await?;
    }

    // Insert all entry IDs
    for (id, path, signature) in entry_ids {
        sqlx::query(
            "INSERT INTO ids (id, path, signature, unavailable) VALUES (?, ?, ?, 0)
             ON CONFLICT(path) DO UPDATE SET id = ?, signature = ?, unavailable = 0",
        )
        .bind(id)
        .bind(path)
        .bind(signature)
        .bind(id)
        .bind(signature)
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;
    Ok(())
}

pub(super) async fn assign_title_tree_ids(
    title: &mut Title,
    parent_id: Option<String>,
    library_path: &Path,
    storage: &Storage,
    title_index: &IdIndex,
    entry_index: &IdIndex,
    pending_ids: &mut PendingIds,
) -> Result<()> {
    title.parent_id = parent_id;
    if let Some(id) = find_existing_title_id(library_path, title, storage, title_index).await? {
        title.id = id;
    } else {
        let relative_path = title
            .path
            .strip_prefix(library_path)
            .map_err(|_| {
                crate::error::Error::Internal(format!(
                    "Path {} is not within library root {}",
                    title.path.display(),
                    library_path.display()
                ))
            })?
            .to_string_lossy()
            .to_string();
        pending_ids
            .titles
            .push((title.id.clone(), relative_path, title.signature.clone()));
    }

    for entry in &mut title.entries {
        if let Some(id) = find_existing_entry_id(library_path, entry, storage, entry_index).await? {
            entry.id = id;
        } else {
            let relative_path = entry
                .path
                .strip_prefix(library_path)
                .map_err(|_| {
                    crate::error::Error::Internal(format!(
                        "Path {} is not within library root {}",
                        entry.path.display(),
                        library_path.display()
                    ))
                })?
                .to_string_lossy()
                .to_string();
            pending_ids
                .entries
                .push((entry.id.clone(), relative_path, entry.signature.clone()));
        }
    }

    let parent_id = title.id.clone();
    for nested in &mut title.nested_titles {
        Box::pin(assign_title_tree_ids(
            nested,
            Some(parent_id.clone()),
            library_path,
            storage,
            title_index,
            entry_index,
            pending_ids,
        ))
        .await?;
    }
    Ok(())
}

async fn find_existing_title_id(
    library_path: &Path,
    title: &Title,
    storage: &Storage,
    index: &IdIndex,
) -> Result<Option<String>> {
    let relative_path = title
        .path
        .strip_prefix(library_path)
        .map_err(|_| {
            crate::error::Error::Internal(format!(
                "Path {} is not within library root {}",
                title.path.display(),
                library_path.display()
            ))
        })?
        .to_string_lossy()
        .to_string();

    find_existing_id("titles", &relative_path, &title.signature, storage, index).await
}

async fn find_existing_entry_id(
    library_path: &Path,
    entry: &Entry,
    storage: &Storage,
    index: &IdIndex,
) -> Result<Option<String>> {
    let relative_path = entry
        .path
        .strip_prefix(library_path)
        .map_err(|_| {
            crate::error::Error::Internal(format!(
                "Path {} is not within library root {}",
                entry.path.display(),
                library_path.display()
            ))
        })?
        .to_string_lossy()
        .to_string();

    find_existing_id("ids", &relative_path, &entry.signature, storage, index).await
}

async fn find_existing_id(
    table: &'static str,
    path: &str,
    signature: &str,
    storage: &Storage,
    index: &IdIndex,
) -> Result<Option<String>> {
    let Some((id, should_update)) = index.find(path, signature) else {
        return Ok(None);
    };
    if should_update {
        let update_query = match table {
            "titles" => "UPDATE titles SET path = ?, signature = ?, unavailable = 0 WHERE id = ?",
            "ids" => "UPDATE ids SET path = ?, signature = ?, unavailable = 0 WHERE id = ?",
            _ => unreachable!("ID table is selected internally"),
        };
        sqlx::query(update_query)
            .bind(path)
            .bind(signature)
            .bind(id)
            .execute(storage.pool())
            .await?;
    }
    Ok(Some(id.to_owned()))
}

/// Mark database entries as unavailable if their files no longer exist
/// This is called after scan completes to detect missing files
pub(super) async fn mark_unavailable(library: &Library) -> Result<()> {
    use std::collections::HashSet;

    const CHUNK_SIZE: usize = 500; // Well under SQLite's 999 limit

    let all_titles = library.all_titles();
    let found_title_ids: HashSet<String> =
        all_titles.iter().map(|title| title.id.clone()).collect();
    let found_entry_ids: HashSet<String> = all_titles
        .iter()
        .flat_map(|title| title.entries.iter().map(|entry| entry.id.clone()))
        .collect();

    let mut tx = library.storage.pool().begin().await?;

    // 1. Find and mark missing titles as unavailable
    let db_titles: Vec<String> = sqlx::query_scalar("SELECT id FROM titles WHERE unavailable = 0")
        .fetch_all(&mut *tx)
        .await?;
    let missing_titles: Vec<&String> = db_titles
        .iter()
        .filter(|id| !found_title_ids.contains(*id))
        .collect();

    for chunk in missing_titles.chunks(CHUNK_SIZE) {
        batch_update_unavailable(&mut tx, "titles", chunk, 1).await?;
    }

    // 2. Mark missing entries, including entries beneath removed titles.
    let db_entries: Vec<String> = sqlx::query_scalar("SELECT id FROM ids WHERE unavailable = 0")
        .fetch_all(&mut *tx)
        .await?;
    let missing_entries: Vec<&String> = db_entries
        .iter()
        .filter(|id| !found_entry_ids.contains(*id))
        .collect();

    for chunk in missing_entries.chunks(CHUNK_SIZE) {
        batch_update_unavailable(&mut tx, "ids", chunk, 1).await?;
    }

    // 3. Restore previously unavailable titles that are now found
    let unavailable_titles: Vec<String> =
        sqlx::query_scalar::<_, String>("SELECT id FROM titles WHERE unavailable = 1")
            .fetch_all(&mut *tx)
            .await?;

    let restored_titles: Vec<&String> = unavailable_titles
        .iter()
        .filter(|id| found_title_ids.contains(*id))
        .collect();

    for chunk in restored_titles.chunks(CHUNK_SIZE) {
        batch_update_unavailable(&mut tx, "titles", chunk, 0).await?;
    }

    // 4. Restore previously unavailable entries that are now found
    let unavailable_entries: Vec<String> =
        sqlx::query_scalar::<_, String>("SELECT id FROM ids WHERE unavailable = 1")
            .fetch_all(&mut *tx)
            .await?;

    let restored_entries: Vec<&String> = unavailable_entries
        .iter()
        .filter(|id| found_entry_ids.contains(*id))
        .collect();

    for chunk in restored_entries.chunks(CHUNK_SIZE) {
        batch_update_unavailable(&mut tx, "ids", chunk, 0).await?;
    }

    // Log what we did
    if !missing_titles.is_empty() {
        tracing::info!("Marked {} titles as unavailable", missing_titles.len());
    }
    if !missing_entries.is_empty() {
        tracing::info!("Marked {} entries as unavailable", missing_entries.len());
    }
    if !restored_titles.is_empty() {
        tracing::info!("Restored {} titles as available", restored_titles.len());
    }
    if !restored_entries.is_empty() {
        tracing::info!("Restored {} entries as available", restored_entries.len());
    }

    tx.commit().await?;
    Ok(())
}

/// Helper: batch UPDATE with IN clause
/// Chunks are handled by caller to respect SQLite's parameter limit
async fn batch_update_unavailable(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    table: &str,
    ids: &[&String],
    unavailable: i32,
) -> Result<()> {
    if ids.is_empty() {
        return Ok(());
    }

    let placeholders = ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
    let query_str = format!(
        "UPDATE {} SET unavailable = {} WHERE id IN ({})",
        table, unavailable, placeholders
    );

    let mut query = sqlx::query(&query_str);
    for id in ids {
        query = query.bind(*id);
    }
    query.execute(&mut **tx).await?;
    Ok(())
}

fn path_component_similarity(left: &str, right: &str) -> f64 {
    let left = Path::new(left);
    let right = Path::new(right);
    let component_count = left.components().count().min(right.components().count());
    if component_count == 0 {
        return 0.0;
    }

    let matching_components = left
        .components()
        .rev()
        .zip(right.components().rev())
        .filter(|(left, right)| left == right)
        .count();
    matching_components as f64 / component_count as f64
}

#[cfg(test)]
mod path_similarity_tests {
    use super::path_component_similarity;

    #[test]
    fn moved_path_prefers_matching_trailing_components() {
        let moved_chapter =
            path_component_similarity("old/volume-1/chapter-2.cbz", "new/volume-1/chapter-2.cbz");
        let other_chapter =
            path_component_similarity("old/chapter-2/chapter-2.cbz", "new/volume-1/chapter-2.cbz");

        assert!(moved_chapter > other_chapter);
    }
}
