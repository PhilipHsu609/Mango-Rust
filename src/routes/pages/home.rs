use std::cmp::Reverse;

use askama::Template;
use axum::{extract::State, response::Html};

use super::cards::CardItem;
use crate::{
    auth::User,
    error::Result,
    library::reading::{
        can_start_reading, continue_entry, entry_progress_percent, group_recent_entries,
        title_progress_percent, RecentEntry, StartReadingProfile, RECENT_ITEMS_LIMIT,
    },
    routes::presentation::{render_error, NavigationState},
    AppState,
};

/// Continue reading item (entry with progress)
#[derive(serde::Serialize)]
struct ContinueReadingItem {
    entry: CardItem,
    percentage: f32,
}

/// Recently added item (entry or title with optional percentage)
#[derive(serde::Serialize)]
struct RecentlyAddedItem {
    #[serde(flatten)]
    item: CardItem,
    percentage: f64,
    grouped_count: Option<usize>,
}

/// Home page template
#[derive(Template)]
#[template(path = "home.html")]
struct HomeTemplate {
    nav: NavigationState,
    // User state
    new_user: bool,
    empty_library: bool,
    // Config info
    library_path: String,
    config_path: String,
    scan_interval: u32,
    // Content sections
    continue_reading: Vec<ContinueReadingItem>,
    start_reading: Vec<CardItem>,
    recently_added: Vec<RecentlyAddedItem>,
}

/// GET / - Home page with Continue Reading, Start Reading, Recently Added (requires authentication)
pub async fn home(State(state): State<AppState>, user: User) -> Result<Html<String>> {
    // Get library stats to determine empty_library
    let (title_count, has_any_progress) = {
        let lib = state.library.load();
        let stats = lib.stats();

        // Check if user has any reading progress
        // For now, we'll do a simple check - iterate through titles and check progress
        let mut has_progress = false;
        for title in lib.get_titles() {
            if let Ok(progress) =
                title_progress_percent(title, lib.metadata(), &user.username).await
            {
                if progress > 0.0 {
                    has_progress = true;
                    break;
                }
            }
        }

        (stats.titles, has_progress)
    };

    let empty_library = title_count == 0;
    let new_user = !has_any_progress;

    // Get library path and config path from state
    let library_path = state.config.library_path.display().to_string();
    let config_path = dirs::config_dir()
        .map(|p| p.join("mango/config.yml").display().to_string())
        .unwrap_or_else(|| "~/.config/mango/config.yml".to_string());
    let scan_interval = state.config.scan_interval_minutes;

    // Get home page content sections
    let (continue_reading, start_reading, recently_added) = {
        let lib = state.library.load();
        let mut cr_items = Vec::new();
        let mut sr_items = Vec::new();
        let mut ra_items = Vec::new();

        const MAX_ITEMS: usize = 8;
        let one_month_ago = chrono::Utc::now().timestamp() - (30 * 24 * 60 * 60);

        // Collect data for all titles
        let titles = lib.all_titles();
        let mut info_by_title = std::collections::HashMap::with_capacity(titles.len());

        for (title_index, title) in titles.iter().enumerate() {
            let info = match lib.metadata().read(&title.path).await {
                Ok(info) => info,
                Err(_) => continue,
            };
            info_by_title.insert(title.id.clone(), info.clone());

            // Continue Reading: one Mango-selected entry per title.
            let mut sort_title_overrides = std::collections::HashMap::new();
            for entry in &title.entries {
                if let Some(sort_title) = state.storage.get_entry_sort_title(&entry.id).await? {
                    sort_title_overrides.insert(entry.id.clone(), sort_title);
                }
            }
            if let Some((entry, previous)) =
                continue_entry(title, &user.username, &info, &sort_title_overrides)
            {
                let last_read = info
                    .get_last_read(&user.username, &entry.title)
                    .or_else(|| {
                        previous.and_then(|entry| info.get_last_read(&user.username, &entry.title))
                    });
                let progress = info.get_progress(&user.username, &entry.title).unwrap_or(0);
                let percentage = entry_progress_percent(progress, entry.pages);

                cr_items.push((
                    last_read.unwrap_or(i64::MIN),
                    ContinueReadingItem {
                        entry: CardItem::from_entry(entry, title, &info),
                        percentage,
                    },
                ));
            }

            // Recently added: entries added within last month
            for (entry_index, entry) in title.entries.iter().enumerate() {
                if let Some(date_added) = info.get_date_added(&entry.title) {
                    if date_added > one_month_ago {
                        let progress = info.get_progress(&user.username, &entry.title).unwrap_or(0);
                        let percentage = if entry.pages > 0 {
                            (progress as f64 / entry.pages as f64) * 100.0
                        } else {
                            0.0
                        };

                        ra_items.push(RecentEntry {
                            title_id: title.id.clone(),
                            date_added,
                            percentage,
                            item: (title_index, entry_index),
                        });
                    }
                }
            }
        }

        for title in lib.get_titles() {
            let progress = title_progress_percent(title, lib.metadata(), &user.username)
                .await
                .unwrap_or(0.0);
            if can_start_reading(title, f64::from(progress), StartReadingProfile::Browser) {
                let info = info_by_title.get(&title.id).cloned().unwrap_or_default();
                sr_items.push(CardItem::from_title(
                    &title.id,
                    &title.title,
                    title.entries.len(),
                    title.entries.first().map(|entry| entry.id.as_str()),
                    title.entries.first().map(|entry| entry.title.as_str()),
                    &info,
                ));
            }
        }

        cr_items.truncate(MAX_ITEMS);

        // Sort continue_reading by last_read (most recent first) and take top items
        cr_items.sort_by_key(|(last_read, _)| Reverse(*last_read));
        let continue_reading: Vec<ContinueReadingItem> = cr_items
            .into_iter()
            .take(MAX_ITEMS)
            .map(|(_, item)| item)
            .collect();

        // Shuffle start_reading titles (random selection like original Mango)
        use rand::seq::SliceRandom;
        let mut rng = rand::thread_rng();
        sr_items.shuffle(&mut rng);
        sr_items.truncate(MAX_ITEMS);

        // Group recent entries by title, then limit the number of cards
        let recently_added: Vec<RecentlyAddedItem> =
            group_recent_entries(ra_items, RECENT_ITEMS_LIMIT)
                .into_iter()
                .map(|group| {
                    let title = titles[group.item.0];
                    let info = info_by_title.get(&title.id).cloned().unwrap_or_default();
                    let entry = &title.entries[group.item.1];
                    let item = if group.grouped_count > 1 {
                        let mut item = CardItem::from_title(
                            &title.id,
                            &title.title,
                            title.entries.len(),
                            title.entries.first().map(|entry| entry.id.as_str()),
                            title.entries.first().map(|entry| entry.title.as_str()),
                            &info,
                        );
                        item.content_label = format!("{} new entries", group.grouped_count);
                        item.grouped_count = Some(group.grouped_count);
                        item
                    } else {
                        CardItem::from_entry(entry, title, &info)
                    };

                    RecentlyAddedItem {
                        item,
                        percentage: group.percentage,
                        grouped_count: Some(group.grouped_count),
                    }
                })
                .collect();

        (continue_reading, sr_items, recently_added)
    };

    let template = HomeTemplate {
        nav: NavigationState::home().with_admin(user.is_admin),
        new_user,
        empty_library,
        library_path,
        config_path,
        scan_interval,
        continue_reading,
        start_reading,
        recently_added,
    };

    Ok(Html(template.render().map_err(render_error)?))
}
