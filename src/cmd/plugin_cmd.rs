//! `hy plugin` command group: install, uninstall, list, search, upgrade, etc.

use std::path::PathBuf;

use clap::{Args, Subcommand};

use crate::error::Result;
use crate::util::fmt;

#[derive(Debug, Args)]
pub struct PluginArgs {
    /// Override repository lookup with a URL, snapshot, directory, or bundle
    #[arg(long = "repo", hide = true)]
    pub repository: Option<String>,
    /// Additional GitHub repositories, one owner/repository per line
    #[arg(long, hide = true)]
    pub with_repos_list: Option<PathBuf>,
    /// GitHub repositories excluded from catalogue discovery
    #[arg(long, hide = true)]
    pub with_ignored_repos_list: Option<PathBuf>,
    #[arg(long)]
    pub pip_index_url: Option<String>,
    #[arg(long)]
    pub pip_extra_index_url: Vec<String>,
    #[arg(long)]
    pub pip_find_links: Vec<String>,
    /// Resolve dependencies from local package sources
    #[arg(long, hide = true)]
    pub offline: bool,
    /// Skip Python environment health checks before installing dependencies
    #[arg(long)]
    pub no_python_environment_check: bool,
    #[command(subcommand)]
    pub command: PluginCommands,
}

// ── CLI definitions ─────────────────────────────────────────────────────

#[derive(Debug, Subcommand)]
pub enum PluginCommands {
    /// Install a plugin from a repository, local directory, zip archive, bundle, or URL
    Install(PluginInstallArgs),
    /// Uninstall an installed plugin
    Uninstall(PluginUninstallArgs),
    /// List installed plugins
    List(PluginStatusArgs),
    /// Search for plugins in the repository
    Search(PluginSearchArgs),
    /// Upgrade an installed plugin
    Upgrade(PluginUpgradeArgs),
    /// Show plugin status / info
    Status(PluginStatusArgs),
    /// Explain IDA's Python environment
    ExplainEnvironment {
        #[arg(long)]
        json: bool,
    },
    /// Lint a plugin archive
    Lint(PluginLintArgs),
    /// Manage plugin bundles for offline installation
    Bundle {
        #[command(subcommand)]
        command: BundleCommands,
    },
    /// Manage plugin repositories
    Repo {
        #[command(subcommand)]
        command: RepoCommands,
    },
    /// Manage plugin configuration
    Config {
        /// Installed plugin name
        plugin: String,
        #[command(subcommand)]
        command: ConfigCommands,
    },
    /// Print the JSON Schema for ida-plugin.json
    #[command(hide = true)]
    Schema(PluginSchemaArgs),
}

#[derive(Debug, Args)]
pub struct PluginInstallArgs {
    /// Plugin name, path to a directory / zip archive / bundle, or URL
    pub source: String,
    /// Force installation even if already installed
    #[arg(short, long)]
    pub force: bool,
    /// Upgrade an existing plugin, or install it if absent
    #[arg(short = 'U', long)]
    pub upgrade: bool,
    #[arg(long)]
    pub no_build_isolation: bool,
    /// Install a local plugin directory by symlinking it into $IDAUSR/plugins/.
    /// Edits to the source tree take effect on the next plugin reload.
    #[arg(short, long)]
    pub editable: bool,
    /// Configuration setting in key=value format (repeatable; use true/false for booleans)
    #[arg(long = "config")]
    pub config: Vec<String>,
}

#[derive(Debug, Args)]
pub struct PluginSchemaArgs {
    /// Write the schema to this file instead of stdout
    #[arg(short, long)]
    pub output: Option<PathBuf>,
    /// Indentation for the emitted JSON
    #[arg(long, default_value_t = 2)]
    pub indent: usize,
}

#[derive(Debug, Subcommand)]
pub enum BundleCommands {
    /// Show plugin bundle metadata
    Info(BundleInfoArgs),
    /// Create a plugin bundle from plugin specs and/or local ZIPs
    Create(BundleCreateArgs),
}

#[derive(Debug, Args)]
pub struct BundleInfoArgs {
    /// Path to the bundle archive
    pub bundle_path: PathBuf,
}

#[derive(Debug, Args)]
pub struct BundleCreateArgs {
    /// Output archive path
    #[arg(long = "path")]
    pub output: PathBuf,
    /// Target platform: 'current', 'all', or a name like 'linux', 'windows', 'macos-arm64' (repeatable)
    #[arg(long = "platform")]
    pub platforms: Vec<String>,
    /// Target Python version: 'current', 'all', or a version like '3.12' (repeatable)
    #[arg(long = "python")]
    pub pythons: Vec<String>,
    /// Exact target ID (e.g. linux-x86_64-cp312)
    #[arg(long = "target", hide = true)]
    pub targets: Vec<String>,
    /// Plugin repository (URL or local snapshot JSON) for resolving specs
    #[arg(long)]
    pub repo: Option<String>,
    /// Plugin specs (name==version[@host]) and/or local plugin ZIPs
    #[arg(required = true)]
    pub plugin_specs: Vec<String>,
}

#[derive(Debug, Args)]
pub struct PluginUninstallArgs {
    /// Plugin name
    pub name: String,
}

#[derive(Debug, Args)]
pub struct PluginSearchArgs {
    /// Search query (plugin name, keyword, or category)
    pub query: Option<String>,
    #[arg(long)]
    pub json: bool,
    #[arg(long)]
    pub offline: bool,
}

#[derive(Debug, Args)]
pub struct PluginUpgradeArgs {
    /// Plugin name or archive path
    pub source: Option<String>,
    #[arg(long)]
    pub no_build_isolation: bool,
}

