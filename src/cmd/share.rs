//! `hcli share` command group: put, get, list, delete.

use std::path::{Path, PathBuf};

use clap::{Args, Subcommand, ValueEnum};
use serde::Serialize;

use crate::error::{Error, Result};

#[derive(Debug, Subcommand)]
pub enum ShareCommands {
    /// Upload a shared file
    Put(PutArgs),
    /// Download a shared file by shortcode
    Get(GetArgs),
    /// List your shared files
    List(ListArgs),
    /// Delete a shared file by shortcode
    Delete(DeleteArgs),
}

#[derive(Debug, Args)]
pub struct PutArgs {
    /// Path to the file to upload
    pub path: PathBuf,
    /// Access control level
    #[arg(short, long, value_enum)]
    pub acl: Option<AccessControl>,
    /// Upload a new version for an existing code
    #[arg(short, long)]
    pub code: Option<String>,
    /// Force upload
    #[arg(short, long)]
    pub force: bool,
}

#[derive(Debug, Clone, Copy, ValueEnum, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum AccessControl {
    Private,
    Authenticated,
    Domain,
}

#[derive(Debug, Args)]
pub struct GetArgs {
    /// The shortcode of the shared file
    pub shortcode: String,
    /// Output directory
    #[arg(short, long)]
    pub output_dir: Option<PathBuf>,
    /// Output file path
    #[arg(short = 'O', long)]
    pub output_file: Option<PathBuf>,
    /// Overwrite existing files
    #[arg(short, long)]
    pub force: bool,
}

#[derive(Debug, Args)]
pub struct ListArgs {
    /// Maximum number of files to display
    #[arg(long, default_value_t = 100, allow_negative_numbers = true)]
    pub limit: i64,
    /// Offset for pagination
    #[arg(long, default_value_t = 0, allow_negative_numbers = true)]
    pub offset: i64,
    /// Disable interactive mode
    #[arg(long, overrides_with = "interactive")]
    pub no_interactive: bool,
    /// Enable interactive mode (the default)
    #[arg(long, overrides_with = "no_interactive")]
    pub interactive: bool,
}

#[derive(Debug, Args)]
pub struct DeleteArgs {
    /// Shortcode of the file to delete
    pub code: String,
    /// Skip confirmation
    #[arg(short, long)]
    pub force: bool,
}

pub async fn run(cmd: ShareCommands) -> Result<()> {
    match cmd {
        ShareCommands::Put(args) => super::share_upload::run(args).await,
        ShareCommands::Get(args) => super::share_operations::get(args).await,
        ShareCommands::List(args) => super::share_list::run(args).await,
        ShareCommands::Delete(args) => super::share_operations::delete(args).await,
    }
}

pub(super) fn expanded_path(path: &Path) -> Result<PathBuf> {
    let expanded = match path.strip_prefix("~") {
        Ok(suffix) => dirs::home_dir()
            .ok_or_else(|| Error::Other("home directory unavailable".into()))?
            .join(suffix),
        Err(_) => path.to_path_buf(),
    };
    Ok(std::path::absolute(expanded)?)
}
