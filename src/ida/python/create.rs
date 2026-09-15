//! Create or recognize IDA's environment, then migrate and configure in source order.

use std::path::{Path, PathBuf};

use serde::Serialize;
use tokio::process::Command;

use super::{env_var, find_command, output, venv_python};
use crate::error::{Error, Result};

mod configure;
mod migration;
mod planning;
mod target;
mod ui;

pub struct CreateOptions {
    pub ida_installation: Option<PathBuf>,
    pub path: Option<PathBuf>,
    pub python_version: Option<String>,
    pub configure: bool,
    pub reinstall_plugins: bool,
    pub interactive: bool,
    pub quiet: bool,
}

#[derive(Debug, Serialize)]
pub struct CreateEnvironmentReport {
    pub venv_path: PathBuf,
    pub python_exe: PathBuf,
    pub python_version: String,
    pub python_version_source: &'static str,
    pub created: bool,
    pub tool: Option<&'static str>,
    pub set_command: String,
    pub configured: bool,
    pub configured_via: Option<String>,
    pub plugin_migrations: Vec<migration::PluginMigration>,
    pub plugins_skipped: bool,
}

pub async fn create_environment(options: CreateOptions) -> Result<CreateEnvironmentReport> {
    let ui = ui::Ui {
        interactive: options.interactive,
        quiet: options.quiet,
    };
    let version =
        planning::determine_version(options.python_version, options.ida_installation.as_deref())
            .await?;
    let path = options.path.unwrap_or_else(|| crate::ida::ida_user_dir().join("venv"));
    let path = expand_path(&path)?;
    ui.print(&format!(
        "Python version for the environment: {} (via {})",
        version.value, version.source
    ));

    let (python_exe, tool) = match target::inspect(&path, &version.value).await? {
        target::Target::Healthy(executable) => {
            ui.print(&format!(
                "{} is already a Python {} virtual environment with pip.",
                path.display(),
                version.value
            ));
            (executable, None)
        }
        target::Target::Missing => {
            let registered = planning::registered_python(version.probe.as_ref());
            let uv = find_command("uv");
            let fallback = if registered.is_none() && uv.is_none() {
                planning::path_python(&version.value).await?
            } else {
                None
            };
            let plan = planning::plan(&path, &version.value, registered, uv, fallback)?;
            let executable = execute(&plan, &path, &version.value, &ui).await?;
            (executable, Some(plan.tool))
        }
        target::Target::Unusable(reason) => {
            return Err(Error::Other(format!(
                "{} {reason}. Existing environments are not replaced; recreate it or use --path.",
                path.display()
            )));
        }
    };
    let created = tool.is_some();
    let (plugin_migrations, plugins_skipped) = if created {
        migration::run(&python_exe, options.reinstall_plugins, &ui).await?
    } else {
        (Vec::new(), false)
    };

    let set_command = if cfg!(windows) {
        format!(
            "[Environment]::SetEnvironmentVariable(\"IDAPYTHON_VENV_EXECUTABLE\", \"{}\", \"User\")",
            python_exe.display()
        )
    } else {
        format!("export IDAPYTHON_VENV_EXECUTABLE=\"{}\"", python_exe.display())
    };
    let already_configured = env_var("IDAPYTHON_VENV_EXECUTABLE").is_some_and(|current| {
        crate::util::realpath::resolve(Path::new(&current))
            .ok()
            .zip(crate::util::realpath::resolve(&python_exe).ok())
            .is_some_and(|(current, target)| current == target)
    });
    let configured = if options.configure && already_configured {
        ui.print("$IDAPYTHON_VENV_EXECUTABLE already points to this environment.");
        configure::Outcome::default()
    } else if options.configure {
        configure::run(&python_exe, &ui).await?
    } else {
        ui.print(&format!("Skipped configuring $IDAPYTHON_VENV_EXECUTABLE. To make IDA use this environment:\n  {set_command}"));
        configure::Outcome::default()
    };
    Ok(CreateEnvironmentReport {
        venv_path: path,
        python_exe,
        python_version: version.value,
        python_version_source: version.source,
        created,
        tool,
        set_command,
        configured: configured.configured,
        configured_via: configured.via,
        plugin_migrations,
        plugins_skipped,
    })
}

fn expand_path(path: &Path) -> Result<PathBuf> {
    let path = match path.strip_prefix("~") {
        Ok(suffix) => dirs::home_dir()
            .ok_or_else(|| Error::Other("home directory unavailable".into()))?
            .join(suffix),
        Err(_) => path.to_owned(),
    };
    Ok(std::path::absolute(path)?)
}

async fn execute(
    plan: &planning::Plan,
    path: &Path,
    version: &str,
    ui: &ui::Ui,
) -> Result<PathBuf> {
    std::fs::create_dir_all(path.parent().unwrap())?;
    ui.print(&format!(
        "Creating virtual environment: {} {}",
        plan.executable.display(),
        plan.arguments.iter().map(|arg| arg.to_string_lossy()).collect::<Vec<_>>().join(" ")
    ));
    run(Command::new(&plan.executable).args(&plan.arguments)).await?;
    if plan.tool == "venv" {
        run(Command::new(venv_python(path)).args(["-m", "ensurepip", "--upgrade"])).await?;
    }
    match target::inspect(path, version).await? {
        target::Target::Healthy(executable) => {
            ui.print(&format!("Created {} with Python {version} and pip.", path.display()));
            Ok(executable)
        }
        target::Target::Missing => Err(Error::Other(format!(
            "{} was not created as a virtual environment",
            path.display()
        ))),
        target::Target::Unusable(reason) => {
            Err(Error::Other(format!("created environment {} {reason}", path.display())))
        }
    }
}

async fn run(command: &mut Command) -> Result<()> {
    let result = output(command, 600).await?;
    if result.status.success() {
        return Ok(());
    }
    let detail = if result.stderr.is_empty() {
        &result.stdout
    } else {
        &result.stderr
    };
    Err(Error::Other(format!(
        "environment creation command failed with exit code {}: {}",
        result.status.code().unwrap_or(1),
        String::from_utf8_lossy(detail).trim()
    )))
}
