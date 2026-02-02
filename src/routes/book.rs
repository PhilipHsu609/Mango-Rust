use askama::Template;
use axum::{
    extract::{Path, Query, State},
    response::Html,
};
use serde::Deserialize;

use super::{sort_by_progress, HasProgress};
use crate::{
    auth::User,
    error::{Error, Result},
    library::SortMethod,
    util::render_error,
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
            title: Some(entry_title.to_string()),
            sort_title: None,
        }
    }

    /// Create a card item for a nested title
    fn from_title(
        title_id: &str,
        title_name: &str,
        entry_count: usize,
        first_entry_id: Option<&str>,
    ) -> Self {
        let content_label = if entry_count == 1 {
            "1 entry".to_string()
        } else {
            format!("{} entries", entry_count)
        };

        // Cover URL uses first entry's cover if available
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

impl HasProgress for BookItem {
    fn progress(&self) -> f32 {
        self.progress as f32
    }
}

/// Book page template
#[derive(Template)]
#[template(path = "book.html")]
struct BookTemplate {
    nav: crate::util::NavigationState,
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
        let lib = state.library.read().await;
        let title = lib
            .get_title(&title_id)
            .ok_or_else(|| Error::NotFound(format!("Title not found: {}", title_id)))?;
        title.path.clone()
    };

    // Load/save sort preferences from title's info.json
    let sort_params = crate::util::SortParams {
        sort: params.sort.clone(),
        ascend: params.ascend.clone(),
    };
    let (sort_method_str, ascending) =
        crate::util::get_and_save_sort(&title_path, &user.username, &sort_params).await?;

    // Parse sort method from string
    let sort_method = SortMethod::parse(&sort_method_str);

    // Build the title info and gather all data
    let (title_info, nested_title_items, mut items) = {
        let lib = state.library.read().await;

        // Get the title
        let title = lib
            .get_title(&title_id)
            .ok_or_else(|| Error::NotFound(format!("Title not found: {}", title_id)))?;

        // Build parent breadcrumb chain
        let mut parents = Vec::new();
        let mut current_parent_id = title.parent_id.clone();
        while let Some(pid) = current_parent_id {
            if let Some(parent_title) = lib.get_title(&pid) {
                parents.push(ParentItem {
                    id: parent_title.id.clone(),
                    display_name: parent_title.title.clone(),
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
                if total_entries == 1 { "entry" } else { "entries" }
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
                if total_entries == 1 { "entry" } else { "entries" }
            )
        };

        // Build title info
        let cover_url = title
            .entries
            .first()
            .map(|e| format!("/api/cover/{}/{}", title.id, e.id))
            .unwrap_or_else(|| "/static/img/placeholder.png".to_string());

        let title_info = TitleInfo {
            id: title.id.clone(),
            title: title.title.clone(),
            display_name: title.title.clone(),
            sort_title: None, // TODO: load from info.json if available
            cover_url,
            content_label,
            parents,
        };

        // Build nested titles cards and calculate their progress
        let mut nested_title_items = Vec::new();

        for nested in &title.nested_titles {
            let nested_entry_count = nested.entries.len();
            let first_entry_id = nested.entries.first().map(|e| e.id.as_str());

            let card = BookCardItem::from_title(
                &nested.id,
                &nested.title,
                nested_entry_count,
                first_entry_id,
            );

            // Calculate average progress for nested title
            let mut total_progress = 0.0f64;
            let mut count = 0;
            for entry in &nested.entries {
                let (progress, _) = nested
                    .get_entry_progress(&user.username, &entry.id)
                    .await
                    .unwrap_or((0.0, 0));
                total_progress += progress as f64;
                count += 1;
            }
            let avg_progress = if count > 0 {
                total_progress / count as f64
            } else {
                0.0
            };

            nested_title_items.push(BookItem {
                item: card,
                progress: avg_progress,
            });
        }

        // Build entry items - use sort method if not progress-based
        let all_entries = if matches!(sort_method, SortMethod::Progress) {
            title.get_entries_sorted(SortMethod::Name, true) // Get name-sorted as base
        } else {
            title.get_entries_sorted(sort_method, ascending)
        };

        let mut items = Vec::new();
        for entry in all_entries {
            // Load progress for this entry using Title's method
            let (progress_percentage, _saved_page) = title
                .get_entry_progress(&user.username, &entry.id)
                .await
                .unwrap_or((0.0, 0));

            // Apply search filter if provided
            if let Some(ref search) = params.search {
                if !entry.title.to_lowercase().contains(&search.to_lowercase()) {
                    continue;
                }
            }

            let card = BookCardItem::from_entry(
                &entry.id,
                &entry.title,
                &title.id,
                &title.title,
                entry.pages,
                &entry.path.to_string_lossy(),
            );

            items.push(BookItem {
                item: card,
                progress: progress_percentage as f64,
            });
        }

        (title_info, nested_title_items, items)
    }; // Lock is released here

    // Sort by progress if requested (after calculating progress)
    if matches!(sort_method, SortMethod::Progress) {
        sort_by_progress(&mut items, ascending);
    }

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
    let supported_img_types = "image/jpeg,image/png,image/gif,image/webp".to_string();

    let template = BookTemplate {
        nav: crate::util::NavigationState::library().with_admin(user.is_admin),
        title: title_info,
        sort_options,
        sort_opt,
        nested_title_items,
        items,
        supported_img_types,
    };

    Ok(Html(template.render().map_err(render_error)?))
}
