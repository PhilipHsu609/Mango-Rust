use askama::Template;
use axum::{
    body::Bytes,
    extract::{Extension, State},
    http::header,
    response::Html,
};
use utoipa::OpenApi;

use crate::{error::Result, AppState};

#[derive(Template)]
#[template(path = "api.html")]
struct ApiReferenceTemplate {
    spec_url: String,
}

pub async fn api_reference(State(state): State<AppState>) -> Result<Html<String>> {
    let template = ApiReferenceTemplate {
        spec_url: format!("{}openapi.json", state.config.base_url),
    };
    Ok(Html(template.render().map_err(crate::util::render_error)?))
}

pub async fn openapi_spec(
    Extension(spec): Extension<Bytes>,
) -> ([(header::HeaderName, &'static str); 1], Bytes) {
    ([(header::CONTENT_TYPE, "application/json")], spec)
}

#[derive(OpenApi)]
#[openapi(
    info(
        title = "Mango API",
        version = "0.1.0",
        description = r###"
# A Word of Caution

This API was designed for internal use only, and the design doesn't comply with the resources convention of a RESTful API. Because of this, most of the API endpoints listed here will soon be updated and removed in future versions of Mango, so use them at your own risk!

# Authentication

All endpoints except `/api/login` require authentication. After logging in, the session ID is stored in a cookie named `mango-sessid-{port}`. All admin API endpoints (`/api/admin/...`) require an administrator account.

# Terminologies

- Entry: A `cbz`/`cbr` file in your library. Depending on your organization, an entry can contain a chapter, a volume, or an entire manga.
- Title: A title contains entries and optionally sub-titles. For example, a manga title may contain sub-titles representing volumes; each sub-title may contain entries representing chapters.
- Library: A collection of top-level titles that does not contain entries itself. A Mango instance can only have one library.
"###
    ),
    tags(
        (name = "users", description = "Authentication and user management"),
        (name = "library", description = "Library and metadata operations"),
        (name = "reader", description = "Reader content and image operations"),
        (name = "progress", description = "Reading progress operations"),
        (name = "admin", description = "Administrator-only operations")
    ),
    paths(
        crate::routes::login::api_login,
        crate::routes::api::get_library,
        crate::routes::api::get_title,
        crate::routes::api::get_sort_opt,
        crate::routes::api::update_sort_opt,
        crate::routes::api::get_page,
        crate::routes::api::get_cover,
        crate::routes::api::continue_reading,
        crate::routes::api::start_reading,
        crate::routes::api::recently_added,
        crate::routes::api::list_tags,
        crate::routes::api::get_title_tags,
        crate::routes::api::add_tag,
        crate::routes::api::delete_tag,
        crate::routes::api::download_entry,
        crate::routes::api::get_dimensions,
        crate::routes::api::update_progress,
        crate::routes::admin::scan_library,
        crate::routes::admin::cache_clear_api,
        crate::routes::admin::cache_save_library_api,
        crate::routes::admin::cache_load_library_api,
        crate::routes::admin::cache_invalidate_api,
        crate::routes::admin::get_missing_titles,
        crate::routes::admin::delete_all_missing_titles,
        crate::routes::admin::delete_missing_title,
        crate::routes::admin::get_missing_entries,
        crate::routes::admin::delete_all_missing_entries,
        crate::routes::admin::delete_missing_entry,
        crate::routes::admin::get_users,
        crate::routes::admin::create_user,
        crate::routes::admin::update_user,
        crate::routes::admin::delete_user,
        crate::routes::admin::delete_user_api,
        crate::routes::admin::update_display_name,
        crate::routes::admin::update_sort_title,
        crate::routes::admin::upload_cover,
        crate::routes::admin::bulk_progress,
        crate::routes::admin::thumbnail_progress,
        crate::routes::admin::generate_thumbnails,
        crate::routes::main::change_password_api
    )
)]
pub struct ApiDoc;
