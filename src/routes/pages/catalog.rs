use askama::Template;
use axum::{
    extract::{Path, Query, State},
    response::Html,
};

use super::cards::CardItem;
use crate::{
    auth::User,
    error::Result,
    library::{
        ordering::{compare_title_keys, SortOptions, TitleNameOrder, TitleSortKey},
        reading::title_progress_percent,
        SortMethod,
    },
    routes::presentation::{render_error, NavigationState},
    AppState,
};

/// Query parameters for sorting.
#[derive(serde::Deserialize)]
pub struct SortParams {
    /// Optional sort method (title, modified, auto, progress).
    pub sort: Option<String>,
    /// Optional ascend flag (1 for ascending, 0 for descending).
    pub ascend: Option<String>,
}

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
    display_name: String,
    cover_url: String,
    sort_title: Option<String>,
    mtime: i64,
    entry_count: usize,
    progress: f32,
    progress_display: String,
    first_entry_id: Option<String>,
    first_entry_title: Option<String>,
    #[serde(skip)]
    info: std::sync::Arc<crate::library::metadata::TitleInfo>,
}

/// Item with progress for library template
struct LibraryItem {
    item: CardItem,
    progress: f64,
}

/// Library page template - matches original Mango library.html.ecr
#[derive(Template)]
#[template(path = "library.html")]
struct LibraryTemplate {
    nav: NavigationState,
    titles: Vec<CardItem>,   // For titles.len() in template
    items: Vec<LibraryItem>, // Items with progress for iteration
    sort_options: Vec<(String, String)>,
    sort_opt: Option<SortOption>,
}

pub async fn library(
    State(state): State<AppState>,
    Query(params): Query<SortParams>,
    user: User,
) -> Result<Html<String>> {
    // Get library path for loading/saving sort preferences
    let library_path = state.library.load().path().to_path_buf();

    // Load/save sort preferences from info.json
    let (sort_method_str, ascending) = state
        .library
        .load_full()
        .metadata()
        .get_and_save_sort(
            &library_path,
            &user.username,
            params.sort.as_deref(),
            params.ascend.as_deref(),
        )
        .await?;

    // Parse sort method from string
    let sort_method = SortMethod::parse(&sort_method_str);

    // Get library statistics and title data
    let mut title_data_list = {
        let lib = state.library.load();

        // For progress sorting, we need to calculate progress first, then sort
        // For other methods, use the library's cached sorting
        let sorted_titles = lib.get_titles();

        // Calculate progress for each title
        let mut title_data_list = Vec::new();
        for t in sorted_titles {
            let progress_pct = title_progress_percent(t, lib.metadata(), &user.username)
                .await
                .unwrap_or(0.0);
            let info = lib.metadata().read(&t.path).await?;
            let display_name = if info.display_name.is_empty() {
                t.title.clone()
            } else {
                info.display_name.clone()
            };
            let cover_url = if info.cover_url.is_empty() {
                t.entries
                    .first()
                    .and_then(|entry| {
                        info.entry_cover_url
                            .get(&entry.title)
                            .filter(|url| !url.is_empty())
                            .cloned()
                    })
                    .or_else(|| {
                        t.entries
                            .first()
                            .map(|entry| format!("/api/cover/{}/{}", t.id, entry.id))
                    })
                    .unwrap_or_else(|| "/static/img/placeholder.png".to_string())
            } else {
                info.cover_url.clone()
            };
            let sort_title = state.storage.get_title_sort_title(&t.id).await?;
            title_data_list.push(TitleData {
                id: t.id.clone(),
                name: t.title.clone(),
                display_name,
                cover_url,
                sort_title,
                mtime: t.mtime,
                entry_count: t.entries.len(),
                progress: progress_pct,
                progress_display: format!("{:.1}", progress_pct),
                first_entry_id: t.entries.first().map(|e| e.id.clone()),
                first_entry_title: t.entries.first().map(|e| e.title.clone()),
                info,
            });
        }

        title_data_list
    }; // Lock is released here

    title_data_list.sort_by(|left, right| {
        compare_title_keys(
            TitleSortKey {
                name: left.sort_title.as_deref().unwrap_or(&left.name),
                mtime: left.mtime,
                progress: f64::from(left.progress),
            },
            TitleSortKey {
                name: right.sort_title.as_deref().unwrap_or(&right.name),
                mtime: right.mtime,
                progress: f64::from(right.progress),
            },
            SortOptions {
                method: sort_method,
                ascending,
            },
            TitleNameOrder::Natural,
        )
    });

    // Convert TitleData to CardItem and create LibraryItem list
    let mut titles = Vec::with_capacity(title_data_list.len());
    let mut items = Vec::with_capacity(title_data_list.len());

    for td in title_data_list {
        let mut card_item = CardItem::from_title(
            &td.id,
            &td.name,
            td.entry_count,
            td.first_entry_id.as_deref(),
            td.first_entry_title.as_deref(),
            &td.info,
        );
        card_item.sort_title = Some(td.sort_title.clone().unwrap_or_else(|| td.name.clone()));
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
        nav: NavigationState::library().with_admin(user.is_admin),
        titles,
        items,
        sort_options,
        sort_opt,
    };

    Ok(Html(template.render().map_err(render_error)?))
}

