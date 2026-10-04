use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};

use super::media::join_base_url;
use crate::{
    error::Result,
    library::{
        chapter_sort::{compare_numerically, ChapterSorter},
        Entry, SortMethod,
    },
    AppState,
};

/// API route: GET /api/library
/// Returns Mango's library object with title and entry JSON.
#[utoipa::path(get, path = "/api/library", tag = "library", summary = "Get the library", params(("depth" = Option<i32>, Query, description = "Maximum nested-title depth"), ("percentage" = Option<String>, Query, description = "Include reading percentages"), ("slim" = Option<String>, Query, description = "Use the slim response format")), responses((status = 200, description = "Library returned")))]
pub async fn get_library(
    State(state): State<AppState>,
    crate::auth::Username(username): crate::auth::Username,
    Query(params): Query<CatalogQuery>,
) -> axum::response::Response {
    let lib = state.library.load_full();
    let cache = lib.progress_cache();
    let library_info = crate::library::progress::TitleInfo::load(&state.config.library_path)
        .await
        .unwrap_or_default();
    let (library_sort, library_ascending) = library_info
        .get_sort_by(&username)
        .unwrap_or_else(|| ("auto".to_string(), true));
    let (sort_method, ascending) = SortMethod::from_params(
        Some(&library_sort),
        Some(if library_ascending { "1" } else { "0" }),
    );
    let depth = catalog_depth(params.depth.as_deref());
    let mut titles = Vec::new();
    let mut ordered_titles = Vec::with_capacity(lib.get_titles().len());
    for title in lib.get_titles() {
        let sort_title = match state.storage.get_title_sort_title(&title.id).await {
            Ok(sort_title) => sort_title.unwrap_or_else(|| title.title.clone()),
            Err(error) => {
                return Json(serde_json::json!({
                    "success": false,
                    "error": error.to_string()
                }))
                .into_response();
            }
        };
        let info = cache.get_title_info(&title.id).unwrap_or_default();
        let percentage = title_progress_percentage(title, cache, &username);
        ordered_titles.push((title, sort_title, percentage, info));
    }
    ordered_titles.sort_by(
        |(left, left_sort, left_progress, _), (right, right_sort, right_progress, _)| {
            match sort_method {
                SortMethod::TimeModified => left
                    .mtime
                    .cmp(&right.mtime)
                    .then_with(|| compare_numerically(left_sort, right_sort)),
                SortMethod::Progress => left_progress
                    .total_cmp(right_progress)
                    .then_with(|| compare_numerically(left_sort, right_sort)),
                SortMethod::Name | SortMethod::TimeAdded | SortMethod::Auto => {
                    compare_numerically(left_sort, right_sort)
                }
            }
        },
    );
    if !ascending {
        ordered_titles.reverse();
    }
    let mut title_percentages = Vec::with_capacity(ordered_titles.len());

    for (title, _, percentage, info) in ordered_titles {
        title_percentages.push(percentage);
        match mango_title_response(
            &state,
            title,
            &info,
            cache,
            &username,
            Vec::new(),
            TitleResponseOptions {
                depth,
                include_percentages: params.percentage.is_some(),
                slim: params.slim.is_some(),
                sort_context: Some((sort_method, ascending)),
            },
        )
        .await
        {
            Ok(response) => titles.push(response),
            Err(error) => {
                return Json(serde_json::json!({
                    "success": false,
                    "error": error.to_string()
                }))
                .into_response();
            }
        }
    }

    Json(MangoLibraryResponse {
        dir: state.config.library_path.to_string_lossy().into_owned(),
        titles,
        title_percentages: params.percentage.map(|_| title_percentages),
    })
    .into_response()
}

