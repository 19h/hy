//! Repository downloads and redirect-scoped authentication.

use sha2::{Digest, Sha256};

use super::Location;
use crate::error::{Error, Result};

mod file_url;
mod github_url;

pub(super) use file_url::path as local_file_path;

pub fn credential_host(url: &url::Url) -> bool {
    url.scheme() == "https"
        && url.host_str().is_some_and(|host| {
            host == "plugins.hex-rays.com" || host.ends_with(".plugins.hex-rays.com")
        })
}

/// Recompute credentials per redirect; never forward x-api-key to mirrors or presigned storage.
pub async fn fetch(url: &str) -> Result<Vec<u8>> {
    if let Some(path) = local_file_path(url)? {
        if !crate::util::python_path::exists(&path)? {
            return Err(Error::Other(format!("File not found: {}", path.display())));
        }
        return Ok(std::fs::read(path)?);
    }
    let mut current = url::Url::parse(url).map_err(|e| Error::Other(e.to_string()))?;
    let secure = current.scheme() == "https";
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(std::time::Duration::from_secs(30))
        .build()?;
    for _ in 0..=10 {
        if !matches!(current.scheme(), "https" | "http") || (secure && current.scheme() != "https")
        {
            return Err(Error::Other("unsupported protocol or HTTPS redirect downgrade".into()));
        }
        let mut request = client
            .get(current.clone())
            .header("User-Agent", concat!("hy/", env!("CARGO_PKG_VERSION")));
        if credential_host(&current) {
            request = request.headers(crate::auth::request_headers(false).await?);
        } else if current.host_str() == Some("api.github.com")
            && let Some(token) = &crate::config::Env::global().github_token
        {
            request = request.bearer_auth(token);
        }
        let response = request.send().await?;
        if response.status().is_redirection()
            && let Some(location) = response.headers().get("location").and_then(|v| v.to_str().ok())
        {
            current = current.join(location).map_err(|e| Error::Other(e.to_string()))?;
            continue;
        }
        if !response.status().is_success() {
            return Err(Error::from_status(response.status().as_u16(), ""));
        }
        return Ok(response.bytes().await?.to_vec());
    }
    Err(Error::Other("too many repository redirects".into()))
}

pub(super) fn verify_checksum(location: &Location, bytes: &[u8]) -> Result<()> {
    let actual = format!("{:x}", Sha256::digest(bytes));
    if actual != location.sha256 {
        return Err(Error::Other(format!(
            "hash mismatch: expected {} but found {actual} for {}",
            location.sha256, location.url,
        )));
    }
    Ok(())
}

#[derive(serde::Deserialize)]
struct Release {
    assets: Vec<ReleaseAsset>,
}

#[derive(serde::Deserialize)]
struct ReleaseAsset {
    name: String,
    browser_download_url: String,
    #[serde(default)]
    size: u64,
}

pub async fn github_archive(source: &str) -> Result<Vec<u8>> {
    let github_url::ReleaseSource {
        owner,
        repository,
        tag,
    } = github_url::parse(source)?;
    let release = match tag {
        Some(tag) => format!("tags/{tag}"),
        None => "latest".into(),
    };
    let endpoint = format!(
        "{}/repos/{owner}/{repository}/releases/{release}",
        crate::config::Env::global().github_api_url.trim_end_matches('/')
    );
    let release: Release = serde_json::from_slice(&fetch(&endpoint).await?)?;
    let assets: Vec<_> =
        release.assets.iter().filter(|asset| asset.name.to_lowercase().ends_with(".zip")).collect();
    let asset = match assets.as_slice() {
        [asset] => *asset,
        [] => return Err(Error::NotFound("no .zip asset in GitHub release".into())),
        _ => {
            return Err(Error::PluginInstall(format!(
                "multiple .zip assets in GitHub release: {}",
                assets.iter().map(|asset| asset.name.as_str()).collect::<Vec<_>>().join(", ")
            )));
        }
    };
    if asset.size > 100 * 1024 * 1024 {
        return Err(Error::PluginInstall("GitHub release asset exceeds 100 MiB".into()));
    }
    fetch(&asset.browser_download_url).await
}
