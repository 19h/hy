//! hy — Hex-Rays Command-line Interface.
//!
//! A modern CLI for managing IDA Pro installations, licenses, downloads,
//! plugins, file sharing, and more.

mod api;
mod auth;
mod cmd;
mod config;
mod error;
mod ida;
mod plugin;
mod update;
mod util;

use clap::Parser;
use std::time::Duration;

use crate::config::Env;
use crate::error::Error;
use crate::update::BackgroundUpdateChecker;
use crate::util::io::is_binary;

/// hy — Hex-Rays Command-line Interface
#[derive(Debug, Parser)]
#[command(
    name = "hy",
    version,
    about = "Hex-Rays CLI for managing IDA installation, licenses, and more",
    long_about = None,
    propagate_version = true,
)]
struct Cli {
    /// Run without prompting the user
    #[arg(short, long, global = true)]
    quiet: bool,

    /// Force authentication type (interactive|key)
    #[arg(short = 'a', long = "auth", global = true)]
    auth_type: Option<String>,

    /// Force specific credentials by name
    #[arg(short = 's', long = "auth-credentials", global = true)]
    auth_credentials: Option<String>,

    /// Disable automatic update checking
    #[arg(long, global = true)]
    disable_updates: bool,

    #[command(subcommand)]
    command: cmd::Commands,
}

#[tokio::main]
async fn main() {
    // Initialise logging.
    let env = Env::global();
    if env.debug {
        tracing_subscriber::fmt()
            .with_env_filter("hcli=debug")
            .with_writer(std::io::stderr)
            .init();
    }

    let cli = Cli::parse();

    // Start background update check (for binary distributions).
    let update_checker = if is_binary() && !cli.disable_updates && !env.disable_updates {
        let mut checker = BackgroundUpdateChecker::new();
        checker.start();
        Some(checker)
    } else {
        None
    };

    // Dispatch.
    let result = dispatch(cli).await;

    // Show update notification if available.
    if let Some(ref checker) = update_checker {
        if let Some(msg) = checker.get_result(Duration::from_secs(2)) {
            eprintln!("{msg}");
        }
    }

    // Handle errors.
    if let Err(e) = result {
        handle_error(&e);
        std::process::exit(1);
    }
}

async fn dispatch(cli: Cli) -> error::Result<()> {
    use cmd::Commands;

    // Initialise auth for every command so credentials are loaded from disk.
    {
        let mut auth = crate::auth::AuthService::global();
        auth.init(cli.auth_credentials.as_deref());
    }

    match cli.command {
        Commands::Login(args) => cmd::login::run(args).await,
        Commands::Logout(args) => cmd::logout::run(args).await,
        Commands::Whoami => cmd::whoami::run().await,
        Commands::Update(args) => cmd::update::run(args).await,
        Commands::Download(args) => cmd::download::run(args).await,
        Commands::Commands => {
            // Print all subcommands.
            eprintln!("Use `hy --help` to see all available commands.");
            Ok(())
        }
        Commands::Auth { command } => cmd::auth_cmd::run(command).await,
        Commands::Share { command } => cmd::share::run(command).await,
        Commands::License { command } => cmd::license::run(command).await,
        Commands::Ida { command } => cmd::ida_cmd::run(command).await,
        Commands::Plugin { command } => cmd::plugin_cmd::run(command).await,
        Commands::Extension { command } => cmd::extension::run(command).await,
        Commands::Ke { command } => cmd::ke::run(command).await,
    }
}

fn handle_error(err: &Error) {
    use owo_colors::OwoColorize;

    match err {
        Error::NoSpace {
            path,
            required,
            available,
        } => {
            eprintln!(
                "{}",
                format!("Error: No space left on device at {}", path.display()).red()
            );
            if let (Some(req), Some(avail)) = (required, available) {
                eprintln!(
                    "  Required: {} bytes, Available: {} bytes",
                    req, avail
                );
            }
            if cfg!(unix) {
                eprintln!(
                    "\n{}",
                    "Suggestion: set the TMPDIR environment variable to use a different temp directory.".yellow()
                );
            }
        }
        Error::Authentication(_) => {
            eprintln!(
                "{}",
                "Authentication failed. Check your credentials or run `hy login`.".red()
            );
        }
        Error::NotLoggedIn => {
            eprintln!(
                "{}",
                "Not logged in. Run `hy login` first.".red()
            );
        }
        Error::NotFound(msg) => {
            eprintln!("{}", format!("Not found: {msg}").red());
        }
        Error::RateLimit => {
            eprintln!(
                "{}",
                "Rate limit exceeded. Please try again later.".red()
            );
        }
        Error::Api { status, message } => {
            eprintln!("{}", format!("API error ({status}): {message}").red());
        }
        other => {
            eprintln!("{}", format!("Error: {other}").red());
            if Env::global().debug {
                eprintln!("  Debug: {other:?}");
            }
        }
    }
}
