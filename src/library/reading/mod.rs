mod recent;

#[cfg(test)]
mod tests;

use std::collections::HashMap;

use super::metadata::{MetadataStore, TitleInfo};
use super::ordering::{self, EntryOrdering, SortMethod, SortOptions};
use super::{Entry, Title};
use crate::error::Result;

pub(crate) use recent::{group_recent_entries, RecentEntry, RECENT_ITEMS_LIMIT};

pub fn entry_progress_fraction(progress: i32, pages: usize) -> f64 {
    if pages == 0 {
        0.0
    } else {
        progress.clamp(0, pages as i32) as f64 / pages as f64
    }
}

/// Browser cards retain their existing raw percentage calculation. Catalog and
/// continuation selection clamp independently; changing this would change badges.
pub fn entry_progress_percent(progress: i32, pages: usize) -> f32 {
    if pages == 0 {
        0.0
    } else {
        progress as f32 / pages as f32 * 100.0
    }
}

pub fn title_progress_fraction(title: &Title, store: &MetadataStore, username: &str) -> f64 {
    fn totals(title: &Title, store: &MetadataStore, username: &str) -> (usize, f64) {
        let info = store.cached(&title.path);
        let mut pages = 0;
        let mut read = 0.0;
        for entry in &title.entries {
            pages += entry.pages;
            read += info
                .as_ref()
                .and_then(|info| info.get_progress(username, &entry.title))
                .unwrap_or(0)
                .clamp(0, entry.pages as i32) as f64;
        }
        for nested in &title.nested_titles {
            let (nested_pages, nested_read) = totals(nested, store, username);
            pages += nested_pages;
            read += nested_read;
        }
        (pages, read)
    }
    let (pages, read) = totals(title, store, username);
    if pages == 0 {
        0.0
    } else {
        read / pages as f64
    }
}

pub async fn title_progress_percent(
    title: &Title,
    store: &MetadataStore,
    username: &str,
) -> Result<f32> {
    let mut pages = 0;
    let mut read = 0;
    for title in std::iter::once(title).chain(title.deep_titles()) {
        let info = store.read(&title.path).await?;
        for entry in &title.entries {
            pages += entry.pages;
            read += info
                .get_progress(username, &entry.title)
                .unwrap_or(0)
                .clamp(0, entry.pages as i32) as usize;
        }
    }
    Ok(if pages == 0 {
        0.0
    } else {
        read as f32 / pages as f32 * 100.0
    })
}

#[derive(Clone, Copy)]
pub enum StartReadingProfile {
    Catalog,
    Browser,
}

pub fn can_start_reading(title: &Title, progress: f64, profile: StartReadingProfile) -> bool {
    progress == 0.0 && (!matches!(profile, StartReadingProfile::Catalog) || title.total_pages() > 0)
}

/// Catalog continuation truncates before timestamp sorting, unlike the home
/// page's sort-before-limit feed. Preserve this established API distinction.
pub fn order_continue_candidates<T>(entries: &mut Vec<(Option<i64>, T, f64)>) {
    entries.truncate(8);
    entries.sort_by_key(|entry| std::cmp::Reverse(entry.0));
}

pub fn continue_entry<'a>(
    title: &'a Title,
    username: &str,
    info: &TitleInfo,
    overrides: &HashMap<String, String>,
) -> Option<(&'a Entry, Option<&'a Entry>)> {
    let (method, ascending) = info
        .get_sort_by(username)
        .map(|(method, ascending)| (SortMethod::parse(&method), ascending))
        .unwrap_or((SortMethod::Auto, true));
    let mut entries: Vec<_> = title
        .entries
        .iter()
        .map(|entry| {
            (
                entry,
                overrides
                    .get(&entry.id)
                    .map(String::as_str)
                    .unwrap_or(&entry.title),
            )
        })
        .collect();
    ordering::sort_entries(
        &mut entries,
        info,
        username,
        SortOptions { method, ascending },
        EntryOrdering::Continuation,
    );
    let progress = |entry: &Entry| {
        info.get_progress(username, &entry.title)
            .unwrap_or(0)
            .min(entry.pages as i32)
    };
    let mut index = entries.iter().rposition(|(entry, _)| progress(entry) > 0)?;
    if progress(entries[index].0) >= entries[index].0.pages as i32 {
        if index + 1 < entries.len() {
            index += 1;
        } else {
            index = entries
                .iter()
                .position(|(entry, _)| progress(entry) < entry.pages as i32)?;
        }
    }
    let previous = index.checked_sub(1).map(|index| entries[index].0);
    Some((entries[index].0, previous))
}
