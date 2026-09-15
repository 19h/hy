//! CLI parsing and deterministic command inventory.

mod status;

use crate::config::Env;
use clap::{CommandFactory, FromArgMatches, Parser};

/// hy — Hex-Rays Command-line Interface
#[derive(Debug, Parser)]
#[command(
    name = "hy",
    version,
    about = "Hex-Rays CLI for managing IDA installation, licenses, and more",
    long_about = None,
    propagate_version = true,
)]
pub(crate) struct Cli {
    /// Force authentication type (interactive|key)
    #[arg(short = 'a', long = "auth")]
    pub(crate) auth_type: Option<String>,

    /// Force specific credentials by name
    #[arg(short = 's', long = "auth-credentials", global = true)]
    pub(crate) auth_credentials: Option<String>,

    /// Disable automatic update checking
    #[arg(long, global = true)]
    pub(crate) disable_updates: bool,

    #[command(subcommand)]
    pub(crate) command: crate::cmd::Commands,
}

pub(crate) fn parse() -> Cli {
    let environment = Env::global();
    let arguments: Vec<_> = std::env::args_os().collect();
    let help_requested = arguments.len() == 1
        || arguments.iter().skip(1).any(|argument| argument == "--help" || argument == "-h");
    let matches = crate::extensions::augment(Cli::command())
        .name(environment.binary_name.clone())
        .bin_name(environment.binary_name.clone())
        .version(environment.full_version())
        .after_help(if help_requested {
            status::summary() + &crate::extensions::help_summary()
        } else {
            String::new()
        })
        .get_matches_from(arguments);
    Cli::from_arg_matches(&matches).unwrap_or_else(|error| error.exit())
}

pub(crate) fn print_commands() {
    let mut rows = Vec::new();
    collect_commands(
        &crate::extensions::augment(Cli::command()),
        &Env::global().binary_name,
        &mut rows,
    );
    rows.sort_by(|left, right| left.0.cmp(&right.0));
    println!("All Available Commands\n");
    for (path, description) in &rows {
        println!("{path:<40} {description}");
    }
    println!("\nTotal commands: {}", rows.len());
}

fn collect_commands(command: &clap::Command, prefix: &str, rows: &mut Vec<(String, String)>) {
    for child in command.get_subcommands() {
        if child.is_hide_set() || child.get_name() == "help" {
            continue;
        }
        let path = format!("{prefix} {}", child.get_name());
        if child.has_subcommands() {
            collect_commands(child, &path, rows);
        } else {
            let description = child.get_about().map(ToString::to_string).unwrap_or_default();
            rows.push((
                path,
                description.lines().next().unwrap_or("No description available").into(),
            ));
        }
    }
}
