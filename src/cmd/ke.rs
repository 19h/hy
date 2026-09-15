//! Knowledge Explorer entry points and legacy aliases for IDA management.

use clap::{Args, Subcommand};

use crate::error::Result;

use super::{ida_instances, ida_protocol, ida_sources};

#[derive(Debug, Subcommand)]
pub enum KeCommands {
    /// Register the ida:// protocol handler and discover IDA instances
    Setup(KeSetupArgs),
    /// Open Knowledge Explorer in the browser
    Open(KeOpenArgs),
    /// Manage IDA instances for KE
    #[command(subcommand)]
    Ida(ida_instances::Commands),
    /// Manage KE knowledge sources
    #[command(subcommand)]
    Source(ida_sources::Commands),
}

#[derive(Debug, Args)]
pub struct KeSetupArgs {
    /// Force protocol-handler re-registration
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

pub async fn run(command: KeCommands) -> Result<()> {
    match command {
        KeCommands::Setup(args) => {
            let command = if args.unregister {
                ida_protocol::Commands::Unregister
            } else {
                ida_protocol::Commands::Register {
                    force: args.force,
                }
            };
            ida_protocol::run(command).await
        }
        KeCommands::Open(args) => open(args).await,
        KeCommands::Ida(command) => ida_instances::run(command).await,
        KeCommands::Source(command) => ida_sources::run(command),
    }
}

async fn open(args: KeOpenArgs) -> Result<()> {
    let url = args.url.unwrap_or_else(|| "https://ke.hex-rays.com".into());
    if url.starts_with("https://") || url.starts_with("http://") {
        crate::util::io::open_url(&url);
        return Ok(());
    }
    crate::ida::links::open(&url, false, 120.0, false).await
}
