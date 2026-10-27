use super::*;
use std::path::PathBuf;

fn entry(id: &str, path: &str, size_bytes: u64, size_known: bool) -> Entry {
    Entry {
        id: id.to_string(),
        path: PathBuf::from(path),
        title: id.to_string(),
        signature: String::new(),
        ctime: 0,
        mtime: 0,
        pages: 0,
        image_files: Vec::new(),
        size_bytes,
        err_msg: None,
        size_known,
        is_directory: false,
    }
}

fn title(id: &str, entries: Vec<Entry>, children: Vec<Title>) -> Title {
    Title {
        id: id.to_string(),
        path: PathBuf::from(format!("/library/{id}")),
        title: id.to_string(),
        signature: String::new(),
        contents_signature: String::new(),
        mtime: 0,
        entries,
        parent_id: None,
        nested_titles: children,
    }
}

fn report(titles: Vec<Title>, issues: Vec<StorageIssue>) -> StorageReport {
    aggregate(
        Path::new("/library"),
        &titles
            .into_iter()
            .map(|title| (title.id.clone(), Arc::new(title)))
            .collect(),
        Some(123),
        false,
        issues,
    )
}

#[test]
fn zero_unknown_and_unreadable_sizes_have_distinct_meanings() {
    let mut unreadable = entry("broken", "/library/Series/broken.cbz", 5, true);
    unreadable.err_msg = Some("Unreadable archive".to_string());
    let result = report(
        vec![title(
            "Series",
            vec![
                entry("zero", "/library/Series/zero.cbz", 0, true),
                entry("unknown", "/library/Series/unknown.cbz", 123, false),
                unreadable,
            ],
            vec![],
        )],
        vec![],
    );
    assert_eq!(result.total_bytes, 5);
    assert!(!result.complete);
    let series = &result.titles[0];
    assert_eq!(series.entry_count, 3);
    assert_eq!(series.measured_entries, 2);
    assert!(series
        .issues
        .iter()
        .any(|issue| issue.message == "Unreadable archive"));
    assert!(series
        .issues
        .iter()
        .any(|issue| issue.message.contains("not been measured")));

    let zero = report(
        vec![title(
            "Zero",
            vec![entry("zero", "/library/Zero/zero.cbz", 0, true)],
            vec![],
        )],
        vec![],
    );
    assert_eq!(zero.total_bytes, 0);
    assert!(zero.complete);
    assert_eq!(zero.titles[0].measured_entries, 1);
}

#[test]
fn unsafe_payloads_are_lower_bounds() {
    let mut loose = entry("loose", "/library/Series/Pages", 9, true);
    loose.image_files = vec!["001.png".to_string(), "002.png".to_string()];
    loose.is_directory = true;
    let mut escaping = entry("escape-page", "/library/Series/Other", 20, true);
    escaping.image_files = vec!["../outside.png".to_string()];
    escaping.is_directory = true;
    let result = report(
        vec![title(
            "Series",
            vec![
                loose,
                escaping,
                entry("outside", "/outside.cbz", 100, true),
                entry("parent", "/library/Series/../outside.cbz", 100, true),
            ],
            vec![],
        )],
        vec![],
    );
    assert_eq!(result.total_bytes, 9);
    assert!(!result.complete);
    assert_eq!(result.titles[0].measured_entries, 1);
    assert_eq!(result.titles[0].issues.len(), 3);
    assert!(result.titles[0]
        .issues
        .iter()
        .all(|issue| !issue.path.starts_with('/')));
}

