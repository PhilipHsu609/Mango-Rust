pub mod cache;
pub(crate) mod chapter_sort;
pub mod entry;
pub mod media;
pub mod progress;
pub mod progress_cache;
pub mod title;

pub mod scan;
mod snapshot;

pub use entry::Entry;
pub use progress::TitleInfo;
pub use progress_cache::ProgressCache;
pub use snapshot::{Library, LibraryStats, SharedLibrary, SortMethod};
pub use title::Title;

/// Trait for types that can be sorted by name and modification time
pub trait Sortable {
    /// Get the title/name for natural ordering comparison
    fn sort_name(&self) -> &str;

    /// Get the modification time for time-based sorting
    fn sort_mtime(&self) -> i64;
}

/// Sort a slice of Sortable items by name using natural ordering
pub fn sort_by_name<T: Sortable>(items: &mut [T], ascending: bool) {
    if ascending {
        items.sort_by(|a, b| natord::compare(a.sort_name(), b.sort_name()));
    } else {
        items.sort_by(|a, b| natord::compare(b.sort_name(), a.sort_name()));
    }
}

/// Sort a slice of Sortable items by modification time
pub fn sort_by_mtime<T: Sortable>(items: &mut [T], ascending: bool) {
    if ascending {
        // Oldest first
        items.sort_by_key(|a| a.sort_mtime());
    } else {
        // Newest first
        items.sort_by_key(|b| std::cmp::Reverse(b.sort_mtime()));
    }
}
