use askama::Template;
use axum::{
    extract::{Path, Query, State},
    response::Html,
};

use super::{sort_by_progress, HasProgress};
use crate::{
    auth::User,
    error::Result,
    library::SortMethod,
    util::{render_error, SortParams},
    AppState,
};

/// Title data for library template
#[derive(serde::Serialize)]
struct TitleData {
    id: String,
    name: String,
    entry_count: usize,
    progress: f32,                  // Progress percentage (0.0 - 100.0) for sorting
    progress_display: String,       // Formatted progress for display (e.g., "0.0")
    first_entry_id: Option<String>, // For cover thumbnail URL
}

impl HasProgress for TitleData {
    fn progress(&self) -> f32 {
        self.progress
    }
}

/// Library page template
#[derive(Template)]
#[template(path = "library.html")]
struct LibraryTemplate {
    nav: crate::util::NavigationState,
    title_count: usize,
    sort_name_asc: bool,
    sort_name_desc: bool,
    sort_time_asc: bool,
    sort_time_desc: bool,
    sort_progress_asc: bool,
    sort_progress_desc: bool,
    titles: Vec<TitleData>,
}

/// Home page template
#[derive(Template)]
#[template(path = "home.html")]
struct HomeTemplate {
    nav: crate::util::NavigationState,
}

/// GET / - Home page with Continue Reading, Start Reading, Recently Added (requires authentication)
pub async fn home(State(_state): State<AppState>, user: User) -> Result<Html<String>> {
    // TODO: Implement Continue Reading, Start Reading, Recently Added logic
    let template = HomeTemplate {
        nav: crate::util::NavigationState::home().with_admin(user.is_admin),
    };

    Ok(Html(template.render().map_err(render_error)?))
}

pub async fn library(
    State(state): State<AppState>,
    Query(params): Query<SortParams>,
    user: User,
) -> Result<Html<String>> {
    // Get library path for loading/saving sort preferences
    let library_path = state.library.read().await.path().to_path_buf();

    // Load/save sort preferences from info.json
    let (sort_method_str, ascending) =
        crate::util::get_and_save_sort(&library_path, &user.username, &params).await?;

    // Parse sort method from string
    let sort_method = SortMethod::parse(&sort_method_str);

    // Get library statistics and title data
    let (title_count, mut titles) = {
        let lib = state.library.read().await;
        let stats = lib.stats();

        // For progress sorting, we need to calculate progress first, then sort
        // For other methods, use the library's cached sorting
        let sorted_titles = if matches!(sort_method, SortMethod::Progress) {
            lib.get_titles_sorted_cached(&user.username, SortMethod::Name, true)
                .await // Get name-sorted as base
        } else {
            lib.get_titles_sorted_cached(&user.username, sort_method, ascending)
                .await
        };

        // Calculate progress for each title
        let mut titles = Vec::new();
        for t in sorted_titles {
            let progress_pct = t.get_title_progress(&user.username).await.unwrap_or(0.0);
            titles.push(TitleData {
                id: t.id.clone(),
                name: t.title.clone(),
                entry_count: t.entries.len(),
                progress: progress_pct,
                progress_display: format!("{:.1}", progress_pct),
                first_entry_id: t.entries.first().map(|e| e.id.clone()),
            });
        }

        (stats.titles, titles)
    }; // Lock is released here

    // Sort by progress if requested (after calculating progress)
    if matches!(sort_method, SortMethod::Progress) {
        sort_by_progress(&mut titles, ascending);
    }

    // Determine which sort option is selected
    let sort_name_asc = matches!(sort_method, SortMethod::Name) && ascending;
    let sort_name_desc = matches!(sort_method, SortMethod::Name) && !ascending;
    let sort_time_asc = matches!(sort_method, SortMethod::TimeModified) && ascending;
    let sort_time_desc = matches!(sort_method, SortMethod::TimeModified) && !ascending;
    let sort_progress_asc = matches!(sort_method, SortMethod::Progress) && ascending;
    let sort_progress_desc = matches!(sort_method, SortMethod::Progress) && !ascending;

    let template = LibraryTemplate {
        nav: crate::util::NavigationState::library().with_admin(user.is_admin),
        title_count,
        sort_name_asc,
        sort_name_desc,
        sort_time_asc,
        sort_time_desc,
        sort_progress_asc,
        sort_progress_desc,
        titles,
    };

    Ok(Html(template.render().map_err(render_error)?))
}