#[derive(Debug, Args)]
pub struct PluginStatusArgs {
    /// Plugin names (omit to show all)
    pub names: Vec<String>,
    #[arg(long)]
    pub json: bool,
    #[arg(long = "skip-upgrade-check", visible_alias = "offline")]
    pub offline: bool,
}

#[derive(Debug, Args)]
pub struct PluginLintArgs {
    /// Plugin directory, ZIP archive, or archive URL
    pub path: String,
}

#[derive(Debug, Subcommand)]
pub enum RepoCommands {
    /// List configured repositories
    List,
    /// Add a repository
    Add(RepoAddArgs),
    /// Remove a repository
    Remove(RepoRemoveArgs),
    /// Set the repository used by unprefixed plugin references
    SetDefault(RepoRemoveArgs),
    /// Create a JSON snapshot of the repository
    #[command(hide = true)]
    Snapshot,
}

#[derive(Debug, Args)]
pub struct RepoAddArgs {
    pub name: String,
    /// Repository URL
    pub url: String,
}

#[derive(Debug, Args)]
pub struct RepoRemoveArgs {
    /// Repository URL
    pub name: String,
}

#[derive(Debug, Subcommand)]
pub enum ConfigCommands {
    /// Get a plugin setting
    Get(ConfigGetArgs),
    /// Set a plugin setting
    Set(ConfigSetArgs),
    /// Delete a plugin setting
    Del(ConfigDelArgs),
    /// List all plugin settings
    List,
    /// Interactively configure plugin settings
    Setup,
    /// Export plugin settings as JSON
    Export,
    /// Import plugin settings from JSON
    Import(ConfigImportArgs),
}

#[derive(Debug, Args)]
pub struct ConfigGetArgs {
    /// Setting key
    pub key: String,
}

#[derive(Debug, Args)]
pub struct ConfigSetArgs {
    /// Setting key
    pub key: String,
    /// Setting value
    pub value: String,
}

#[derive(Debug, Args)]
pub struct ConfigDelArgs {
    /// Setting key
    pub key: String,
}

#[derive(Debug, Args)]
pub struct ConfigImportArgs {
    /// JSON string (reads from stdin if omitted)
    pub json: Option<String>,
}

// ── dispatch ────────────────────────────────────────────────────────────

pub async fn run(args: PluginArgs) -> Result<()> {
    let context = crate::plugin::PluginContext {
        repository: args.repository,
        github: crate::plugin::index::github::Options {
            repositories_file: args.with_repos_list,
            ignored_file: args.with_ignored_repos_list,
        },
        pip: crate::ida::python::PipOptions {
            index_url: args.pip_index_url,
            extra_index_urls: args.pip_extra_index_url,
            find_links: crate::ida::python::normalize_find_links(args.pip_find_links)?,
            no_index: args.offline,
            skip_environment_check: args.no_python_environment_check,
            ..Default::default()
        },
    };
    if !matches!(
        args.command,
        PluginCommands::Schema(_)
            | PluginCommands::ExplainEnvironment { .. }
            | PluginCommands::Uninstall(_)
            | PluginCommands::Config { .. }
            | PluginCommands::Lint(_)
    ) {
        context.validate_pip_sources()?;
    }
    match args.command {
        PluginCommands::List(args) => super::plugin_status::run(args, &context).await,
        PluginCommands::Install(args) => super::plugin_ops::install(args, &context).await,
        PluginCommands::Uninstall(args) => run_uninstall(args).await,
        PluginCommands::Upgrade(args) => super::plugin_upgrade::run(args, &context).await,
        PluginCommands::Search(args) => super::plugin_search::run(args, &context).await,
        PluginCommands::Status(args) => super::plugin_status::run(args, &context).await,
        PluginCommands::ExplainEnvironment {
            json,
        } => {
            super::ida_cmd::run_python_command(super::ida_cmd::PythonArgs {
                no_python_environment_check: false,
                command: super::ida_cmd::PythonCommands::ExplainEnvironment {
                    json,
                },
            })
            .await
        }
        PluginCommands::Lint(args) => super::plugin_lint::run(args).await,
        PluginCommands::Bundle {
            command,
        } => super::plugin_bundle::run(command, &context).await,
        PluginCommands::Repo {
            command,
        } => super::plugin_ops::repo(command, &context).await,
        PluginCommands::Config {
            plugin,
            command,
        } => super::plugin_config::run(&plugin, command),
        PluginCommands::Schema(args) => run_schema(args),
    }
}

// ── plugin schema ───────────────────────────────────────────────────────

fn run_schema(args: PluginSchemaArgs) -> Result<()> {
    let schema = crate::plugin::ida_plugin_json_schema();
    let mut payload = serde_json::to_string_pretty(&schema)?;
    if args.indent != 2 {
        // Re-render with the requested indentation.
        let indent_bytes = " ".repeat(args.indent);
        let mut buf = Vec::new();
        let formatter = serde_json::ser::PrettyFormatter::with_indent(indent_bytes.as_bytes());
        let mut ser = serde_json::Serializer::with_formatter(&mut buf, formatter);
        serde::Serialize::serialize(&schema, &mut ser)?;
        payload = String::from_utf8_lossy(&buf).into_owned();
    }

    if let Some(ref output) = args.output {
        if let Some(parent) = output.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(output, format!("{payload}\n"))?;
        fmt::success(&format!("Wrote schema to {}", output.display()));
    } else {
        println!("{payload}");
    }
    Ok(())
}

// ── plugin uninstall ────────────────────────────────────────────────────

async fn run_uninstall(args: PluginUninstallArgs) -> Result<()> {
    crate::plugin::uninstall(&args.name).await?;
    fmt::success(&format!("Plugin '{}' uninstalled.", args.name));
    Ok(())
}
