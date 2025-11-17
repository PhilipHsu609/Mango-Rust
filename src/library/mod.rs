pub mod entry;
pub mod progress;
pub mod title;

// Library manager module
mod manager;

pub use entry::Entry;
pub use manager::{spawn_periodic_scanner, Library, LibraryStats, SharedLibrary, SortMethod};
pub use progress::TitleInfo;
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