/// Change Password page template
#[derive(Template)]
#[template(path = "change-password.html")]
struct ChangePasswordTemplate {
    nav: crate::util::NavigationState,
}

/// GET /change-password - Change password page (requires authentication)
pub async fn change_password_page(user: User) -> Result<Html<String>> {
    let template = ChangePasswordTemplate {
        nav: crate::util::NavigationState::home().with_admin(user.is_admin), // No specific page active for change password
    };

    Ok(Html(template.render().map_err(render_error)?))
}

/// Request body for change password API endpoint
#[derive(serde::Deserialize)]
pub struct ChangePasswordRequest {
    pub current_password: String,
    pub new_password: String,
}

/// POST /api/user/change-password - Change user's password (requires authentication)
pub async fn change_password_api(
    State(state): State<AppState>,
    user: User,
    axum::Json(request): axum::Json<ChangePasswordRequest>,
) -> Result<axum::http::StatusCode> {
    // Validate new password length
    if request.new_password.len() < 6 {
        return Err(crate::error::Error::BadRequest(
            "New password must be at least 6 characters".to_string(),
        ));
    }

    // Change the password
    state
        .storage
        .change_password(
            &user.username,
            &request.current_password,
            &request.new_password,
        )
        .await?;

    Ok(axum::http::StatusCode::OK)
}

// ========== Tags Page Handlers ==========

#[derive(Template)]
#[template(path = "tags.html")]
struct TagsTemplate {
    nav: crate::util::NavigationState,
    tags: Vec<TagWithCount>,
}

#[derive(serde::Serialize)]
struct TagWithCount {
    tag: String,
    encoded_tag: String,
    count: usize,
}

/// GET /tags - List all tags with their usage counts
pub async fn list_tags_page(State(state): State<AppState>, user: User) -> Result<Html<String>> {
    let storage = &state.storage;
    let tags = storage.list_tags().await?;

    // Count titles for each tag and prepare display data
    let mut tags_with_counts = Vec::new();
    for tag in tags {
        let title_ids = storage.get_tag_titles(&tag).await?;
        let count = title_ids.len();

        // URL-encode the tag for links
        let encoded_tag =
            percent_encoding::percent_encode(tag.as_bytes(), percent_encoding::NON_ALPHANUMERIC)
                .to_string();

        tags_with_counts.push(TagWithCount {
            tag,
            encoded_tag,
            count,
        });
    }

    // Sort by count desc, then by tag name asc (case-insensitive)
    tags_with_counts.sort_by(|a, b| {
        b.count
            .cmp(&a.count)
            .then_with(|| a.tag.to_lowercase().cmp(&b.tag.to_lowercase()))
    });

    let template = TagsTemplate {
        nav: crate::util::NavigationState::tags().with_admin(user.is_admin),
        tags: tags_with_counts,
    };
    Ok(Html(template.render().map_err(render_error)?))
}

#[derive(Template)]
#[template(path = "tag.html")]
struct TagTemplate {
    nav: crate::util::NavigationState,
    tag: String,
    title_count: usize,
    titles: Vec<TitleData>,
    sort_name_asc: bool,
    sort_name_desc: bool,
    sort_time_asc: bool,
    sort_time_desc: bool,
    sort_progress_asc: bool,
    sort_progress_desc: bool,
}

