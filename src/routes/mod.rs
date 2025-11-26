pub mod admin;
pub mod api;
pub mod book;
pub mod login;
pub mod main;
pub mod opds;
pub mod progress;
pub mod reader;

pub use admin::{
    admin_dashboard, create_user, delete_all_missing_entries, delete_missing_entry,
    delete_user, get_missing_entries, get_users, missing_items_page, scan_library,
    update_user, users_page,
};
pub use api::{
    add_tag, continue_reading, delete_tag, download_entry, get_cover, get_library, get_page,
    get_stats, get_title, get_title_tags, list_tags, recently_added, start_reading,
};
pub use book::get_book;
pub use login::{get_login, logout, post_login};
pub use main::{change_password_api, change_password_page, home, library, list_tags_page, view_tag_page};
pub use opds::{opds_index, opds_title};
pub use progress::{get_all_progress, get_progress, save_progress};
pub use reader::reader;

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
            a_progress
                .partial_cmp(&b_progress)
                .unwrap_or(std::cmp::Ordering::Equal)
        } else {
            b_progress
                .partial_cmp(&a_progress)
                .unwrap_or(std::cmp::Ordering::Equal)
        }
    });
}
