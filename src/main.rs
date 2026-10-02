mod cli;
use mango_rust::{server, Config};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

async fn run_server(config_path: Option<&str>) -> Result<(), String> {
    let config = Config::load(config_path).map_err(|error| error.to_string())?;
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

    server::run(config).await.map_err(|error| error.to_string())
}

#[tokio::main]
async fn main() {
    let result = match cli::run().await {
        Ok(cli::Outcome::RunServer { config_path }) => run_server(config_path.as_deref()).await,
        Ok(cli::Outcome::Complete) => Ok(()),
        Err(error) => Err(error),
    };

    if let Err(error) = result {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
