//! `hy ke` command group: Knowledge Explorer management.

use std::path::PathBuf;

use clap::{Args, Subcommand};
use dialoguer::Select;
use owo_colors::OwoColorize;

use crate::config::ConfigStore;
use crate::error::Result;
use crate::util::fmt;

// ── CLI definitions ─────────────────────────────────────────────────────

#[derive(Debug, Subcommand)]
pub enum KeCommands {
    /// Register the ida:// protocol handler and discover IDA instances
    Setup(KeSetupArgs),
    /// Open Knowledge Explorer in the browser
    Open(KeOpenArgs),
    /// Manage IDA instances for KE
    #[command(subcommand)]
    Ida(KeIdaCommands),
    /// Manage KE knowledge sources
    #[command(subcommand)]
    Source(KeSourceCommands),
}

#[derive(Debug, Args)]
pub struct KeSetupArgs {
    /// Force re-registration
    #[arg(short, long)]
    pub force: bool,
    /// Unregister the protocol handler
    #[arg(long)]
    pub unregister: bool,
}

#[derive(Debug, Args)]
pub struct KeOpenArgs {
    /// URL to open (ida://source/path or https://ke.hex-rays.com)
    pub url: Option<String>,
}

#[derive(Debug, Subcommand)]
pub enum KeIdaCommands {
    /// List IDA instances registered with KE
    List,
    /// Add an IDA instance
    Add(KeIdaAddArgs),
    /// Remove an IDA instance
    Remove(KeIdaRemoveArgs),
    /// Switch the default IDA instance
    Switch(KeIdaSwitchArgs),
}

#[derive(Debug, Args)]
pub struct KeIdaAddArgs {
    /// Instance name
    pub name: Option<String>,
    /// Path to IDA installation
    pub path: Option<PathBuf>,
    /// Auto-discover IDA installations
    #[arg(long)]
    pub auto: bool,
}

#[derive(Debug, Args)]
pub struct KeIdaRemoveArgs {
    /// Name of the IDA instance to remove
    pub name: Option<String>,
    /// Remove all instances
    #[arg(long)]
    pub all: bool,
}

#[derive(Debug, Args)]
pub struct KeIdaSwitchArgs {
    /// Name of the instance to set as default
    pub name: Option<String>,
}

#[derive(Debug, Subcommand)]
pub enum KeSourceCommands {
    /// List configured sources
    List,
    /// Add a knowledge source
    Add(KeSourceAddArgs),
    /// Remove a knowledge source
    Remove(KeSourceRemoveArgs),
}

#[derive(Debug, Args)]
pub struct KeSourceAddArgs {
    /// Source name
    pub name: String,
    /// Source directory path
    pub path: PathBuf,
}

#[derive(Debug, Args)]
pub struct KeSourceRemoveArgs {
    /// Source name
    pub name: String,
}

// ── dispatch ────────────────────────────────────────────────────────────

pub async fn run(cmd: KeCommands) -> Result<()> {
    match cmd {
        KeCommands::Setup(args) => run_setup(args).await,
        KeCommands::Open(args) => run_open(args).await,
        KeCommands::Ida(cmd) => match cmd {
            KeIdaCommands::List => run_ida_list().await,
            KeIdaCommands::Add(args) => run_ida_add(args).await,
            KeIdaCommands::Remove(args) => run_ida_remove(args).await,
            KeIdaCommands::Switch(args) => run_ida_switch(args).await,
        },
        KeCommands::Source(cmd) => match cmd {
            KeSourceCommands::List => run_source_list().await,
            KeSourceCommands::Add(args) => run_source_add(args).await,
            KeSourceCommands::Remove(args) => run_source_remove(args).await,
        },
    }
}

// ── ke setup ────────────────────────────────────────────────────────────

