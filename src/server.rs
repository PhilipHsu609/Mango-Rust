use arc_swap::ArcSwap;
use askama::Template;
use axum::{
    body::{to_bytes, Body},
    extract::State,
    http::{header, HeaderValue, Method, Request, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{delete, get, patch, post, put},
    Router,
};
use std::sync::Arc;
use tower_http::{services::ServeDir, trace::TraceLayer};
use tower_sessions::{Expiry, Session, SessionManagerLayer};
use tower_sessions_sqlx_store::SqliteStore;
use utoipa::OpenApi;

use crate::{
    auth::{require_auth, SESSION_TOKEN_KEY},
    config::Config,
    error::Result,
    library::{spawn_periodic_scanner, Library},
    routes::{
        add_tag, admin_dashboard, api_login, api_reference, bulk_progress, cache_clear_api,
        cache_debug_page, cache_invalidate_api, cache_load_library_api, cache_save_library_api,
        change_password_api, change_password_page, continue_reading, create_user,
        delete_all_missing_entries, delete_all_missing_titles, delete_missing_entry,
        delete_missing_title, delete_tag, delete_user, delete_user_api, download_entry,
        generate_thumbnails, get_book, get_cover, get_dimensions, get_library, get_login,
        get_missing_entries, get_missing_titles, get_page, get_sort_opt, get_title, get_title_tags,
        get_users, home, library as library_page, list_tags, list_tags_page, logout,
        missing_items_page, opds_index, opds_title, openapi_spec, post_login, reader,
        reader_continue, recently_added, scan_library, start_reading, thumbnail_progress,
        update_display_name, update_progress, update_sort_opt, update_sort_title, update_user,
        upload_cover, user_edit_page, user_edit_post, user_edit_post_existing, users_page,
        view_tag_page, ApiDoc,
    },
    util::NavigationState,
    Storage,
};

/// Shared application state
#[derive(Clone)]
pub struct AppState {
    pub storage: Storage,
    pub library: Arc<ArcSwap<Library>>,
    pub config: Arc<Config>,
    pub session_store: SqliteStore,
}

// Mango's `cors` OPTIONS route applies to these roots and their descendants.
fn preflight_path(path: &str) -> bool {
    ["/api", "/uploads", "/img"].iter().any(|prefix| {
        path.strip_prefix(prefix)
            .is_some_and(|suffix| suffix.is_empty() || suffix.starts_with('/'))
    })
}

async fn mango_cors(request: Request<Body>, next: Next) -> Response {
    let path = request.uri().path();
    let preflight = request.method() == Method::OPTIONS && preflight_path(path);
    let api_path = path.starts_with("/api");
    let mut response = if preflight {
        let mut response = StatusCode::OK.into_response();
        response
            .headers_mut()
            .insert(header::CONTENT_TYPE, HeaderValue::from_static("text/html"));
        response
            .headers_mut()
            .insert(header::CONTENT_LENGTH, HeaderValue::from_static("0"));
        response
    } else {
        next.run(request).await
    };

    if preflight || (api_path && response.status() != StatusCode::NOT_FOUND) {
        let headers = response.headers_mut();
        headers.insert("access-control-allow-origin", HeaderValue::from_static("*"));
        headers.insert(
            "access-control-allow-methods",
            HeaderValue::from_static("HEAD,GET,PUT,POST,DELETE,OPTIONS"),
        );
        headers.insert(
            "access-control-allow-headers",
            HeaderValue::from_static(
                "X-Requested-With,X-HTTP-Method-Override, Content-Type, Cache-Control, Accept,Authorization",
            ),
        );
    }
    response
}

// tower-sessions 0.11 only persists modified sessions. Renew an existing session
// on reads too, so its stored expiry and browser cookie roll together.
async fn refresh_active_session(session: Session, request: Request<Body>, next: Next) -> Response {
    if session.id().is_some() && !session.is_empty().await {
        session.set_expiry(Some(Expiry::OnInactivity(time::Duration::days(365))));
    }
    next.run(request).await
}

#[derive(Template)]
#[template(path = "error.html")]
struct ErrorPageTemplate {
    nav: NavigationState,
    message: String,
}

fn is_non_browser_error_path(path: &str) -> bool {
    [
        "/api", "/uploads", "/img", "/static", "/css", "/js", "/opds",
    ]
    .iter()
    .any(|prefix| {
        path.strip_prefix(prefix)
            .is_some_and(|suffix| suffix.is_empty() || suffix.starts_with('/'))
    })
}

async fn browser_error_pages(
    State(state): State<AppState>,
    session: Session,
    request: Request<Body>,
    next: Next,
) -> Response {
    let path = request.uri().path().to_owned();
    let response = next.run(request).await;
    if is_non_browser_error_path(&path)
        || !(response.status().is_client_error() || response.status().is_server_error())
    {
        return response;
    }

    let (mut parts, body) = response.into_parts();
    let body = to_bytes(body, 16 * 1024).await.unwrap_or_default();
    let message = std::str::from_utf8(&body)
        .ok()
        .map(str::trim)
        .filter(|message| !message.is_empty())
        .map(str::to_owned)
        .unwrap_or_else(|| {
            format!(
                "HTTP {}: {}",
                parts.status.as_u16(),
                parts.status.canonical_reason().unwrap_or("Error")
            )
        });
    let is_admin = match session.get::<String>(SESSION_TOKEN_KEY).await {
        Ok(Some(token)) => state.storage.verify_admin(&token).await.unwrap_or(false),
        _ => false,
    };
    let nav = NavigationState {
        home_active: false,
        library_active: false,
        tags_active: false,
        admin_active: false,
        is_admin,
    };
    let html = ErrorPageTemplate { nav, message }
        .render()
        .unwrap_or_else(|_| "<!DOCTYPE html><html><body><p>Error</p></body></html>".to_owned());

    parts.headers.remove(header::CONTENT_LENGTH);
    parts.headers.remove(header::CONTENT_ENCODING);
    parts.headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("text/html; charset=utf-8"),
    );
    Response::from_parts(parts, Body::from(html))
}

