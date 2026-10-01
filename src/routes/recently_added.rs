use std::collections::HashMap;

pub(super) const RECENT_ITEMS_LIMIT: usize = 8;
const SECONDS_PER_DAY: u64 = 24 * 60 * 60;

pub(super) struct RecentEntry<T> {
    pub title_id: String,
    pub date_added: i64,
    pub percentage: f32,
    pub item: T,
}

pub(super) struct GroupedRecentEntry<T> {
    pub title_id: String,
    pub date_added: i64,
    pub percentage: f32,
    pub grouped_count: usize,
    pub item: T,
}

pub(super) fn group_recent_entries<T>(
    mut entries: Vec<RecentEntry<T>>,
    limit: usize,
) -> Vec<GroupedRecentEntry<T>> {
    let mut latest_by_title = HashMap::<String, i64>::new();
    for entry in &entries {
        if let Some(latest) = latest_by_title.get_mut(entry.title_id.as_str()) {
            *latest = (*latest).max(entry.date_added);
        } else {
            latest_by_title.insert(entry.title_id.clone(), entry.date_added);
        }
    }

    // Keep equal-second entries from one recent title batch adjacent.
    entries.sort_by(|a, b| {
        let a_title_latest = latest_by_title
            .get(a.title_id.as_str())
            .expect("every recent entry has a title timestamp");
        let b_title_latest = latest_by_title
            .get(b.title_id.as_str())
            .expect("every recent entry has a title timestamp");
        b.date_added
            .cmp(&a.date_added)
            .then_with(|| b_title_latest.cmp(a_title_latest))
            .then_with(|| a.title_id.cmp(&b.title_id))
    });

    let mut groups: Vec<GroupedRecentEntry<T>> = Vec::with_capacity(entries.len().min(limit));
    for entry in entries {
        let can_group = groups.last().is_some_and(|last| {
            last.title_id == entry.title_id
                && last.date_added.abs_diff(entry.date_added) < SECONDS_PER_DAY
        });

        if can_group {
            let last = groups.last_mut().expect("checked for a recent group");
            last.grouped_count += 1;
            last.percentage = -1.0;
        } else {
            if groups.len() == limit {
                break;
            }

            groups.push(GroupedRecentEntry {
                title_id: entry.title_id,
                date_added: entry.date_added,
                percentage: entry.percentage,
                grouped_count: 1,
                item: entry.item,
            });
        }
    }

    groups
}

#[cfg(test)]
mod tests {
    use super::{group_recent_entries, RecentEntry};

    fn entry(
        title_id: &str,
        date_added: i64,
        item: &'static str,
        percentage: f32,
    ) -> RecentEntry<&'static str> {
        RecentEntry {
            title_id: title_id.to_string(),
            date_added,
            percentage,
            item,
        }
    }

    #[test]
    fn groups_new_entries_for_same_title_within_24_hours() {
        let groups = group_recent_entries(
            vec![
                entry("a", 200_000, "newest-a", 12.5),
                entry("b", 100_000, "b", 20.0),
                entry("a", 113_601, "older-a", 45.0),
            ],
            8,
        );

        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].item, "newest-a");
        assert_eq!(groups[0].grouped_count, 2);
        assert_eq!(groups[0].percentage, -1.0);
        assert_eq!(groups[1].item, "b");
        assert_eq!(groups[1].percentage, 20.0);
    }

    #[test]
    fn separates_titles_and_exact_24_hour_boundaries() {
        let groups = group_recent_entries(
            vec![
                entry("a", 200_000, "a-new", 0.0),
                entry("a", 113_600, "a-at-boundary", 0.0),
                entry("b", 100_000, "b-new", 0.0),
                entry("b", 13_601, "b-within-day", 0.0),
            ],
            8,
        );

        assert_eq!(groups.len(), 3);
        assert_eq!(
            groups
                .iter()
                .map(|group| group.title_id.as_str())
                .collect::<Vec<_>>(),
            vec!["a", "a", "b"]
        );
        assert_eq!(
            groups
                .iter()
                .map(|group| group.grouped_count)
                .collect::<Vec<_>>(),
            vec![1, 1, 2]
        );
        assert_eq!(groups[2].percentage, -1.0);
    }

    #[test]
    fn limit_applies_to_groups_after_entries_are_combined() {
        let groups = group_recent_entries(
            vec![
                entry("a", 300_000, "a-newest", 0.0),
                entry("a", 299_000, "a-middle", 0.0),
                entry("a", 298_000, "a-oldest", 0.0),
                entry("b", 200_000, "b", 0.0),
                entry("c", 100_000, "c", 0.0),
            ],
            2,
        );

        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].grouped_count, 3);
        assert_eq!(groups[1].item, "b");
    }
    #[test]
    fn equal_timestamps_keep_entries_of_the_same_title_together() {
        let groups = group_recent_entries(
            vec![
                entry("a", 200_000, "a-newest", 0.0),
                entry("b", 199_000, "b", 0.0),
                entry("a", 199_000, "a-same-second", 0.0),
            ],
            8,
        );

        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].title_id, "a");
        assert_eq!(groups[0].grouped_count, 2);
        assert_eq!(groups[1].title_id, "b");
    }
}
