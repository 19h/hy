//! Protocol commands and the shared legacy KE setup entry point.

use clap::Subcommand;

use crate::error::Result;
use crate::util::fmt;

#[derive(Debug, Subcommand)]
pub enum Commands {
    /// Register the ida:// handler and discover IDA when no instances are registered
    Register {
        /// Force reinstall even if already configured
        #[arg(long)]
        force: bool,
    },
    /// Remove the ida:// protocol handler
    Unregister,
}

pub async fn run(command: Commands) -> Result<()> {
    match command {
        // Upstream accepts --force but always reinstalls the handler. It does
        // not authorize replacing existing instance registrations or defaults.
        Commands::Register {
            force: _,
        } => {
            crate::ida::register_protocol_handler(
                &crate::util::io::executable_path().to_string_lossy(),
            )?;
            fmt::success("Registered ida:// protocol handler.");
            super::ida_instances::ensure_registered().await
        }
        Commands::Unregister => {
            crate::ida::unregister_protocol_handler()?;
            fmt::success("Protocol handler unregistered.");
            Ok(())
        }
    }
}
