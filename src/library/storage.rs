//! Frozen, scan-backed accounting of supported indexed entry payloads.

use std::collections::{HashMap, HashSet};
use std::path::{Component, Path};
use std::sync::Arc;

use serde::Serialize;
use utoipa::ToSchema;

use super::{media, Entry, Title};

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct StorageReport {
    pub measured_at: Option<i64>,
    pub total_bytes: u64,
    pub complete: bool,
    pub stale: bool,
    pub titles: Vec<StorageTitle>,
    pub issues: Vec<StorageIssue>,
}

impl StorageReport {
    /// Library size for the admin menu. `None` when no size has been measured;
    /// `≥` marks a lower bound from partial coverage.
    pub fn total_label(&self) -> Option<String> {
        let known = self.complete || self.titles.iter().any(|title| title.measured_entries > 0);
        known.then(|| {
            let prefix = if self.complete { "" } else { "≥ " };
            // Whole bytes have no fraction, matching the storage page.
            let size = if self.total_bytes < 1024 {
                format!("{} B", self.total_bytes)
            } else {
                let options = humansize::BINARY.decimal_places(1).decimal_zeroes(1);
                humansize::format_size(self.total_bytes, options)
            };
            format!("{prefix}{size}")
        })
    }
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct StorageTitle {
    pub id: String,
    pub name: String,
    pub path: String,
    pub size_bytes: u64,
    pub entry_count: usize,
    pub measured_entries: usize,
    pub complete: bool,
    pub issues: Vec<StorageIssue>,
    #[schema(no_recursion)]
    pub children: Vec<StorageTitle>,
}

#[derive(Debug, Clone, Serialize, serde::Deserialize, ToSchema)]
pub struct StorageIssue {
    pub path: String,
    pub message: String,
}

impl StorageIssue {
    pub(crate) fn new(root: &Path, path: &Path, message: impl Into<String>) -> Self {
        Self {
            // Never expose an outside-root absolute path through this API.
            path: relative_path(root, path)
                .map(|path| path.to_string_lossy().into_owned())
                .unwrap_or_default(),
            message: message.into(),
        }
    }
}

fn relative_path<'a>(root: &Path, path: &'a Path) -> Option<&'a Path> {
    let relative = path.strip_prefix(root).ok()?;
    if relative.components().any(
        |part| !matches!(part, Component::Normal(name) if !name.to_string_lossy().starts_with('.')),
    ) {
        return None;
    }
    Some(relative)
}

type PayloadPath<'a> = (&'a Path, &'a std::ffi::OsStr);

fn loose_payload_paths<'a>(root: &Path, entry: &'a Entry) -> Option<HashSet<PayloadPath<'a>>> {
    relative_path(root, &entry.path)?;
    if !entry.is_directory || entry.image_files.is_empty() {
        return None;
    }
    let mut paths = HashSet::with_capacity(entry.image_files.len());
    for name in &entry.image_files {
        let path = Path::new(name);
        if path.components().count() != 1
            || !matches!(path.components().next(), Some(Component::Normal(_)))
            || name.starts_with('.')
            || !media::is_image_file(name)
        {
            return None;
        }
        paths.insert((entry.path.as_path(), std::ffi::OsStr::new(name)));
    }
    Some(paths)
}

pub(super) fn aggregate(
    root: &Path,
    titles: &HashMap<String, Arc<Title>>,
    measured_at: Option<i64>,
    stale: bool,
    mut issues: Vec<StorageIssue>,
) -> StorageReport {
    let mut ids = HashSet::new();
    let mut paths = HashSet::new();
    // Ownership of any duplicate is deterministic, independent of HashMap order.
    let mut roots: Vec<_> = titles.values().map(Arc::as_ref).collect();
    roots.sort_by(|left, right| left.path.cmp(&right.path).then(left.id.cmp(&right.id)));
    let mut rows: Vec<_> = roots
        .into_iter()
        .map(|title| title_row(root, title, &issues, &mut ids, &mut paths))
        .collect();
    sort_rows(&mut rows);
    let mut total_bytes = 0;
    for title in &rows {
        if !add_bytes(&mut total_bytes, title.size_bytes) {
            issues.push(StorageIssue::new(
                root,
                root,
                "Library size exceeds the representable byte range",
            ));
        }
    }
    StorageReport {
        measured_at,
        total_bytes,
        complete: issues.is_empty() && rows.iter().all(|title| title.complete),
        stale,
        titles: rows,
        issues,
    }
}

