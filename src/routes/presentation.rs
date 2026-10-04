use crate::error::Error;
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
