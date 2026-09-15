//! `hy ida` command group: install, set-default, accept-eula.

use std::path::PathBuf;

use clap::{Args, Subcommand};

use super::ida_instances;
use crate::error::Result;
use crate::util::fmt;

#[derive(Debug, Subcommand)]
pub enum IdaCommands {
    /// Open an ida:// link or list running IDA instances
    Open(IdaOpenArgs),
    /// List registered IDA installations
    List,
    /// Register an IDA installation
    Add(ida_instances::AddArgs),
    /// Remove registered IDA installations
    Remove(ida_instances::RemoveArgs),
    /// Switch the default IDA installation
    Switch(ida_instances::SwitchArgs),
    /// Manage IDB lookup sources
    #[command(subcommand)]
    Source(super::ida_sources::Commands),
    /// Register or unregister the ida:// protocol
    #[command(subcommand)]
    Protocol(super::ida_protocol::Commands),
    /// Work in IDA's Python environment
    Python(PythonArgs),
    /// Install IDA from a downloaded installer
    Install(IdaInstallArgs),
    /// Set or show the idalib directory (deprecated: use ida switch)
    SetDefault(IdaSetDefaultArgs),
    /// Accept the IDA EULA for an installation
    AcceptEula(IdaAcceptEulaArgs),
}

#[derive(Debug, Args)]
pub struct IdaOpenArgs {
    pub uri: Option<String>,
    #[arg(long)]
    pub list: bool,
    #[arg(long)]
    pub no_launch: bool,
    #[arg(long, default_value_t = 120.0, allow_hyphen_values = true)]
    pub timeout: f64,
    #[arg(long)]
    pub skip_analysis: bool,
}

#[derive(Debug, Args)]
pub struct PythonArgs {
    /// Do not warn about IDA's Python environment before executing a command
    #[arg(long)]
    pub no_python_environment_check: bool,
    #[command(subcommand)]
    pub command: PythonCommands,
}

#[derive(Debug, Subcommand)]
pub enum PythonCommands {
    /// Run IDA's Python interpreter with all remaining arguments
    #[command(disable_version_flag = true, trailing_var_arg = true)]
    Exec {
        #[arg(allow_hyphen_values = true)]
        args: Vec<std::ffi::OsString>,
    },
    /// Locate a script installed in IDA's Python environment
    FindScript {
        name: String,
    },
    /// Run an installed script with all remaining arguments
    #[command(disable_version_flag = true, trailing_var_arg = true)]
    RunScript {
        /// Script name followed by its arguments
        #[arg(required = true, num_args = 1.., allow_hyphen_values = true)]
        invocation: Vec<std::ffi::OsString>,
    },
    /// Diagnose IDA's Python environment
    Doctor {
        #[arg(long)]
        json: bool,
    },
    /// Explain IDA and Python environment resolution
    ExplainEnvironment {
        #[arg(long)]
        json: bool,
    },
    /// Create and configure IDA's Python virtual environment
    CreateEnvironment(CreateEnvironmentArgs),
}

#[derive(Debug, Args)]
pub struct CreateEnvironmentArgs {
    #[arg(long)]
    pub path: Option<PathBuf>,
    #[arg(long)]
    pub python_version: Option<String>,
    #[arg(long)]
    pub no_configure_env_var: bool,
    #[arg(long)]
    pub no_reinstall_plugins: bool,
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, Args)]
pub struct IdaInstallArgs {
    /// Path to the installer file
    pub installer: Option<PathBuf>,
    /// Installer asset key or tag, such as ida-pro:latest
    #[arg(short = 'd', long)]
    pub download_id: Option<String>,
    /// License identifier to download and install
    #[arg(short = 'l', long)]
    pub license_id: Option<String>,
    /// Installation directory
    #[arg(short = 'i', long = "install-dir", alias = "prefix")]
    pub prefix: Option<PathBuf>,
    /// Accept the EULA non-interactively
    #[arg(short = 'a', long, action = clap::ArgAction::SetTrue, default_value_t = true, overrides_with = "no_accept_eula")]
    pub accept_eula: bool,
    #[arg(short = 'A', long, overrides_with = "accept_eula")]
    pub no_accept_eula: bool,
    /// Set the installed IDA as the default
    #[arg(long, action = clap::ArgAction::SetTrue, default_value_t = true, overrides_with = "no_set_default")]
    pub set_default: bool,
    #[arg(long, overrides_with = "set_default")]
    pub no_set_default: bool,
    /// Show installation actions without executing the installer
    #[arg(long)]
    pub dry_run: bool,
    /// Accept installation confirmation prompts
    #[arg(short = 'y', long)]
    pub yes: bool,
    /// Create IDA's Python virtual environment after installation
    #[arg(long)]
    pub create_python_environment: bool,
}

