// Mango-Rust Library Root
// Tier 1 MVP modules

pub mod config;
pub mod storage;
pub mod auth;
pub mod server;
pub mod routes;
pub mod library;

// Re-exports
pub use config::Config;
pub use storage::Storage;
pub use library::Library;
pub use server::AppState;

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
        Archive(#[from] zip::result::ZipError),

        #[error("Config error: {0}")]
        Config(String),

        #[error("Authentication failed")]
        AuthFailed,

        #[error("Not found")]
        NotFound,

        #[error("Internal server error: {0}")]
        Internal(String),
    }

    impl IntoResponse for Error {
        fn into_response(self) -> Response {
            let status = match &self {
                Error::AuthFailed => StatusCode::UNAUTHORIZED,
                Error::NotFound => StatusCode::NOT_FOUND,
                Error::Database(_) | Error::Io(_) | Error::Internal(_) | Error::Archive(_) => {
                    StatusCode::INTERNAL_SERVER_ERROR
                }
                Error::Config(_) => StatusCode::INTERNAL_SERVER_ERROR,
            };

            (status, self.to_string()).into_response()
        }
    }
}
