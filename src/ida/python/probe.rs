//! IDA probe acquisition, with real-user startup followed by an isolated fallback.

use std::path::Path;

use serde::Deserialize;
use serde_json::Value;

use super::Probe;
use crate::error::{Error, Result};

mod batch;
mod files;

#[cfg(test)]
mod tests;

const SOURCE: &str = include_str!("probe/source.py");

#[derive(Deserialize)]
struct Observation {
    #[serde(default)]
    frozen: bool,
    prefix: String,
    base_prefix: String,
    executable: Option<String>,
    virtual_env: Option<String>,
    idapython_venv_executable: Option<String>,
    version_major: i64,
    version_minor: i64,
    #[serde(default)]
    externally_managed: bool,
}

impl From<Observation> for Probe {
    fn from(info: Observation) -> Self {
        Self {
            frozen: info.frozen,
            prefix: info.prefix,
            base_prefix: info.base_prefix,
            executable: info.executable,
            virtual_env: info.virtual_env,
            idapython_venv_executable: info.idapython_venv_executable,
            version: format!("{}.{}", info.version_major, info.version_minor),
            externally_managed: info.externally_managed,
        }
    }
}

/// Cache successful observations. Failed calls leave the next caller free to retry.
pub async fn probe_ida() -> Result<Probe> {
    static CACHE: tokio::sync::OnceCell<Probe> = tokio::sync::OnceCell::const_new();
    CACHE
        .get_or_try_init(|| async {
            let root = crate::ida::current_install_dir().ok_or(Error::IdaNotFound)?;
            probe_installation(&root).await
        })
        .await
        .cloned()
}

/// Probe an explicitly selected installation without changing the process cache.
pub async fn probe_installation(root: &Path) -> Result<Probe> {
    let idat =
        crate::ida::idat_path(root).ok_or_else(|| Error::NotFound("idat executable".into()))?;
    if cfg!(target_os = "linux") {
        let path = std::path::absolute(&idat)?;
        let path = path.to_string_lossy();
        if path.contains("9.2") && path.contains(' ') {
            tracing::warn!(
                "invoking idat on IDA 9.2/Linux with a space in the full path, you might encounter HCLI GitHub issue #99"
            );
        }
    }
    let document = acquire(&idat, &crate::ida::ida_user_dir()).await?;
    Ok(serde_json::from_value::<Observation>(document)?.into())
}

async fn acquire(idat: &Path, idausr: &Path) -> Result<Value> {
    if idausr.is_dir() {
        match batch::run(idat, idausr, true, SOURCE).await {
            Ok(document) => return Ok(document),
            Err(Error::IdaProbe(_)) => (),
            Err(error) => return Err(error),
        }
        let temporary = tempfile::tempdir()?;
        let isolated = temporary.path().join("idausr");
        files::prepare(idausr, &isolated)?;
        batch::run(idat, &isolated, false, SOURCE).await
    } else {
        batch::run(idat, idausr, false, SOURCE).await
    }
}