#[derive(Debug, Args)]
pub struct IdaSetDefaultArgs {
    /// Path to the IDA installation directory
    pub path: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct IdaAcceptEulaArgs {
    /// Path to the IDA installation directory (uses default if omitted)
    pub path: Option<PathBuf>,
}

pub async fn run(cmd: IdaCommands) -> Result<()> {
    match cmd {
        IdaCommands::Open(args) => {
            if args.list {
                for instance in crate::ida::ipc::discover().await {
                    println!(
                        "PID {}: {}",
                        instance.pid,
                        instance
                            .idb_path
                            .as_deref()
                            .unwrap_or(std::path::Path::new("(no database)"))
                            .display()
                    );
                }
                Ok(())
            } else {
                let uri = args.uri.ok_or_else(|| {
                    crate::error::Error::Other("URI is required unless --list is used".into())
                })?;
                crate::ida::links::open(&uri, args.no_launch, args.timeout, args.skip_analysis)
                    .await
            }
        }
        IdaCommands::List => ida_instances::run(ida_instances::Commands::List).await,
        IdaCommands::Add(args) => ida_instances::run(ida_instances::Commands::Add(args)).await,
        IdaCommands::Remove(args) => {
            ida_instances::run(ida_instances::Commands::Remove(args)).await
        }
        IdaCommands::Switch(args) => {
            ida_instances::run(ida_instances::Commands::Switch(args)).await
        }
        IdaCommands::Source(command) => super::ida_sources::run(command),
        IdaCommands::Protocol(command) => super::ida_protocol::run(command).await,
        IdaCommands::Python(command) => run_python_command(command).await,
        IdaCommands::Install(args) => super::ida_install::run(args).await,
        IdaCommands::SetDefault(args) => run_set_default(args).await,
        IdaCommands::AcceptEula(args) => run_accept_eula(args).await,
    }
}

pub async fn run_python_command(args: PythonArgs) -> Result<()> {
    use crate::ida::python;
    let skip_environment_check = args.no_python_environment_check;
    match args.command {
        PythonCommands::Exec {
            args,
        } => python::run_python(&args, skip_environment_check).await,
        PythonCommands::RunScript {
            invocation,
        } => {
            let (name, args) = invocation
                .split_first()
                .ok_or_else(|| crate::error::Error::Other("a script name is required".into()))?;
            let name = name.to_str().ok_or_else(|| {
                crate::error::Error::Other("script name is not valid UTF-8".into())
            })?;
            python::run_script(name, args, skip_environment_check).await
        }
        PythonCommands::FindScript {
            name,
        } => {
            let resolved = python::resolve_for_execution(skip_environment_check).await?;
            println!("{}", python::find_script(&resolved.exe, &name).await?.display());
            Ok(())
        }
        PythonCommands::Doctor {
            json,
        } => {
            let report = python::doctor().await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                report.print_text();
            }
            if !report.ok {
                Err(crate::error::Error::ChildExit(1))
            } else {
                Ok(())
            }
        }
        PythonCommands::ExplainEnvironment {
            json,
        } => {
            let report = python::explain().await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                report.print_text();
            }
            Ok(())
        }
        PythonCommands::CreateEnvironment(args) => {
            use std::io::IsTerminal;
            let report = python::create_environment(python::CreateOptions {
                ida_installation: None,
                path: args.path,
                python_version: args.python_version,
                configure: !args.no_configure_env_var,
                reinstall_plugins: !args.no_reinstall_plugins,
                interactive: !args.json
                    && std::io::stdin().is_terminal()
                    && std::io::stdout().is_terminal(),
                quiet: args.json,
            })
            .await?;
            if args.json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            }
            Ok(())
        }
    }
}

async fn run_set_default(args: IdaSetDefaultArgs) -> Result<()> {
    fmt::warning("'ida set-default' is deprecated. Use 'ida switch' instead.");
    let Some(path) = args.path else {
        let config = crate::plugin::read_ida_config()?;
        match config["Paths"]["ida-install-dir"].as_str().filter(|path| !path.is_empty()) {
            Some(path) => println!("Default IDA installation: {path}"),
            None => println!("No default IDA installation set."),
        }
        let installations = crate::ida::find_standard_installations().await;
        if installations.is_empty() {
            println!("\nNo standard installations found.");
        } else {
            println!("\nAvailable installations:");
            for installation in installations {
                println!("  - {}", installation.display());
            }
        }
        return Ok(());
    };
    let path = crate::util::files::absolute_path(&path)?;
    if !path.exists() {
        fmt::error(&format!("Path does not exist: {}", path.display()));
        return Ok(());
    }
    let path = path.canonicalize()?;
    if !path.is_dir() || crate::ida::ida_binary_path(&path).is_none() {
        fmt::error(&format!("Not a valid IDA installation directory: {}", path.display()));
        return Ok(());
    }
    crate::plugin::set_ida_installation_directory(&path)?;
    fmt::success(&format!("Default IDA set to: {}", path.display()));
    Ok(())
}

async fn run_accept_eula(args: IdaAcceptEulaArgs) -> Result<()> {
    let installation = match args.path {
        Some(path) => path,
        None => crate::ida::resolve_install_dir()?.path,
    };
    crate::ida::accept_eula(&installation).await?;
    fmt::success("EULA accepted.");
    Ok(())
}
