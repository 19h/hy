//! GitHub release discovery, download, and binary replacement.

use std::path::{Path, PathBuf};

use regex::Regex;
use reqwest::blocking::Client;
use semver::{Version, VersionReq};
use serde::Deserialize;

use crate::config::Env;
use crate::error::{Error, Result};
use crate::util::io::check_free_space;

/// Minimal representation of a GitHub release asset.
#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
pub struct ReleaseAsset {
    pub id: u64,
    pub name: String,
    pub size: u64,
    pub browser_download_url: String,
}

/// Minimal representation of a GitHub release.
#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
pub struct Release {
    pub tag_name: String,
    pub prerelease: bool,
    pub draft: bool,
    pub assets: Vec<ReleaseAsset>,
}

/// GitHub repo coordinates.
#[derive(Debug, Clone)]
pub struct GitHubRepo {
    pub owner: String,
    pub repo: String,
    pub token: Option<String>,
}

impl GitHubRepo {
    /// Parse from a GitHub URL like `https://github.com/Owner/Repo`.
    pub fn from_url(url: &str) -> Result<Self> {
        let parsed =
            url::Url::parse(url).map_err(|e| Error::Other(format!("Invalid GitHub URL: {e}")))?;
        let mut segments = parsed
            .path_segments()
            .ok_or_else(|| Error::Other("GitHub URL has no path segments".into()))?;
        let owner = segments
            .next()
            .ok_or_else(|| Error::Other("Missing owner in GitHub URL".into()))?
            .to_owned();
        let repo = segments
            .next()
            .ok_or_else(|| Error::Other("Missing repo in GitHub URL".into()))?
            .trim_end_matches(".git")
            .to_owned();

        Ok(Self {
            owner,
            repo,
            token: Env::global().github_token.clone(),
        })
    }

    fn api_url(&self, path: &str) -> String {
        format!(
            "{}/repos/{}/{}{path}",
            Env::global().github_api_url,
            self.owner,
            self.repo
        )
    }

    fn client(&self) -> Result<Client> {
        let mut builder = Client::builder()
            .user_agent(format!("hcli/{}", Env::global().version))
            .timeout(std::time::Duration::from_secs(30));

        if let Some(ref token) = self.token {
            builder = builder.default_headers({
                let mut h = reqwest::header::HeaderMap::new();
                h.insert(
                    reqwest::header::AUTHORIZATION,
                    format!("Bearer {token}").parse().unwrap(),
                );
                h
            });
        }

        builder.build().map_err(Error::from)
    }
}

/// Fetch all releases from a GitHub repository.
pub fn get_releases(repo: &GitHubRepo) -> Result<Vec<Release>> {
    let client = repo.client()?;
    let mut all = Vec::new();
    let mut page = 1u32;

    loop {
        let url = repo.api_url(&format!("/releases?per_page=100&page={page}"));
        let resp: Vec<Release> = client.get(&url).send()?.json()?;
        if resp.is_empty() {
            break;
        }
        all.extend(resp);
        page += 1;
    }

    Ok(all)
}

/// Get all available versions, filtering out drafts.
pub fn available_versions(repo: &GitHubRepo) -> Result<Vec<Version>> {
    let releases = get_releases(repo)?;
    let mut versions: Vec<Version> = releases
        .iter()
        .filter(|r| !r.draft)
        .filter_map(|r| super::version::parse_version(&r.tag_name))
        .collect();
    versions.sort();
    versions.reverse();
    Ok(versions)
}

/// Find the latest version matching a version requirement.
pub fn compatible_version(
    repo: &GitHubRepo,
    req: &VersionReq,
    include_dev: bool,
) -> Result<Option<Version>> {
    let versions = available_versions(repo)?;
    Ok(versions
        .into_iter()
        .find(|v| req.matches(v) && (include_dev || !super::version::is_dev_version(v))))
}

/// Get assets for a specific release tag, filtered by regex.
pub fn get_assets(repo: &GitHubRepo, tag: &str, mask: &Regex) -> Result<Vec<ReleaseAsset>> {
    let client = repo.client()?;
    let url = repo.api_url(&format!("/releases/tags/{tag}"));
    let release: Release = client.get(&url).send()?.json()?;
    Ok(release
        .assets
        .into_iter()
        .filter(|a| mask.is_match(&a.name))
        .collect())
}

/// Download a release asset to a temporary file.
pub fn download_asset(repo: &GitHubRepo, asset: &ReleaseAsset) -> Result<PathBuf> {
    let client = repo.client()?;
    let tmp_dir = tempfile::tempdir()?;
    let target = tmp_dir.keep().join(&asset.name);

    check_free_space(target.parent().unwrap_or(Path::new(".")), asset.size)?;

    let mut resp = client
        .get(&asset.browser_download_url)
        .header(reqwest::header::ACCEPT, "application/octet-stream")
        .send()?;

    let mut file = std::fs::File::create(&target)?;
    std::io::copy(&mut resp, &mut file)?;

    Ok(target)
}

/// Atomically replace the running binary with a downloaded update.
pub fn update_binary(asset: &ReleaseAsset, repo: &GitHubRepo, binary_path: &Path) -> Result<bool> {
    let downloaded = download_asset(repo, asset)?;

    // On Windows: rename current → .bak, move new → current.
    if cfg!(target_os = "windows") {
        let backup = binary_path.with_extension("exe.bak");
        let _ = std::fs::remove_file(&backup);
        std::fs::rename(binary_path, &backup)?;
        std::fs::rename(&downloaded, binary_path)?;
    } else {
        // Unix: overwrite in place (or rename).
        #[cfg(unix)]
        {
            let perms = std::fs::metadata(binary_path)?.permissions();
            std::fs::rename(&downloaded, binary_path)?;
            std::fs::set_permissions(binary_path, perms)?;
        }
        #[cfg(not(unix))]
        {
            std::fs::rename(&downloaded, binary_path)?;
        }
    }

    Ok(true)
}
