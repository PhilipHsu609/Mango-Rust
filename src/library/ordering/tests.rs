use super::{sort_entries, EntryOrdering, SortMethod, SortOptions};
use crate::library::{Entry, TitleInfo};

fn entry(name: &str, ctime: i64) -> Entry {
    Entry {
        id: name.into(),
        path: std::path::PathBuf::new(),
        title: name.into(),
        signature: String::new(),
        ctime,
        mtime: 0,
        pages: 10,
        image_files: Vec::new(),
        size_bytes: 0,
        err_msg: None,
    }
}

#[test]
fn continuation_date_fallback_uses_ctime_while_catalog_uses_zero() {
    let a = entry("A", 20);
    let b = entry("B", 10);
    let info = TitleInfo::default();
    let options = SortOptions {
        method: SortMethod::TimeAdded,
        ascending: true,
    };
    let mut items = [(&a, "A"), (&b, "B")];
    sort_entries(&mut items, &info, "reader", options, EntryOrdering::Catalog);
    assert_eq!(items.map(|(entry, _)| entry.id.as_str()), ["A", "B"]);
    sort_entries(
        &mut items,
        &info,
        "reader",
        options,
        EntryOrdering::Continuation,
    );
    assert_eq!(items.map(|(entry, _)| entry.id.as_str()), ["B", "A"]);
}

#[test]
fn reader_clamps_progress_while_book_retains_raw_percentage() {
    let a = entry("A", 0);
    let b = entry("B", 0);
    let mut info = TitleInfo::default();
    info.set_progress("reader", "A", 20);
    info.set_progress("reader", "B", 10);
    let options = SortOptions {
        method: SortMethod::Progress,
        ascending: true,
    };
    let mut items = [(&a, "A"), (&b, "B")];
    sort_entries(&mut items, &info, "reader", options, EntryOrdering::Book);
    assert_eq!(items.map(|(entry, _)| entry.id.as_str()), ["B", "A"]);
    sort_entries(&mut items, &info, "reader", options, EntryOrdering::Reader);
    assert_eq!(items.map(|(entry, _)| entry.id.as_str()), ["A", "B"]);
}

#[test]
fn automatic_catalog_order_detects_fractional_chapters_not_natural_numbers() {
    let a = entry("Chapter 1.10", 0);
    let b = entry("Chapter 1.2", 0);
    let info = TitleInfo::default();
    let options = SortOptions {
        method: SortMethod::Auto,
        ascending: true,
    };
    let mut items = [(&a, a.title.as_str()), (&b, b.title.as_str())];
    sort_entries(&mut items, &info, "reader", options, EntryOrdering::Catalog);
    assert_eq!(
        items.map(|(entry, _)| entry.id.as_str()),
        ["Chapter 1.10", "Chapter 1.2"]
    );
    sort_entries(&mut items, &info, "reader", options, EntryOrdering::Book);
    assert_eq!(
        items.map(|(entry, _)| entry.id.as_str()),
        ["Chapter 1.2", "Chapter 1.10"]
    );
}
