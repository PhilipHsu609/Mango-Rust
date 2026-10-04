use askama::Template;
use axum::{
    extract::{Path, Query, State},
    response::Html,
};
use serde::Deserialize;

use crate::{
    auth::User,
    error::{Error, Result},
    library::{
        ordering::{
            compare_title_keys, sort_entries, EntryOrdering, SortOptions, TitleNameOrder,
            TitleSortKey,
        },
        reading::{entry_progress_percent, title_progress_percent},
        Entry, SortMethod, Title,
    },
    routes::presentation::render_error,
    AppState,
};

/// Query parameters for book page
#[derive(Deserialize)]
pub struct BookParams {
    pub sort: Option<String>,
    pub ascend: Option<String>,
    pub search: Option<String>,
}

/// Sort option for templates - matches original Mango SortOptions
#[derive(serde::Serialize, Clone)]
struct SortOption {
    method: String,
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

/// Parent breadcrumb item
#[derive(serde::Serialize, Clone)]
struct ParentItem {
    id: String,
    display_name: String,
}

/// Title info for the page header and edit modal
#[derive(serde::Serialize)]
struct TitleInfo {
    id: String,
    title: String,
    display_name: String,
    sort_title: Option<String>,
    cover_url: String,
    content_label: String,
    parents: Vec<ParentItem>,
}

/// Card item for the book page - unified structure for entries and nested titles
/// Matches the fields expected by templates/components/card.html
#[derive(serde::Serialize, Clone)]
struct BookCardItem {
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

    // Optional metadata
    title: Option<String>,
    sort_title: Option<String>,
}

impl BookCardItem {
    /// Create a card item for an entry
    fn from_entry(entry: &Entry, book: &Title, info: &crate::library::metadata::TitleInfo) -> Self {
        let display_name = info
            .entry_display_name
            .get(&entry.title)
            .filter(|name| !name.is_empty())
            .map(String::as_str)
            .unwrap_or(&entry.title);
        let book_display_name = if info.display_name.is_empty() {
            &book.title
        } else {
            &info.display_name
        };
        let cover_url = if entry.err_msg.is_some() {
            "/static/img/icons/icon_x192.png".to_string()
        } else {
            info.entry_cover_url
                .get(&entry.title)
                .filter(|url| !url.is_empty())
                .cloned()
                .unwrap_or_else(|| format!("/api/cover/{}/{}", book.id, entry.id))
        };
        Self {
            id: entry.id.clone(),
            is_entry: true,
            display_name: display_name.to_string(),
            cover_url,
            book_id: book.id.clone(),
            book_display_name: book_display_name.to_string(),
            pages: entry.pages,
            encoded_path: percent_encoding::percent_encode(
                entry.path.to_string_lossy().as_bytes(),
                percent_encoding::NON_ALPHANUMERIC,
            )
            .to_string(),
            encoded_title: percent_encoding::percent_encode(
                entry.title.as_bytes(),
                percent_encoding::NON_ALPHANUMERIC,
            )
            .to_string(),
            encoded_book_title: percent_encoding::percent_encode(
                book.title.as_bytes(),
                percent_encoding::NON_ALPHANUMERIC,
            )
            .to_string(),
            err_msg: entry.err_msg.clone(),
            content_label: String::new(),
            title: Some(entry.title.clone()),
            sort_title: None,
        }
    }

