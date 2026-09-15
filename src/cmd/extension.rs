//! `hy extension` command group: list, create.
//!
//! Python entry points are inspected by the compatibility runtime at startup.

use clap::Subcommand;

use crate::error::Result;
use crate::util::fmt;

#[derive(Debug, Subcommand)]
pub enum ExtensionCommands {
    /// List installed extensions
    List,
    /// Create a new extension scaffold
    Create,
}

pub async fn run(cmd: ExtensionCommands) -> Result<()> {
    match cmd {
        ExtensionCommands::List => {
            let extensions = &crate::extensions::catalog().extensions;
            if extensions.is_empty() {
                fmt::info("No extensions installed");
            } else {
                let names: Vec<_> =
                    extensions.iter().map(|extension| extension.name.as_str()).collect();
                println!("Extensions: {}", names.join(", "));
            }
            Ok(())
        }
        ExtensionCommands::Create => {
            println!("You can create a new hcli extension using the following command:\n");
            println!("pipx run cookiecutter gh:Hex-RaysSA/ida-hcli-extension-template\n");
            println!(
                "This will guide you through generating a new extension project based on the official template."
            );
            Ok(())
        }
    }
}