/// API route: GET /api/book/:tid
/// Returns Mango's title JSON contract.
#[utoipa::path(get, path = "/api/book/{tid}", tag = "library", summary = "Get a title", params(("tid" = String, Path, description = "Title ID"), ("depth" = Option<i32>, Query, description = "Maximum nested-title depth"), ("percentage" = Option<String>, Query, description = "Include reading percentages"), ("slim" = Option<String>, Query, description = "Use the slim response format")), responses((status = 200, description = "Title returned")))]
pub async fn get_title(
    State(state): State<AppState>,
    Path(title_id): Path<String>,
    crate::auth::Username(username): crate::auth::Username,
    Query(params): Query<CatalogQuery>,
) -> Result<axum::response::Response> {
    let lib = state.library.load_full();
    let Some(title) = lib.get_title(&title_id) else {
        return Ok((
            StatusCode::NOT_FOUND,
            format!("Title ID `{title_id}` not found"),
        )
            .into_response());
    };
    let info = lib
        .progress_cache()
        .get_title_info(&title.id)
        .unwrap_or_default();
    let response = match mango_title_response(
        &state,
        title,
        &info,
        lib.progress_cache(),
        &username,
        title_parent_summaries(&lib, title),
        TitleResponseOptions {
            depth: catalog_depth(params.depth.as_deref()),
            include_percentages: params.percentage.is_some(),
            slim: params.slim.is_some(),
            sort_context: None,
        },
    )
    .await
    {
        Ok(response) => response,
        Err(error) => {
            return Ok((StatusCode::NOT_FOUND, error.to_string()).into_response());
        }
    };

    Ok(Json(response).into_response())
}

#[derive(Deserialize)]
pub struct CatalogQuery {
    depth: Option<String>,
    percentage: Option<String>,
    slim: Option<String>,
}

fn catalog_depth(depth: Option<&str>) -> i32 {
    depth.and_then(|value| value.parse().ok()).unwrap_or(-1)
}

#[derive(Serialize)]
pub(super) struct MangoEntry {
    path: String,
    title: String,
    size: String,
    id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    err_msg: Option<String>,
    zip_path: String,
    title_id: String,
    title_title: String,
    sort_title: String,
    pages: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    display_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cover_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    mtime: Option<i64>,
}

#[derive(Serialize)]
pub(super) struct MangoTitleSummary {
    dir: String,
    title: String,
    id: String,
    signature: u64,
    sort_title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    display_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cover_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    mtime: Option<i64>,
    parents: Vec<MangoTitleParent>,
}

#[derive(Serialize)]
pub(super) struct MangoTitleResponse {
    #[serde(flatten)]
    title: MangoTitleSummary,
    #[serde(skip_serializing_if = "Option::is_none")]
    titles: Option<Vec<MangoTitleResponse>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    entries: Option<Vec<MangoEntry>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    title_percentages: Option<Vec<f64>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    entry_percentages: Option<Vec<f64>>,
}

#[derive(Serialize)]
struct MangoLibraryResponse {
    dir: String,
    titles: Vec<MangoTitleResponse>,
    #[serde(skip_serializing_if = "Option::is_none")]
    title_percentages: Option<Vec<f64>>,
}

#[derive(Clone, Serialize)]
pub(super) struct MangoTitleParent {
    title: String,
    id: String,
}

pub(super) async fn mango_entry_response(
    state: &AppState,
    title: &crate::library::Title,
    entry: &Entry,
    info: &crate::library::progress::TitleInfo,
    sort_title_override: Option<&str>,
    slim: bool,
) -> Result<MangoEntry> {
    let path = entry.path.to_string_lossy().into_owned();
    let size = if entry.size_bytes == 0 {
        tokio::fs::metadata(&entry.path).await?.len()
    } else {
        entry.size_bytes
    };
    let sort_title = if let Some(sort_title) = sort_title_override {
        sort_title.to_string()
    } else {
        state
            .storage
            .get_entry_sort_title(&entry.id)
            .await?
            .unwrap_or_else(|| entry.title.clone())
    };
    let display_name = info
        .entry_display_name
        .get(&entry.title)
        .filter(|name| !name.is_empty())
        .cloned()
        .unwrap_or_else(|| entry.title.clone());
    let cover_url = if entry.err_msg.is_some() {
        format!("{}static/img/icons/icon_x192.png", state.config.base_url)
    } else {
        info.entry_cover_url
            .get(&entry.title)
            .filter(|url| !url.is_empty())
            .map(|url| join_base_url(&state.config.base_url, url))
            .unwrap_or_else(|| {
                format!(
                    "{}api/cover/{}/{}",
                    state.config.base_url, title.id, entry.id
                )
            })
    };

    Ok(MangoEntry {
        path: path.clone(),
        title: entry.title.clone(),
        size: humansize::format_size(size, humansize::BINARY),
        id: entry.id.clone(),
        err_msg: entry.err_msg.clone(),
        zip_path: path,
        title_id: title.id.clone(),
        title_title: title.title.clone(),
        sort_title,
        pages: entry.pages,
        display_name: (!slim).then_some(display_name),
        cover_url: (!slim).then_some(cover_url),
        mtime: (!slim).then_some(entry.mtime),
    })
}

