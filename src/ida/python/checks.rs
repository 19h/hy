//! Independent, bounded subprocess observations used by creation and diagnostics.

use std::path::Path;
use std::process::Stdio;
use std::time::Duration;

use tokio::process::Command;

use crate::error::Result;

#[cfg(test)]
mod tests;
mod text;

pub(super) async fn version(executable: &Path) -> Result<Option<String>> {
    let mut command = version_command(executable);
    let Ok(Ok(result)) = tokio::time::timeout(Duration::from_secs(10), command.output()).await
    else {
        return Ok(None);
    };
    version_output(result.status.success(), &result.stdout, &result.stderr)
}

fn version_command(executable: &Path) -> Command {
    let mut command = Command::new(executable);
    command
        .args(["-c", "import sys; print(f'{sys.version_info.major}.{sys.version_info.minor}')"])
        .stdin(Stdio::inherit())
        .kill_on_drop(true);
    command
}

fn version_output(success: bool, stdout: &[u8], stderr: &[u8]) -> Result<Option<String>> {
    // subprocess.run(text=True) decodes both streams before checking exit status.
    let stdout = text::decode(stdout)?;
    text::decode(stderr)?;
    if !success {
        return Ok(None);
    }
    let version = crate::util::strings::python_trim(&stdout);
    Ok((!version.is_empty()).then(|| version.to_owned()))
}

pub(super) async fn has_pip(executable: &Path) -> bool {
    let mut command = pip_command(executable);
    tokio::time::timeout(Duration::from_secs(10), command.output())
        .await
        .is_ok_and(|result| result.is_ok_and(|output| output.status.success()))
}

fn pip_command(executable: &Path) -> Command {
    let mut command = Command::new(executable);
    command.args(["-c", "import pip"]).stdin(Stdio::inherit()).kill_on_drop(true);
    command
}
