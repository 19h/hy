//! One interpreter process owns extension registration, routing and execution.

use std::path::{Path, PathBuf};
use std::process::Stdio;

use serde::Deserialize;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::process::Command;

use super::Catalog;
use crate::config::Env;
use crate::error::{Error, Result};

const BRIDGE: &str = include_str!("bridge.py");
const MAX_REPORT_BYTES: u32 = 1_048_576;

#[derive(Deserialize)]
struct Report {
    token: String,
    #[serde(flatten)]
    discovery: Discovery,
}

#[derive(Deserialize)]
#[serde(tag = "status", rename_all = "lowercase")]
enum Discovery {
    Ready {
        selected: bool,
        catalog: Catalog,
    },
    Failed {
        message: String,
    },
}

pub(super) fn runtime() -> Option<PathBuf> {
    if let Some(value) = std::env::var_os("HCLI_EXTENSION_PYTHON") {
        return (!value.is_empty()).then(|| PathBuf::from(value));
    }
    crate::ida::python::find_command("python3")
        .or_else(|| crate::ida::python::find_command("python"))
}

pub(super) async fn inspect(runtime: &Path) -> Result<(Catalog, Option<i32>)> {
    let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)).await?;
    let token_file = tempfile::Builder::new().prefix("hy-extension-").rand_bytes(24).tempfile()?;
    let token = token_file.path().file_name().unwrap().to_string_lossy();
    let environment = Env::global();
    let mut child = Command::new(runtime)
        .arg("-c")
        .arg(BRIDGE)
        .arg(listener.local_addr()?.port().to_string())
        .arg(token.as_ref())
        .args(std::env::args_os().skip(1))
        .env("HCLI_BINARY_NAME", &environment.binary_name)
        .env("HCLI_VERSION", &environment.version)
        .env("HCLI_VERSION_EXTRA", &environment.version_extra)
        .env("PYTHONUTF8", "1")
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .kill_on_drop(true)
        .spawn()
        .map_err(|error| {
            Error::Other(format!("extension runtime {}: {error}", runtime.display()))
        })?;

    let (mut connection, _) = tokio::select! {
        connected = listener.accept() => connected?,
        status = child.wait() => {
            return Err(Error::Other(format!("extension runtime exited before discovery: {}", status?)));
        }
    };
    let size = match connection.read_u32().await {
        Ok(size) => size,
        Err(error) if error.kind() == std::io::ErrorKind::UnexpectedEof => {
            // An extension may raise SystemExit during registration. Reap the
            // interpreter before returning so its status and final output survive.
            let status = child.wait().await?;
            return Ok((Catalog::default(), Some(status.code().unwrap_or(1))));
        }
        Err(error) => return Err(error.into()),
    };
    if size > MAX_REPORT_BYTES {
        return Err(Error::Other("extension command report exceeds 1 MiB".into()));
    }
    let mut bytes = vec![0; size as usize];
    connection.read_exact(&mut bytes).await?;
    let report: Report = serde_json::from_slice(&bytes)?;
    if report.token != token {
        return Err(Error::Other("extension runtime report has an invalid session token".into()));
    }
    let selected = matches!(
        report.discovery,
        Discovery::Ready {
            selected: true,
            ..
        }
    );
    connection
        .write_all(if selected {
            b"run\n"
        } else {
            b"stop\n"
        })
        .await?;
    drop(connection);
    let status = child.wait().await?;
    match report.discovery {
        Discovery::Failed {
            message,
        } => Err(Error::Other(format!("extension discovery failed: {message}"))),
        Discovery::Ready {
            catalog,
            selected: true,
        } => Ok((catalog, Some(status.code().unwrap_or(1)))),
        Discovery::Ready {
            catalog,
            selected: false,
        } if status.success() => Ok((catalog, None)),
        Discovery::Ready {
            ..
        } => Err(Error::Other(format!("extension runtime failed after discovery: {status}"))),
    }
}
