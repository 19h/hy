//! Ordered source registrations shared by ida source and the legacy KE alias.

use std::path::PathBuf;

use clap::{Args, Subcommand};
use indexmap::IndexMap;

use crate::config::{ConfigStore, Env};
use crate::error::{Error, Result};
use crate::util::fmt;

type Sources = IndexMap<String, String>;

#[derive(Debug, Subcommand)]
pub enum Commands {
    /// List configured sources
    List,
    /// Add a directory for IDB lookup
    Add(AddArgs),
    /// Remove a configured source
    Remove {
        /// Source name
        name: String,
    },
}

#[derive(Debug, Args)]
pub struct AddArgs {
    /// Lowercase alphanumeric source name, optionally containing hyphens
    pub name: String,
    /// Directory to search for IDB files
    pub path: PathBuf,
    /// Replace an existing registration
    #[arg(long)]
    pub force: bool,
}

pub fn run(command: Commands) -> Result<()> {
    match command {
        Commands::List => list(),
        Commands::Add(args) => add(args),
        Commands::Remove {
            name,
        } => remove(&name),
    }
}

fn sources() -> Result<Sources> {
    ConfigStore::global()
        .get_value("idb.sources")
        .filter(|value| !value.is_null())
        .cloned()
        .map(serde_json::from_value)
        .transpose()
        .map(|value| value.unwrap_or_default())
        .map_err(Error::from)
}

fn list() -> Result<()> {
    let sources = sources()?;
    if sources.is_empty() {
        fmt::warning("No sources configured.");
        fmt::info(&format!(
            "Add sources with: {} ida source add <name> <path>",
            Env::global().binary_name
        ));
        return Ok(());
    }
    println!("Sources ({}):", sources.len());
    for (name, path) in sources {
        let status = if std::path::Path::new(&path).exists() {
            ""
        } else {
            " (not found)"
        };
        println!("  {name} -> {path}{status}");
    }
    Ok(())
}

fn add(args: AddArgs) -> Result<()> {
    // Click's exists=True path conversion precedes the upstream command body.
    if !args.path.try_exists()? {
        return Err(Error::NotFound(format!("Path does not exist: {}", args.path.display())));
    }
    if args.name == "localhost" {
        return Err(Error::Other("Reserved source name: 'localhost'".into()));
    }
    if !valid_name(&args.name) {
        return Err(Error::Other("source name must match [a-z0-9][a-z0-9-]*".into()));
    }
    let path = crate::util::files::absolute_path(&args.path)?.canonicalize()?;
    if !path.is_dir() {
        return Err(Error::Other(format!("Path is not a directory: {}", path.display())));
    }
    let mut sources = sources()?;
    if let Some(existing) = sources.get(&args.name)
        && !args.force
    {
        fmt::warning(&format!("Source '{}' already exists: {existing}", args.name));
        fmt::info("Use --force to overwrite.");
        return Ok(());
    }
    sources.insert(args.name.clone(), path.to_string_lossy().into_owned());
    ConfigStore::global().set_value("idb.sources", serde_json::to_value(sources)?)?;
    fmt::success(&format!("Added source '{}': {}", args.name, path.display()));
    Ok(())
}

fn valid_name(name: &str) -> bool {
    // Python's $ also accepts one final newline; preserve the upstream grammar.
    let name = name.strip_suffix('\n').unwrap_or(name);
    let alphanumeric = |byte: u8| byte.is_ascii_lowercase() || byte.is_ascii_digit();
    name.bytes().next().is_some_and(alphanumeric)
        && name.bytes().all(|byte| alphanumeric(byte) || byte == b'-')
}

fn remove(name: &str) -> Result<()> {
    let mut sources = sources()?;
    if sources.shift_remove(name).is_none() {
        fmt::warning(&format!("Source not found: '{name}'"));
        fmt::info(&format!(
            "Use `{} ida source list` to see configured sources.",
            Env::global().binary_name
        ));
        return Ok(());
    }
    ConfigStore::global().set_value("idb.sources", serde_json::to_value(sources)?)?;
    fmt::success(&format!("Removed source: '{name}'"));
    Ok(())
}