fn title_row<'a>(
    root: &Path,
    title: &'a Title,
    scan_issues: &[StorageIssue],
    ids: &mut HashSet<&'a str>,
    paths: &mut HashSet<PayloadPath<'a>>,
) -> StorageTitle {
    let path = relative_path(root, &title.path)
        .map(|path| path.to_string_lossy().into_owned())
        .unwrap_or_default();
    let mut row = StorageTitle {
        id: title.id.clone(),
        name: title.title.clone(),
        path,
        size_bytes: 0,
        entry_count: 0,
        measured_entries: 0,
        complete: true,
        issues: Vec::new(),
        children: Vec::new(),
    };
    for issue in scan_issues {
        if Path::new(&issue.path).starts_with(&row.path) {
            row.issues.push(issue.clone());
            row.complete = false;
        }
    }
    for entry in &title.entries {
        let duplicate_id = !ids.insert(entry.id.as_str());
        if !entry.is_directory
            && media::is_archive(&entry.path)
            && relative_path(root, &entry.path).is_some()
        {
            let payload = (
                entry.path.parent().unwrap(),
                entry.path.file_name().unwrap(),
            );
            if !paths.insert(payload) {
                continue;
            }
            row.entry_count += 1;
        } else {
            let Some(payloads) = loose_payload_paths(root, entry) else {
                row.entry_count += 1;
                row.complete = false;
                // Caches written before `is_directory` existed land here until rescanned.
                let message = if entry.size_known {
                    "Unsafe or unsupported indexed payload excluded"
                } else {
                    "Entry size has not been measured"
                };
                row.issues
                    .push(StorageIssue::new(root, &entry.path, message));
                continue;
            };
            if payloads.iter().all(|path| paths.contains(path)) {
                continue;
            }
            row.entry_count += 1;
            if payloads.len() != entry.image_files.len()
                || payloads.iter().any(|path| paths.contains(path))
            {
                row.complete = false;
                row.issues.push(StorageIssue::new(
                    root,
                    &entry.path,
                    "Overlapping indexed payloads excluded from this entry's total",
                ));
                continue;
            }
            paths.extend(payloads);
        }
        if duplicate_id {
            row.issues.push(StorageIssue::new(root, &entry.path, "Entry identity is shared by distinct payloads; sizes are counted by physical payload"));
        }
        if entry.size_known {
            if !add_bytes(&mut row.size_bytes, entry.size_bytes) {
                row.complete = false;
                row.issues.push(StorageIssue::new(
                    root,
                    &title.path,
                    "Title size exceeds the representable byte range",
                ));
            }
            row.measured_entries += 1;
        } else {
            row.complete = false;
            row.issues.push(StorageIssue::new(
                root,
                &entry.path,
                "Entry size has not been measured",
            ));
        }
        if let Some(message) = &entry.err_msg {
            row.issues
                .push(StorageIssue::new(root, &entry.path, message.clone()));
        }
    }
    let mut children: Vec<_> = title.nested_titles.iter().collect();
    children.sort_by(|left, right| left.path.cmp(&right.path).then(left.id.cmp(&right.id)));
    for child in children {
        let child = title_row(root, child, scan_issues, ids, paths);
        if !add_bytes(&mut row.size_bytes, child.size_bytes) {
            row.complete = false;
            row.issues.push(StorageIssue::new(
                root,
                &title.path,
                "Title size exceeds the representable byte range",
            ));
        }
        row.entry_count += child.entry_count;
        row.measured_entries += child.measured_entries;
        row.complete &= child.complete;
        row.children.push(child);
    }
    sort_rows(&mut row.children);
    row
}

fn add_bytes(total: &mut u64, bytes: u64) -> bool {
    match total.checked_add(bytes) {
        Some(sum) => {
            *total = sum;
            true
        }
        None => {
            *total = u64::MAX;
            false
        }
    }
}

fn sort_rows(rows: &mut [StorageTitle]) {
    rows.sort_by(|left, right| {
        right
            .size_bytes
            .cmp(&left.size_bytes)
            .then_with(|| left.name.cmp(&right.name))
            .then_with(|| left.path.cmp(&right.path))
            .then_with(|| left.id.cmp(&right.id))
    });
}

#[cfg(test)]
mod tests;
