//! Batch invocation and log framing. Exit status and stdout do not select the result.

use std::path::Path;
use std::process::Stdio;

use serde_json::Value;
use tokio::process::Command;

use crate::error::{Error, Result};

pub(super) fn command(idat: &Path, idausr: &Path, interactive: bool) -> Command {
    let mut command = Command::new(idat);
    for key in ["VIRTUAL_ENV", "PYTHONHOME", "PYTHONPATH", "PATH"] {
        command.env_remove(key);
    }
    if let Some(venv) = super::super::environment::shell_venv() {
        command.env("VIRTUAL_ENV", venv);
    }
    command.env("IDAUSR", idausr);
    if interactive {
        command.env("IDA_IS_INTERACTIVE", "1");
    }
    command.stdin(Stdio::inherit()).kill_on_drop(true);
    command
}

pub(super) async fn run(
    idat: &Path,
    idausr: &Path,
    interactive: bool,
    source: &str,
) -> Result<Value> {
    let temporary = tempfile::tempdir()?;
    let script = temporary.path().join("idat-script.py");
    let log = temporary.path().join("ida.log");
    std::fs::write(&script, source)?;
    command(idat, idausr, interactive)
        .args(["-a", "-A", "-c", "-t"])
        .arg(format!("-L{}", log.display()))
        .arg(format!("-S{}", script.display()))
        .output()
        .await?;
    if !log.exists() {
        return Err(Error::IdaProbe(format!(
            "failed to invoke idat: log file was not created: {}",
            log.display(),
        )));
    }
    parse(&std::fs::read(log)?)
}

pub(super) fn parse(bytes: &[u8]) -> Result<Value> {
    let text = String::from_utf8_lossy(bytes).replace("\r\n", "\n").replace('\r', "\n");
    let lines: Vec<_> = text
        .split_terminator(|c| {
            matches!(
                c,
                '\n' | '\r' | '\x0b' | '\x0c' | '\x1c'
                    ..='\x1e' | '\u{85}' | '\u{2028}' | '\u{2029}'
            )
        })
        .collect();
    for line in &lines {
        if let Some(document) = line.strip_prefix("__hcli__:") {
            return Ok(serde_json::from_str(document)?);
        }
    }
    let tail = lines[lines.len().saturating_sub(20)..].join("\n");
    let mut message =
        "failed to invoke idat: could not find expected lines in log output".to_owned();
    if !tail.is_empty() {
        message.push_str(":\n");
        message.push_str(&tail);
    }
    Err(Error::IdaProbe(message))
}