async fn run_setup(args: KeSetupArgs) -> Result<()> {
    if args.unregister {
        crate::ida::unregister_protocol_handler()?;
        fmt::success("Protocol handler unregistered.");
        return Ok(());
    }

    let exe = crate::util::io::executable_path();
    let exe_str = exe.to_string_lossy();
    crate::ida::register_protocol_handler(&exe_str)?;
    fmt::success("Registered ida:// protocol handler.");

    // Auto-discover and register IDA installations.
    let installations = crate::ida::find_standard_installations();
    if installations.is_empty() {
        fmt::info("No IDA installations found in standard locations.");
        fmt::info("Use `hy ke ida add` to manually register an installation.");
        return Ok(());
    }

    let valid: Vec<_> = installations
        .iter()
        .filter(|p| is_ida_dir(p))
        .cloned()
        .collect();

    if valid.is_empty() {
        fmt::info("Found installations but none appear to be valid IDA directories.");
        return Ok(());
    }

    eprintln!();
    eprintln!("  Found {} IDA installation(s):", valid.len());
    let mut last_name = String::new();
    for p in &valid {
        let name = generate_instance_name(p);
        let store = ConfigStore::global();
        let instances = store.get_string_map("ke.ida.instances");
        if !instances.contains_key(&name) || args.force {
            drop(store);
            let mut store = ConfigStore::global();
            let mut instances = store.get_string_map("ke.ida.instances");
            let path_str = p.to_string_lossy().to_string();
            instances.insert(name.clone(), path_str);
            store.set_nested(
                "ke.ida.instances",
                serde_json::to_value(&instances).unwrap(),
            );
            eprintln!("    + {:<24} {}", name, p.display().to_string().dimmed());
        } else {
            eprintln!(
                "    = {:<24} {} (already registered)",
                name,
                p.display().to_string().dimmed()
            );
        }
        last_name = name;
    }

    // Set default to last alphabetically sorted instance if none set.
    {
        let store = ConfigStore::global();
        let needs_default =
            store.get_nested_str("ke.ida.default").is_none() && !last_name.is_empty();
        drop(store);
        if needs_default {
            let mut store = ConfigStore::global();
            let instances = store.get_string_map("ke.ida.instances");
            let mut names: Vec<String> = instances.keys().cloned().collect();
            names.sort();
            if let Some(default_name) = names.last() {
                store.set_nested(
                    "ke.ida.default",
                    serde_json::Value::String(default_name.clone()),
                );
                eprintln!();
                fmt::success(&format!("Default IDA instance set to: {default_name}"));
            }
        }
    }

    Ok(())
}

// ── ke open ─────────────────────────────────────────────────────────────

async fn run_open(args: KeOpenArgs) -> Result<()> {
    let url_str = args
        .url
        .unwrap_or_else(|| "https://ke.hex-rays.com".to_string());

    // If it's an HTTP URL, just open in browser.
    if url_str.starts_with("http://") || url_str.starts_with("https://") {
        crate::util::io::open_url(&url_str);
        fmt::info("Opening Knowledge Explorer in the browser...");
        return Ok(());
    }

    // Parse ida:// URL.
    if !url_str.starts_with("ida://") {
        fmt::error(&format!("Invalid URL scheme: {url_str}"));
        fmt::info("Expected ida://source_name/file/path");
        return Ok(());
    }

    let without_scheme = &url_str["ida://".len()..];
    let (source_name, file_path) = match without_scheme.find('/') {
        Some(idx) => (&without_scheme[..idx], &without_scheme[idx + 1..]),
        None => (without_scheme, ""),
    };

    // Look up source.
    let store = ConfigStore::global();
    let sources = store.get_string_map("ke.sources");
    let source_path = match sources.get(source_name) {
        Some(p) => PathBuf::from(p),
        None => {
            fmt::error(&format!("Unknown source: {source_name}"));
            fmt::info("Use `hy ke source list` to see configured sources.");
            return Ok(());
        }
    };

    let full_path = source_path.join(file_path);
    if !full_path.exists() {
        fmt::error(&format!("File not found: {}", full_path.display()));
        return Ok(());
    }

    // Find IDA binary.
    let ida_bin = resolve_ida_binary(&store);
    drop(store);

    let ida_bin = match ida_bin {
        Some(p) => p,
        None => {
            fmt::error("No IDA installation found.");
            fmt::info("Use `hy ke ida add` to register an IDA installation.");
            return Ok(());
        }
    };

    // Log the URL resolution.
    if let Ok(mut log) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open("/tmp/hy_urls.log")
    {
        use std::io::Write;
        let _ = writeln!(
            log,
            "{} -> {} (ida: {})",
            url_str,
            full_path.display(),
            ida_bin.display()
        );
    }

    // Launch IDA.
    fmt::info(&format!(
        "Opening {} with {}",
        full_path.display(),
        ida_bin.display()
    ));

    if let Err(e) = crate::ida::launch_ida(&ida_bin, Some(&full_path)) {
        fmt::error(&format!("Failed to launch IDA: {e}"));
    }

    Ok(())
}

