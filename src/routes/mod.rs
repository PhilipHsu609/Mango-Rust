pub mod admin;
pub mod api;
pub mod book;
pub mod login;
pub mod main;
pub mod opds;
pub mod reader;
pub mod reference;

mod recently_added;

/// Trait for types that have a progress field (as f32 percentage)
pub trait HasProgress {
    fn progress(&self) -> f32;
}

/// Sort a slice of items by progress percentage
/// Items must implement HasProgress trait (have a progress field)
pub fn sort_by_progress<T: HasProgress>(items: &mut [T], ascending: bool) {
    items.sort_by(|a, b| {
        let ord = a
            .progress()
            .partial_cmp(&b.progress())
            .unwrap_or(std::cmp::Ordering::Equal);
        if ascending {
            ord
        } else {
            ord.reverse()
        }
    });
}

/// Calculate progress percentage from current page and total pages
pub fn calculate_progress_percentage(progress: i32, total_pages: usize) -> f32 {
    if total_pages > 0 {
        (progress as f32 / total_pages as f32) * 100.0
    } else {
        0.0
    }
}