/// GET /tags/:tag - Show filtered library view for a specific tag
pub async fn view_tag_page(
    State(state): State<AppState>,
    Path(tag): Path<String>,
    Query(params): Query<crate::util::SortParams>,
    user: User,
) -> Result<Html<String>> {
    let storage = &state.storage;
    let lib = state.library.read().await;

    // Get all title IDs with this tag
    let title_ids = storage.get_tag_titles(&tag).await?;

    if title_ids.is_empty() {
        return Err(crate::error::Error::NotFound(format!(
            "Tag '{}' not found",
            tag
        )));
    }

    // Get title objects for these IDs
    let mut titles: Vec<TitleData> = title_ids
        .iter()
        .filter_map(|id| {
            lib.get_title(id).map(|title| {
                TitleData {
                    id: title.id.clone(),
                    name: title.title.clone(),
                    entry_count: title.entries.len(),
                    first_entry_id: title.entries.first().map(|e| e.id.clone()),
                    progress: 0.0, // Will be filled later
                    progress_display: String::from("0.0"),
                }
            })
        })
        .collect();

    // Load progress for each title
    for title_data in &mut titles {
        let title = lib.get_title(&title_data.id).unwrap();
        let progress_pct = title.get_title_progress(&user.username).await?;
        title_data.progress = progress_pct;
        title_data.progress_display = format!("{:.1}", progress_pct);
    }

    // Determine sort method
    let (sort_method, ascending) =
        crate::library::SortMethod::from_params(params.sort.as_deref(), params.ascend.as_deref());

    // Sort titles based on method
    match sort_method {
        crate::library::SortMethod::Name => {
            titles.sort_by(|a, b| {
                if ascending {
                    natord::compare(&a.name, &b.name)
                } else {
                    natord::compare(&b.name, &a.name)
                }
            });
        }
        crate::library::SortMethod::TimeModified => {
            // For modified sort, we need to get the mtime from the actual titles
            titles.sort_by(|a, b| {
                let a_title = lib.get_title(&a.id).unwrap();
                let b_title = lib.get_title(&b.id).unwrap();
                let a_mtime = a_title.mtime;
                let b_mtime = b_title.mtime;
                if ascending {
                    a_mtime.cmp(&b_mtime)
                } else {
                    b_mtime.cmp(&a_mtime)
                }
            });
        }
        crate::library::SortMethod::Progress => {
            if ascending {
                crate::routes::sort_by_progress(&mut titles, true);
            } else {
                crate::routes::sort_by_progress(&mut titles, false);
            }
        }
        crate::library::SortMethod::Auto => {
            // Auto sort defaults to Name ascending
            titles.sort_by(|a, b| natord::compare(&a.name, &b.name));
        }
    }

    // Determine which sort option is active
    let (
        sort_name_asc,
        sort_name_desc,
        sort_time_asc,
        sort_time_desc,
        sort_progress_asc,
        sort_progress_desc,
    ) = match (sort_method, ascending) {
        (crate::library::SortMethod::Name, true) => (true, false, false, false, false, false),
        (crate::library::SortMethod::Name, false) => (false, true, false, false, false, false),
        (crate::library::SortMethod::TimeModified, true) => {
            (false, false, true, false, false, false)
        }
        (crate::library::SortMethod::TimeModified, false) => {
            (false, false, false, true, false, false)
        }
        (crate::library::SortMethod::Progress, true) => (false, false, false, false, true, false),
        (crate::library::SortMethod::Progress, false) => (false, false, false, false, false, true),
        (crate::library::SortMethod::Auto, _) => (true, false, false, false, false, false),
    };

    let template = TagTemplate {
        nav: crate::util::NavigationState::tags().with_admin(user.is_admin),
        tag,
        title_count: titles.len(),
        titles,
        sort_name_asc,
        sort_name_desc,
        sort_time_asc,
        sort_time_desc,
        sort_progress_asc,
        sort_progress_desc,
    };

    Ok(Html(template.render().map_err(render_error)?))
}
