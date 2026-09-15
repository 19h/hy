//! Native executable update selection and confirmation.

use clap::{Args, ValueEnum};
use dialoguer::Confirm;

use crate::config::Env;
use crate::error::{Error, Result};
use crate::update::{GitHubRepo, compatible_version, get_assets, update_binary};
use crate::util::fmt;
use crate::util::io::{arch_name, executable_path, is_binary, os_name};

#[derive(Debug, Args)]
pub struct UpdateArgs {
    #[arg(short = 'm', long, value_enum, default_value = "auto", hide = true)]
    pub mode: UpdateMode,
    /// Force update
    #[arg(short, long)]
    pub force: bool,

    /// Automatically install update if available
    #[arg(long)]
    pub auto_install: bool,

    /// Include pre-release versions
    #[arg(long)]
    pub include_prereleases: bool,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum UpdateMode {
    Auto,
    Binary,
    Pypi,
}

pub async fn run(args: UpdateArgs) -> Result<()> {
    tokio::task::spawn_blocking(move || run_blocking(args))
        .await
        .map_err(|error| Error::UpdateFailed(format!("update worker failed: {error}")))?
}

fn run_blocking(args: UpdateArgs) -> Result<()> {
    let env = Env::global();
    if matches!(args.mode, UpdateMode::Pypi) {
        return Err(Error::UpdateFailed(
            "PyPI update mode is unavailable for the native Rust executable".into(),
        ));
    }
    if matches!(args.mode, UpdateMode::Auto) && !is_binary() {
        fmt::info(
            "Development build: update the source checkout and rebuild with cargo build --release.",
        );
        return Ok(());
    }

    fmt::info("Checking for updates...");

    let repo = GitHubRepo::from_url(&env.github_url)?;

    let op = if args.force {
        ">="
    } else {
        ">"
    };
    let req_str = format!("{op}{}", env.version);
    let latest = compatible_version(&repo, &req_str, args.include_prereleases)?;

    let Some(latest) = latest else {
        fmt::success(&format!("Already using the latest version ({})", env.version));
        return Ok(());
    };

    // Find matching asset for this platform.
    let mask = regex::Regex::new(&format!(".*-{}-{}.*", os_name(), arch_name())).unwrap();

    let assets = get_assets(&repo, &latest.tag, &mask)?;

    if assets.len() != 1 {
        return Ok(());
    }

    eprintln!("Update available: {} -> {}", env.version, latest.version);

    if !args.auto_install {
        let confirm = Confirm::new()
            .with_prompt(format!("Install update to {}?", latest.version))
            .default(true)
            .interact()
            .unwrap_or(false);
        if !confirm {
            fmt::warning("Update cancelled.");
            return Ok(());
        }
    }

    let binary = executable_path();
    update_binary(&assets[0], &repo, &binary)?;
    fmt::success(&format!("Successfully updated to {}", latest.version));

    Ok(())
}
