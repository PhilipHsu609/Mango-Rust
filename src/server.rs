use axum::{
    middleware,
    routing::get,
    Router,
};
use tower_http::trace::TraceLayer;
use tower_sessions::{Expiry, SessionManagerLayer};
use tower_sessions_sqlx_store::SqliteStore;

use crate::{
    auth::require_auth,
    config::Config,
    error::Result,
    routes::{get_login, home, logout, post_login},
    Storage,
};

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
        // Protected routes (auth required)
        .route("/", get(home))
        .route("/logout", get(logout))
        // Add state and middleware
        .layer(middleware::from_fn_with_state(storage.clone(), require_auth))
        .layer(session_layer)
        .layer(TraceLayer::new_for_http())
        .with_state(storage);

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