pub(super) async fn mango_title_summary(
    state: &AppState,
    title: &crate::library::Title,
    info: &crate::library::progress::TitleInfo,
    parents: Vec<MangoTitleParent>,
    slim: bool,
) -> Result<MangoTitleSummary> {
    let sort_title = state
        .storage
        .get_title_sort_title(&title.id)
        .await?
        .unwrap_or_else(|| title.title.clone());
    let display_name = if info.display_name.is_empty() {
        title.title.clone()
    } else {
        info.display_name.clone()
    };
    let cover_url = if !info.cover_url.is_empty() {
        join_base_url(&state.config.base_url, &info.cover_url)
    } else if let Some(entry) = title.entries.first() {
        mango_entry_response(state, title, entry, info, None, false)
            .await?
            .cover_url
            .unwrap_or_default()
    } else {
        format!("{}img/icons/icon_x192.png", state.config.base_url)
    };

    Ok(MangoTitleSummary {
        dir: title.path.to_string_lossy().into_owned(),
        title: title.title.clone(),
        id: title.id.clone(),
        signature: title.signature.parse().unwrap_or_default(),
        sort_title,
        display_name: (!slim).then_some(display_name),
        cover_url: (!slim).then_some(cover_url),
        mtime: (!slim).then_some(title.mtime),
        parents,
    })
}

pub(super) fn title_parent_summaries(
    library: &crate::library::Library,
    title: &crate::library::Title,
) -> Vec<MangoTitleParent> {
    library
        .parent_titles(title)
        .into_iter()
        .map(|parent| MangoTitleParent {
            title: parent.title.clone(),
            id: parent.id.clone(),
        })
        .collect()
}

#[derive(Clone, Copy)]
pub(super) struct TitleResponseOptions {
    pub(super) depth: i32,
    pub(super) include_percentages: bool,
    pub(super) slim: bool,
    pub(super) sort_context: Option<(SortMethod, bool)>,
}

