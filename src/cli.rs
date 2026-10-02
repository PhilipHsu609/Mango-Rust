use clap::{Args, CommandFactory, Parser, Subcommand};
use mango_rust::{Config, Storage};

#[derive(Parser)]
#[command(name = "mango-rust", version)]
struct Cli {
    /// Path to the configuration file
    #[arg(short, long, global = true, value_name = "PATH")]
    config: Option<String>,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Run administrative tools
    Admin(AdminArgs),
}

#[derive(Args)]
struct AdminArgs {
    #[command(subcommand)]
    command: Option<AdminCommand>,
}

#[derive(Subcommand)]
enum AdminCommand {
    /// Manage users
    User(UserArgs),
}

#[derive(Args)]
struct UserArgs {
    #[command(subcommand)]
    command: Option<UserCommand>,
}

#[derive(Subcommand)]
enum UserCommand {
    /// Add a user
    Add {
        #[arg(short, long, value_name = "USERNAME")]
        username: String,
        #[arg(short, long, value_name = "PASSWORD")]
        password: String,
        /// Grant administrator access
        #[arg(short, long)]
        admin: bool,
    },
    /// Delete a user
    Delete { username: String },
    /// Update a user's username, password, or admin status
    Update {
        /// Existing username
        username: String,
        /// New username (defaults to the existing username)
        #[arg(short = 'u', long = "username", value_name = "USERNAME")]
        new_username: Option<String>,
        /// Leave the password unchanged when omitted
        #[arg(short, long, value_name = "PASSWORD")]
        password: Option<String>,
        /// Grant administrator access
        #[arg(short, long)]
        admin: bool,
    },
    /// List users and their administrator status
    List,
}

pub(super) enum Outcome {
    RunServer { config_path: Option<String> },
    Complete,
}

pub(super) async fn run() -> Result<Outcome, String> {
    let cli = Cli::parse();
    let config_path = cli.config;

    match cli.command {
        None => Ok(Outcome::RunServer { config_path }),
        Some(Commands::Admin(admin)) => match admin.command {
            None => {
                print_help(&["admin"]);
                Ok(Outcome::Complete)
            }
            Some(AdminCommand::User(user)) => match user.command {
                None => {
                    print_help(&["admin", "user"]);
                    Ok(Outcome::Complete)
                }
                Some(command) => {
                    run_user_command(command, config_path.as_deref()).await?;
                    Ok(Outcome::Complete)
                }
            },
        },
    }
}

fn print_help(path: &[&str]) {
    let mut command = Cli::command();
    let mut current = &mut command;
    for name in path {
        let Some(subcommand) = current.find_subcommand_mut(name) else {
            return;
        };
        current = subcommand;
    }
    let _ = current.print_help();
    println!();
}

async fn run_user_command(command: UserCommand, config_path: Option<&str>) -> Result<(), String> {
    let config = Config::load(config_path).map_err(|error| error.to_string())?;
    let storage = Storage::new(&config.database_url())
        .await
        .map_err(|error| error.to_string())?;

    match command {
        UserCommand::Add {
            username,
            password,
            admin,
        } => {
            storage
                .create_user(&username, &password, admin)
                .await
                .map_err(|error| error.to_string())?;
            println!("Created user '{username}'");
        }
        UserCommand::Delete { username } => {
            storage
                .delete_user(&username)
                .await
                .map_err(|error| error.to_string())?;
            println!("Deleted user '{username}'");
        }
        UserCommand::Update {
            username,
            new_username,
            password,
            admin,
        } => {
            let new_username = new_username.as_deref().unwrap_or(&username);
            storage
                .update_user(&username, new_username, password.as_deref(), admin)
                .await
                .map_err(|error| error.to_string())?;
            println!("Updated user '{username}'");
        }
        UserCommand::List => {
            let users = storage
                .list_users()
                .await
                .map_err(|error| error.to_string())?;
            let width = users
                .iter()
                .map(|(username, _)| username.len())
                .max()
                .unwrap_or(0)
                .max("username".len());
            println!("{:<width$}  admin access", "username");
            for (username, is_admin) in users {
                println!("{username:<width$}  {is_admin}");
            }
        }
    }

    Ok(())
}
