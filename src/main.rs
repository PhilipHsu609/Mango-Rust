use mango_rust::{server, Config, Storage};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

const USER_UPDATE_USAGE: &str =
    "Usage: mango-rust [-c|--config <PATH>] admin user update <username> --password <password>";

fn extract_config_path(args: Vec<String>) -> Result<(Option<String>, Vec<String>), String> {
    let mut config_path = None;
    let mut remaining = Vec::with_capacity(args.len());
    let mut args = args.into_iter();

    while let Some(arg) = args.next() {
        if arg == "-c" || arg == "--config" {
            config_path = Some(
                args.next()
                    .filter(|path| !path.is_empty())
                    .ok_or_else(|| "--config requires a path".to_string())?,
            );
        } else if let Some(path) = arg.strip_prefix("--config=") {
            if path.is_empty() {
                return Err("--config requires a path".to_string());
            }
            config_path = Some(path.to_string());
        } else {
            remaining.push(arg);
        }
    }

    Ok((config_path, remaining))
}

async fn run_admin_command(args: &[String], config_path: Option<&str>) -> Result<(), String> {
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        println!("{USER_UPDATE_USAGE}");
        return Ok(());
    }

    if args.len() < 3 || args[0] != "user" || args[1] != "update" {
        return Err(USER_UPDATE_USAGE.to_string());
    }

    let username = &args[2];
    let mut password = None;
    let mut index = 3;
    while index < args.len() {
        let arg = &args[index];
        if arg == "-p" || arg == "--password" {
            index += 1;
            password = Some(
                args.get(index)
                    .ok_or_else(|| USER_UPDATE_USAGE.to_string())?
                    .clone(),
            );
        } else if let Some(value) = arg.strip_prefix("--password=") {
            password = Some(value.to_string());
        } else {
            return Err(format!("Unknown option: {arg}\n{USER_UPDATE_USAGE}"));
        }
        index += 1;
    }

    let password = password.ok_or_else(|| USER_UPDATE_USAGE.to_string())?;
    let config = Config::load(config_path).map_err(|error| error.to_string())?;
    let database_url = config.database_url();
    let storage = Storage::new(&database_url)
        .await
        .map_err(|error| error.to_string())?;

    if !storage
        .username_exists(username)
        .await
        .map_err(|error| error.to_string())?
    {
        return Err(format!("User not found: {username}"));
    }

    let is_admin = storage
        .username_is_admin(username)
        .await
        .map_err(|error| error.to_string())?;
    storage
        .update_user(username, username, Some(&password), is_admin)
        .await
        .map_err(|error| error.to_string())?;

    println!("Updated password for user '{username}'");
    Ok(())
}

#[tokio::main]
async fn main() {
    let (config_path, args) = extract_config_path(std::env::args().skip(1).collect())
        .unwrap_or_else(|error| {
            eprintln!("{error}");
            std::process::exit(2);
        });
    if args.first().is_some_and(|arg| arg == "admin") {
        if let Err(error) = run_admin_command(&args[1..], config_path.as_deref()).await {
            eprintln!("{error}");
            std::process::exit(2);
        }
        return;
    }

    // Load configuration
    let config = Config::load(config_path.as_deref()).unwrap_or_else(|e| {
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
