use super::{
    compare_title_keys, sort_entries, EntryOrdering, SortMethod, SortOptions, TitleNameOrder,
    TitleSortKey,
};
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

#[test]
fn numeric_title_order_matches_mango_integer_and_token_rules() {
    use std::cmp::Ordering::{Equal, Less};

    for (left, right, expected) in [
        ("Chapter 2", "Chapter 10", Less),
        ("Chapter 01", "Chapter 1", Equal),
        ("Chapter", "Chapter 1", Less),
        (
            "Chapter 999999999999999999999999",
            "Chapter 1000000000000000000000000",
            Less,
        ),
    ] {
        for ascending in [true, false] {
            let actual = compare_title_keys(
                TitleSortKey {
                    name: left,
                    mtime: 0,
                    progress: 0.0,
                },
                TitleSortKey {
                    name: right,
                    mtime: 0,
                    progress: 0.0,
                },
                SortOptions {
                    method: SortMethod::Name,
                    ascending,
                },
                TitleNameOrder::Numeric,
            );
            assert_eq!(
                actual,
                if ascending {
                    expected
                } else {
                    expected.reverse()
                }
            );
        }
    }
}

fn assert_automatic_catalog_order(input: &[&str], expected: &[&str]) {
    let entries: Vec<_> = input.iter().map(|name| entry(name, 0)).collect();
    let mut items: Vec<_> = entries
        .iter()
        .map(|entry| (entry, entry.title.as_str()))
        .collect();
    sort_entries(
        &mut items,
        &TitleInfo::default(),
        "reader",
        SortOptions {
            method: SortMethod::Auto,
            ascending: true,
        },
        EntryOrdering::Catalog,
    );
    assert_eq!(
        items
            .iter()
            .map(|(entry, _)| entry.title.as_str())
            .collect::<Vec<_>>(),
        expected,
    );
}

#[test]
fn automatic_catalog_order_matches_mango_chapter_fixture() {
    assert_automatic_catalog_order(
        &[
            "Ch.04",
            "Ch. 3",
            "Vol.2 Ch. 2.5",
            "Vol.1 Ch.02",
            "Vol.1 Ch.01",
        ],
        &[
            "Vol.1 Ch.01",
            "Vol.1 Ch.02",
            "Vol.2 Ch. 2.5",
            "Ch. 3",
            "Ch.04",
        ],
    );
}

#[test]
fn automatic_catalog_order_sorts_fractional_chapters_numerically() {
    assert_automatic_catalog_order(
        &["Chapter 0.1", "Chapter 0.01"],
        &["Chapter 0.01", "Chapter 0.1"],
    );
}

#[test]
fn automatic_catalog_order_handles_mixed_volume_and_episode_names() {
    assert_automatic_catalog_order(
        &[
            "Vol. 1 Ch. 1",
            "Vol. 2 Ch. 2",
            "Season 1 Episode 100",
            "Season 2 Episode 200",
        ],
        &[
            "Season 1 Episode 100",
            "Season 2 Episode 200",
            "Vol. 1 Ch. 1",
            "Vol. 2 Ch. 2",
        ],
    );
}

#[test]
fn parses_mango_date_added_sort_name() {
    assert_eq!(SortMethod::parse("time_added"), SortMethod::TimeAdded);
    assert_eq!(SortMethod::parse("added"), SortMethod::TimeAdded);
}

#[test]
fn query_sort_parameters_parse_aliases_and_integer_direction() {
    for (sort, expected) in [
        ("TITLE", SortMethod::Name),
        ("time", SortMethod::TimeModified),
        ("progress", SortMethod::Progress),
        ("unknown", SortMethod::Auto),
    ] {
        assert_eq!(
            SortMethod::from_params(Some(sort), Some("0")),
            (expected, false)
        );
        assert_eq!(
            SortMethod::from_params(Some(sort), Some("-1")),
            (expected, true)
        );
        assert_eq!(
            SortMethod::from_params(Some(sort), Some("invalid")),
            (expected, true)
        );
    }
}
