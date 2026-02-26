//! `hy ida` command group: install, set-default, accept-eula.

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
    /// Accept the IDA EULA for an installation
    AcceptEula(IdaAcceptEulaArgs),
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

#[derive(Debug, Args)]
pub struct IdaAcceptEulaArgs {
    /// Path to the IDA installation directory (uses default if omitted)
    pub path: Option<PathBuf>,
}

pub async fn run(cmd: IdaCommands) -> Result<()> {
    match cmd {
        IdaCommands::Install(args) => run_install(args).await,
        IdaCommands::SetDefault(args) => run_set_default(args).await,
        IdaCommands::AcceptEula(args) => run_accept_eula(args).await,
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

async fn run_accept_eula(args: IdaAcceptEulaArgs) -> Result<()> {
    let install_dir = match args.path {
        Some(p) => p,
        None => match crate::ida::current_install_dir() {
            Some(d) => d,
            None => {
                fmt::error("No IDA installation found. Specify a path or set a default.");
                return Ok(());
            }
        },
    };

    if !install_dir.exists() {
        fmt::error(&format!("Directory not found: {}", install_dir.display()));
        return Ok(());
    }

    // Look for idalib / idapyswitch to invoke EULA acceptance.
    let ida_path = if cfg!(target_os = "macos") {
        install_dir.join("Contents").join("MacOS")
    } else {
        install_dir.clone()
    };

    let idalib = if cfg!(target_os = "windows") {
        ida_path.join("idalib.exe")
    } else {
        ida_path.join("idalib")
    };

    if idalib.exists() {
        fmt::info("Accepting EULA via idalib...");
        let status = std::process::Command::new(&idalib)
            .arg("--accept-eula")
            .status();

        match status {
            Ok(s) if s.success() => {
                fmt::success("EULA accepted.");
            }
            Ok(s) => {
                fmt::warning(&format!(
                    "idalib exited with status {}. EULA may not have been accepted.",
                    s.code().unwrap_or(-1)
                ));
            }
            Err(e) => {
                fmt::error(&format!("Failed to run idalib: {e}"));
            }
        }
    } else {
        fmt::warning("idalib not found in the installation directory.");
        fmt::info("EULA acceptance requires IDA Pro or IDA Teams (not IDA Free/Home).");
        fmt::info(&format!("Checked: {}", idalib.display()));
    }

    Ok(())
}
