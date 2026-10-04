/// Utility functions shared across the codebase
use crate::error::{Error, Result};
use serde::Deserialize;
use std::path::Path;

/// Query parameters for sorting
#[derive(Deserialize)]
pub struct SortParams {
    /// Optional sort method (title, modified, auto, progress)
    pub sort: Option<String>,
    /// Optional ascend flag (1 for ascending, 0 for descending)
    pub ascend: Option<String>,
}

/// Navigation state for templates
/// Tracks which page is currently active in the navigation menu
/// and user permission level for conditional UI rendering
#[derive(Debug, Clone, serde::Serialize)]
pub struct NavigationState {
    pub home_active: bool,
    pub library_active: bool,
    pub tags_active: bool,
    pub admin_active: bool,
    pub is_admin: bool,
}

impl NavigationState {
    /// Create navigation state with home page active
    pub fn home() -> Self {
        Self {
            home_active: true,
            library_active: false,
            tags_active: false,
            admin_active: false,
            is_admin: false,
        }
    }

    /// Create navigation state with library page active
    pub fn library() -> Self {
        Self {
            home_active: false,
            library_active: true,
            tags_active: false,
            admin_active: false,
            is_admin: false,
        }
    }

    /// Create navigation state with tags page active
    pub fn tags() -> Self {
        Self {
            home_active: false,
            library_active: false,
            tags_active: true,
            admin_active: false,
            is_admin: false,
        }
    }

    /// Create navigation state with admin page active
    pub fn admin() -> Self {
        Self {
            home_active: false,
            library_active: false,
            tags_active: false,
            admin_active: true,
            is_admin: false,
        }
    }

    /// Builder method to set admin permission status
    /// Use this to indicate whether the current user has admin privileges
    pub fn with_admin(mut self, is_admin: bool) -> Self {
        self.is_admin = is_admin;
        self
    }
}

/// Helper function to convert template render errors to Error::Internal
/// Use this instead of duplicating error handling across route handlers
pub fn render_error<E: std::fmt::Display>(e: E) -> Error {
    Error::Internal(format!("Template render error: {}", e))
}

/// Get sort preferences for a user from info.json
/// If query params are provided, saves them and returns them
/// Otherwise, returns saved preferences or defaults
///
/// Returns (sort_method, ascending) tuple
pub async fn get_and_save_sort(
    dir: &Path,
    username: &str,
    params: &SortParams,
) -> Result<(String, bool)> {
    use crate::library::progress::TitleInfo;

    let mut info = TitleInfo::load(dir).await?;

    // If query params exist, use them and save to info.json
    if let Some(method) = &params.sort {
        let ascending = params
            .ascend
            .as_ref()
            .and_then(|s| s.parse::<i32>().ok())
            .map(|v| v != 0)
            .unwrap_or(true);

        info.set_sort_by(username, method, ascending);
        info.save(dir).await?;

        return Ok((method.clone(), ascending));
    }

    // Otherwise, load saved preferences or use defaults
    if let Some((method, ascending)) = info.get_sort_by(username) {
        Ok((method, ascending))
    } else {
        Ok(("auto".to_string(), true))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn navigation_states_select_one_route_and_allow_admin_override() {
        let cases = [
            (NavigationState::home(), [true, false, false, false]),
            (NavigationState::library(), [false, true, false, false]),
            (NavigationState::tags(), [false, false, true, false]),
            (NavigationState::admin(), [false, false, false, true]),
        ];

        for (state, expected_active) in cases {
            assert_eq!(
                [
                    state.home_active,
                    state.library_active,
                    state.tags_active,
                    state.admin_active,
                ],
                expected_active
            );
            assert!(!state.is_admin);
        }

        assert!(NavigationState::home().with_admin(true).is_admin);
        assert!(!NavigationState::home().with_admin(false).is_admin);
    }
    #[tokio::test]
    async fn absent_sort_prefers_auto_ascending() {
        let dir = tempfile::tempdir().unwrap();
        let params = SortParams {
            sort: None,
            ascend: None,
        };

        assert_eq!(
            get_and_save_sort(dir.path(), "admin", &params)
                .await
                .unwrap(),
            ("auto".to_string(), true)
        );
        assert_eq!(
            crate::library::SortMethod::from_params(None, None),
            (crate::library::SortMethod::Auto, true)
        );
    }
}
