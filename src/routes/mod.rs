pub mod login;
pub mod main;
pub mod api;
pub mod book;
pub mod reader;
pub mod progress;

pub use login::{get_login, logout, post_login};
pub use main::{home, library};
pub use api::{get_cover, get_library, get_page, get_stats, get_title};
pub use book::get_book;
pub use reader::reader;
pub use progress::{get_all_progress, get_progress, save_progress};

/// Trait for types that have a progress field (as a String)
pub trait HasProgress {
    fn progress(&self) -> &str;
}

/// Sort a slice of items by progress percentage
/// Items must implement HasProgress trait (have a progress field)
pub fn sort_by_progress<T: HasProgress>(items: &mut [T], ascending: bool) {
    items.sort_by(|a, b| {
        let a_progress: f32 = a.progress().parse().unwrap_or(0.0);
        let b_progress: f32 = b.progress().parse().unwrap_or(0.0);
        if ascending {
            a_progress.partial_cmp(&b_progress).unwrap_or(std::cmp::Ordering::Equal)
        } else {
            b_progress.partial_cmp(&a_progress).unwrap_or(std::cmp::Ordering::Equal)
        }
    });
}
