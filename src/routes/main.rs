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

/// Sort option for templates - matches original Mango SortOptions
#[derive(serde::Serialize, Clone)]
struct SortOption {
    method: String, // "auto", "title", "time_modified", "progress"
    ascend: bool,
}

impl SortOption {
    fn new(method: &str, ascend: bool) -> Self {
        Self {
            method: method.to_string(),
            ascend,
        }
    }
}

/// Title data for library/tag templates (internal for progress sorting)
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

/// Item with progress for library template
struct LibraryItem {
    item: HomeCardItem,
    progress: f64,
}

/// Library page template - matches original Mango library.html.ecr
#[derive(Template)]
#[template(path = "library.html")]
struct LibraryTemplate {
    nav: crate::util::NavigationState,
    titles: Vec<HomeCardItem>,  // For titles.len() in template
    items: Vec<LibraryItem>,    // Items with progress for iteration
    sort_options: Vec<(String, String)>,
    sort_opt: Option<SortOption>,
}

/// Card item for home page - unified structure for entries and titles
/// Matches the fields expected by templates/components/card.html
#[derive(serde::Serialize, Clone)]
struct HomeCardItem {
    // Common fields
    id: String,
    is_entry: bool,
    display_name: String,
    cover_url: String,

    // Entry-specific fields (used when is_entry = true)
    book_id: String,
    book_display_name: String,
    pages: usize,
    encoded_path: String,
    encoded_title: String,
    encoded_book_title: String,
    err_msg: Option<String>,

    // Title-specific fields (used when is_entry = false)
    content_label: String,
    grouped_count: Option<usize>,

    // Optional metadata
    title: Option<String>,
    sort_title: Option<String>,
}

impl HomeCardItem {
    /// Create a card item for an entry
    #[allow(dead_code)]
    fn from_entry(
        entry_id: &str,
        entry_title: &str,
        book_id: &str,
        book_title: &str,
        pages: usize,
        entry_path: &str,
    ) -> Self {
        Self {
            id: entry_id.to_string(),
            is_entry: true,
            display_name: entry_title.to_string(),
            cover_url: format!("/api/cover/{}/{}", book_id, entry_id),
            book_id: book_id.to_string(),
            book_display_name: book_title.to_string(),
            pages,
            encoded_path: percent_encoding::percent_encode(
                entry_path.as_bytes(),
                percent_encoding::NON_ALPHANUMERIC,
            )
            .to_string(),
            encoded_title: percent_encoding::percent_encode(
                entry_title.as_bytes(),
                percent_encoding::NON_ALPHANUMERIC,
            )
            .to_string(),
            encoded_book_title: percent_encoding::percent_encode(
                book_title.as_bytes(),
                percent_encoding::NON_ALPHANUMERIC,
            )
            .to_string(),
            err_msg: None,
            content_label: String::new(),
            grouped_count: None,
            title: Some(entry_title.to_string()),
            sort_title: Some(entry_title.to_string()),
        }
    }

    /// Create a card item for a title
    #[allow(dead_code)]
    fn from_title(title_id: &str, title_name: &str, entry_count: usize, first_entry_id: Option<&str>) -> Self {
        let content_label = if entry_count == 1 {
            "1 entry".to_string()
        } else {
            format!("{} entries", entry_count)
        };

        // Cover URL uses first entry's cover if available (requires both tid and eid)
        let cover_url = first_entry_id
            .map(|eid| format!("/api/cover/{}/{}", title_id, eid))
            .unwrap_or_else(|| "/static/img/placeholder.png".to_string());

        Self {
            id: title_id.to_string(),
            is_entry: false,
            display_name: title_name.to_string(),
            cover_url,
            book_id: String::new(),
            book_display_name: String::new(),
            pages: 0,
            encoded_path: String::new(),
            encoded_title: String::new(),
            encoded_book_title: String::new(),
            err_msg: None,
            content_label,
            grouped_count: None,
            title: Some(title_name.to_string()),
            sort_title: Some(title_name.to_string()),
        }
    }
}

/// Continue reading item (entry with progress)
#[derive(serde::Serialize)]
struct ContinueReadingItem {
    entry: HomeCardItem,
    percentage: f32,
}

