//! IDA 9.4 JSON IPC, with bounded reads and connected-peer credential checks.
use crate::error::{Error, Result};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(2);
const READ_TIMEOUT: Duration = Duration::from_secs(5);
const MAX_RESPONSE_BYTES: usize = 1024 * 1024;

#[derive(Debug, Clone)]
pub struct Instance {
    pub pid: u32,
    pub socket: PathBuf,
    pub idb_path: Option<PathBuf>,
}

async fn exchange<S: AsyncRead + AsyncWrite + Unpin>(
    stream: &mut S,
    command: &Value,
) -> Result<Value> {
    tokio::time::timeout(CONNECT_TIMEOUT, stream.write_all(&serde_json::to_vec(command)?))
        .await
        .map_err(|_| Error::Other("IDA IPC write timed out".into()))??;
    let mut response = Vec::new();
    let mut chunk = [0; 4096];
    loop {
        let count = match tokio::time::timeout(READ_TIMEOUT, stream.read(&mut chunk)).await {
            Ok(result) => result?,
            Err(_) => break,
        };
        if count == 0 {
            break;
        }
        if response.len() + count > MAX_RESPONSE_BYTES {
            return Err(Error::Other("IPC response exceeds 1 MiB".into()));
        }
        response.extend_from_slice(&chunk[..count]);
        if let Ok(value) = serde_json::from_slice::<Value>(&response) {
            return Ok(value);
        }
    }
    if response.is_empty() {
        return Err(Error::Other("No IDA IPC response received".into()));
    }
    Ok(serde_json::from_slice(&response)?)
}

pub async fn send(path: &Path, command: Value) -> Result<Value> {
    #[cfg(unix)]
    {
        let mut stream =
            tokio::time::timeout(CONNECT_TIMEOUT, tokio::net::UnixStream::connect(path))
                .await
                .map_err(|_| Error::Other("IDA IPC connection timed out".into()))??;
        let uid = stream.peer_cred()?.uid();
        // SAFETY: getuid has no arguments or memory preconditions.
        if uid != unsafe { libc::getuid() } {
            return Err(Error::Other("IPC peer belongs to another user".into()));
        }
        exchange(&mut stream, &command).await
    }
    #[cfg(windows)]
    {
        let mut stream = tokio::net::windows::named_pipe::ClientOptions::new().open(path)?;
        exchange(&mut stream, &command).await
    }
}

/// Query every candidate when relative navigation needs the full inventory.
pub async fn discover() -> Vec<Instance> {
    let mut instances = Vec::new();
    for candidate in candidates() {
        if let Some(instance) = query(candidate).await {
            instances.push(instance);
        }
    }
    instances
}

/// Named navigation stops querying as soon as a matching database responds.
pub async fn find(matches: impl Fn(&Instance) -> bool) -> Option<Instance> {
    find_in(candidates(), matches).await
}

async fn find_in(
    candidates: Vec<Instance>,
    matches: impl Fn(&Instance) -> bool,
) -> Option<Instance> {
    for candidate in candidates {
        if let Some(instance) = query(candidate).await
            && matches(&instance)
        {
            return Some(instance);
        }
    }
    None
}

fn candidates() -> Vec<Instance> {
    #[cfg(unix)]
    {
        unix::candidates()
    }
    #[cfg(windows)]
    {
        windows::candidates()
    }
}

async fn query(mut instance: Instance) -> Option<Instance> {
    let info = send(&instance.socket, json!({"cmd": "get_info"})).await.ok()?;
    if info["status"] != "ok" {
        return None;
    }
    instance.idb_path =
        info["idb_path"].as_str().filter(|path| !path.is_empty()).map(PathBuf::from);
    Some(instance)
}

pub async fn navigate(instance: &Instance, uri: &str) -> Result<()> {
    let result = send(&instance.socket, json!({"cmd":"open_ida_link","uri":uri})).await?;
    if result["status"] != "ok" {
        return Err(Error::Other(
            result["message"].as_str().unwrap_or("IDA navigation failed").into(),
        ));
    }
    Ok(())
}

#[cfg(all(test, unix))]
mod tests;