/// Resolve the IDA binary to use: first check KE default instance, then standard discovery.
fn resolve_ida_binary(store: &ConfigStore) -> Option<PathBuf> {
    // Try KE default instance.
    if let Some(default_name) = store.get_nested_str("ke.ida.default") {
        let instances = store.get_string_map("ke.ida.instances");
        if let Some(path_str) = instances.get(default_name) {
            let install_dir = PathBuf::from(path_str);
            if let Some(bin) = crate::ida::ida_binary_path(&install_dir) {
                return Some(bin);
            }
        }
    }

    // Fall back to current IDA install dir.
    crate::ida::current_install_dir()
        .and_then(|d| crate::ida::ida_binary_path(&d))
}

// ── ke ida list ─────────────────────────────────────────────────────────

async fn run_ida_list() -> Result<()> {
    let store = ConfigStore::global();
    let instances = store.get_string_map("ke.ida.instances");
    let default_name = store
        .get_nested_str("ke.ida.default")
        .unwrap_or("")
        .to_owned();

    if instances.is_empty() {
        fmt::info("No IDA instances registered.");
        fmt::info("Use `hy ke ida add --auto` to discover installations.");
        return Ok(());
    }

    let mut names: Vec<_> = instances.keys().collect();
    names.sort();

    let mut valid_count = 0;

    let mut table = crate::util::tui::Table::new(&["Name", "Path", "Status", "Default"]);
    for name in &names {
        let path = &instances[*name];
        let path_buf = PathBuf::from(path);
        let status_str = if !path_buf.exists() {
            "Missing".red().to_string()
        } else if is_ida_dir(&path_buf) {
            valid_count += 1;
            "Valid".green().to_string()
        } else {
            "Invalid".yellow().to_string()
        };

        let is_default = if *name == &default_name { "*" } else { "" };

        table.add_row(vec![
            (*name).clone(),
            path.dimmed().to_string(),
            status_str,
            is_default.to_string(),
        ]);
    }
    table.print();

    eprintln!();
    eprintln!(
        "  {valid_count}/{} valid instance(s).",
        instances.len()
    );

    Ok(())
}

// ── ke ida add ──────────────────────────────────────────────────────────

async fn run_ida_add(args: KeIdaAddArgs) -> Result<()> {
    if args.auto {
        return run_ida_add_auto().await;
    }

    let (name, path) = match (args.name, args.path) {
        (Some(n), Some(p)) => (n, p),
        _ => {
            fmt::error("Both NAME and PATH are required (or use --auto).");
            return Ok(());
        }
    };

    let path = path.canonicalize().unwrap_or(path);
    if !path.exists() || !path.is_dir() {
        fmt::error(&format!("Not a valid directory: {}", path.display()));
        return Ok(());
    }

    if !is_ida_dir(&path) {
        fmt::warning(&format!(
            "{} does not appear to be a valid IDA installation.",
            path.display()
        ));
    }

    let mut store = ConfigStore::global();
    let mut instances = store.get_string_map("ke.ida.instances");
    if instances.contains_key(&name) {
        fmt::warning(&format!("Instance '{name}' already exists, updating path."));
    }
    instances.insert(name.clone(), path.to_string_lossy().to_string());
    store.set_nested(
        "ke.ida.instances",
        serde_json::to_value(&instances).unwrap(),
    );

    fmt::success(&format!("Added instance '{name}' -> {}", path.display()));

    // Set as default if this is the only one.
    if instances.len() == 1 {
        store.set_nested(
            "ke.ida.default",
            serde_json::Value::String(name.clone()),
        );
        fmt::info(&format!("Set '{name}' as default."));
    }

    Ok(())
}

