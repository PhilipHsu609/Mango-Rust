use crate::library::{Entry, Title};

/// Card item for home page - unified structure for entries and titles
/// Matches the fields expected by templates/components/card.html
#[derive(serde::Serialize, Clone)]
pub(super) struct CardItem {
    // Common fields
    pub(super) id: String,
    pub(super) is_entry: bool,
    pub(super) display_name: String,
    pub(super) cover_url: String,

    // Entry-specific fields (used when is_entry = true)
    pub(super) book_id: String,
    pub(super) book_display_name: String,
    pub(super) pages: usize,
    pub(super) encoded_path: String,
    pub(super) encoded_title: String,
    pub(super) encoded_book_title: String,
    pub(super) err_msg: Option<String>,

    // Title-specific fields (used when is_entry = false)
    pub(super) content_label: String,
    pub(super) grouped_count: Option<usize>,

    // Optional metadata
    pub(super) title: Option<String>,
    pub(super) sort_title: Option<String>,
}

impl CardItem {
    /// Create a card item for an entry.
    pub(super) fn from_entry(
        entry: &Entry,
        book: &Title,
        info: &crate::library::metadata::TitleInfo,
    ) -> Self {
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
            grouped_count: None,
            title: Some(entry.title.clone()),
            sort_title: Some(entry.title.clone()),
        }
    }

    /// Create a card item for a title.
    pub(super) fn from_title(
        title_id: &str,
        title_name: &str,
        entry_count: usize,
        first_entry_id: Option<&str>,
        first_entry_title: Option<&str>,
        info: &crate::library::metadata::TitleInfo,
    ) -> Self {
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
        let content_label = if entry_count == 1 {
            "1 entry".to_string()
        } else {
            format!("{} entries", entry_count)
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
            grouped_count: None,
            title: Some(title_name.to_string()),
            sort_title: Some(title_name.to_string()),
        }
    }
}
