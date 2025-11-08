use mango_rust::{Config, server};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() {
    // Load configuration
    let config = Config::load(None).unwrap_or_else(|e| {
        eprintln!("Failed to load config: {}", e);
        std::process::exit(1);
    });

    // Initialize tracing with configured log level
    let log_level = match config.log_level.as_str() {
        "trace" => "mango_rust=trace,tower_http=debug,tower_sessions=debug",
        "debug" => "mango_rust=debug,tower_http=debug,tower_sessions=info",
        "info" => "mango_rust=info,tower_http=info,tower_sessions=warn",
        "warn" => "mango_rust=warn,tower_http=warn,tower_sessions=warn",
        "error" => "mango_rust=error,tower_http=error,tower_sessions=error",
        _ => "mango_rust=info,tower_http=info,tower_sessions=warn",
    };

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| log_level.into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Run server
    if let Err(e) = server::run(config).await {
        tracing::error!("Server error: {}", e);
        std::process::exit(1);
    }
}