async fn run_ida_add_auto() -> Result<()> {
    let installations = crate::ida::find_standard_installations();
    let valid: Vec<_> = installations
        .iter()
        .filter(|p| is_ida_dir(p))
        .cloned()
        .collect();

    if valid.is_empty() {
        fmt::info("No valid IDA installations found in standard locations.");
        return Ok(());
    }

    // Display found installations.
    eprintln!("  Found {} IDA installation(s):", valid.len());
    eprintln!();

    let items: Vec<String> = valid
        .iter()
        .map(|p| {
            format!(
                "{:<24} {}",
                generate_instance_name(p),
                p.display()
            )
        })
        .collect();

    // Use multi-select if dialoguer supports it, otherwise add all.
    let selections: Vec<usize> = dialoguer::MultiSelect::new()
        .with_prompt("Select installations to register")
        .items(&items)
        .defaults(&vec![true; items.len()])
        .interact()
        .unwrap_or_else(|_| (0..items.len()).collect());

    if selections.is_empty() {
        fmt::info("No installations selected.");
        return Ok(());
    }

    let mut store = ConfigStore::global();
    let mut instances = store.get_string_map("ke.ida.instances");
    let mut last_name = String::new();

    for &idx in &selections {
        let p = &valid[idx];
        let name = generate_instance_name(p);
        instances.insert(name.clone(), p.to_string_lossy().to_string());
        fmt::success(&format!("  + {name}"));
        last_name = name;
    }

    store.set_nested(
        "ke.ida.instances",
        serde_json::to_value(&instances).unwrap(),
    );

    // Set default to last alphabetically if none set.
    if store.get_nested_str("ke.ida.default").is_none() && !last_name.is_empty() {
        let mut names: Vec<String> = instances.keys().cloned().collect();
        names.sort();
        if let Some(default_name) = names.last() {
            store.set_nested(
                "ke.ida.default",
                serde_json::Value::String(default_name.clone()),
            );
            fmt::info(&format!("Default set to: {default_name}"));
        }
    }

    Ok(())
}

// ── ke ida remove ───────────────────────────────────────────────────────

async fn run_ida_remove(args: KeIdaRemoveArgs) -> Result<()> {
    let mut store = ConfigStore::global();

    if args.all {
        store.set_nested(
            "ke.ida.instances",
            serde_json::Value::Object(Default::default()),
        );
        store.remove_nested("ke.ida.default");
        fmt::success("All IDA instances removed.");
        return Ok(());
    }

    let name = match args.name {
        Some(n) => n,
        None => {
            let instances = store.get_string_map("ke.ida.instances");
            if instances.is_empty() {
                fmt::info("No instances to remove.");
                return Ok(());
            }
            let mut names: Vec<String> = instances.keys().cloned().collect();
            names.sort();
            let selection = Select::new()
                .with_prompt("Select instance to remove")
                .items(&names)
                .interact()
                .map_err(|_| crate::error::Error::Other("Cancelled".into()))?;
            names[selection].clone()
        }
    };

    let mut instances = store.get_string_map("ke.ida.instances");
    if instances.remove(&name).is_none() {
        fmt::error(&format!("Instance '{name}' not found."));
        return Ok(());
    }

    store.set_nested(
        "ke.ida.instances",
        serde_json::to_value(&instances).unwrap(),
    );

    // Handle default reassignment.
    let was_default = store
        .get_nested_str("ke.ida.default")
        .map(|s| s == name)
        .unwrap_or(false);

    if was_default {
        if instances.is_empty() {
            store.remove_nested("ke.ida.default");
        } else {
            let mut names: Vec<String> = instances.keys().cloned().collect();
            names.sort();
            let new_default = names.last().unwrap().clone();
            store.set_nested(
                "ke.ida.default",
                serde_json::Value::String(new_default.clone()),
            );
            fmt::info(&format!("Default switched to: {new_default}"));
        }
    }

    fmt::success(&format!("Removed instance '{name}'."));
    Ok(())
}