/// Recently added item (entry or title with optional percentage)
#[derive(serde::Serialize)]
struct RecentlyAddedItem {
    #[serde(flatten)]
    item: HomeCardItem,
    percentage: f32,
    grouped_count: Option<usize>,
}

/// Home page template
#[derive(Template)]
#[template(path = "home.html")]
struct HomeTemplate {
    nav: crate::util::NavigationState,
    // User state
    new_user: bool,
    empty_library: bool,
    // Config info
    library_path: String,
    config_path: String,
    scan_interval: u32,
    // Content sections
    continue_reading: Vec<ContinueReadingItem>,
    start_reading: Vec<HomeCardItem>,
    recently_added: Vec<RecentlyAddedItem>,
}

/// GET / - Home page with Continue Reading, Start Reading, Recently Added (requires authentication)
pub async fn home(State(state): State<AppState>, user: User) -> Result<Html<String>> {
    // Get library stats to determine empty_library
    let (title_count, has_any_progress) = {
        let lib = state.library.load();
        let stats = lib.stats();

        // Check if user has any reading progress
        // For now, we'll do a simple check - iterate through titles and check progress
        let mut has_progress = false;
        for title in lib.get_titles() {
            if let Ok(progress) = title.get_title_progress(&user.username).await {
                if progress > 0.0 {
                    has_progress = true;
                    break;
                }
            }
        }

        (stats.titles, has_progress)
    };

    let empty_library = title_count == 0;
    let new_user = !has_any_progress;

    // Get library path and config path from state
    let library_path = state.config.library_path.display().to_string();
    let config_path = dirs::config_dir()
        .map(|p| p.join("mango/config.yml").display().to_string())
        .unwrap_or_else(|| "~/.config/mango/config.yml".to_string());
    let scan_interval = state.config.scan_interval_minutes;

    // Get home page content sections
    let (continue_reading, start_reading, recently_added) = {
        use crate::library::progress::TitleInfo;

        let lib = state.library.load();
        let mut cr_items = Vec::new();
        let mut sr_items = Vec::new();
        let mut ra_items = Vec::new();

        const MAX_ITEMS: usize = 8;
        let one_month_ago = chrono::Utc::now().timestamp() - (30 * 24 * 60 * 60);

        // Collect data for all titles
        for title in lib.get_titles() {
            let info = match TitleInfo::load(&title.path).await {
                Ok(info) => info,
                Err(_) => continue,
            };

            // Check title progress for start_reading
            let title_progress = title.get_title_progress(&user.username).await.unwrap_or(0.0);
            if title_progress == 0.0 && sr_items.len() < MAX_ITEMS {
                sr_items.push(HomeCardItem::from_title(
                    &title.id,
                    &title.title,
                    title.entries.len(),
                    title.entries.first().map(|e| e.id.as_str()),
                ));
            }

            // Process entries for continue_reading and recently_added
            for entry in &title.entries {
                // Continue reading: entries with last_read timestamp
                if let Some(last_read) = info.get_last_read(&user.username, &entry.id) {
                    let progress = info.get_progress(&user.username, &entry.id).unwrap_or(0);
                    let percentage = if entry.pages > 0 {
                        (progress as f32 / entry.pages as f32) * 100.0
                    } else {
                        0.0
                    };

                    // Only include entries that are partially read (0 < progress < 100%)
                    if percentage > 0.0 && percentage < 100.0 {
                        cr_items.push((
                            last_read,
                            ContinueReadingItem {
                                entry: HomeCardItem::from_entry(
                                    &entry.id,
                                    &entry.title,
                                    &title.id,
                                    &title.title,
                                    entry.pages,
                                    &entry.path.to_string_lossy(),
                                ),
                                percentage,
                            },
                        ));
                    }
                }

                // Recently added: entries added within last month
                if let Some(date_added) = info.get_date_added(&entry.id) {
                    if date_added > one_month_ago {
                        let progress = info.get_progress(&user.username, &entry.id).unwrap_or(0);
                        let percentage = if entry.pages > 0 {
                            (progress as f32 / entry.pages as f32) * 100.0
                        } else {
                            0.0
                        };

                        ra_items.push((
                            date_added,
                            RecentlyAddedItem {
                                item: HomeCardItem::from_entry(
                                    &entry.id,
                                    &entry.title,
                                    &title.id,
                                    &title.title,
                                    entry.pages,
                                    &entry.path.to_string_lossy(),
                                ),
                                percentage,
                                grouped_count: None,
                            },
                        ));
                    }
                }
            }
        }

        // Sort continue_reading by last_read (most recent first) and take top items
        cr_items.sort_by(|a, b| b.0.cmp(&a.0));
        let continue_reading: Vec<ContinueReadingItem> = cr_items
            .into_iter()
            .take(MAX_ITEMS)
            .map(|(_, item)| item)
            .collect();

        // Shuffle start_reading titles (random selection like original Mango)
        use rand::seq::SliceRandom;
        let mut rng = rand::thread_rng();
        sr_items.shuffle(&mut rng);
        sr_items.truncate(MAX_ITEMS);

        // Sort recently_added by date_added (most recent first)
        ra_items.sort_by(|a, b| b.0.cmp(&a.0));
        let recently_added: Vec<RecentlyAddedItem> = ra_items
            .into_iter()
            .take(MAX_ITEMS)
            .map(|(_, item)| item)
            .collect();

        (continue_reading, sr_items, recently_added)
    };

    let template = HomeTemplate {
        nav: crate::util::NavigationState::home().with_admin(user.is_admin),
        new_user,
        empty_library,
        library_path,
        config_path,
        scan_interval,
        continue_reading,
        start_reading,
        recently_added,
    };

    Ok(Html(template.render().map_err(render_error)?))
}

