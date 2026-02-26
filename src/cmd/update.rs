//! `hcli update` command.

use clap::Args;
use dialoguer::Confirm;
use semver::VersionReq;

use crate::config::Env;
use crate::error::Result;
use crate::update::{compatible_version, get_assets, update_binary, GitHubRepo, parse_version};
use crate::util::fmt;
use crate::util::io::{arch_name, executable_path, is_binary, os_name};

#[derive(Debug, Args)]
pub struct UpdateArgs {
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

pub async fn run(args: UpdateArgs) -> Result<()> {
    let env = Env::global();

    if !is_binary() {
        eprintln!("\nTo update, run:");
        eprintln!("  uv tool upgrade ida-hcli");
        eprintln!("or");
        eprintln!("  pipx upgrade ida-hcli");
        return Ok(());
    }

    fmt::info("Checking for updates...");

    let repo = GitHubRepo::from_url(&env.github_url)?;
    let _current = parse_version(&env.version);

    let op = if args.force { ">=" } else { ">" };
    let req_str = format!("{op}{}", env.version);
    let req = VersionReq::parse(&req_str)
        .map_err(|e| crate::error::Error::UpdateFailed(format!("bad version req: {e}")))?;

    let latest = compatible_version(&repo, &req, args.include_prereleases)?;

    let Some(latest) = latest else {
        fmt::success(&format!(
            "Already using the latest version ({})",
            env.version
        ));
        return Ok(());
    };

    eprintln!(
        "Update available: {} -> {}",
        env.version, latest
    );

    // Find matching asset for this platform.
    let mask = regex::Regex::new(&format!(
        ".*-{}-{}.*",
        os_name(),
        arch_name()
    ))
    .unwrap();

    let tag = format!("v{latest}");
    let assets = get_assets(&repo, &tag, &mask)?;

    if assets.len() != 1 {
        fmt::error(&format!(
            "Expected 1 asset for this platform, found {}",
            assets.len()
        ));
        return Ok(());
    }

    if !args.auto_install {
        let confirm = Confirm::new()
            .with_prompt(format!("Install update to {latest}?"))
            .default(true)
            .interact()
            .unwrap_or(false);
        if !confirm {
            fmt::warning("Update cancelled.");
            return Ok(());
        }
    }

    let binary = executable_path();
    if update_binary(&assets[0], &repo, &binary)? {
        fmt::success(&format!("Successfully updated to {latest}"));
    } else {
        fmt::success(&format!(
            "Already using the latest version ({})",
            env.version
        ));
    }

    Ok(())
}