// ── ke ida switch ───────────────────────────────────────────────────────

async fn run_ida_switch(args: KeIdaSwitchArgs) -> Result<()> {
    let mut store = ConfigStore::global();
    let instances = store.get_string_map("ke.ida.instances");

    if instances.is_empty() {
        fmt::info("No IDA instances registered.");
        return Ok(());
    }

    let name = match args.name {
        Some(n) => {
            if !instances.contains_key(&n) {
                fmt::error(&format!("Instance '{n}' not found."));
                return Ok(());
            }
            n
        }
        None => {
            let default_name = store
                .get_nested_str("ke.ida.default")
                .unwrap_or("")
                .to_owned();
            let mut names: Vec<String> = instances.keys().cloned().collect();
            names.sort();
            let items: Vec<String> = names
                .iter()
                .map(|n| {
                    if *n == default_name {
                        format!("{n} (current)")
                    } else {
                        n.clone()
                    }
                })
                .collect();

            let selection = Select::new()
                .with_prompt("Select default IDA instance")
                .items(&items)
                .interact()
                .map_err(|_| crate::error::Error::Other("Cancelled".into()))?;
            names[selection].clone()
        }
    };

    store.set_nested(
        "ke.ida.default",
        serde_json::Value::String(name.clone()),
    );
    fmt::success(&format!("Default IDA instance set to: {name}"));
    Ok(())
}

// ── ke source list ──────────────────────────────────────────────────────

async fn run_source_list() -> Result<()> {
    let store = ConfigStore::global();
    let sources = store.get_string_map("ke.sources");

    if sources.is_empty() {
        fmt::info("No knowledge sources configured.");
        fmt::info("Use `hy ke source add <name> <path>` to add one.");
        return Ok(());
    }

    let mut names: Vec<_> = sources.keys().collect();
    names.sort();

    eprintln!("  {:<24} Path", "Name");
    eprintln!("  {}", "-".repeat(74));
    for name in &names {
        eprintln!("  {:<24} {}", name, sources[*name].dimmed());
    }

    Ok(())
}

// ── ke source add ───────────────────────────────────────────────────────

async fn run_source_add(args: KeSourceAddArgs) -> Result<()> {
    let path = args.path.canonicalize().unwrap_or(args.path);
    if !path.exists() || !path.is_dir() {
        fmt::error(&format!("Not a valid directory: {}", path.display()));
        return Ok(());
    }

    let mut store = ConfigStore::global();
    let mut sources = store.get_string_map("ke.sources");

    if sources.contains_key(&args.name) {
        fmt::warning(&format!(
            "Source '{}' already exists, updating path.",
            args.name
        ));
    }

    sources.insert(args.name.clone(), path.to_string_lossy().to_string());
    store.set_nested("ke.sources", serde_json::to_value(&sources).unwrap());

    fmt::success(&format!(
        "Added source '{}' -> {}",
        args.name,
        path.display()
    ));
    Ok(())
}

// ── ke source remove ────────────────────────────────────────────────────

async fn run_source_remove(args: KeSourceRemoveArgs) -> Result<()> {
    let mut store = ConfigStore::global();
    let mut sources = store.get_string_map("ke.sources");

    if sources.remove(&args.name).is_none() {
        fmt::error(&format!("Source '{}' not found.", args.name));
        return Ok(());
    }

    store.set_nested("ke.sources", serde_json::to_value(&sources).unwrap());
    fmt::success(&format!("Removed source '{}'.", args.name));
    Ok(())
}

// ── helpers ─────────────────────────────────────────────────────────────

/// Check if a directory contains a valid IDA binary.
fn is_ida_dir(path: &std::path::Path) -> bool {
    crate::ida::ida_binary_path(path).is_some()
}

/// Generate a short instance name from an IDA installation path.
fn generate_instance_name(path: &std::path::Path) -> String {
    let name = path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    // Strip .app suffix on macOS.
    let name = name.strip_suffix(".app").unwrap_or(&name);
    name.to_lowercase()
        .replace(' ', "-")
        .replace("ida-professional", "ida-pro")
}
