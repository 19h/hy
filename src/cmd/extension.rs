//! `hy extension` command group: list, create.
//!
//! In the Python version, extensions are discovered via Python `entry_points`.
//! The Rust binary does not support dynamically loaded extensions, so `list`
//! always reports none and `create` prints the cookiecutter command for
//! scaffolding a Python-based hcli extension (matching the Python behaviour).

use clap::{Args, Subcommand};

use crate::error::Result;
use crate::util::fmt;

#[derive(Debug, Subcommand)]
pub enum ExtensionCommands {
    /// List installed extensions
    List,
    /// Create a new extension scaffold
    Create(ExtensionCreateArgs),
}

#[derive(Debug, Args)]
pub struct ExtensionCreateArgs {
    /// Extension name
    pub name: Option<String>,
}

pub async fn run(cmd: ExtensionCommands) -> Result<()> {
    match cmd {
        ExtensionCommands::List => {
            fmt::info("No extensions installed.");
            Ok(())
        }
        ExtensionCommands::Create(_args) => {
            eprintln!("  To create a new hcli extension, run:");
            eprintln!();
            eprintln!(
                "    pipx run cookiecutter gh:Hex-RaysSA/ida-hcli-extension-template"
            );
            eprintln!();
            Ok(())
        }
    }
}