#[test]
fn skipped_regions_propagate_to_ancestors_without_double_counting() {
    let mut child = title(
        "Series/Volume",
        vec![entry("nested", "/library/Series/Volume/1.cbz", 30, true)],
        vec![],
    );
    child.parent_id = Some("Series".to_string());
    let result = report(
        vec![
            title(
                "Series",
                vec![entry("direct", "/library/Series/1.cbz", 10, true)],
                vec![child],
            ),
            title(
                "B",
                vec![entry("other", "/library/B/1.cbz", 20, true)],
                vec![],
            ),
        ],
        vec![StorageIssue {
            path: "Series/Volume/Skipped".to_string(),
            message: "Permission denied".to_string(),
        }],
    );
    assert_eq!(result.total_bytes, 60);
    assert!(!result.complete);
    assert_eq!(result.titles[0].id, "Series");
    assert_eq!(result.titles[0].size_bytes, 40);
    assert_eq!(result.titles[0].children[0].size_bytes, 30);
    assert!(!result.titles[0].complete);
    assert!(!result.titles[0].children[0].complete);
    assert!(result.titles[1].complete);
}

#[test]
fn legacy_messagepack_entry_sizes_remain_unknown() {
    for size in [0, 123] {
        let old = entry("old", "/library/Series/old.cbz", size, true);
        // Persistent MessagePack caches serialize structs as positional arrays.
        let encoded = rmp_serde::to_vec(&(
            &old.id,
            &old.path,
            &old.title,
            &old.signature,
            old.ctime,
            old.mtime,
            old.pages,
            &old.image_files,
            old.size_bytes,
            &old.err_msg,
        ))
        .unwrap();
        let decoded: Entry = rmp_serde::from_slice(&encoded).unwrap();
        assert_eq!(decoded.size_bytes, size);
        assert!(!decoded.size_known);
        let titles = HashMap::from([(
            "Series".to_string(),
            Arc::new(title("Series", vec![decoded], vec![])),
        )]);
        let cached = aggregate(Path::new("/library"), &titles, None, true, Vec::new());
        assert!(cached.stale);
        assert_eq!(cached.measured_at, None);
        assert_eq!(cached.total_bytes, 0);
        assert!(!cached.complete);
        assert_eq!(cached.titles[0].measured_entries, 0);
    }
}

#[test]
fn legacy_loose_entries_are_unmeasured_not_unsafe() {
    // Caches written before `is_directory` existed deserialize it as false.
    let old = entry("old", "/library/Series/Chapter 1", 5, false);
    let result = report(vec![title("Series", vec![old], vec![])], vec![]);
    let issues = &result.titles[0].issues;
    assert_eq!(issues.len(), 1);
    assert_eq!(issues[0].message, "Entry size has not been measured");
    assert!(!result.complete);
}

#[test]
fn size_overflow_is_a_lower_bound_not_wrapped_or_complete() {
    let result = report(
        vec![
            title(
                "A",
                vec![
                    entry("large", "/library/A/large.cbz", u64::MAX, true),
                    entry("small", "/library/A/small.cbz", 1, true),
                ],
                vec![],
            ),
            title(
                "B",
                vec![entry("other", "/library/B/other.cbz", 1, true)],
                vec![],
            ),
        ],
        vec![],
    );
    assert_eq!(result.total_bytes, u64::MAX);
    assert!(!result.complete);
    assert!(!result.titles[0].complete);
    assert!(result
        .issues
        .iter()
        .any(|issue| issue.message.contains("representable")));
}

#[test]
fn admin_menu_label_marks_lower_bounds_and_hides_unmeasured_libraries() {
    let measured = |size| {
        vec![title(
            "Series",
            vec![entry("a", "/library/Series/1.cbz", size, true)],
            vec![],
        )]
    };
    assert_eq!(
        report(measured(3 * 1024 * 1024), vec![])
            .total_label()
            .as_deref(),
        Some("3.0 MiB")
    );
    assert_eq!(
        report(measured(0), vec![]).total_label().as_deref(),
        Some("0 B")
    );
    let skipped = vec![StorageIssue::new(
        Path::new("/library"),
        Path::new("/library/Series/Hidden"),
        "Unable to scan region: permission denied",
    )];
    assert_eq!(
        report(measured(1536), skipped).total_label().as_deref(),
        Some("≥ 1.5 KiB")
    );
    let unmeasured = vec![title(
        "Series",
        vec![entry("a", "/library/Series/1.cbz", 0, false)],
        vec![],
    )];
    assert_eq!(report(unmeasured, vec![]).total_label(), None);
}
