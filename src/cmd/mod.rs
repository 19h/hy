//! CLI command definitions (clap derive).

pub mod auth_cmd;
pub mod download;
pub mod extension;
pub mod ida_cmd;
pub mod ke;
pub mod license;
pub mod login;
pub mod logout;
pub mod plugin_cmd;
pub mod share;
pub mod update;
pub mod whoami;

use clap::Subcommand;

/// Top-level subcommands.
#[derive(Debug, Subcommand)]
pub enum Commands {
    /// Log in to the Hex-Rays portal
    Login(login::LoginArgs),

    /// Log out and remove stored credentials
    Logout(logout::LogoutArgs),

    /// Display the currently logged-in user
    Whoami,

    /// Check for updates
    Update(update::UpdateArgs),

    /// Download IDA binaries, SDKs, and utilities
    Download(download::DownloadArgs),

    /// List all available commands
    Commands,

    /// Manage authentication credentials
    Auth {
        #[command(subcommand)]
        command: auth_cmd::AuthCommands,
    },

    /// Share files with Hex-Rays
    Share {
        #[command(subcommand)]
        command: share::ShareCommands,
    },

    /// Manage IDA licenses
    License {
        #[command(subcommand)]
        command: license::LicenseCommands,
    },

    /// Manage IDA installations
    Ida {
        #[command(subcommand)]
        command: ida_cmd::IdaCommands,
    },

    /// Manage IDA plugins
    Plugin {
        #[command(subcommand)]
        command: plugin_cmd::PluginCommands,
    },

    /// Manage extensions
    Extension {
        #[command(subcommand)]
        command: extension::ExtensionCommands,
    },

    /// Manage Knowledge Explorer
    Ke {
        #[command(subcommand)]
        command: ke::KeCommands,
    },
}
