//! Upstream catalogue paths and publication; metadata expires after one day.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::error::{Error, Result};
use crate::util::{python_json::Text, python_path};

pub(super) fn candidates_path() -> Result<PathBuf> {
    Ok(directory(&[])?.join("candidate_repos.json"))
}

pub(super) fn metadata_path(repository: &str) -> Result<PathBuf> {
    let (owner, repo) = super::discovery::parse_repository(repository)?;
    Ok(directory(&[owner, repo])?.join("releases.json"))
}

pub(super) fn directory(parts: &[&str]) -> Result<PathBuf> {
    let root = prepare_root()?;
    create_directory(directory_path(&root, parts)?)
}

pub(super) fn candidate_metadata_path(repository: &Text) -> Result<PathBuf> {
    let (owner, repo) = super::discovery::parse_name(repository)?;
    Ok(directory_text(&[&owner, &repo])?.join("releases.json"))
}

pub(super) fn directory_text(parts: &[&Text]) -> Result<PathBuf> {
    let root = prepare_root()?;
    let mut path = python_path::join(&root, "");
    for part in parts {
        let text = part.to_utf8().map_err(|_| {
            Error::GitHubValue(format!(
                "Invalid path component: '{}'. Must contain only ASCII characters",
                part.diagnostic()
            ))
        })?;
        validate_component(&text)?;
        path = python_path::join(&path, &text);
    }
    create_directory(path)
}

fn prepare_root() -> Result<PathBuf> {
    let root = crate::util::cache::default_cache_dir();
    // The source default-root helper creates its directory, but an explicit
    // HCLI_CACHE_DIR is created only after component validation succeeds.
    if std::env::var_os("HCLI_CACHE_DIR").is_none_or(|value| value.is_empty()) {
        std::fs::create_dir_all(&root)?;
    }
    Ok(root)
}

fn create_directory(path: PathBuf) -> Result<PathBuf> {
    reject_nul(&path)?;
    std::fs::create_dir_all(&path)?;
    Ok(path)
}

fn directory_path(root: &Path, parts: &[&str]) -> Result<PathBuf> {
    let mut path = python_path::join(root, "");
    for part in parts {
        validate_component(part)?;
        path = python_path::join(&path, part);
    }
    Ok(path)
}

pub(super) fn validate_component(part: &str) -> Result<()> {
    let reason = if part.is_empty() || matches!(part, "." | "..") {
        Some("")
    } else if !part.is_ascii() {
        Some(" Must contain only ASCII characters")
    } else if part.contains(['\t', '\n', '\r']) {
        Some(" Cannot contain tabs or newlines")
    } else if part.contains(['/', '\\']) {
        Some(" Cannot contain slashes")
    } else {
        None
    };
    if let Some(reason) = reason {
        return Err(Error::GitHubValue(format!("Invalid path component: '{part}'.{reason}")));
    }
    Ok(())
}

pub fn read(path: &Path, lifetime: Option<Duration>) -> Result<Option<Vec<u8>>> {
    read_path(path, lifetime, SystemTime::now)
}

fn read_path(
    path: &Path,
    lifetime: Option<Duration>,
    clock: impl FnOnce() -> SystemTime,
) -> Result<Option<Vec<u8>>> {
    if !python_path::exists(path)? {
        return Ok(None);
    }
    if let Some(lifetime) = lifetime {
        let now = clock();
        let modified = std::fs::metadata(path)?.modified()?;
        // Python subtracts two binary64 epoch timestamps, including future dates.
        let age = epoch_seconds(now) - epoch_seconds(modified);
        if age > lifetime.as_secs_f64() {
            std::fs::remove_file(path)?;
            return Ok(None);
        }
    }
    Ok(Some(std::fs::read(path)?))
}

fn epoch_seconds(time: SystemTime) -> f64 {
    match time.duration_since(UNIX_EPOCH) {
        Ok(duration) => duration.as_secs_f64(),
        Err(error) => -error.duration().as_secs_f64(),
    }
}

pub fn write(path: &Path, bytes: &[u8]) -> Result<()> {
    reject_nul(path)?;
    // Path.write_bytes follows existing links and does not create asset-name parents.
    std::fs::write(path, bytes)?;
    Ok(())
}

pub(super) fn write_text(path: &Path, mut text: String) -> Result<()> {
    if cfg!(windows) {
        text = text.replace('\n', "\r\n");
    }
    write(path, text.as_bytes())
}

fn reject_nul(path: &Path) -> Result<()> {
    if path.as_os_str().as_encoded_bytes().contains(&0) {
        return Err(Error::GitHubValue("embedded null byte".into()));
    }
    Ok(())
}

#[cfg(test)]
mod layout_tests;
#[cfg(test)]
mod tests;