pub async fn library(
    State(state): State<AppState>,
    Query(params): Query<SortParams>,
    user: User,
) -> Result<Html<String>> {
    // Get library path for loading/saving sort preferences
    let library_path = state.library.load().path().to_path_buf();

    // Load/save sort preferences from info.json
    let (sort_method_str, ascending) =
        crate::util::get_and_save_sort(&library_path, &user.username, &params).await?;

    // Parse sort method from string
    let sort_method = SortMethod::parse(&sort_method_str);

    // Get library statistics and title data
    let mut title_data_list = {
        let lib = state.library.load();

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
        let mut title_data_list = Vec::new();
        for t in sorted_titles {
            let progress_pct = t.get_title_progress(&user.username).await.unwrap_or(0.0);
            title_data_list.push(TitleData {
                id: t.id.clone(),
                name: t.title.clone(),
                entry_count: t.entries.len(),
                progress: progress_pct,
                progress_display: format!("{:.1}", progress_pct),
                first_entry_id: t.entries.first().map(|e| e.id.clone()),
            });
        }

        title_data_list
    }; // Lock is released here

    // Sort by progress if requested (after calculating progress)
    if matches!(sort_method, SortMethod::Progress) {
        sort_by_progress(&mut title_data_list, ascending);
    }

    // Convert TitleData to HomeCardItem and create LibraryItem list
    let mut titles = Vec::with_capacity(title_data_list.len());
    let mut items = Vec::with_capacity(title_data_list.len());

    for td in title_data_list {
        let card_item = HomeCardItem::from_title(
            &td.id,
            &td.name,
            td.entry_count,
            td.first_entry_id.as_deref(),
        );
        items.push(LibraryItem {
            item: card_item.clone(),
            progress: td.progress as f64,
        });
        titles.push(card_item);
    }

    // Build sort options matching original Mango
    let sort_options = vec![
        ("auto".to_string(), "Auto".to_string()),
        ("title".to_string(), "Name".to_string()),
        ("time_modified".to_string(), "Date Modified".to_string()),
        ("progress".to_string(), "Progress".to_string()),
    ];

    // Build current sort option
    let sort_opt = Some(SortOption::new(&sort_method_str, ascending));

    let template = LibraryTemplate {
        nav: crate::util::NavigationState::library().with_admin(user.is_admin),
        titles,
        items,
        sort_options,
        sort_opt,
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
    let lib = state.library.load();

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
