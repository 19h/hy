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
    run(exe, dependencies, options, wheelhouse, Operation::Install).await
}

pub async fn verify_dependencies(
    exe: &Path,
    dependencies: &[String],
    options: &PipOptions,
    wheelhouse: Option<&Path>,
) -> Result<()> {
    run(exe, dependencies, options, wheelhouse, Operation::Resolve).await
}

#[derive(Clone, Copy)]
enum Operation {
    Resolve,
    Install,
}

async fn run(
    exe: &Path,
    dependencies: &[String],
    options: &PipOptions,
    wheelhouse: Option<&Path>,
    operation: Operation,
) -> Result<()> {
    let result = command(exe, dependencies, options, wheelhouse, operation).output().await?;
    if !result.status.success() {
        return Err(Error::PythonPackages(errors::message(
            exe,
            &result.stdout,
            &result.stderr,
            &crate::config::Env::global().binary_name,
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
    let mut cmd = Command::new(exe);
    cmd.args(["-m", "pip", "install"]);
    if matches!(operation, Operation::Resolve) {
        cmd.arg("--dry-run");
    }
    cmd.args(options.arguments());
    cmd.args(dependencies).stdin(Stdio::inherit()).kill_on_drop(true);
    // Upstream inherits the process environment and stdin, captures both outputs,
    // and imposes no timeout on pip. Interpreter probes have separate policies.
    cmd
}