// ========== Tags Page Handlers ==========

#[derive(Template)]
#[template(path = "tags.html")]
struct TagsTemplate {
    nav: NavigationState,
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

    // Mango orders tags by descending count, then case-sensitive tag name.
    tags_with_counts.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.tag.cmp(&b.tag)));

    let template = TagsTemplate {
        nav: NavigationState::tags().with_admin(user.is_admin),
        tags: tags_with_counts,
    };
    Ok(Html(template.render().map_err(render_error)?))
}

#[derive(Template)]
#[template(path = "tag.html")]
struct TagTemplate {
    nav: NavigationState,
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
    Query(params): Query<SortParams>,
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
            lib.get_title(id).map(|title| TitleData {
                id: title.id.clone(),
                name: title.title.clone(),
                display_name: title.title.clone(),
                cover_url: title
                    .entries
                    .first()
                    .map(|entry| format!("/api/cover/{}/{}", title.id, entry.id))
                    .unwrap_or_else(|| "/static/img/placeholder.png".to_string()),
                sort_title: None,
                mtime: title.mtime,
                entry_count: title.entries.len(),
                first_entry_id: title.entries.first().map(|e| e.id.clone()),
                first_entry_title: title.entries.first().map(|e| e.title.clone()),
                progress: 0.0,
                progress_display: String::from("0.0"),
                info: Default::default(),
            })
        })
        .collect();

    // Load progress for each title
    for title_data in &mut titles {
        let title = lib.get_title(&title_data.id).unwrap();
        let info = lib.metadata().read(&title.path).await?;
        title_data.display_name = if info.display_name.is_empty() {
            title.title.clone()
        } else {
            info.display_name.clone()
        };
        if !info.cover_url.is_empty() {
            title_data.cover_url = info.cover_url.clone();
        }
        title_data.sort_title = state.storage.get_title_sort_title(&title.id).await?;
        title_data.info = info;
        let progress_pct = title_progress_percent(title, lib.metadata(), &user.username).await?;
        title_data.progress = progress_pct;
        title_data.progress_display = format!("{:.1}", progress_pct);
    }

    // Determine sort method
    let (sort_method, ascending) =
        crate::library::SortMethod::from_params(params.sort.as_deref(), params.ascend.as_deref());

    titles.sort_by(|left, right| {
        compare_title_keys(
            TitleSortKey {
                name: left.sort_title.as_deref().unwrap_or(&left.name),
                mtime: left.mtime,
                progress: f64::from(left.progress),
            },
            TitleSortKey {
                name: right.sort_title.as_deref().unwrap_or(&right.name),
                mtime: right.mtime,
                progress: f64::from(right.progress),
            },
            SortOptions {
                method: sort_method,
                ascending,
            },
            TitleNameOrder::Natural,
        )
    });

    // Determine which sort option is active
    let (
        sort_name_asc,
        sort_name_desc,
        sort_time_asc,
        sort_time_desc,
        sort_progress_asc,
        sort_progress_desc,
    ) = match (sort_method, ascending) {
        (crate::library::SortMethod::Name, true)
        | (crate::library::SortMethod::TimeAdded, true) => {
            (true, false, false, false, false, false)
        }
        (crate::library::SortMethod::Name, false)
        | (crate::library::SortMethod::TimeAdded, false) => {
            (false, true, false, false, false, false)
        }
        (crate::library::SortMethod::TimeModified, true) => {
            (false, false, true, false, false, false)
        }
        (crate::library::SortMethod::TimeModified, false) => {
            (false, false, false, true, false, false)
        }
        (crate::library::SortMethod::Progress, true) => (false, false, false, false, true, false),
        (crate::library::SortMethod::Progress, false) => (false, false, false, false, false, true),
        (crate::library::SortMethod::Auto, true) => (true, false, false, false, false, false),
        (crate::library::SortMethod::Auto, false) => (false, true, false, false, false, false),
    };

    let template = TagTemplate {
        nav: NavigationState::tags().with_admin(user.is_admin),
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