pub(super) async fn mango_title_response(
    state: &AppState,
    title: &crate::library::Title,
    info: &crate::library::progress::TitleInfo,
    cache: &crate::library::ProgressCache,
    username: &str,
    parents: Vec<MangoTitleParent>,
    options: TitleResponseOptions,
) -> Result<MangoTitleResponse> {
    let TitleResponseOptions {
        depth,
        include_percentages,
        slim,
        sort_context,
    } = options;
    let summary = mango_title_summary(state, title, info, parents.clone(), slim).await?;
    if depth == 0 {
        return Ok(MangoTitleResponse {
            title: summary,
            titles: None,
            entries: None,
            title_percentages: None,
            entry_percentages: None,
        });
    }

    let (sort_method, ascending) = sort_context.unwrap_or_else(|| {
        info.get_sort_by(username)
            .map(|(method, ascending)| (SortMethod::parse(&method), ascending))
            .unwrap_or((SortMethod::Auto, true))
    });
    let mut nested_titles = Vec::with_capacity(title.nested_titles.len());
    let mut nested_order = Vec::with_capacity(title.nested_titles.len());
    for nested in &title.nested_titles {
        let sort_title = state
            .storage
            .get_title_sort_title(&nested.id)
            .await?
            .unwrap_or_else(|| nested.title.clone());
        let percentage = title_progress_percentage(nested, cache, username);
        nested_order.push((nested, sort_title, percentage));
    }
    nested_order.sort_by(
        |(left, left_sort, left_progress), (right, right_sort, right_progress)| match sort_method {
            SortMethod::TimeModified => left
                .mtime
                .cmp(&right.mtime)
                .then_with(|| compare_numerically(left_sort, right_sort)),
            SortMethod::Progress => left_progress
                .total_cmp(right_progress)
                .then_with(|| compare_numerically(left_sort, right_sort)),
            SortMethod::Name | SortMethod::TimeAdded | SortMethod::Auto => {
                compare_numerically(left_sort, right_sort)
            }
        },
    );
    if !ascending {
        nested_order.reverse();
    }
    let mut title_percentages = Vec::with_capacity(title.nested_titles.len());
    let mut child_parents = parents;
    child_parents.push(MangoTitleParent {
        title: title.title.clone(),
        id: title.id.clone(),
    });
    for (nested, _, percentage) in nested_order {
        let nested_info = cache.get_title_info(&nested.id).unwrap_or_default();
        title_percentages.push(percentage);
        nested_titles.push(
            Box::pin(mango_title_response(
                state,
                nested,
                &nested_info,
                cache,
                username,
                child_parents.clone(),
                TitleResponseOptions {
                    depth: if depth > 0 { depth - 1 } else { depth },
                    sort_context: Some((sort_method, ascending)),
                    ..options
                },
            ))
            .await?,
        );
    }

    let mut entries_with_sort_title = Vec::with_capacity(title.entries.len());
    for entry in &title.entries {
        let sort_title = state
            .storage
            .get_entry_sort_title(&entry.id)
            .await?
            .unwrap_or_else(|| entry.title.clone());
        entries_with_sort_title.push((entry, sort_title));
    }
    let chapter_sorter = if matches!(sort_method, SortMethod::Auto) {
        let sort_titles = entries_with_sort_title
            .iter()
            .map(|(_, sort_title)| sort_title.as_str())
            .collect::<Vec<_>>();
        Some(ChapterSorter::new(&sort_titles))
    } else {
        None
    };
    entries_with_sort_title.sort_by(|(left, left_sort), (right, right_sort)| match sort_method {
        SortMethod::TimeModified => left
            .mtime
            .cmp(&right.mtime)
            .then_with(|| compare_numerically(left_sort, right_sort)),
        SortMethod::TimeAdded => info
            .get_date_added(&left.title)
            .unwrap_or_default()
            .cmp(&info.get_date_added(&right.title).unwrap_or_default())
            .then_with(|| compare_numerically(left_sort, right_sort)),
        SortMethod::Progress => {
            let left_progress = entry_progress_percentage(
                info.get_progress(username, &left.title).unwrap_or(0),
                left.pages,
            );
            let right_progress = entry_progress_percentage(
                info.get_progress(username, &right.title).unwrap_or(0),
                right.pages,
            );
            left_progress
                .total_cmp(&right_progress)
                .then_with(|| compare_numerically(left_sort, right_sort))
        }
        SortMethod::Name => compare_numerically(left_sort, right_sort),
        SortMethod::Auto => chapter_sorter
            .as_ref()
            .expect("auto sorting builds a chapter sorter")
            .compare(left_sort, right_sort)
            .then_with(|| compare_numerically(left_sort, right_sort)),
    });
    if !ascending {
        entries_with_sort_title.reverse();
    }
    let mut entries = Vec::with_capacity(title.entries.len());
    let mut entry_percentages = Vec::with_capacity(title.entries.len());
    for (entry, sort_title) in entries_with_sort_title {
        let progress = info.get_progress(username, &entry.title).unwrap_or(0);
        entry_percentages.push(entry_progress_percentage(progress, entry.pages));
        entries.push(
            mango_entry_response(state, title, entry, info, Some(sort_title.as_str()), slim)
                .await?,
        );
    }

    Ok(MangoTitleResponse {
        title: summary,
        titles: Some(nested_titles),
        entries: Some(entries),
        title_percentages: include_percentages.then_some(title_percentages),
        entry_percentages: include_percentages.then_some(entry_percentages),
    })
}

pub(super) fn title_progress_percentage(
    title: &crate::library::Title,
    cache: &crate::library::ProgressCache,
    username: &str,
) -> f64 {
    let mut total_pages = 0usize;
    let mut read_pages = 0f64;
    for nested in std::iter::once(title).chain(title.deep_titles()) {
        let info = cache.get_title_info(&nested.id).unwrap_or_default();
        for entry in &nested.entries {
            total_pages += entry.pages;
            read_pages += info
                .get_progress(username, &entry.title)
                .unwrap_or(0)
                .clamp(0, entry.pages as i32) as f64;
        }
    }
    if total_pages == 0 {
        0.0
    } else {
        read_pages / total_pages as f64
    }
}

pub(super) fn entry_progress_percentage(progress: i32, pages: usize) -> f64 {
    if pages == 0 {
        return 0.0;
    }
    progress.clamp(0, pages as i32) as f64 / pages as f64
}

#[cfg(test)]
mod parity_contract_tests {
    use super::entry_progress_percentage;

    #[test]
    fn progress_percentage_preserves_float64_precision() {
        assert_eq!(entry_progress_percentage(1, 3), 1.0_f64 / 3.0);
    }
}
