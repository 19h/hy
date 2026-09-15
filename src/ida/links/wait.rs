//! Startup polling and the separate, cancellable auto-analysis wait.

use std::future::Future;
use std::path::Path;
use std::time::Duration;

use serde_json::{Value, json};

use crate::error::{Error, Result};
use crate::ida::ipc;
use crate::util::{fmt, tui};

pub(super) async fn database<F, Q>(timeout: f64, mut find: F) -> Result<ipc::Instance>
where
    F: FnMut() -> Q,
    Q: Future<Output = Option<ipc::Instance>>,
{
    let started = tokio::time::Instant::now();
    let mut interval = 0.1_f64;
    while started.elapsed().as_secs_f64() < timeout {
        if let Some(instance) = find().await {
            return Ok(instance);
        }
        tokio::time::sleep(Duration::from_secs_f64(interval)).await;
        interval = (interval * 1.5).min(2.0);
    }
    Err(Error::Other(format!(
        "IDA startup timed out after {timeout} s while waiting for an IDB instance"
    )))
}

pub(super) async fn analysis(socket: &Path) -> Result<()> {
    #[cfg(unix)]
    let mut interrupts = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::interrupt())?;
    #[cfg(windows)]
    let mut interrupts = tokio::signal::windows::ctrl_c()?;
    if tui::is_interactive() {
        fmt::info("Waiting for auto-analysis to complete (Ctrl+C to skip)...");
    }
    tokio::select! {
        result = poll_analysis(|| ipc::send(socket, json!({"cmd":"is_analysis_complete"}))) => result,
        interrupted = interrupts.recv() => {
            interrupted.ok_or_else(|| Error::Other("interrupt signal stream closed".into()))?;
            fmt::info("Analysis wait cancelled by user");
            Ok(())
        }
    }
}

async fn poll_analysis<F, Q>(mut query: F) -> Result<()>
where
    F: FnMut() -> Q,
    Q: Future<Output = Result<Value>>,
{
    loop {
        if analysis_complete(&query().await?)? {
            return Ok(());
        }
        tokio::time::sleep(Duration::from_secs(5)).await;
    }
}

fn analysis_complete(response: &Value) -> Result<bool> {
    match response.get("status") {
        Some(Value::String(status)) if status == "ok" => {
            Ok(response.get("analysis_complete").is_some_and(truthy))
        }
        None => Err(Error::Other("IDA analysis query returned no status".into())),
        Some(Value::String(status)) if status == "error" => Err(Error::Other(
            response["message"].as_str().unwrap_or("IDA analysis query failed").into(),
        )),
        _ => Ok(false),
    }
}

fn truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(value) => *value,
        Value::Number(value) => value.as_f64() != Some(0.0),
        Value::String(value) => !value.is_empty(),
        Value::Array(value) => !value.is_empty(),
        Value::Object(value) => !value.is_empty(),
    }
}

#[cfg(test)]
mod tests;
