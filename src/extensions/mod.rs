//! Python entry-point compatibility while native commands retain Rust dispatch.

mod process;

use std::sync::OnceLock;

use serde::Deserialize;

static CATALOG: OnceLock<Catalog> = OnceLock::new();

#[derive(Debug, Default, Deserialize)]
pub struct Catalog {
    pub extensions: Vec<Extension>,
    pub commands: Vec<Command>,
    pub removed: Vec<Vec<String>>,
}

#[derive(Debug, Deserialize)]
pub struct Extension {
    pub name: String,
    pub version: String,
}

#[derive(Debug, Deserialize)]
pub struct Command {
    pub name: String,
    pub help: String,
    pub hidden: bool,
    pub children: Vec<Command>,
    pub owned: bool,
}

pub fn catalog() -> &'static Catalog {
    CATALOG.get_or_init(Catalog::default)
}

/// Return an exit code when the extension runtime handled this invocation.
pub async fn initialize() -> crate::error::Result<Option<i32>> {
    let Some(runtime) = process::runtime() else {
        return Ok(None);
    };
    let (catalog, exit_code) = process::inspect(&runtime).await?;
    let _ = CATALOG.set(catalog);
    Ok(exit_code)
}

pub fn help_summary() -> String {
    let extensions = &catalog().extensions;
    if extensions.is_empty() {
        return String::new();
    }
    let labels: Vec<_> = extensions
        .iter()
        .map(|extension| format!("{} [v{}]", extension.name, extension.version))
        .collect();
    format!("\n\nExtensions: {}", labels.join(", "))
}

/// Include installed command names in native help and the command inventory.
pub fn augment(mut native: clap::Command) -> clap::Command {
    for command in &catalog().commands {
        native = merge(native, command);
    }
    for path in &catalog().removed {
        native = hide_removed(native, path);
    }
    native
}

fn hide_removed(parent: clap::Command, path: &[String]) -> clap::Command {
    let Some((name, rest)) = path.split_first() else {
        return parent.hide(true);
    };
    if parent.find_subcommand(name).is_none() {
        return parent;
    }
    parent.mut_subcommand(name, |child| hide_removed(child, rest))
}

fn merge(mut parent: clap::Command, extension: &Command) -> clap::Command {
    if parent.find_subcommand(&extension.name).is_some() {
        parent = parent.mut_subcommand(&extension.name, |mut command| {
            if extension.owned {
                command = command.about(extension.help.clone()).hide(extension.hidden);
            }
            for child in &extension.children {
                command = merge(command, child);
            }
            command
        });
    } else {
        parent = parent.subcommand(proxy(extension));
    }
    parent
}

fn proxy(extension: &Command) -> clap::Command {
    let mut command = clap::Command::new(extension.name.clone())
        .about(extension.help.clone())
        .hide(extension.hidden);
    for child in &extension.children {
        command = command.subcommand(proxy(child));
    }
    command
}
