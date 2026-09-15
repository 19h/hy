//! CLI command definitions (clap derive).

pub mod asset_cmd;
pub mod auth_cmd;
pub mod download;
mod download_picker;
pub mod extension;
pub mod ida_cmd;
mod ida_install;
mod ida_instances;
mod ida_protocol;
mod ida_sources;
pub mod ke;
pub mod license;
mod license_download;
mod license_install;
mod license_list;
pub mod login;
pub mod logout;
pub mod mcp;
mod plugin_bundle;
mod plugin_bundle_sources;
mod plugin_bundle_targets;
pub mod plugin_cmd;
mod plugin_config;
mod plugin_lint;
pub mod plugin_ops;
mod plugin_search;
mod plugin_settings;
mod plugin_status;
mod plugin_upgrade;
pub mod share;
mod share_display;
mod share_list;
mod share_operations;
mod share_prompt;
mod share_upload;
pub mod update;
pub mod whoami;

use clap::Subcommand;

/// Top-level subcommands.
#[derive(Debug, Subcommand)]
pub enum Commands {
    /// Install IDA MCP integrations
    Mcp {
        #[command(subcommand)]
        command: mcp::McpCommands,
    },
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
    #[command(name = "commands")]
    CommandTree,

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
    Plugin(plugin_cmd::PluginArgs),

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

    /// Low-level bucket asset operations
    #[command(hide = true)]
    Asset {
        #[command(subcommand)]
        command: asset_cmd::AssetCommands,
    },
}

impl Commands {
    /// Upstream AuthCommand applies global auth constraints to these commands.
    /// Local credential management and optional-auth repository commands do not.
    pub fn enforces_auth_options(&self) -> bool {
        matches!(
            self,
            Self::Download(_)
                | Self::Share { .. }
                | Self::Asset { .. }
                | Self::License {
                    command: license::LicenseCommands::List(_) | license::LicenseCommands::Get(_)
                }
                | Self::Auth {
                    command: auth_cmd::AuthCommands::Key {
                        command: auth_cmd::KeyCommands::List
                    }
                }
        )
    }
}
