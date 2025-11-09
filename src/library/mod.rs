pub mod entry;
pub mod title;
pub mod library;
pub mod progress;

pub use entry::Entry;
pub use title::Title;
pub use library::{Library, LibraryStats, SharedLibrary, SortMethod, spawn_periodic_scanner};
pub use progress::TitleInfo;

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
        items.sort_by(|a, b| a.sort_mtime().cmp(&b.sort_mtime()));
    } else {
        // Newest first
        items.sort_by(|a, b| b.sort_mtime().cmp(&a.sort_mtime()));
    }
}
