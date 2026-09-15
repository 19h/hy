//! Bundle wheel downloads have a distinct pip policy from package installation.

use std::path::Path;
use std::process::Stdio;

use tokio::process::Command;

use super::PipTarget;
use crate::error::{Error, Result};
use crate::ida::python::{self, PipOptions};

#[cfg(test)]
mod tests;

pub(super) async fn wheelhouse(
    dependencies: &[String],
    target: &PipTarget,
    destination: &Path,
    options: &PipOptions,
) -> Result<()> {
    let python = python::resolve().await?;
    let output = command(&python.exe, dependencies, target, destination, options)?.output().await?;
    if !output.status.success() {
        return Err(Error::Other(failure(target, &output.stdout, &output.stderr)));
    }
    Ok(())
}

fn command(
    python: &Path,
    dependencies: &[String],
    target: &PipTarget,
    destination: &Path,
    options: &PipOptions,
) -> Result<Command> {
    let mut command = Command::new(python);
    command.args(["-m", "pip", "download"]);
    command.args(target.pip_download_args()?);
    command.arg("--dest").arg(destination);
    if let Some(index) = options.index_url.as_ref().filter(|index| !index.is_empty()) {
        command.arg("--index-url").arg(index);
    }
    for index in &options.extra_index_urls {
        command.arg("--extra-index-url").arg(index);
    }
    for link in &options.find_links {
        command.arg("--find-links").arg(link);
    }
    if options.no_index {
        command.arg("--no-index");
    }
    // The source download helper ignores installation-only flags and inherits
    // environment, cwd and stdin. Output is captured without a fixed deadline.
    command.args(dependencies).stdin(Stdio::inherit()).kill_on_drop(true);
    Ok(command)
}

fn failure(target: &PipTarget, stdout: &[u8], stderr: &[u8]) -> String {
    format!(
        "pip download failed for target {}:\n{}\n{}",
        target.id(),
        String::from_utf8_lossy(stdout),
        String::from_utf8_lossy(stderr)
    )
}
