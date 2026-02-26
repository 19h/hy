//! `hcli extension` command group: list, create.

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
    pub name: String,
}

pub async fn run(cmd: ExtensionCommands) -> Result<()> {
    match cmd {
        ExtensionCommands::List => {
            fmt::info("No extensions installed.");
            // In the Python version, extensions are discovered via entry_points.
            // In the Rust version this would use a different mechanism.
            Ok(())
        }
        ExtensionCommands::Create(args) => {
            fmt::info(&format!(
                "Extension scaffolding for '{}' is not yet implemented.",
                args.name
            ));
            Ok(())
        }
    }
}
