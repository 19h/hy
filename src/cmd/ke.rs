//! `hcli ke` command group: Knowledge Explorer management.

use clap::{Args, Subcommand};

use crate::error::Result;
use crate::util::fmt;

#[derive(Debug, Subcommand)]
pub enum KeCommands {
    /// Set up Knowledge Explorer
    Setup,
    /// Open Knowledge Explorer in the browser
    Open,
    /// Manage IDA instances for KE
    Ida {
        #[command(subcommand)]
        command: KeIdaCommands,
    },
    /// Manage KE sources
    Source {
        #[command(subcommand)]
        command: KeSourceCommands,
    },
}

#[derive(Debug, Subcommand)]
pub enum KeIdaCommands {
    /// List IDA instances registered with KE
    List,
    /// Add an IDA instance
    Add(KeIdaAddArgs),
    /// Remove an IDA instance
    Remove(KeIdaRemoveArgs),
    /// Switch the active IDA instance
    Switch,
}

#[derive(Debug, Args)]
pub struct KeIdaAddArgs {
    /// Path to IDA installation
    pub path: std::path::PathBuf,
}

#[derive(Debug, Args)]
pub struct KeIdaRemoveArgs {
    /// Name or path of the IDA instance
    pub name: String,
}

#[derive(Debug, Subcommand)]
pub enum KeSourceCommands {
    /// List configured sources
    List,
    /// Add a source
    Add(KeSourceAddArgs),
    /// Remove a source
    Remove(KeSourceRemoveArgs),
}

#[derive(Debug, Args)]
pub struct KeSourceAddArgs {
    /// Source URL or path
    pub source: String,
}

#[derive(Debug, Args)]
pub struct KeSourceRemoveArgs {
    /// Source name
    pub name: String,
}

pub async fn run(cmd: KeCommands) -> Result<()> {
    match cmd {
        KeCommands::Setup => {
            fmt::info("Knowledge Explorer setup is not yet implemented.");
            Ok(())
        }
        KeCommands::Open => {
            fmt::info("Opening Knowledge Explorer...");
            crate::util::io::open_url("https://ke.hex-rays.com");
            Ok(())
        }
        KeCommands::Ida { command } => {
            fmt::info(&format!("KE IDA command: {command:?}"));
            Ok(())
        }
        KeCommands::Source { command } => {
            fmt::info(&format!("KE source command: {command:?}"));
            Ok(())
        }
    }
}
