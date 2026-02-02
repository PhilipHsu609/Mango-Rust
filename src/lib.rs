// Mango-Rust Library Root
// Tier 1 MVP modules

pub mod auth;
pub mod config;
pub mod library;
pub mod routes;
pub mod server;
pub mod storage;
pub mod util;

// Re-exports
pub use config::Config;
pub use library::Library;
pub use server::AppState;
pub use storage::Storage;

// Common types and utilities
pub mod error {
    use axum::{
        http::StatusCode,
        response::{IntoResponse, Response},
    };

    pub type Result<T> = std::result::Result<T, Error>;

    #[derive(Debug, thiserror::Error)]
    pub enum Error {
        #[error("Database error: {0}")]
        Database(#[from] sqlx::Error),

        #[error("IO error: {0}")]
        Io(#[from] std::io::Error),

        #[error("Archive error: {0}")]
        Archive(#[from] compress_tools::Error),

        #[error("JSON error: {0}")]
        Json(#[from] serde_json::Error),

        #[error("Cache corrupted: {0}")]
        CacheCorrupted(String),

        #[error("Cache serialization failed: {0}")]
        CacheSerialization(String),

        #[error("Config error: {0}")]
        Config(String),

        #[error("Authentication failed")]
        AuthFailed,

        #[error("Not found: {0}")]
        NotFound(String),

        #[error("{0}")]
        BadRequest(String),

        #[error("Conflict: {0}")]
        Conflict(String),

        #[error("Forbidden: {0}")]
        Forbidden(String),

        #[error("Internal server error: {0}")]
        Internal(String),
    }

    impl IntoResponse for Error {
        fn into_response(self) -> Response {
            let status = match &self {
                Error::AuthFailed => StatusCode::UNAUTHORIZED,
                Error::NotFound(_) => StatusCode::NOT_FOUND,
                Error::BadRequest(_) => StatusCode::BAD_REQUEST,
                Error::Conflict(_) => StatusCode::CONFLICT,
                Error::Forbidden(_) => StatusCode::FORBIDDEN,
                Error::Database(_)
                | Error::Io(_)
                | Error::Internal(_)
                | Error::Archive(_)
                | Error::Json(_)
                | Error::CacheCorrupted(_)
                | Error::CacheSerialization(_) => StatusCode::INTERNAL_SERVER_ERROR,
                Error::Config(_) => StatusCode::INTERNAL_SERVER_ERROR,
            };

            (status, self.to_string()).into_response()
        }
    }
}
