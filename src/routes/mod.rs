pub mod admin;
pub mod api;
pub mod book;
pub mod login;
pub mod main;
pub mod opds;
pub mod progress;
pub mod reader;

pub use admin::{
    admin_dashboard, bulk_progress, cache_clear_api, cache_debug_page, cache_invalidate_api,
    cache_load_library_api, cache_save_library_api, create_user, delete_all_missing_entries,
    delete_missing_entry, delete_user, delete_user_api, generate_thumbnails, get_missing_entries,
    get_users, missing_items_page, scan_library, thumbnail_progress, update_display_name,
    update_sort_title, update_user, upload_cover, user_edit_page, user_edit_post,
    user_edit_post_existing, users_page,
};
pub use api::{
    add_tag, continue_reading, delete_tag, download_entry, get_cover, get_dimensions, get_library,
    get_page, get_stats, get_title, get_title_tags, list_tags, recently_added, start_reading,
    update_progress,
};
pub use book::get_book;
pub use login::{get_login, logout, post_login};
pub use main::{
    change_password_api, change_password_page, home, library, list_tags_page, view_tag_page,
};
pub use opds::{opds_index, opds_title};
pub use progress::{get_all_progress, get_progress, save_progress};
pub use reader::{reader, reader_continue};

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
