//! `hy plugin` command group: install, uninstall, list, search, upgrade, etc.

use std::path::PathBuf;

use clap::{Args, Subcommand};
use owo_colors::OwoColorize;

use crate::error::Result;
use crate::util::fmt;

// ── CLI definitions ─────────────────────────────────────────────────────

#[derive(Debug, Subcommand)]
pub enum PluginCommands {
    /// Install a plugin from a zip archive or URL
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
        PluginCommands::Repo { command } => run_repo(command).await,
        PluginCommands::Config { command } => run_config(command).await,
    }
}

// ── plugin list ─────────────────────────────────────────────────────────

async fn run_list() -> Result<()> {
    let plugins = crate::plugin::installed_plugins()?;
    if plugins.is_empty() {
        fmt::warning("No plugins installed.");
        return Ok(());
    }

    eprintln!("  {:<30} {}", "Name", "Version");
    eprintln!("  {}", "-".repeat(50));
    for (name, ver) in &plugins {
        eprintln!(
            "  {:<30} {}",
            name,
            ver.as_deref().unwrap_or("unknown").dimmed()
        );
    }
    Ok(())
}

// ── plugin install ──────────────────────────────────────────────────────

async fn run_install(args: PluginInstallArgs) -> Result<()> {
    let path = PathBuf::from(&args.source);
    if !path.exists() {
        fmt::error(&format!("File not found: {}", args.source));
        return Ok(());
    }

    let ida_version = crate::ida::current_install_dir()
        .as_deref()
        .and_then(crate::ida::detect_ida_version);

    let result =
        crate::plugin::install_from_archive(&path, ida_version.as_deref(), args.force)?;
    fmt::success(&format!("Plugin installed at: {}", result.display()));
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
    let installed_map: std::collections::HashMap<String, Option<String>> =
        installed.into_iter().collect();

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

    eprintln!(
        "  {:<24} {:<12} {:<14} {}",
        "Name", "Version", "Status", "URL"
    );
    eprintln!("  {}", "-".repeat(80));

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

        let host = &plugin.host;

        eprintln!("  {:<24} {:<12} {:<14} {}", name, latest_version, status, host.dimmed());
    }

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
            if let Some(ref cats) = meta.categories {
                if cats.iter().any(|c| c.to_lowercase().contains(query)) {
                    return true;
                }
            }
            if let Some(ref kws) = meta.keywords {
                if kws.iter().any(|k| k.to_lowercase().contains(query)) {
                    return true;
                }
            }
            if let Some(ref authors) = meta.authors {
                if authors
                    .iter()
                    .any(|a| a.name.to_lowercase().contains(query))
                {
                    return true;
                }
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
            eprintln!("  {:<30} {}", "Name", "Version");
            eprintln!("  {}", "-".repeat(50));
            for (name, ver) in &plugins {
                eprintln!(
                    "  {:<30} {}",
                    name,
                    ver.as_deref().unwrap_or("unknown").dimmed()
                );
            }
            eprintln!();
            eprintln!("  {} plugin(s) installed.", plugins.len());
        }
    }
    Ok(())
}

// ── plugin lint ─────────────────────────────────────────────────────────

async fn run_lint(args: PluginLintArgs) -> Result<()> {
    let metadata = crate::plugin::read_metadata_from_archive(&args.path)?;

    eprintln!("  Plugin: {} v{}", metadata.name, metadata.version);
    if !metadata.description.is_empty() {
        eprintln!("  Description: {}", metadata.description);
    }
    if let Some(ref ep) = metadata.entry_point {
        eprintln!("  Entry point: {ep}");
    }

    // Recommendations.
    let mut warnings = Vec::new();

    if metadata.description.is_empty() {
        warnings.push("Missing description");
    }
    if metadata.categories.as_ref().map_or(true, |c| c.is_empty()) {
        warnings.push("No categories specified");
    }
    if metadata.keywords.as_ref().map_or(true, |k| k.is_empty()) {
        warnings.push("No keywords specified");
    }
    if metadata.ida_versions.as_ref().map_or(true, |v| v.is_empty()) {
        warnings.push("No IDA versions specified (will match all versions)");
    }
    if metadata.platforms.as_ref().map_or(true, |p| p.is_empty()) {
        warnings.push("No platforms specified (will match all platforms)");
    }
    if metadata.license.is_none() {
        warnings.push("No license specified");
    }
    if metadata.authors.is_none() && metadata.author.is_none() {
        warnings.push("No author information");
    }

    if warnings.is_empty() {
        fmt::success("Plugin archive looks good.");
    } else {
        eprintln!();
        fmt::warning("Recommendations:");
        for w in &warnings {
            eprintln!("    - {}", w.yellow());
        }
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
                if let Some(settings) = config.get_mut("Settings") {
                    if let Some(obj) = settings.as_object_mut() {
                        obj.remove("plugin-repository");
                    }
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
                "  {:<24} {:<24} {}",
                "Key", "Value", "Description"
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
    if let Ok(metadata) = crate::plugin::read_installed_metadata(plugin_name) {
        if let Some(ref settings) = metadata.settings {
            if let Some(descriptor) = settings.get(key) {
                match descriptor.setting_type.as_str() {
                    "boolean" => {
                        let lower = raw.to_lowercase();
                        return serde_json::Value::Bool(
                            lower == "true" || lower == "1" || lower == "yes",
                        );
                    }
                    _ => {}
                }
            }
        }
    }

    // Fallback: try JSON parsing, then treat as string.
    serde_json::from_str(raw).unwrap_or(serde_json::Value::String(raw.to_owned()))
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