/// Build and run the Axum server
pub async fn run(config: Config) -> Result<()> {
    // Initialize tracing
    tracing::info!("Starting Mango-Rust server");
    tracing::info!("Host: {}:{}", config.host, config.port);
    tracing::info!("Base URL: {}", config.base_url);
    tracing::info!("Library path: {}", config.library_path.display());

    // Initialize storage (connects to database, runs migrations)
    let database_url = config.database_url();
    tracing::info!("Connecting to database: {}", database_url);
    let storage = Storage::new(&database_url).await?;
    tracing::info!("Database initialized at {}", config.db_path.display());

    // Wrap config in Arc early (needed for periodic scanner)
    let config = Arc::new(config);

    // Initialize library scanner
    tracing::info!("Initializing library");
    let mut library = Library::new(config.library_path.clone(), storage.clone(), &config);

    // Try to load from cache first (fast)
    let cache_loaded = library.try_load_from_cache().await?;

    // Use ArcSwap for lock-free reads
    let library = Arc::new(ArcSwap::from_pointee(library));

    // If cache didn't load, spawn background scan task (non-blocking, double-buffer)
    if !cache_loaded {
        tracing::info!("Cache not available, starting background library scan...");
        let library_clone = library.clone();
        let storage_clone = storage.clone();
        let config_clone = config.clone();
        tokio::spawn(async move {
            let start = std::time::Instant::now();
            // Build new library instance in background
            let mut new_lib = Library::new(
                config_clone.library_path.clone(),
                storage_clone,
                &config_clone,
            );
            match new_lib.scan().await {
                Ok(_) => {
                    let stats = new_lib.stats();
                    // Atomically swap the new library in
                    library_clone.store(Arc::new(new_lib));
                    tracing::info!(
                        "Background library scan completed in {:.2}s - {} titles, {} entries",
                        start.elapsed().as_secs_f64(),
                        stats.titles,
                        stats.entries
                    );
                }
                Err(e) => {
                    tracing::error!("Background library scan failed: {}", e);
                }
            }
        });
    }

    // Start periodic scanner if configured (similar to original Mango)
    if config.scan_interval_minutes > 0 {
        tracing::info!(
            "Starting periodic library scanner (interval: {} minutes)",
            config.scan_interval_minutes
        );
        spawn_periodic_scanner(
            library.clone(),
            storage.clone(),
            config.clone(),
            config.scan_interval_minutes as u64,
        );
    } else {
        tracing::info!("Periodic library scanning disabled (scan_interval_minutes = 0)");
    }

    tracing::info!("Library initialization complete (server ready)");

    // Create session store (uses same database)
    let session_store = SqliteStore::new(storage.pool().clone());
    session_store
        .migrate()
        .await
        .map_err(|e| crate::error::Error::Internal(format!("Session migration failed: {}", e)))?;

    // Create application state
    let app_state = AppState {
        storage: storage.clone(),
        library,
        config: config.clone(),
        session_store: session_store.clone(),
    };

    let session_layer = SessionManagerLayer::new(session_store)
        .with_name(format!("mango-sessid-{}", config.port))
        .with_path(config.base_url.clone())
        .with_secure(false) // Set to true in production with HTTPS
        .with_expiry(Expiry::OnInactivity(time::Duration::days(365)));

    // Build router
    let api_document = axum::body::Bytes::from(ApiDoc::openapi().to_json()?);
    let app = Router::new()
        // Public routes (no auth required)
        .route("/login", get(get_login).post(post_login))
        .route("/api", get(api_reference))
        .route("/openapi.json", get(openapi_spec))
        .route("/api/login", post(api_login))
        // Static files (no auth required)
        .nest_service("/static", ServeDir::new("static"))
        .nest_service("/uploads", ServeDir::new(config.upload_path.clone()))
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
        .route("/admin/missing", get(missing_items_page))
        .route("/admin/user", get(users_page))
        .route("/admin/user/edit", get(user_edit_page).post(user_edit_post))
        .route("/admin/user/edit/:username", post(user_edit_post_existing))
        // Cache debug route
        .route("/debug/cache", get(cache_debug_page))
        // Admin API routes
        .route("/api/admin/scan", post(scan_library))
        // Cache API routes
        .route("/api/cache/clear", post(cache_clear_api))
        .route("/api/cache/save-library", post(cache_save_library_api))
        .route("/api/cache/load-library", post(cache_load_library_api))
        .route("/api/cache/invalidate", post(cache_invalidate_api))
        .route(
            "/api/admin/titles/missing",
            get(get_missing_titles).delete(delete_all_missing_titles),
        )
        .route(
            "/api/admin/titles/missing/:id",
            delete(delete_missing_title),
        )
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
        .route("/api/admin/user/delete/:username", delete(delete_user_api))
        // Reader routes
        .route("/reader/:tid/:eid", get(reader_continue))
        .route("/reader/:tid/:eid/:page", get(reader))
        // API routes
        .route("/api/library", get(get_library))
        .route("/api/book/:tid", get(get_title))
        .route("/api/sort_opt", get(get_sort_opt).put(update_sort_opt))
        .route("/api/page/:tid/:eid/:page", get(get_page))
        .route("/api/cover/:tid/:eid", get(get_cover))
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
        .route("/api/progress/:tid/:page", put(update_progress))
        // Dimensions API (for reader)
        .route("/api/dimensions/:tid/:eid", get(get_dimensions))
        // User API
        .route("/api/user/change-password", post(change_password_api))
        // Admin metadata API
        .route("/api/admin/sort_title/:tid", put(update_sort_title))
        .route(
            "/api/admin/display_name/:tid/:name",
            put(update_display_name),
        )
        .route(
            "/api/admin/upload/cover",
            post(upload_cover).layer(axum::extract::DefaultBodyLimit::disable()),
        )
        // Bulk progress API
        .route("/api/bulk_progress/:action/:tid", put(bulk_progress))
        // Thumbnail generation API
        .route("/api/admin/thumbnail_progress", get(thumbnail_progress))
        .route("/api/admin/generate_thumbnails", post(generate_thumbnails))
        // Add state and middleware
        .layer(axum::Extension(api_document))
        .layer(middleware::from_fn_with_state(
            app_state.clone(),
            require_auth,
        ))
        .layer(middleware::from_fn_with_state(
            app_state.clone(),
            browser_error_pages,
        ))
        .layer(middleware::from_fn(refresh_active_session))
        .layer(session_layer)
        .layer(TraceLayer::new_for_http())
        .layer(middleware::from_fn(mango_cors))
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
