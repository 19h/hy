//! Separate pip resolution and installation; callers control lifecycle and validation.

use std::path::Path;
use std::process::Stdio;

use tokio::process::Command;

use super::PipOptions;
use crate::error::{Error, Result};

mod errors;

#[cfg(test)]
mod tests;

pub async fn install_dependencies(
    exe: &Path,
    dependencies: &[String],
    options: &PipOptions,
    wheelhouse: Option<&Path>,
) -> Result<()> {
    install_with_constraints(exe, dependencies, &[], options, wheelhouse).await
}

pub(crate) async fn install_with_constraints(
    exe: &Path,
    dependencies: &[String],
    constraints: &[String],
    options: &PipOptions,
    wheelhouse: Option<&Path>,
) -> Result<()> {
    run(exe, dependencies, constraints, options, wheelhouse, Operation::Install).await
}

pub(crate) async fn verify_with_constraints(
    exe: &Path,
    dependencies: &[String],
    constraints: &[String],
    options: &PipOptions,
    wheelhouse: Option<&Path>,
) -> Result<()> {
    run(exe, dependencies, constraints, options, wheelhouse, Operation::Resolve).await
}

#[derive(Clone, Copy)]
enum Operation {
    Resolve,
    Install,
}

async fn run(
    exe: &Path,
    dependencies: &[String],
    constraints: &[String],
    options: &PipOptions,
    wheelhouse: Option<&Path>,
    operation: Operation,
) -> Result<()> {
    let mut command = command(exe, dependencies, options, wheelhouse, operation);
    let constraint_file = if constraints.is_empty() {
        None
    } else {
        use std::io::Write;
        let mut file = tempfile::NamedTempFile::new()?;
        for constraint in constraints {
            writeln!(file, "{constraint}")?;
        }
        file.flush()?;
        command.arg("--constraint").arg(file.path());
        Some(file)
    };
    let result = command.output().await?;
    drop(constraint_file);
    if !result.status.success() {
        return Err(Error::PythonPackages(errors::message(
            exe,
            &result.stdout,
            &result.stderr,
            crate::config::Env::global().display_command(),
        )));
    }
    Ok(())
}

fn command(
    exe: &Path,
    dependencies: &[String],
    options: &PipOptions,
    wheelhouse: Option<&Path>,
    operation: Operation,
) -> Command {
    let effective = wheelhouse.map(|path| options.with_wheelhouse(path));
    let options = effective.as_ref().unwrap_or(options);
    let mut cmd = super::command(exe);
    cmd.args(["-m", "pip", "install"]);
    if matches!(operation, Operation::Resolve) {
        cmd.arg("--dry-run");
    }
    cmd.args(options.arguments());
    cmd.args(dependencies).stdin(Stdio::inherit()).kill_on_drop(true);
    // Preserve stdin and capture diagnostics while applying the selected
    // interpreter's environment. Pip has no timeout.
    cmd
}
