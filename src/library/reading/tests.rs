use super::continue_entry;
use crate::library::{Entry, Title, TitleInfo};

fn continue_reading_title() -> Title {
    Title {
        id: "title".into(),
        path: std::path::PathBuf::new(),
        title: "Title".into(),
        signature: String::new(),
        contents_signature: String::new(),
        mtime: 0,
        entries: (1..=3)
            .map(|number| Entry {
                id: format!("entry-{number}"),
                path: std::path::PathBuf::new(),
                title: format!("Volume {number}"),
                signature: String::new(),
                mtime: 0,
                ctime: 0,
                pages: 10,
                image_files: Vec::new(),
                size_bytes: 0,
                err_msg: None,
            })
            .collect(),
        parent_id: None,
        nested_titles: Vec::new(),
    }
}
#[test]
fn continue_reading_selects_one_progressed_entry_or_its_next_entry() {
    let title = continue_reading_title();
    let mut info = TitleInfo::default();
    info.set_progress("admin", "Volume 1", 10);
    info.set_progress("admin", "Volume 2", 3);

    let (selected, _) = continue_entry(&title, "admin", &info, &Default::default()).unwrap();
    assert_eq!(selected.id, "entry-2");

    info.set_progress("admin", "Volume 2", 10);
    let (selected, previous) = continue_entry(&title, "admin", &info, &Default::default()).unwrap();
    assert_eq!(selected.id, "entry-3");
    assert_eq!(previous.unwrap().id, "entry-2");
}
#[test]
fn continue_reading_uses_date_added_sort_order() {
    let title = continue_reading_title();
    let mut info = TitleInfo::default();
    info.set_sort_by("admin", "time_added", true);
    info.set_date_added("Volume 2", 1_600_000_000);
    info.set_date_added("Volume 1", 1_700_000_000);
    info.set_progress("admin", "Volume 1", 2);

    let (selected, _) = continue_entry(&title, "admin", &info, &Default::default()).unwrap();
    assert_eq!(selected.id, "entry-1");
}
#[test]
fn continue_reading_uses_progress_sort_order() {
    let title = continue_reading_title();
    let mut info = TitleInfo::default();
    info.set_sort_by("admin", "progress", true);
    info.set_progress("admin", "Volume 1", 10);
    info.set_progress("admin", "Volume 2", 3);

    let (selected, _) = continue_entry(&title, "admin", &info, &Default::default()).unwrap();
    assert_eq!(selected.id, "entry-3");
}

#[test]
fn continue_reading_uses_entry_sort_title_overrides() {
    let title = continue_reading_title();
    let mut info = TitleInfo::default();
    info.set_progress("admin", "Volume 1", 2);
    info.set_progress("admin", "Volume 2", 3);
    let sort_titles = std::collections::HashMap::from([
        ("entry-1".to_string(), "Z".to_string()),
        ("entry-2".to_string(), "A".to_string()),
        ("entry-3".to_string(), "M".to_string()),
    ]);

    let (selected, _) = continue_entry(&title, "admin", &info, &sort_titles).unwrap();
    assert_eq!(selected.id, "entry-1");
}

#[test]
fn continue_reading_falls_back_to_first_unfinished_when_latest_is_last() {
    let title = continue_reading_title();
    let mut info = TitleInfo::default();
    info.set_progress("admin", "Volume 3", 10);

    let (selected, previous) = continue_entry(&title, "admin", &info, &Default::default()).unwrap();
    assert_eq!(selected.id, "entry-1");
    assert!(previous.is_none());
}
