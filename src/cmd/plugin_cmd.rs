//! `hy plugin` command group: install, uninstall, list, search, upgrade, etc.

use std::path::PathBuf;

use clap::{Args, Subcommand};
use owo_colors::OwoColorize;

use crate::error::Result;
use crate::util::{fmt, tui};

// ── CLI definitions ─────────────────────────────────────────────────────

#[derive(Debug, Subcommand)]
pub enum PluginCommands {
    /// Install a plugin from a repository, local directory, zip archive, bundle, or URL
    Install(PluginInstallArgs),
    /// Uninstall an installed plugin
    Uninstall(PluginUninstallArgs),
    /// List installed plugins
    List,
    /// Search for plugins in the repository
    Search(PluginSearchArgs),
    /// Upgrade an installed plugin
    Upgrade(PluginUpgradeArgs),
    /// Show plugin status / info
    Status(PluginStatusArgs),
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
}

#[derive(Debug, Args)]
pub struct PluginUpgradeArgs {
    /// Plugin name or archive path
    pub source: String,
}

#[derive(Debug, Args)]
pub struct PluginStatusArgs {
    /// Plugin name (omit to show all)
    pub name: Option<String>,
}

#[derive(Debug, Args)]
pub struct PluginLintArgs {
    /// Path or URL to the plugin archive
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
    /// Create a JSON snapshot of the repository
    #[command(hide = true)]
    Snapshot,
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
    /// Delete a plugin setting
    Del(ConfigDelArgs),
    /// List all plugin settings
    List(ConfigListArgs),
    /// Export plugin settings as JSON
    Export(ConfigExportArgs),
    /// Import plugin settings from JSON
    Import(ConfigImportArgs),
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

#[derive(Debug, Args)]
pub struct ConfigDelArgs {
    /// Plugin name
    pub plugin: String,
    /// Setting key
    pub key: String,
}

#[derive(Debug, Args)]
pub struct ConfigListArgs {
    /// Plugin name
    pub plugin: String,
}

#[derive(Debug, Args)]
pub struct ConfigExportArgs {
    /// Plugin name
    pub plugin: String,
}

#[derive(Debug, Args)]
pub struct ConfigImportArgs {
    /// Plugin name
    pub plugin: String,
    /// JSON string (reads from stdin if omitted)
    pub json: Option<String>,
}

// ── dispatch ────────────────────────────────────────────────────────────

pub async fn run(cmd: PluginCommands) -> Result<()> {
    match cmd {
        PluginCommands::List => run_list().await,
        PluginCommands::Install(args) => run_install(args).await,
        PluginCommands::Uninstall(args) => run_uninstall(args).await,
        PluginCommands::Upgrade(args) => run_upgrade(args).await,
        PluginCommands::Search(args) => run_search(args).await,
        PluginCommands::Status(args) => run_status(args).await,
        PluginCommands::Lint(args) => run_lint(args).await,
        PluginCommands::Bundle { command } => run_bundle(command).await,
        PluginCommands::Repo { command } => run_repo(command).await,
        PluginCommands::Config { command } => run_config(command).await,
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

// ── plugin list ─────────────────────────────────────────────────────────

async fn run_list() -> Result<()> {
    let plugins = crate::plugin::installed_plugins()?;
    if plugins.is_empty() {
        fmt::warning("No plugins installed.");
        return Ok(());
    }

    print_installed_table(&plugins);
    Ok(())
}

fn print_installed_table(plugins: &[crate::plugin::InstalledPlugin]) {
    let mut table = tui::Table::new(&["Name", "Version", ""]);
    for plugin in plugins {
        table.add_row(vec![
            plugin.name.clone(),
            plugin
                .version
                .as_deref()
                .unwrap_or("unknown")
                .dimmed()
                .to_string(),
            if plugin.editable {
                "editable".yellow().to_string()
            } else {
                String::new()
            },
        ]);
    }
    table.print();
}

// ── plugin install ──────────────────────────────────────────────────────

async fn run_install(args: PluginInstallArgs) -> Result<()> {
    let ida_version = crate::ida::current_install_dir()
        .as_deref()
        .and_then(crate::ida::detect_ida_version);

    // Validate --config syntax up front, before touching the filesystem.
    for item in &args.config {
        if !item.contains('=') {
            fmt::error(&format!("Invalid config format: '{item}', expected key=value"));
            return Ok(());
        }
    }

    let source_as_path = std::path::PathBuf::from(&args.source);

    // Editable install: symlink a source directory into the plugins dir.
    if args.editable {
        if !source_as_path.is_dir() {
            fmt::error(&format!(
                "--editable requires a directory containing ida-plugin.json, got: {}",
                args.source
            ));
            return Ok(());
        }
        let metadata = crate::plugin::read_metadata_from_directory(&source_as_path)?;
        let target = crate::plugin::install_editable(&source_as_path, ida_version.as_deref(), args.force)?;
        apply_install_settings(&metadata, &args.config)?;
        fmt::success(&format!(
            "Installed plugin: {}=={} (editable) -> {}",
            metadata.name,
            metadata.version,
            target.display()
        ));
        return Ok(());
    }

    // Local directory install: copy the source tree.
    if source_as_path.is_dir() {
        if !source_as_path.join("ida-plugin.json").is_file() {
            fmt::error(&format!(
                "Directory {} does not contain ida-plugin.json.",
                args.source
            ));
            return Ok(());
        }
        let metadata = crate::plugin::read_metadata_from_directory(&source_as_path)?;
        let target =
            crate::plugin::install_from_directory(&source_as_path, ida_version.as_deref(), args.force)?;
        apply_install_settings(&metadata, &args.config)?;
        fmt::success(&format!(
            "Installed plugin: {}=={} -> {}",
            metadata.name,
            metadata.version,
            target.display()
        ));
        return Ok(());
    }

    // Plugin bundle: install every plugin contained in the bundle.
    if source_as_path.is_file() && crate::plugin::bundle::is_plugin_bundle_zip(&source_as_path) {
        return install_from_bundle(&source_as_path, ida_version.as_deref(), args.force, &args.config);
    }

    let source_path = if args.source.starts_with("http://") || args.source.starts_with("https://") {
        fmt::info(&format!("Downloading plugin from {}...", args.source));
        let client = crate::api::ApiClient::new()?;
        let ts = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis();
        let temp_dir = std::env::temp_dir().join(format!("hy-plugin-{ts}"));
        client
            .download_file(&args.source, &temp_dir, Some("plugin.zip"), false, false, None)
            .await?
    } else {
        let p = std::path::PathBuf::from(&args.source);
        if p.exists() {
            p
        } else {
            // Treat as a plugin name from the repository.
            let repo_url = match crate::plugin::get_repo_url() {
                Some(url) => url,
                None => {
                    fmt::error(&format!("File not found: {}", args.source));
                    fmt::warning("Or, if trying to install by name, no plugin repository configured.");
                    return Ok(());
                }
            };

            fmt::info(&format!("Looking up plugin '{}' in repository...", args.source));
            let body = reqwest::get(&repo_url).await?.text().await?;
            let repo: RepoSnapshot = serde_json::from_str(&body).map_err(|e| {
                crate::error::Error::Other(format!("Failed to parse repository JSON: {e}"))
            })?;

            let plugin = repo.plugins.into_iter().find(|p| p.name == args.source);
            let Some(plugin) = plugin else {
                fmt::error(&format!("Plugin '{}' not found in repository.", args.source));
                return Ok(());
            };

            let latest_version = plugin.versions.keys().max().map(|s| s.as_str());
            let Some(ver) = latest_version else {
                fmt::error(&format!("No versions found for plugin '{}'.", args.source));
                return Ok(());
            };

            // Grab the first archive location for the latest version.
            let locations = plugin.versions.get(ver).unwrap();
            let Some(loc) = locations.first() else {
                fmt::error(&format!("No download location for '{}' v{ver}.", args.source));
                return Ok(());
            };

            fmt::info(&format!("Downloading '{}' v{} from {}...", args.source, ver, loc.url));
            let client = crate::api::ApiClient::new()?;
            let ts = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis();
            let temp_dir = std::env::temp_dir().join(format!("hy-plugin-{ts}"));
            client
                .download_file(&loc.url, &temp_dir, Some("plugin.zip"), false, false, None)
                .await?
        }
    };

    // A downloaded file could itself be a bundle.
    if crate::plugin::bundle::is_plugin_bundle_zip(&source_path) {
        let result =
            install_from_bundle(&source_path, ida_version.as_deref(), args.force, &args.config);
        if args.source.starts_with("http") || !std::path::PathBuf::from(&args.source).exists() {
            let _ = std::fs::remove_dir_all(source_path.parent().unwrap());
        }
        return result;
    }

    let metadata = crate::plugin::read_metadata_from_archive(&source_path)?;
    let result = crate::plugin::install_from_archive(&source_path, ida_version.as_deref(), args.force)?;
    apply_install_settings(&metadata, &args.config)?;
    fmt::success(&format!(
        "Installed plugin: {}=={} -> {}",
        metadata.name,
        metadata.version,
        result.display()
    ));

    // Clean up temp file if we downloaded it
    if args.source.starts_with("http") || !std::path::PathBuf::from(&args.source).exists() {
        let _ = std::fs::remove_dir_all(source_path.parent().unwrap());
    }

    Ok(())
}

/// Install every plugin archive contained in a bundle zip.
fn install_from_bundle(
    bundle_path: &std::path::Path,
    ida_version: Option<&str>,
    force: bool,
    config: &[String],
) -> Result<()> {
    let manifest = crate::plugin::bundle::read_bundle_manifest(bundle_path)?;
    let plugins = crate::plugin::bundle::list_bundle_plugins(bundle_path)?;

    if plugins.is_empty() {
        fmt::warning("Bundle contains no plugins.");
        return Ok(());
    }

    fmt::info(&format!(
        "Installing {} plugin(s) from bundle (built {} by {} {})...",
        plugins.len(),
        manifest.built_at,
        manifest.created_by.tool,
        manifest.created_by.version
    ));

    let mut installed = 0usize;
    for plugin in &plugins {
        let archive = crate::plugin::bundle::extract_bundled_plugin(bundle_path, &plugin.entry_name)?;
        match crate::plugin::install_from_archive(&archive, ida_version, force) {
            Ok(_) => {
                apply_install_settings(&plugin.metadata, config)?;
                fmt::success(&format!(
                    "Installed plugin: {}=={}",
                    plugin.metadata.name, plugin.metadata.version
                ));
                installed += 1;
            }
            Err(e) => {
                fmt::error(&format!(
                    "Failed to install {}: {e}",
                    plugin.metadata.name
                ));
            }
        }
        let _ = std::fs::remove_dir_all(archive.parent().unwrap());
    }

    if plugins
        .iter()
        .any(|p| p.metadata.python_dependencies.as_ref().is_some_and(|d| !d.is_empty()))
    {
        fmt::warning(
            "Some plugins declare Python dependencies; install them from the bundle wheelhouse with pip if needed.",
        );
    }

    fmt::info(&format!("{installed}/{} plugin(s) installed.", plugins.len()));
    Ok(())
}

/// Apply `--config key=value` settings after a successful install, or
/// interactively prompt for required, unset settings.
fn apply_install_settings(
    metadata: &crate::plugin::PluginMetadata,
    config: &[String],
) -> Result<()> {
    let Some(ref descriptors) = metadata.settings else {
        if !config.is_empty() {
            fmt::warning("Plugin declares no settings; ignoring --config values.");
        }
        return Ok(());
    };

    if !config.is_empty() {
        for item in config {
            let Some((key, raw)) = item.split_once('=') else {
                return Err(crate::error::Error::Other(format!(
                    "invalid config format: {item}, expected key=value"
                )));
            };
            if !descriptors.contains_key(key) {
                fmt::warning(&format!("Unknown setting '{key}' for plugin '{}'.", metadata.name));
                continue;
            }
            let value = parse_setting_value(&metadata.name, key, raw);
            crate::plugin::set_plugin_setting(&metadata.name, key, value)?;
            eprintln!("    {key} = {raw}");
        }
        return Ok(());
    }

    // No CLI config: prompt for required settings that have no default and
    // are not configured yet.
    let interactive = {
        use std::io::IsTerminal;
        console::Term::stderr().is_term() && std::io::stdin().is_terminal()
    };
    let mut keys: Vec<&String> = descriptors.keys().collect();
    keys.sort();

    for key in keys {
        let descr = &descriptors[key];
        let required = descr.required.unwrap_or(false);
        if !required || descr.default.is_some() {
            continue;
        }
        if crate::plugin::get_plugin_setting(&metadata.name, key).is_some() {
            continue;
        }
        if !interactive {
            fmt::warning(&format!(
                "Setting '{key}' is required; configure it with: hy plugin config set {} {key} <value>",
                metadata.name
            ));
            continue;
        }

        let prompt = descr
            .description
            .as_deref()
            .filter(|d| !d.is_empty())
            .map(|d| format!("{key} ({d})"))
            .unwrap_or_else(|| key.clone());

        let value = match descr.setting_type.as_str() {
            "boolean" => serde_json::Value::Bool(tui::confirm(&prompt, false)),
            _ => {
                if let Some(ref choices) = descr.choices {
                    match tui::select(&prompt, choices, 0) {
                        Some(idx) => serde_json::Value::String(choices[idx].clone()),
                        None => continue,
                    }
                } else {
                    let answer = tui::input(&prompt, "");
                    if answer.is_empty() {
                        fmt::warning(&format!(
                            "Setting '{key}' left unset; configure it with: hy plugin config set {} {key} <value>",
                            metadata.name
                        ));
                        continue;
                    }
                    serde_json::Value::String(answer)
                }
            }
        };
        crate::plugin::set_plugin_setting(&metadata.name, key, value)?;
    }

    Ok(())
}

// ── plugin uninstall ────────────────────────────────────────────────────

async fn run_uninstall(args: PluginUninstallArgs) -> Result<()> {
    crate::plugin::uninstall(&args.name)?;
    fmt::success(&format!("Plugin '{}' uninstalled.", args.name));
    Ok(())
}

// ── plugin upgrade ──────────────────────────────────────────────────────

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

// ── plugin search ───────────────────────────────────────────────────────

async fn run_search(args: PluginSearchArgs) -> Result<()> {
    // Load the repo from the configured URL.
    let repo_url = match crate::plugin::get_repo_url() {
        Some(url) => url,
        None => {
            fmt::warning("No plugin repository configured.");
            fmt::info("Set a repository URL in ida-config.json at:");
            fmt::info("  Settings.plugin-repository.url");
            return Ok(());
        }
    };

    fmt::info(&format!("Fetching plugin repository from {repo_url}..."));

    let body = reqwest::get(&repo_url).await?.text().await?;
    let repo: RepoSnapshot = serde_json::from_str(&body).map_err(|e| {
        crate::error::Error::Other(format!("Failed to parse repository JSON: {e}"))
    })?;

    let installed = crate::plugin::installed_plugins()?;
    let installed_map: std::collections::HashMap<String, Option<String>> = installed
        .into_iter()
        .map(|p| (p.name, p.version))
        .collect();

    let _ida_version = crate::plugin::detect_current_ida_version();

    let matching: Vec<&RepoPlugin> = if let Some(ref query) = args.query {
        let q = query.to_lowercase();
        repo.plugins
            .iter()
            .filter(|p| does_plugin_match_query(&q, p))
            .collect()
    } else {
        repo.plugins.iter().collect()
    };

    if matching.is_empty() {
        if let Some(ref query) = args.query {
            fmt::info(&format!("No plugins matching '{query}'."));
        } else {
            fmt::info("No plugins found in the repository.");
        }
        return Ok(());
    }

    let mut table = tui::Table::new(&["Name", "Version", "Status", "Host"]);
    for plugin in &matching {
        let name = &plugin.name;
        let latest_version = plugin
            .versions
            .keys()
            .max()
            .map(|s| s.as_str())
            .unwrap_or("?");

        let status = match installed_map.get(name) {
            Some(Some(v)) => {
                if v.as_str() < latest_version {
                    format!("upgradable ({})", v).yellow().to_string()
                } else {
                    "installed".green().to_string()
                }
            }
            Some(None) => "installed".green().to_string(),
            None => "available".dimmed().to_string(),
        };

        table.add_row(vec![
            name.clone(),
            latest_version.to_string(),
            status,
            plugin.host.dimmed().to_string(),
        ]);
    }
    table.print();

    eprintln!();
    eprintln!("  {} plugin(s) found.", matching.len());

    Ok(())
}

fn does_plugin_match_query(query: &str, plugin: &RepoPlugin) -> bool {
    if plugin.name.to_lowercase().contains(query) {
        return true;
    }
    // Check across all version metadata.
    for locations in plugin.versions.values() {
        for loc in locations {
            let meta = &loc.metadata.plugin;
            if meta.description.to_lowercase().contains(query) {
                return true;
            }
            if let Some(ref cats) = meta.categories
                && cats.iter().any(|c| c.to_lowercase().contains(query)) {
                    return true;
                }
            if let Some(ref kws) = meta.keywords
                && kws.iter().any(|k| k.to_lowercase().contains(query)) {
                    return true;
                }
            if let Some(ref authors) = meta.authors
                && authors
                    .iter()
                    .any(|a| a.name.to_lowercase().contains(query))
                {
                    return true;
                }
        }
    }
    false
}

// ── plugin status ───────────────────────────────────────────────────────

async fn run_status(args: PluginStatusArgs) -> Result<()> {
    if let Some(name) = args.name {
        if crate::plugin::is_installed(&name) {
            match crate::plugin::read_installed_metadata(&name) {
                Ok(meta) => {
                    eprintln!("  Plugin: {} v{}", meta.name, meta.version);
                    if !meta.description.is_empty() {
                        eprintln!("  Description: {}", meta.description);
                    }
                    if let Some(ref platforms) = meta.platforms {
                        eprintln!("  Platforms: {}", platforms.join(", "));
                    }
                    if let Some(ref ida_versions) = meta.ida_versions {
                        eprintln!("  IDA versions: {}", ida_versions.join(", "));
                    }
                }
                Err(_) => {
                    eprintln!("  Plugin '{}' is installed (no metadata).", name);
                }
            }
        } else {
            fmt::info(&format!("Plugin '{}' is not installed.", name));
        }
    } else {
        let plugins = crate::plugin::installed_plugins()?;
        if plugins.is_empty() {
            fmt::info("No plugins installed.");
        } else {
            print_installed_table(&plugins);
            eprintln!();
            eprintln!("  {} plugin(s) installed.", plugins.len());
        }
    }
    Ok(())
}

// ── plugin lint ─────────────────────────────────────────────────────────

async fn run_lint(args: PluginLintArgs) -> Result<()> {
    let source_path = if args.path.starts_with("http://") || args.path.starts_with("https://") {
        fmt::info(&format!("Downloading plugin from {}...", args.path));
        let client = crate::api::ApiClient::new()?;
        let ts = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis();
        let temp_dir = std::env::temp_dir().join(format!("hy-plugin-lint-{ts}"));
        client
            .download_file(&args.path, &temp_dir, Some("plugin.zip"), false, false, None)
            .await?
    } else {
        let p = std::path::PathBuf::from(&args.path);
        if !p.exists() {
            fmt::error(&format!("File not found: {}", args.path));
            return Ok(());
        }
        p
    };

    let metadata = match crate::plugin::read_metadata_from_archive(&source_path) {
        Ok(m) => m,
        Err(e) => {
            if args.path.starts_with("http") {
                let _ = std::fs::remove_dir_all(source_path.parent().unwrap());
            }
            return Err(e);
        }
    };

    eprintln!("  Plugin: {} v{}", metadata.name, metadata.version);
    if !metadata.description.is_empty() {
        eprintln!("  Description: {}", metadata.description);
    }
    if let Some(ref ep) = metadata.entry_point {
        eprintln!("  Entry point: {ep}");
    }

    if let Some(ref ida_versions) = metadata.ida_versions {
        eprintln!("  IDA versions: {}", ida_versions.join(", "));
    }

    if let Some(ref deps) = metadata.python_dependencies
        && !deps.is_empty() {
            eprintln!("  Dependencies: {}", deps.join(", "));
        }

    // Recommendations.
    let mut warnings = Vec::new();

    if metadata.description.is_empty() {
        warnings.push("Missing description");
    }
    if metadata.categories.as_ref().is_none_or(|c| c.is_empty()) {
        warnings.push("No categories specified");
    }
    if metadata.keywords.as_ref().is_none_or(|k| k.is_empty()) {
        warnings.push("No keywords specified");
    }
    if metadata.ida_versions.as_ref().is_none_or(|v| v.is_empty()) {
        warnings.push("No IDA versions specified (will match all versions)");
    }
    if metadata.platforms.as_ref().is_none_or(|p| p.is_empty()) {
        warnings.push("No platforms specified (will match all platforms)");
    }
    if metadata.license.is_none() {
        warnings.push("No license specified");
    }
    if metadata.authors.is_none() && metadata.author.is_none() {
        warnings.push("No author information");
    }

    use owo_colors::OwoColorize;
    if warnings.is_empty() {
        fmt::success("Plugin archive looks good.");
    } else {
        eprintln!();
        fmt::warning("Recommendations:");
        for w in &warnings {
            eprintln!("    - {}", w.yellow());
        }
    }

    if args.path.starts_with("http") {
        let _ = std::fs::remove_dir_all(source_path.parent().unwrap());
    }

    Ok(())
}

// ── plugin repo ─────────────────────────────────────────────────────────

async fn run_repo(cmd: RepoCommands) -> Result<()> {
    match cmd {
        RepoCommands::List => {
            match crate::plugin::get_repo_url() {
                Some(url) => {
                    eprintln!("  Configured repository:");
                    eprintln!("    {url}");
                }
                None => {
                    fmt::info("No plugin repository configured.");
                    fmt::info("Set one in ida-config.json at Settings.plugin-repository.url");
                }
            }
            Ok(())
        }
        RepoCommands::Add(args) => {
            // Write to ida-config.json.
            let user_dir = crate::ida::ida_user_dir();
            let config_path = user_dir.join("ida-config.json");
            let mut config: serde_json::Value = if config_path.exists() {
                let text = std::fs::read_to_string(&config_path)?;
                serde_json::from_str(&text).unwrap_or(serde_json::json!({}))
            } else {
                serde_json::json!({})
            };

            let settings = config
                .as_object_mut()
                .unwrap()
                .entry("Settings")
                .or_insert(serde_json::json!({}));
            let repo = settings
                .as_object_mut()
                .unwrap()
                .entry("plugin-repository")
                .or_insert(serde_json::json!({}));
            repo.as_object_mut()
                .unwrap()
                .insert("url".to_owned(), serde_json::Value::String(args.url.clone()));

            if let Some(parent) = config_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(&config_path, serde_json::to_string_pretty(&config)?)?;

            fmt::success(&format!("Repository URL set to: {}", args.url));
            Ok(())
        }
        RepoCommands::Remove(_args) => {
            let user_dir = crate::ida::ida_user_dir();
            let config_path = user_dir.join("ida-config.json");
            if config_path.exists() {
                let text = std::fs::read_to_string(&config_path)?;
                let mut config: serde_json::Value =
                    serde_json::from_str(&text).unwrap_or(serde_json::json!({}));
                if let Some(settings) = config.get_mut("Settings")
                    && let Some(obj) = settings.as_object_mut() {
                        obj.remove("plugin-repository");
                    }
                std::fs::write(&config_path, serde_json::to_string_pretty(&config)?)?;
            }
            fmt::success("Repository configuration removed.");
            Ok(())
        }
        RepoCommands::Snapshot => {
            let repo_url = match crate::plugin::get_repo_url() {
                Some(url) => url,
                None => {
                    fmt::error("No plugin repository configured.");
                    return Ok(());
                }
            };

            let body = reqwest::get(&repo_url).await?.text().await?;
            // Output raw JSON to stdout (not stderr) for piping.
            println!("{body}");
            Ok(())
        }
    }
}

// ── plugin config ───────────────────────────────────────────────────────

async fn run_config(cmd: ConfigCommands) -> Result<()> {
    match cmd {
        ConfigCommands::Get(args) => {
            match crate::plugin::get_plugin_setting(&args.plugin, &args.key) {
                Some(val) => {
                    // Print booleans as "true"/"false", strings without quotes.
                    match val {
                        serde_json::Value::Bool(b) => eprintln!("{b}"),
                        serde_json::Value::String(s) => eprintln!("{s}"),
                        other => eprintln!("{other}"),
                    }
                }
                None => {
                    fmt::info(&format!(
                        "Setting '{}' not found for plugin '{}'.",
                        args.key, args.plugin
                    ));
                }
            }
            Ok(())
        }
        ConfigCommands::Set(args) => {
            // Try to parse the value based on plugin metadata descriptor type.
            let value = parse_setting_value(&args.plugin, &args.key, &args.value);
            crate::plugin::set_plugin_setting(&args.plugin, &args.key, value)?;
            fmt::success(&format!(
                "Set {}.{} = {}",
                args.plugin, args.key, args.value
            ));
            Ok(())
        }
        ConfigCommands::Del(args) => {
            crate::plugin::del_plugin_setting(&args.plugin, &args.key)?;
            fmt::success(&format!(
                "Deleted setting '{}' from plugin '{}'.",
                args.key, args.plugin
            ));
            Ok(())
        }
        ConfigCommands::List(args) => {
            let settings = crate::plugin::get_all_plugin_settings(&args.plugin);
            if settings.is_empty() {
                fmt::info(&format!("No settings for plugin '{}'.", args.plugin));
                return Ok(());
            }

            // Try to load metadata for descriptions.
            let metadata = crate::plugin::read_installed_metadata(&args.plugin).ok();
            let descriptors = metadata
                .as_ref()
                .and_then(|m| m.settings.as_ref());

            eprintln!(
                "  {:<24} {:<24} Description",
                "Key", "Value"
            );
            eprintln!("  {}", "-".repeat(74));

            for (key, val) in &settings {
                let val_str = match val {
                    serde_json::Value::Bool(b) => b.to_string(),
                    serde_json::Value::String(s) => s.clone(),
                    other => other.to_string(),
                };

                let desc = descriptors
                    .and_then(|d| d.get(key))
                    .and_then(|s| s.description.as_deref())
                    .unwrap_or("");

                eprintln!(
                    "  {:<24} {:<24} {}",
                    key,
                    val_str,
                    desc.dimmed()
                );
            }

            Ok(())
        }
        ConfigCommands::Export(args) => {
            let settings = crate::plugin::get_all_plugin_settings(&args.plugin);
            let json = serde_json::to_string_pretty(&settings)?;
            println!("{json}");
            Ok(())
        }
        ConfigCommands::Import(args) => {
            let json_str = match args.json {
                Some(s) => s,
                None => {
                    // Read from stdin.
                    use std::io::Read;
                    let mut buf = String::new();
                    std::io::stdin().read_to_string(&mut buf)?;
                    buf
                }
            };

            let map: serde_json::Map<String, serde_json::Value> =
                serde_json::from_str(&json_str).map_err(|e| {
                    crate::error::Error::Other(format!("Invalid JSON: {e}"))
                })?;

            for (key, value) in &map {
                crate::plugin::set_plugin_setting(&args.plugin, key, value.clone())?;
            }
            fmt::success(&format!(
                "Imported {} setting(s) for plugin '{}'.",
                map.len(),
                args.plugin
            ));
            Ok(())
        }
    }
}

/// Parse a setting value string into a JSON value, using plugin metadata hints if available.
fn parse_setting_value(
    plugin_name: &str,
    key: &str,
    raw: &str,
) -> serde_json::Value {
    // Try to load the setting descriptor for type info.
    if let Ok(metadata) = crate::plugin::read_installed_metadata(plugin_name)
        && let Some(ref settings) = metadata.settings
        && let Some(descriptor) = settings.get(key)
        && descriptor.setting_type.as_str() == "boolean"
    {
        let lower = raw.to_lowercase();
        return serde_json::Value::Bool(lower == "true" || lower == "1" || lower == "yes");
    }

    // Fallback: try JSON parsing, then treat as string.
    serde_json::from_str(raw).unwrap_or(serde_json::Value::String(raw.to_owned()))
}

// ── plugin bundle ───────────────────────────────────────────────────────

async fn run_bundle(cmd: BundleCommands) -> Result<()> {
    match cmd {
        BundleCommands::Info(args) => run_bundle_info(args),
        BundleCommands::Create(args) => run_bundle_create(args).await,
    }
}

fn run_bundle_info(args: BundleInfoArgs) -> Result<()> {
    use crate::plugin::bundle;

    if !bundle::is_plugin_bundle_zip(&args.bundle_path) {
        fmt::error(&format!(
            "{} is not a plugin bundle",
            args.bundle_path.display()
        ));
        return Ok(());
    }

    let manifest = bundle::read_bundle_manifest(&args.bundle_path)?;
    let plugins = bundle::list_bundle_plugins(&args.bundle_path)?;

    eprintln!("  {}: {}", "plugin bundle".bold(), args.bundle_path.display());
    eprintln!("    built: {}", manifest.built_at);
    eprintln!(
        "    created by: {} {}",
        manifest.created_by.tool, manifest.created_by.version
    );
    eprintln!(
        "    targets: {}",
        manifest
            .target_platform_tags
            .iter()
            .map(|t| t.id.clone())
            .collect::<Vec<_>>()
            .join(", ")
    );

    if plugins.is_empty() {
        eprintln!("    plugins: (none)");
    } else {
        eprintln!("    plugins: {}", plugins.len());
        for plugin in &plugins {
            eprintln!(
                "      {}: {}",
                plugin.metadata.name, plugin.metadata.version
            );
        }
    }
    Ok(())
}

/// Map a bundle platform (`linux-x86_64`) to the IDA plugin platform
/// identifier used in ida-plugin.json (`linux`).
fn bundle_platform_to_ida_platform(platform: &str) -> &'static str {
    match platform {
        "windows-x86_64" => "win",
        "linux-x86_64" => "linux",
        "macos-aarch64" => "macarm",
        "macos-x86_64" => "macx64",
        _ => "linux",
    }
}

fn detect_current_python_version() -> Result<String> {
    for name in &["python3", "python"] {
        if let Ok(output) = std::process::Command::new(name).arg("--version").output()
            && output.status.success() {
                let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
                // "Python 3.12.4" -> "3.12"
                if let Some(version) = text.strip_prefix("Python ") {
                    let parts: Vec<&str> = version.split('.').collect();
                    if parts.len() >= 2 {
                        return Ok(format!("{}.{}", parts[0], parts[1]));
                    }
                }
            }
    }
    Err(crate::error::Error::Other(
        "could not detect the current Python version; pass --python <version> explicitly".into(),
    ))
}

fn resolve_bundle_targets(args: &BundleCreateArgs) -> Result<Vec<crate::plugin::bundle::PipTarget>> {
    use crate::plugin::bundle::{
        current_bundle_platform, resolve_platform_alias, PipTarget, ALL_PLATFORMS,
        SUPPORTED_PYTHON_VERSIONS,
    };

    if !args.targets.is_empty() && (!args.platforms.is_empty() || !args.pythons.is_empty()) {
        return Err(crate::error::Error::Other(
            "--target cannot be combined with --platform or --python".into(),
        ));
    }

    if !args.targets.is_empty() {
        return args.targets.iter().map(|t| PipTarget::parse(t)).collect();
    }

    if args.platforms.is_empty() {
        return Err(crate::error::Error::Other(
            "--platform is required\n  use --platform current for this machine, or --platform all for all supported platforms".into(),
        ));
    }
    if args.pythons.is_empty() {
        return Err(crate::error::Error::Other(
            "--python is required\n  use --python current for this machine, or --python all for all supported versions".into(),
        ));
    }

    let mut platforms = Vec::new();
    for p in &args.platforms {
        match p.trim().to_lowercase().as_str() {
            "all" => platforms.extend(ALL_PLATFORMS.iter().map(|s| s.to_string())),
            "current" => platforms.push(current_bundle_platform().to_string()),
            _ => platforms.push(resolve_platform_alias(p)?),
        }
    }

    let mut pythons = Vec::new();
    for py in &args.pythons {
        match py.trim().to_lowercase().as_str() {
            "all" => pythons.extend(SUPPORTED_PYTHON_VERSIONS.iter().map(|s| s.to_string())),
            "current" => pythons.push(detect_current_python_version()?),
            other => pythons.push(other.to_string()),
        }
    }

    let mut seen = std::collections::HashSet::new();
    let mut targets = Vec::new();
    for platform in &platforms {
        for python in &pythons {
            let target = PipTarget::new(platform, python)?;
            if seen.insert(target.id()) {
                targets.push(target);
            }
        }
    }
    Ok(targets)
}

/// Load a repository snapshot for bundle spec resolution: from an explicit
/// `--repo` (URL or local JSON file) or the configured repository URL.
async fn load_repo_snapshot(repo_arg: Option<&str>) -> Result<Option<RepoSnapshot>> {
    let body = match repo_arg {
        Some(spec) if std::path::Path::new(spec).is_file() => {
            std::fs::read_to_string(spec)?
        }
        Some(url) => reqwest::get(url).await?.text().await?,
        None => match crate::plugin::get_repo_url() {
            Some(url) => reqwest::get(&url).await?.text().await?,
            None => return Ok(None),
        },
    };
    let snapshot: RepoSnapshot = serde_json::from_str(&body).map_err(|e| {
        crate::error::Error::Other(format!("Failed to parse repository JSON: {e}"))
    })?;
    Ok(Some(snapshot))
}

async fn run_bundle_create(args: BundleCreateArgs) -> Result<()> {
    use crate::plugin::bundle::{self, ResolvedPluginArchive};
    use sha2::{Digest, Sha256};

    let targets = resolve_bundle_targets(&args)?;

    eprintln!("  targets ({}):", targets.len());
    for target in &targets {
        eprintln!(
            "    {}  Python {}  ({})",
            target.ida_platform,
            target.python_version,
            target.id()
        );
    }

    let target_platforms: Vec<String> = {
        let mut platforms: Vec<String> =
            targets.iter().map(|t| t.ida_platform.clone()).collect();
        platforms.sort();
        platforms.dedup();
        platforms
    };

    // Resolve each spec to one or more plugin archives.
    let mut repo_snapshot: Option<Option<RepoSnapshot>> = None;
    let mut archives: Vec<ResolvedPluginArchive> = Vec::new();

    for spec in &args.plugin_specs {
        let spec_path = std::path::PathBuf::from(spec);
        if spec_path.is_file() && spec.ends_with(".zip") {
            let spinner = tui::spinner(format!("resolving {spec}"));
            let bytes = std::fs::read(&spec_path)?;
            let metadata = crate::plugin::read_metadata_from_archive(&spec_path)?;
            spinner.finish_and_clear();
            archives.push(ResolvedPluginArchive {
                name: metadata.name,
                version: metadata.version,
                bytes,
                platforms: Vec::new(),
            });
            continue;
        }

        // Repository spec: name==version[@host]
        let (bare_spec, host) = match spec.rsplit_once('@') {
            Some((s, h)) if !h.contains('=') => (s.to_string(), Some(h.to_string())),
            _ => (spec.clone(), None),
        };
        let Some((name, version)) = bare_spec.split_once("==") else {
            let example = match host {
                Some(ref h) => format!("{bare_spec}==1.0.0@{h}"),
                None => format!("{bare_spec}==1.0.0"),
            };
            return Err(crate::error::Error::Other(format!(
                "repository plugin specs must include exact version (e.g. {example})"
            )));
        };

        if repo_snapshot.is_none() {
            repo_snapshot = Some(load_repo_snapshot(args.repo.as_deref()).await?);
        }
        let Some(Some(ref snapshot)) = repo_snapshot else {
            return Err(crate::error::Error::Other(
                "no plugin repository available to resolve spec (configure one or pass --repo)".into(),
            ));
        };

        let plugin = snapshot
            .plugins
            .iter()
            .find(|p| p.name == name && host.as_ref().is_none_or(|h| &p.host == h))
            .ok_or_else(|| {
                crate::error::Error::Other(format!("plugin '{name}' not found in repository"))
            })?;
        let locations = plugin.versions.get(version).ok_or_else(|| {
            crate::error::Error::Other(format!("version {version} not found for plugin '{name}'"))
        })?;

        // Pick a compatible archive per target platform and dedupe by hash.
        let mut archives_by_hash: std::collections::HashMap<String, Vec<u8>> =
            std::collections::HashMap::new();
        let mut hash_by_platform: std::collections::HashMap<String, String> =
            std::collections::HashMap::new();

        for platform in &target_platforms {
            let ida_platform = bundle_platform_to_ida_platform(platform);
            let location = locations
                .iter()
                .find(|loc| {
                    loc.metadata
                        .plugin
                        .platforms
                        .as_ref()
                        .is_none_or(|p| {
                            p.is_empty() || p.iter().any(|x| x == ida_platform || x == "all")
                        })
                })
                .ok_or_else(|| {
                    crate::error::Error::Other(format!(
                        "no archive of '{name}=={version}' is compatible with {platform}"
                    ))
                })?;

            let spinner = tui::spinner(format!("resolving {spec} for {platform}"));
            let bytes = reqwest::get(&location.url).await?.bytes().await?.to_vec();
            spinner.finish_and_clear();

            let hash = format!("{:x}", Sha256::digest(&bytes));
            archives_by_hash.entry(hash.clone()).or_insert(bytes);
            hash_by_platform.insert(platform.clone(), hash);
        }

        let unique_hashes: std::collections::HashSet<&String> =
            hash_by_platform.values().collect();
        let needs_suffix = unique_hashes.len() > 1;

        for (hash, bytes) in archives_by_hash {
            let platforms = if needs_suffix {
                let mut p: Vec<String> = hash_by_platform
                    .iter()
                    .filter(|(_, h)| **h == hash)
                    .map(|(platform, _)| platform.clone())
                    .collect();
                p.sort();
                p
            } else {
                Vec::new()
            };
            archives.push(ResolvedPluginArchive {
                name: name.to_string(),
                version: version.to_string(),
                bytes,
                platforms,
            });
        }
    }

    let spinner = tui::spinner("building bundle");
    let spinner_ref = &spinner;
    bundle::create_bundle(&args.output, &archives, &targets, move |msg| {
        spinner_ref.set_message(msg.to_string());
    })?;
    spinner.finish_and_clear();

    fmt::success(&format!("Created plugin bundle: {}", args.output.display()));
    eprintln!("    plugins: {}", args.plugin_specs.len());
    eprintln!("    targets: {}", targets.len());
    for target in &targets {
        eprintln!("      {}  Python {}", target.ida_platform, target.python_version);
    }
    Ok(())
}

// ── Repo snapshot model ─────────────────────────────────────────────────

/// Simplified plugin repository snapshot model (matching the Python JSONFilePluginRepo format).
#[derive(Debug, Clone, serde::Deserialize)]
struct RepoSnapshot {
    #[serde(default)]
    plugins: Vec<RepoPlugin>,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct RepoPlugin {
    name: String,
    #[serde(default)]
    host: String,
    #[serde(default)]
    versions: std::collections::HashMap<String, Vec<RepoPluginArchiveLocation>>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, serde::Deserialize)]
struct RepoPluginArchiveLocation {
    #[serde(default)]
    url: String,
    #[serde(default)]
    sha256: Option<String>,
    metadata: RepoPluginManifest,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct RepoPluginManifest {
    plugin: RepoPluginMeta,
}

#[allow(dead_code)]
#[derive(Debug, Clone, serde::Deserialize)]
struct RepoPluginMeta {
    name: String,
    #[serde(default)]
    version: String,
    #[serde(default)]
    description: String,
    categories: Option<Vec<String>>,
    keywords: Option<Vec<String>>,
    authors: Option<Vec<RepoContact>>,
    maintainers: Option<Vec<RepoContact>>,
    #[serde(default)]
    ida_versions: Option<Vec<String>>,
    #[serde(default)]
    platforms: Option<Vec<String>>,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct RepoContact {
    name: String,
}
