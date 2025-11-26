use axum::{
    middleware,
    routing::{delete, get, patch, post, put},
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
        add_tag, admin_dashboard, change_password_api, change_password_page, continue_reading,
        create_user, delete_all_missing_entries, delete_missing_entry, delete_tag, delete_user,
        download_entry, get_all_progress, get_book, get_cover, get_library, get_login,
        get_missing_entries, get_page, get_progress, get_stats, get_title, get_title_tags,
        get_users, home, library as library_page, list_tags, list_tags_page, logout,
        missing_items_page, opds_index, opds_title, post_login, reader, recently_added,
        save_progress, scan_library, start_reading, update_user, users_page, view_tag_page,
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
        .route("/change-password", get(change_password_page))
        .route("/logout", get(logout))
        // Tags routes
        .route("/tags", get(list_tags_page))
        .route("/tags/:tag", get(view_tag_page))
        // Admin routes (requires admin access)
        .route("/admin", get(admin_dashboard))
        .route("/admin/missing-items", get(missing_items_page))
        .route("/admin/users", get(users_page))
        // Admin API routes
        .route("/api/admin/scan", post(scan_library))
        .route(
            "/api/admin/entries/missing",
            get(get_missing_entries).delete(delete_all_missing_entries),
        )
        .route(
            "/api/admin/entries/missing/:id",
            delete(delete_missing_entry),
        )
        .route("/api/admin/users", get(get_users).post(create_user))
        .route(
            "/api/admin/users/:username",
            patch(update_user).delete(delete_user),
        )
        // Reader routes
        .route("/reader/:tid/:eid/:page", get(reader))
        // API routes
        .route("/api/library", get(get_library))
        .route("/api/title/:id", get(get_title))
        .route("/api/page/:tid/:eid/:page", get(get_page))
        .route("/api/cover/:tid/:eid", get(get_cover))
        .route("/api/stats", get(get_stats))
        .route("/api/download/:tid/:eid", get(download_entry))
        // OPDS catalog routes
        .route("/opds", get(opds_index))
        .route("/opds/book/:title_id", get(opds_title))
        // Tags API routes
        .route("/api/tags", get(list_tags))
        .route("/api/tags/:tid", get(get_title_tags))
        .route("/api/admin/tags/:tid/:tag", put(add_tag).delete(delete_tag))
        // Home page API routes
        .route("/api/library/continue_reading", get(continue_reading))
        .route("/api/library/start_reading", get(start_reading))
        .route("/api/library/recently_added", get(recently_added))
        // Progress API
        .route(
            "/api/progress/:tid/:eid",
            get(get_progress).post(save_progress),
        )
        .route("/api/progress", get(get_all_progress).put(save_progress))
        // User API
        .route("/api/user/change-password", post(change_password_api))
        // Add state and middleware
        .layer(middleware::from_fn_with_state(
            app_state.clone(),
            require_auth,
        ))
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
