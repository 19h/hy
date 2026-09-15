//! Registered IDA instances shared by ida and the legacy ke ida aliases.

use std::path::{Path, PathBuf};

use clap::{Args, Subcommand};
use indexmap::IndexMap;
use serde_json::{Value, json};

use crate::config::ConfigStore;
use crate::error::{Error, Result};
use crate::ida;
use crate::util::{fmt, tui};

type Instances = IndexMap<String, String>;

mod listing;
mod registration;
mod setup;

pub(super) use setup::ensure_registered;

#[derive(Debug, Subcommand)]
pub enum Commands {
    /// List registered IDA installations
    List,
    /// Register an IDA installation
    Add(AddArgs),
    /// Remove registered IDA installations
    Remove(RemoveArgs),
    /// Switch the default IDA installation
    Switch(SwitchArgs),
}

#[derive(Debug, Args)]
pub struct AddArgs {
    /// Instance name
    pub name: Option<String>,
    /// Path to IDA installation
    pub path: Option<PathBuf>,
    /// Auto-discover IDA installations
    #[arg(long)]
    pub auto: bool,
}

#[derive(Debug, Args)]
pub struct RemoveArgs {
    /// Name of the IDA instance to remove
    #[arg(required_unless_present = "all", conflicts_with = "all")]
    pub name: Option<String>,
    /// Remove all instances
    #[arg(long)]
    pub all: bool,
}

#[derive(Debug, Args)]
pub struct SwitchArgs {
    /// Name of the instance to set as default
    pub name: Option<String>,
}

pub(super) async fn run(command: Commands) -> Result<()> {
    match command {
        Commands::List => list(),
        Commands::Add(args) => registration::add(args).await,
        Commands::Remove(args) => remove(args),
        Commands::Switch(args) => switch(args),
    }
}

fn instances() -> Result<Instances> {
    ConfigStore::global()
        .get_value("ida.instances")
        .filter(|value| !value.is_null())
        .cloned()
        .map(serde_json::from_value)
        .transpose()
        .map(|value| value.unwrap_or_default())
        .map_err(Error::from)
}

#[derive(Clone, Copy, Default, Eq, Ord, PartialEq, PartialOrd)]
struct Version {
    major: u64,
    minor: u64,
}

impl Version {
    fn detect(name: &str, path: &str) -> Option<Self> {
        let version = ida::instance_version(Path::new(path), name)?;
        let (major, minor) = version.split_once('.')?;
        Some(Self {
            major: major.parse().ok()?,
            minor: minor.parse().ok()?,
        })
    }
}

fn best_default(instances: &Instances) -> Option<String> {
    instances
        .iter()
        .map(|(name, path)| (Version::detect(name, path), name))
        .max()
        .map(|(_, name)| name.clone())
}

fn list() -> Result<()> {
    let instances = instances()?;
    let default = ConfigStore::global().get_str("ida.default").unwrap_or("").to_owned();
    if instances.is_empty() {
        fmt::info("No IDA instances registered.");
        return Ok(());
    }
    listing::print(&instances, &default);
    Ok(())
}

fn remove(args: RemoveArgs) -> Result<()> {
    let mut instances = instances()?;
    if instances.is_empty() {
        fmt::info("No IDA instances registered.");
        return Ok(());
    }
    if args.all {
        ConfigStore::global().commit_changes([
            ("ida.instances".into(), Some(json!({}))),
            ("ida.default".into(), None),
        ])?;
        fmt::success("All IDA instances removed.");
        return Ok(());
    }
    let name = args.name.ok_or_else(|| Error::Other("an instance name is required".into()))?;
    if instances.shift_remove(&name).is_none() {
        return Err(Error::NotFound(format!("Instance '{name}'")));
    }
    let mut changes = vec![("ida.instances".into(), Some(serde_json::to_value(&instances)?))];
    let mut store = ConfigStore::global();
    if store.get_str("ida.default") == Some(&name) {
        changes.push(("ida.default".into(), best_default(&instances).map(Value::String)));
    }
    store.commit_changes(changes)?;
    fmt::success(&format!("Removed instance '{name}'."));
    Ok(())
}

fn switch(args: SwitchArgs) -> Result<()> {
    let instances = instances()?;
    if instances.is_empty() {
        return Err(Error::NotFound("No IDA instances registered.".into()));
    }
    let default = ConfigStore::global().get_str("ida.default").unwrap_or("").to_owned();
    let name = if let Some(name) = args.name.filter(|name| !name.is_empty()) {
        name
    } else {
        let labels: Vec<_> = instances
            .iter()
            .map(|(name, path)| {
                format!(
                    "{name} ({path}){}",
                    if *name == default {
                        " [CURRENT DEFAULT]"
                    } else {
                        ""
                    }
                )
            })
            .collect();
        let index = instances.get_index_of(&default).unwrap_or(0);
        let selected = dialoguer::Select::with_theme(&tui::theme())
            .with_prompt("Select default IDA instance")
            .items(&labels)
            .default(index)
            .interact_opt()
            .map_err(|error| Error::Other(error.to_string()))?;
        let Some(selected) = selected else {
            return Ok(());
        };
        instances.get_index(selected).expect("selection index belongs to the inventory").0.clone()
    };
    let path = instances.get(&name).ok_or_else(|| Error::NotFound(format!("Instance '{name}'")))?;
    ConfigStore::global().set_str("ida.default", &name)?;
    if ida::is_idalib_capable(Path::new(path)) {
        crate::plugin::set_ida_installation_directory(Path::new(path))?;
    }
    fmt::success(&format!("Default IDA instance set to: {name}"));
    Ok(())
}