    /// Create a card item for a nested title
    fn from_title(
        title_id: &str,
        title_name: &str,
        entry_count: usize,
        first_entry_id: Option<&str>,
        first_entry_title: Option<&str>,
        info: &crate::library::metadata::TitleInfo,
    ) -> Self {
        let content_label = if entry_count == 1 {
            "1 entry".to_string()
        } else {
            format!("{} entries", entry_count)
        };
        let display_name = if info.display_name.is_empty() {
            title_name
        } else {
            &info.display_name
        };
        let default_cover = first_entry_title
            .and_then(|entry_title| {
                info.entry_cover_url
                    .get(entry_title)
                    .filter(|url| !url.is_empty())
                    .cloned()
            })
            .or_else(|| first_entry_id.map(|eid| format!("/api/cover/{}/{}", title_id, eid)))
            .unwrap_or_else(|| "/static/img/placeholder.png".to_string());
        let cover_url = if info.cover_url.is_empty() {
            default_cover
        } else {
            info.cover_url.clone()
        };
        Self {
            id: title_id.to_string(),
            is_entry: false,
            display_name: display_name.to_string(),
            cover_url,
            book_id: String::new(),
            book_display_name: String::new(),
            pages: 0,
            encoded_path: String::new(),
            encoded_title: String::new(),
            encoded_book_title: String::new(),
            err_msg: None,
            content_label,
            title: Some(title_name.to_string()),
            sort_title: None,
        }
    }
}

/// Item with progress for book template (entries or nested titles)
struct BookItem {
    item: BookCardItem,
    progress: f64,
}

/// Book page template
#[derive(Template)]
#[template(path = "book.html")]
struct BookTemplate {
    nav: crate::routes::presentation::NavigationState,
    title: TitleInfo,
    sort_options: Vec<(&'static str, &'static str)>,
    sort_opt: Option<SortOption>,
    nested_title_items: Vec<BookItem>,
    items: Vec<BookItem>,
    supported_img_types: String,
}

pub async fn get_book(
    State(state): State<AppState>,
    Path(title_id): Path<String>,
    Query(params): Query<BookParams>,
    user: User,
) -> Result<Html<String>> {
    // Get title path for loading/saving sort preferences
    let title_path = {
        let lib = state.library.load();
        let title = lib
            .get_title(&title_id)
            .ok_or_else(|| Error::NotFound(format!("Title not found: {}", title_id)))?;
        title.path.clone()
    };

    // Load/save sort preferences from title's info.json
    let (sort_method_str, ascending) = state
        .library
        .load_full()
        .metadata()
        .get_and_save_sort(
            &title_path,
            &user.username,
            params.sort.as_deref(),
            params.ascend.as_deref(),
        )
        .await?;

    // Parse sort method from string
    let sort_method = SortMethod::parse(&sort_method_str);

    // Build the title info and gather all data
    let (title_info, nested_title_items, items) = {
        let lib = state.library.load();

        // Get the title
        let title = lib
            .get_title(&title_id)
            .ok_or_else(|| Error::NotFound(format!("Title not found: {}", title_id)))?;
        let info = lib.metadata().read(&title.path).await?;

        // Build parent breadcrumb chain
        let mut parents = Vec::new();
        let mut current_parent_id = title.parent_id.clone();
        while let Some(pid) = current_parent_id {
            if let Some(parent_title) = lib.get_title(&pid) {
                let parent_info = lib.metadata().read(&parent_title.path).await?;
                parents.push(ParentItem {
                    id: parent_title.id.clone(),
                    display_name: if parent_info.display_name.is_empty() {
                        parent_title.title.clone()
                    } else {
                        parent_info.display_name.clone()
                    },
                });
                current_parent_id = parent_title.parent_id.clone();
            } else {
                break;
            }
        }
        parents.reverse(); // Reverse to get root -> parent order

        // Count total entries (including nested)
        let total_entries = title.entries.len();
        let total_titles = title.nested_titles.len();

        let content_label = if total_titles > 0 && total_entries > 0 {
            format!(
                "{} {} and {} {}",
                total_titles,
                if total_titles == 1 { "title" } else { "titles" },
                total_entries,
                if total_entries == 1 {
                    "entry"
                } else {
                    "entries"
                }
            )
        } else if total_titles > 0 {
            format!(
                "{} {}",
                total_titles,
                if total_titles == 1 { "title" } else { "titles" }
            )
        } else {
            format!(
                "{} {}",
                total_entries,
                if total_entries == 1 {
                    "entry"
                } else {
                    "entries"
                }
            )
        };

        // Build title info
        let default_cover = title
            .entries
            .first()
            .and_then(|entry| {
                info.entry_cover_url
                    .get(&entry.title)
                    .filter(|url| !url.is_empty())
                    .cloned()
            })
            .or_else(|| {
                title
                    .entries
                    .first()
                    .map(|entry| format!("/api/cover/{}/{}", title.id, entry.id))
            })
            .unwrap_or_else(|| "/static/img/placeholder.png".to_string());
        let cover_url = if info.cover_url.is_empty() {
            default_cover
        } else {
            info.cover_url.clone()
        };
        let display_name = if info.display_name.is_empty() {
            title.title.clone()
        } else {
            info.display_name.clone()
        };
        let sort_title = state.storage.get_title_sort_title(&title.id).await?;
        let title_info = TitleInfo {
            id: title.id.clone(),
            title: title.title.clone(),
            display_name,
            sort_title,
            cover_url,
            content_label,
            parents,
        };

        let mut nested_order = Vec::with_capacity(title.nested_titles.len());
        for nested in &title.nested_titles {
            let sort_title = state
                .storage
                .get_title_sort_title(&nested.id)
                .await?
                .unwrap_or_else(|| nested.title.clone());
            let progress = title_progress_percent(nested, lib.metadata(), &user.username).await?;
            nested_order.push((nested, sort_title, progress));
        }
        nested_order.sort_by(
            |(left, left_sort, left_progress), (right, right_sort, right_progress)| {
                compare_title_keys(
                    TitleSortKey {
                        name: left_sort,
                        mtime: left.mtime,
                        progress: *left_progress as f64,
                    },
                    TitleSortKey {
                        name: right_sort,
                        mtime: right.mtime,
                        progress: *right_progress as f64,
                    },
                    SortOptions {
                        method: sort_method,
                        ascending: true,
                    },
                    TitleNameOrder::Natural,
                )
            },
        );
        if !ascending {
            nested_order.reverse();
        }

        let mut nested_title_items = Vec::with_capacity(nested_order.len());
        for (nested, sort_title, progress) in nested_order {
            let nested_info = lib.metadata().read(&nested.path).await?;
            let mut card = BookCardItem::from_title(
                &nested.id,
                &nested.title,
                nested.entries.len(),
                nested.entries.first().map(|entry| entry.id.as_str()),
                nested.entries.first().map(|entry| entry.title.as_str()),
                &nested_info,
            );
            card.sort_title = Some(sort_title);
            nested_title_items.push(BookItem {
                item: card,
                progress: progress as f64,
            });
        }

        let mut entry_order = Vec::with_capacity(title.entries.len());
        for entry in &title.entries {
            let sort_title = state
                .storage
                .get_entry_sort_title(&entry.id)
                .await?
                .unwrap_or_else(|| entry.title.clone());
            entry_order.push((entry, sort_title));
        }
        sort_entries(
            &mut entry_order,
            &info,
            &user.username,
            SortOptions {
                method: sort_method,
                ascending,
            },
            EntryOrdering::Book,
        );

        let mut items = Vec::new();
        for (entry, sort_title) in entry_order {
            if let Some(ref search) = params.search {
                if !entry.title.to_lowercase().contains(&search.to_lowercase()) {
                    continue;
                }
            }

            let mut card = BookCardItem::from_entry(entry, title, &info);
            card.sort_title = Some(sort_title);
            items.push(BookItem {
                item: card,
                progress: entry_progress_percent(
                    info.get_progress(&user.username, &entry.title).unwrap_or(0),
                    entry.pages,
                ) as f64,
            });
        }

        (title_info, nested_title_items, items)
    }; // Lock is released here

    // Create sort option for template
    let sort_opt = Some(SortOption::new(&sort_method_str, ascending));

    // Sort options for dropdown
    let sort_options = vec![
        ("auto", "Auto"),
        ("title", "Name"),
        ("time_modified", "Date Modified"),
        ("time_added", "Date Added"),
        ("progress", "Progress"),
    ];

    // Supported image types for upload
    let supported_img_types =
        "image/jpeg,image/png,image/webp,image/apng,image/avif,image/gif,image/svg+xml,image/jxl"
            .to_string();

    let template = BookTemplate {
        nav: crate::routes::presentation::NavigationState::library().with_admin(user.is_admin),
        title: title_info,
        sort_options,
        sort_opt,
        nested_title_items,
        items,
        supported_img_types,
    };

    Ok(Html(template.render().map_err(render_error)?))
}
