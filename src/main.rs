//! hy — Hex-Rays Command-line Interface.
//!
//! A modern CLI for managing IDA Pro installations, licenses, downloads,
//! plugins, file sharing, and more.

mod api;
mod auth;
mod cli;
mod cmd;
mod config;
mod error;
mod extensions;
mod ida;
mod plugin;
mod update;
mod util;

use crate::cli::Cli;
use std::time::Duration;

use crate::config::Env;
use crate::error::Error;
use crate::update::BackgroundUpdateChecker;
use crate::util::io::is_binary;

#[tokio::main]
async fn main() {
    // Initialise logging.
    let env = Env::global();
    if env.debug {
        tracing_subscriber::fmt().with_env_filter("hy=debug").with_writer(std::io::stderr).init();
    }

    match extensions::initialize().await {
        Ok(Some(code)) => std::process::exit(code),
        Ok(None) => (),
        Err(error) => {
            handle_error(&error);
            std::process::exit(1);
        }
    }
    let cli = cli::parse();
    if let Err(error) = crate::config::ConfigStore::initialize() {
        handle_error(&error);
        std::process::exit(1);
    }

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
    if result.is_ok()
        && let Some(ref checker) = update_checker
        && let Some(msg) = checker.get_result(Duration::from_secs(2))
    {
        println!("{msg}");
    }

    // Handle errors.
    if let Err(e) = result {
        if let Error::ChildExit(code) = e {
            std::process::exit(code);
        }
        handle_error(&e);
        std::process::exit(1);
    }
}

async fn dispatch(cli: Cli) -> error::Result<()> {
    use cmd::Commands;

    // Initialise auth for every command so credentials are loaded from disk.
    {
        let mut auth = crate::auth::AuthService::global();
        let forced = cli
            .command
            .enforces_auth_options()
            .then_some(cli.auth_credentials.as_deref())
            .flatten();
        auth.init(forced)?;
        if cli.command.enforces_auth_options() {
            auth.validate_command_options(cli.auth_type.as_deref())?;
        }
    }
    if cli.command.enforces_auth_options() {
        crate::auth::request_headers(true).await?;
    }

    match cli.command {
        Commands::Mcp {
            command,
        } => cmd::mcp::run(command).await,
        Commands::Login(args) => cmd::login::run(args).await,
        Commands::Logout(args) => cmd::logout::run(args).await,
        Commands::Whoami => cmd::whoami::run().await,
        Commands::Update(args) => cmd::update::run(args).await,
        Commands::Download(args) => cmd::download::run(args).await,
        Commands::CommandTree => {
            cli::print_commands();
            Ok(())
        }
        Commands::Auth {
            command,
        } => cmd::auth_cmd::run(command).await,
        Commands::Share {
            command,
        } => cmd::share::run(command).await,
        Commands::License {
            command,
        } => cmd::license::run(command).await,
        Commands::Ida {
            command,
        } => cmd::ida_cmd::run(command).await,
        Commands::Plugin(args) => cmd::plugin_cmd::run(args).await,
        Commands::Extension {
            command,
        } => cmd::extension::run(command).await,
        Commands::Ke {
            command,
        } => cmd::ke::run(command).await,
        Commands::Asset {
            command,
        } => cmd::asset_cmd::run(command).await,
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
            eprintln!("{}", format!("Error: No space left on device at {}", path.display()).red());
            if let (Some(req), Some(avail)) = (required, available) {
                eprintln!("  Required: {} bytes, Available: {} bytes", req, avail);
            }
            if cfg!(unix) {
                eprintln!(
                    "\n{}",
                    "Suggestion: set the TMPDIR environment variable to use a different temp directory.".yellow()
                );
            }
        }
        Error::Authentication(message) => {
            eprintln!("{}", format!("Authentication failed: {message}").red());
        }
        Error::NotLoggedIn => {
            eprintln!("{}", "Not logged in. Run `hy login` first.".red());
        }
        Error::NotFound(msg) => {
            eprintln!("{}", format!("Not found: {msg}").red());
        }
        Error::RateLimit => {
            eprintln!("{}", "Rate limit exceeded. Please try again later.".red());
        }
        Error::Api {
            status,
            message,
        } => {
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

#[cfg(test)]
mod tests {
    use super::Cli;
    use crate::cmd::Commands;
    use crate::cmd::ida_cmd::{IdaCommands, PythonArgs, PythonCommands};
    use clap::{CommandFactory, Parser};

    #[test]
    fn command_tree_has_no_conflicting_options() {
        Cli::command().debug_assert();
    }

    #[test]
    fn python_exec_preserves_child_help_argument() {
        let cli = Cli::try_parse_from(["hy", "ida", "python", "exec", "-m", "pip", "--help"])
            .expect("child arguments must be accepted");

        let Commands::Ida {
            command:
                IdaCommands::Python(PythonArgs {
                    command:
                        PythonCommands::Exec {
                            args,
                        },
                    ..
                }),
        } = cli.command
        else {
            panic!("expected Python exec command");
        };

        assert_eq!(args, ["-m", "pip", "--help"]);
    }

    #[test]
    fn python_script_preserves_unknown_options() {
        let cli = Cli::try_parse_from([
            "hy",
            "ida",
            "python",
            "run-script",
            "example",
            "--help",
            "--output",
            "a path with spaces",
        ])
        .expect("script options must be passed through");

        let Commands::Ida {
            command:
                IdaCommands::Python(PythonArgs {
                    command:
                        PythonCommands::RunScript {
                            invocation,
                        },
                    ..
                }),
        } = cli.command
        else {
            panic!("expected Python run-script command");
        };

        assert_eq!(invocation, ["example", "--help", "--output", "a path with spaces"]);
    }
}
