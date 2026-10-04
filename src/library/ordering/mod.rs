mod chapter;

#[cfg(test)]
mod tests;

use std::cmp::Ordering;

use super::{metadata::TitleInfo, Entry, Title};

#[derive(Clone, Copy)]
pub struct SortOptions {
    pub method: SortMethod,
    pub ascending: bool,
}

/// Existing consumers deliberately use different automatic orders and progress
/// precision. Profiles make those contracts explicit rather than conflating them.
#[derive(Clone, Copy)]
pub enum EntryOrdering {
    Catalog,
    Book,
    Reader,
    Continuation,
}

#[derive(Clone, Copy)]
pub enum TitleNameOrder {
    Natural,
    Numeric,
}

pub struct TitleSortKey<'a> {
    pub name: &'a str,
    pub mtime: i64,
    pub progress: f64,
}

pub fn compare_title_keys(
    left: TitleSortKey<'_>,
    right: TitleSortKey<'_>,
    options: SortOptions,
    names: TitleNameOrder,
) -> Ordering {
    let by_name = || match names {
        TitleNameOrder::Natural => natord::compare(left.name, right.name),
        TitleNameOrder::Numeric => chapter::compare_numerically(left.name, right.name),
    };
    let order = match options.method {
        SortMethod::TimeModified => left.mtime.cmp(&right.mtime).then_with(by_name),
        SortMethod::Progress => left.progress.total_cmp(&right.progress).then_with(by_name),
        SortMethod::Name | SortMethod::TimeAdded | SortMethod::Auto => by_name(),
    };
    if options.ascending {
        order
    } else {
        order.reverse()
    }
}

/// Sort already resolved entry/name pairs without copying their names. Both
/// owned route names and borrowed continuation overrides use this interface.
pub fn sort_entries<N: AsRef<str>>(
    items: &mut [(&Entry, N)],
    info: &TitleInfo,
    username: &str,
    options: SortOptions,
    profile: EntryOrdering,
) {
    let numeric = matches!(
        profile,
        EntryOrdering::Catalog | EntryOrdering::Continuation
    );
    let chapter_sorter = if numeric && matches!(options.method, SortMethod::Auto) {
        let names: Vec<_> = items.iter().map(|(_, name)| name.as_ref()).collect();
        Some(chapter::ChapterSorter::new(&names))
    } else {
        None
    };
    items.sort_by(|(left, left_name), (right, right_name)| {
        let by_name = || {
            if numeric {
                chapter::compare_numerically(left_name.as_ref(), right_name.as_ref())
            } else {
                natord::compare(left_name.as_ref(), right_name.as_ref())
            }
        };
        match options.method {
            SortMethod::Name => by_name(),
            SortMethod::Auto => match &chapter_sorter {
                Some(sorter) => sorter
                    .compare(left_name.as_ref(), right_name.as_ref())
                    .then_with(by_name),
                None => by_name(),
            },
            SortMethod::TimeModified => left.mtime.cmp(&right.mtime).then_with(by_name),
            SortMethod::TimeAdded => {
                let date = |entry: &Entry| {
                    info.get_date_added(&entry.title).unwrap_or({
                        if matches!(profile, EntryOrdering::Continuation) {
                            entry.ctime
                        } else {
                            0
                        }
                    })
                };
                date(left).cmp(&date(right)).then_with(by_name)
            }
            SortMethod::Progress => {
                let progress =
                    |entry: &Entry| info.get_progress(username, &entry.title).unwrap_or(0);
                let order = match profile {
                    EntryOrdering::Catalog => {
                        super::reading::entry_progress_fraction(progress(left), left.pages)
                            .total_cmp(&super::reading::entry_progress_fraction(
                                progress(right),
                                right.pages,
                            ))
                    }
                    EntryOrdering::Book => {
                        super::reading::entry_progress_percent(progress(left), left.pages)
                            .total_cmp(&super::reading::entry_progress_percent(
                                progress(right),
                                right.pages,
                            ))
                    }
                    EntryOrdering::Reader | EntryOrdering::Continuation => {
                        let fraction = |entry: &Entry| {
                            if entry.pages == 0 {
                                0.0
                            } else {
                                progress(entry).clamp(0, entry.pages as i32) as f32
                                    / entry.pages as f32
                            }
                        };
                        fraction(left).total_cmp(&fraction(right))
                    }
                };
                order.then_with(by_name)
            }
        }
    });
    if !options.ascending {
        items.reverse();
    }
}

/// Snapshot ordering does not use per-user progress or name overrides. Modified
/// time ties stay stable in either direction, as in the original snapshot query.
pub(super) fn sort_snapshot_titles(items: &mut [&Title], options: SortOptions) {
    items.sort_by(|left, right| {
        let order = match options.method {
            SortMethod::TimeModified => left.mtime.cmp(&right.mtime),
            _ => natord::compare(&left.title, &right.title),
        };
        if options.ascending {
            order
        } else {
            order.reverse()
        }
    });
}
/// Sorting methods for titles and entries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SortMethod {
    /// Sort alphabetically by name/title.
    Name,
    /// Sort by modification time.
    TimeModified,
    /// Sort by added time.
    TimeAdded,
    /// Sort by reading progress.
    Progress,
    /// Smart chapter detection.
    #[default]
    Auto,
}

impl SortMethod {
    /// Parse from string parameter (for API routes)
    /// Matches original Mango API: "title", "modified", "auto"
    pub fn parse(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "title" | "name" => SortMethod::Name,
            "modified" | "time" | "time_modified" => SortMethod::TimeModified,
            "added" | "time_added" => SortMethod::TimeAdded,
            "progress" => SortMethod::Progress,
            "auto" => SortMethod::Auto,
            _ => SortMethod::default(),
        }
    }

    /// Parse sort method and ascend flag from query parameters
    /// Returns (SortMethod, bool) where bool is true for ascending
    pub fn from_params(sort: Option<&str>, ascend: Option<&str>) -> (Self, bool) {
        let method = sort.map(Self::parse).unwrap_or_default();
        let ascending = ascend
            .and_then(|s| s.parse::<i32>().ok())
            .map(|v| v != 0)
            .unwrap_or(true); // Default to ascending
        (method, ascending)
    }
}
