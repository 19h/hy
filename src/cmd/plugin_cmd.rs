//! `hcli plugin` command group: install, uninstall, list, search, upgrade, etc.

use std::path::PathBuf;

use clap::{Args, Subcommand};

use crate::error::Result;
use crate::util::fmt;

#[derive(Debug, Subcommand)]
pub enum PluginCommands {
    /// Install a plugin from a zip archive or URL
    Install(PluginInstallArgs),
    /// Uninstall an installed plugin
    Uninstall(PluginUninstallArgs),
    /// List installed plugins
    List,
    /// Search for plugins
    Search(PluginSearchArgs),
    /// Upgrade an installed plugin
    Upgrade(PluginUpgradeArgs),
    /// Show plugin status / info
    Status(PluginStatusArgs),
    /// Lint a plugin archive
    Lint(PluginLintArgs),
    /// Manage plugin repositories
    Repo {
        #[command(subcommand)]
        command: RepoCommands,
    },
    /// Manage plugin configuration
    Config {
        #[command(subcommand)]
        command: ConfigCommands,
    },
}

#[derive(Debug, Args)]
pub struct PluginInstallArgs {
    /// Path or URL to the plugin archive
    pub source: String,
    /// Force installation even if already installed
    #[arg(short, long)]
    pub force: bool,
}

#[derive(Debug, Args)]
pub struct PluginUninstallArgs {
    /// Plugin name
    pub name: String,
}

#[derive(Debug, Args)]
pub struct PluginSearchArgs {
    /// Search query
    pub query: String,
}

#[derive(Debug, Args)]
pub struct PluginUpgradeArgs {
    /// Plugin name or archive path
    pub source: String,
}

#[derive(Debug, Args)]
pub struct PluginStatusArgs {
    /// Plugin name
    pub name: Option<String>,
}

#[derive(Debug, Args)]
pub struct PluginLintArgs {
    /// Path to the plugin archive
    pub path: PathBuf,
}

#[derive(Debug, Subcommand)]
pub enum RepoCommands {
    /// List configured repositories
    List,
    /// Add a repository
    Add(RepoAddArgs),
    /// Remove a repository
    Remove(RepoRemoveArgs),
}

#[derive(Debug, Args)]
pub struct RepoAddArgs {
    /// Repository URL
    pub url: String,
}

#[derive(Debug, Args)]
pub struct RepoRemoveArgs {
    /// Repository URL
    pub url: String,
}

#[derive(Debug, Subcommand)]
pub enum ConfigCommands {
    /// Get a plugin setting
    Get(ConfigGetArgs),
    /// Set a plugin setting
    Set(ConfigSetArgs),
}

#[derive(Debug, Args)]
pub struct ConfigGetArgs {
    /// Plugin name
    pub plugin: String,
    /// Setting key
    pub key: String,
}

#[derive(Debug, Args)]
pub struct ConfigSetArgs {
    /// Plugin name
    pub plugin: String,
    /// Setting key
    pub key: String,
    /// Setting value
    pub value: String,
}

pub async fn run(cmd: PluginCommands) -> Result<()> {
    match cmd {
        PluginCommands::List => run_list().await,
        PluginCommands::Install(args) => run_install(args).await,
        PluginCommands::Uninstall(args) => run_uninstall(args).await,
        PluginCommands::Upgrade(args) => run_upgrade(args).await,
        PluginCommands::Search(args) => run_search(args).await,
        PluginCommands::Status(args) => run_status(args).await,
        PluginCommands::Lint(args) => run_lint(args).await,
        PluginCommands::Repo { command } => run_repo(command).await,
        PluginCommands::Config { command } => run_config(command).await,
    }
}

async fn run_list() -> Result<()> {
    let plugins = crate::plugin::installed_plugins()?;
    if plugins.is_empty() {
        fmt::warning("No plugins installed.");
        return Ok(());
    }

    eprintln!("{:<30} {}", "Name", "Version");
    eprintln!("{}", "-".repeat(50));
    for (name, ver) in &plugins {
        eprintln!("{:<30} {}", name, ver.as_deref().unwrap_or("unknown"));
    }
    Ok(())
}

async fn run_install(args: PluginInstallArgs) -> Result<()> {
    let path = PathBuf::from(&args.source);
    if !path.exists() {
        fmt::error(&format!("File not found: {}", args.source));
        return Ok(());
    }

    let ida_version = crate::ida::current_install_dir()
        .as_deref()
        .and_then(crate::ida::detect_ida_version);

    let result = crate::plugin::install_from_archive(
        &path,
        ida_version.as_deref(),
        args.force,
    )?;
    fmt::success(&format!("Plugin installed at: {}", result.display()));
    Ok(())
}

async fn run_uninstall(args: PluginUninstallArgs) -> Result<()> {
    crate::plugin::uninstall(&args.name)?;
    fmt::success(&format!("Plugin '{}' uninstalled.", args.name));
    Ok(())
}

async fn run_upgrade(args: PluginUpgradeArgs) -> Result<()> {
    let path = PathBuf::from(&args.source);
    if !path.exists() {
        fmt::error(&format!("File not found: {}", args.source));
        return Ok(());
    }

    let ida_version = crate::ida::current_install_dir()
        .as_deref()
        .and_then(crate::ida::detect_ida_version);

    let result = crate::plugin::upgrade_from_archive(&path, ida_version.as_deref())?;
    fmt::success(&format!("Plugin upgraded at: {}", result.display()));
    Ok(())
}

async fn run_search(_args: PluginSearchArgs) -> Result<()> {
    fmt::info("Plugin search requires a configured repository.");
    fmt::warning("Use `hcli plugin repo list` to see configured repositories.");
    Ok(())
}

async fn run_status(args: PluginStatusArgs) -> Result<()> {
    if let Some(name) = args.name {
        if crate::plugin::is_installed(&name) {
            let plugins = crate::plugin::installed_plugins()?;
            if let Some((_, ver)) = plugins.iter().find(|(n, _)| n == &name) {
                eprintln!(
                    "Plugin '{}' is installed (version: {})",
                    name,
                    ver.as_deref().unwrap_or("unknown")
                );
            }
        } else {
            eprintln!("Plugin '{}' is not installed.", name);
        }
    } else {
        let plugins = crate::plugin::installed_plugins()?;
        eprintln!("{} plugin(s) installed.", plugins.len());
    }
    Ok(())
}

async fn run_lint(args: PluginLintArgs) -> Result<()> {
    let metadata = crate::plugin::read_metadata_from_archive(&args.path)?;
    eprintln!("Plugin: {} v{}", metadata.name, metadata.version);
    if !metadata.description.is_empty() {
        eprintln!("Description: {}", metadata.description);
    }
    if let Some(ref ep) = metadata.entry_point {
        eprintln!("Entry point: {ep}");
    }
    fmt::success("Plugin archive is valid.");
    Ok(())
}

async fn run_repo(_cmd: RepoCommands) -> Result<()> {
    fmt::info("Plugin repository management is not yet implemented.");
    Ok(())
}

async fn run_config(_cmd: ConfigCommands) -> Result<()> {
    fmt::info("Plugin config management is not yet implemented.");
    Ok(())
}
