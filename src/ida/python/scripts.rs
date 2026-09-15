//! Console-script discovery and execution in IDA's selected Python environment.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use super::{ResolvedPython, environment, resolve};
use crate::error::{Error, Result};

mod execution;
mod lookup;

#[cfg(test)]
mod tests;

const RUN_ENTRY_POINT: &str = include_str!("scripts/entry_point.py");

pub async fn resolve_for_execution(skip_environment_check: bool) -> Result<ResolvedPython> {
    let python = resolve().await?;
    if !skip_environment_check {
        environment::warn(&python).await;
    }
    Ok(python)
}

pub async fn run_python(args: &[OsString], skip_environment_check: bool) -> Result<()> {
    let python = resolve_for_execution(skip_environment_check).await?;
    execution::run(execution::command(&python.exe, &python.exe).args(args)).await
}

pub async fn find_script(exe: &Path, name: &str) -> Result<PathBuf> {
    let info = lookup::inspect(exe, name).await?;
    info.path.clone().ok_or_else(|| Error::Other(info.not_found_message()))
}

pub async fn run_script(name: &str, args: &[OsString], skip_environment_check: bool) -> Result<()> {
    let python = resolve_for_execution(skip_environment_check).await?;
    let info = lookup::inspect(&python.exe, name).await?;
    let mut command = match info.path.as_deref() {
        Some(path) if execution::is_executable(path) => execution::command(&python.exe, path),
        Some(path) => {
            let mut command = execution::command(&python.exe, &python.exe);
            command.arg(path);
            command
        }
        None if info.entry_point.is_some() => {
            let mut command = execution::command(&python.exe, &python.exe);
            command.args(["-c", RUN_ENTRY_POINT, name]);
            command
        }
        None => return Err(Error::Other(info.not_found_message())),
    };
    execution::run(command.args(args)).await
}
