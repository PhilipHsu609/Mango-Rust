pub mod login;
pub mod main;
pub mod api;
pub mod book;
pub mod reader;
pub mod progress;

pub use login::{get_login, logout, post_login};
pub use main::{home, library};
pub use api::{get_library, get_page, get_stats, get_title};
pub use book::get_book;
pub use reader::reader;
pub use progress::{get_all_progress, get_progress, save_progress};
