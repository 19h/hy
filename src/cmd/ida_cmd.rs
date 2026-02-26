//! `hcli ida` command group: install, set-default, accept-eula.

use std::path::PathBuf;

use clap::{Args, Subcommand};

use crate::error::Result;
use crate::util::fmt;

#[derive(Debug, Subcommand)]
pub enum IdaCommands {
    /// Install IDA from a downloaded installer
    Install(IdaInstallArgs),
    /// Set the default IDA installation directory
    SetDefault(IdaSetDefaultArgs),
    /// Accept the IDA EULA
    AcceptEula,
}

#[derive(Debug, Args)]
pub struct IdaInstallArgs {
    /// Path to the installer file
    pub installer: PathBuf,
    /// Installation directory
    #[arg(long)]
    pub prefix: Option<PathBuf>,
    /// Accept the EULA non-interactively
    #[arg(long)]
    pub accept_eula: bool,
}

#[derive(Debug, Args)]
pub struct IdaSetDefaultArgs {
    /// Path to the IDA installation directory
    pub path: PathBuf,
}

pub async fn run(cmd: IdaCommands) -> Result<()> {
    match cmd {
        IdaCommands::Install(args) => run_install(args).await,
        IdaCommands::SetDefault(args) => run_set_default(args).await,
        IdaCommands::AcceptEula => run_accept_eula().await,
    }
}

async fn run_install(args: IdaInstallArgs) -> Result<()> {
    let install_dir = args
        .prefix
        .unwrap_or_else(crate::ida::default_install_dir);

    fmt::info(&format!(
        "Installing IDA from {} to {}",
        args.installer.display(),
        install_dir.display()
    ));

    let result = crate::ida::install_ida(&args.installer, &install_dir, args.accept_eula).await?;
    fmt::success(&format!("IDA installed at: {}", result.display()));
    Ok(())
}

async fn run_set_default(args: IdaSetDefaultArgs) -> Result<()> {
    let path = args.path.canonicalize()?;
    if !path.is_dir() {
        fmt::error(&format!("Not a directory: {}", path.display()));
        return Ok(());
    }

    // Write to ida-config.json.
    let config_path = crate::ida::ida_user_dir().join("ida-config.json");
    let mut config: serde_json::Value = if config_path.exists() {
        let text = std::fs::read_to_string(&config_path)?;
        serde_json::from_str(&text).unwrap_or_default()
    } else {
        serde_json::json!({})
    };

    if let Some(paths) = config.get_mut("paths") {
        paths["ida_install_dir"] = serde_json::json!(path.to_string_lossy());
    } else {
        config["paths"] = serde_json::json!({ "ida_install_dir": path.to_string_lossy() });
    }

    if let Some(parent) = config_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&config_path, serde_json::to_string_pretty(&config)?)?;

    fmt::success(&format!("Default IDA set to: {}", path.display()));
    Ok(())
}

async fn run_accept_eula() -> Result<()> {
    // The EULA acceptance is typically handled during installation.
    fmt::info("EULA acceptance is handled during `hcli ida install --accept-eula`.");
    Ok(())
}
