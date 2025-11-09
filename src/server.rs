use axum::{
    middleware,
    routing::get,
    Router,
};
use std::sync::Arc;
use tokio::sync::RwLock;
use tower_http::{services::ServeDir, trace::TraceLayer};
use tower_sessions::{Expiry, SessionManagerLayer};
use tower_sessions_sqlx_store::SqliteStore;

use crate::{
    auth::require_auth,
    config::Config,
    error::Result,
    library::Library,
    routes::{
        get_cover, get_library, get_login, get_page, get_stats, get_title, home, library as library_page, logout, post_login,
        get_book, reader, get_progress, save_progress, get_all_progress,
    },
    Storage,
};

/// Shared application state
#[derive(Clone)]
pub struct AppState {
    pub storage: Storage,
    pub library: Arc<RwLock<Library>>,
}

/// Build and run the Axum server
pub async fn run(config: Config) -> Result<()> {
    // Initialize tracing
    tracing::info!("Starting Mango-Rust server");
    tracing::info!("Host: {}:{}", config.host, config.port);
    tracing::info!("Base URL: {}", config.base_url);
    tracing::info!("Library path: {}", config.library_path.display());

    // Initialize storage (connects to database, runs migrations)
    let database_url = format!("sqlite://{}?mode=rwc", config.db_path.to_string_lossy());
    tracing::info!("Connecting to database: {}", database_url);
    let storage = Storage::new(&database_url).await?;
    tracing::info!("Database initialized at {}", config.db_path.display());

    // Initialize library scanner
    tracing::info!("Initializing library scanner");
    let mut library = Library::new(config.library_path.clone(), storage.clone());
    library.scan().await?;
    let library = Arc::new(RwLock::new(library));
    tracing::info!("Library scan complete");

    // Create application state
    let app_state = AppState {
        storage: storage.clone(),
        library,
    };

    // Create session store (uses same database)
    let session_store = SqliteStore::new(storage.pool().clone());
    session_store
        .migrate()
        .await
        .map_err(|e| crate::error::Error::Internal(format!("Session migration failed: {}", e)))?;

    let session_layer = SessionManagerLayer::new(session_store)
        .with_secure(false) // Set to true in production with HTTPS
        .with_expiry(Expiry::OnInactivity(time::Duration::days(7)));

    // Build router
    let app = Router::new()
        // Public routes (no auth required)
        .route("/login", get(get_login).post(post_login))
        // Static files (no auth required)
        .nest_service("/static", ServeDir::new("static"))
        // Protected routes (auth required)
        .route("/", get(home))
        .route("/library", get(library_page))
        .route("/book/:id", get(get_book))
        .route("/logout", get(logout))
        // Reader routes
        .route("/reader/:tid/:eid/:page", get(reader))
        // API routes
        .route("/api/library", get(get_library))
        .route("/api/title/:id", get(get_title))
        .route("/api/page/:tid/:eid/:page", get(get_page))
        .route("/api/cover/:tid/:eid", get(get_cover))
        .route("/api/stats", get(get_stats))
        // Progress API
        .route("/api/progress/:tid/:eid", get(get_progress).post(save_progress))
        .route("/api/progress", get(get_all_progress))
        // Add state and middleware
        .layer(middleware::from_fn_with_state(app_state.clone(), require_auth))
        .layer(session_layer)
        .layer(TraceLayer::new_for_http())
        .with_state(app_state);

    // Bind and serve
    let addr = format!("{}:{}", config.host, config.port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    tracing::info!("Server listening on {}", addr);
    tracing::info!("Visit http://{}{} to access Mango", addr, config.base_url);

    axum::serve(listener, app)
        .await
        .map_err(|e| crate::error::Error::Internal(format!("Server error: {}", e)))?;

    Ok(())
}
