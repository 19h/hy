//! Repository downloads and redirect-scoped authentication.

use sha2::{Digest, Sha256};

use super::Location;
use crate::error::{Error, Result};

mod file_url;
mod github_http;
mod github_release;
mod github_url;
mod repository;
mod response;
pub(super) mod url_parts;

pub(super) use file_url::path as local_file_path;

#[cfg(test)]
pub(super) use repository::credential_host;

/// Reevaluate credential eligibility on each hop, reusing credentials within one fetch.
pub async fn fetch(url: &str) -> Result<Vec<u8>> {
    if let Some(path) = local_file_path(url)? {
        if !crate::util::python_path::exists(&path)? {
            return Err(Error::Other(format!("File not found: {}", path.display())));
        }
        return Ok(std::fs::read(path)?);
    }
    repository::fetch(url).await
}

pub(super) fn verify_checksum(location: &Location, bytes: &[u8]) -> Result<()> {
    let actual = format!("{:x}", Sha256::digest(bytes));
    if actual != location.sha256 {
        return Err(Error::Other(format!(
            "hash mismatch: expected {} but found {actual} for {}",
            location.sha256,
            location.url.diagnostic(),
        )));
    }
    Ok(())
}

pub async fn github_archive(source: &str) -> Result<Vec<u8>> {
    let source = github_url::parse(source)?;
    let endpoint = format!(
        "{}{}",
        crate::config::Env::global().github_api_url.trim_end_matches('/'),
        github_release::endpoint(&source),
    );
    let release = github_http::fetch(
        &endpoint,
        std::time::Duration::from_secs(30),
        "application/vnd.github.v3+json",
    )
    .await?;
    let download = github_release::select(&source, &release)?;
    github_http::fetch(&download, std::time::Duration::from_secs(60), "*/*").await
}
