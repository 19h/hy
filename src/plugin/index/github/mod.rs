//! Discover GitHub plugin releases and tags, then index their distribution and source archives.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::de::DeserializeOwned;
use serde_json::json;

use super::{ArchiveCatalogue, LoadedRepository};
use crate::error::{Error, Result};

mod cache;
mod http;
mod models;
mod retry;

use models::{GraphResponse, Repository, SearchResponse};

const METADATA_LIFETIME: Duration = Duration::from_secs(86_400);
const FIRST_RELEASE_DATE: &str = "2025-09-01";
const MAX_ASSET_BYTES: u64 = 100 * 1024 * 1024;

#[derive(Debug, Default)]
pub struct Options {
    pub repositories_file: Option<PathBuf>,
    pub ignored_file: Option<PathBuf>,
}

struct Client {
    http: reqwest::Client,
    base: String,
    token: String,
    offline: bool,
}

impl Client {
    fn cache_key(&self, resource: &str) -> String {
        // Account and API origin partition private repository discovery results.
        format!("{}\n{}\n{resource}", self.base, self.token)
    }

    async fn json<T: DeserializeOwned>(&self, request: reqwest::RequestBuilder) -> Result<T> {
        if self.offline {
            return Err(Error::Other("GitHub metadata is unavailable in the local cache".into()));
        }
        let request = request.bearer_auth(&self.token).build()?;
        let response = retry::send(&self.http, request).await?;
        http::require_success(&response)?;
        Ok(response.json().await?)
    }

    async fn candidates(&self) -> Result<BTreeSet<String>> {
        let key = self.cache_key("candidates");
        if let Some(bytes) = cache::read(&key, Some(METADATA_LIFETIME))
            && let Ok(candidates) = serde_json::from_slice(&bytes)
        {
            return Ok(candidates);
        }
        let mut repositories = BTreeSet::new();
        for query in ["filename:ida-plugin.json", "filename:ida-plugin.json fork:true"] {
            for page in 1.. {
                let response: SearchResponse = self
                    .json(self.http.get(format!("{}/search/code", self.base)).query(&[
                        ("q", query.to_owned()),
                        ("per_page", "100".into()),
                        ("page", page.to_string()),
                    ]))
                    .await?;
                let count = response.items.len();
                for item in response.items {
                    repositories.insert(normalize_repository(&item.repository.full_name)?);
                }
                if count < 100 {
                    break;
                }
            }
        }
        cache::write(&key, &serde_json::to_vec(&repositories)?)?;
        Ok(repositories)
    }

    async fn releases(&self, repository: &str) -> Result<Repository> {
        let key = self.cache_key(&format!("releases/{repository}"));
        if let Some(bytes) = cache::read(&key, Some(METADATA_LIFETIME))
            && let Ok(releases) = serde_json::from_slice(&bytes)
        {
            return Ok(releases);
        }
        let (owner, name) = repository.split_once('/').expect("repository validated at discovery");
        let response: GraphResponse = self
            .json(self.http.post(format!("{}/graphql", self.base)).json(&json!({
                "query": include_str!("releases.graphql"),
                "variables": {"owner": owner, "name": name},
            })))
            .await?;
        if let Some(error) = response.errors.iter().find(|error| error.kind != "NOT_FOUND") {
            return Err(Error::Other(format!("GitHub GraphQL: {}", error.message)));
        }
        let repository = response
            .data
            .and_then(|data| data.repository)
            .filter(|repository| repository.default_branch_ref.is_some())
            .ok_or_else(|| Error::NotFound(format!("GitHub repository {repository}")))?;
        cache::write(&key, &serde_json::to_vec(&repository)?)?;
        Ok(repository)
    }

    async fn archive(&self, url: &str) -> Result<Vec<u8>> {
        let key = self.cache_key(&format!("archive/{url}"));
        if let Some(bytes) = cache::read(&key, None) {
            return Ok(bytes);
        }
        if self.offline {
            return Err(Error::Other(format!("archive unavailable offline: {url}")));
        }
        let bytes = if url.starts_with("file://") {
            super::fetch(url).await?
        } else {
            http::download(url).await?
        };
        cache::write(&key, &bytes)?;
        Ok(bytes)
    }
}

fn normalize_repository(value: &str) -> Result<String> {
    let parts: Vec<_> = value.split('/').collect();
    if parts.len() != 2
        || parts.iter().any(|part| {
            part.is_empty()
                || matches!(*part, "." | "..")
                || !part.bytes().all(|byte| byte.is_ascii_alphanumeric() || b"._-".contains(&byte))
        })
    {
        return Err(Error::Other(format!("invalid GitHub repository: {value}")));
    }
    Ok(value.to_lowercase())
}

fn read_list(path: Option<&Path>) -> Result<BTreeSet<String>> {
    let Some(path) = path else {
        return Ok(BTreeSet::new());
    };
    std::fs::read_to_string(path)?
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(normalize_repository)
        .collect()
}

fn archive_urls(repository: &Repository) -> Vec<String> {
    let mut assets = BTreeSet::new();
    let mut sources = BTreeSet::new();
    for release in &repository.releases.nodes {
        if release.published_at.as_deref().is_none_or(|date| date < FIRST_RELEASE_DATE) {
            continue;
        }
        if let Some(tag) = &release.tag {
            sources.insert(tag.target.commit().zipball_url.clone());
        }
        for asset in &release.release_assets.nodes {
            if asset.size <= MAX_ASSET_BYTES
                && asset.name.to_lowercase().ends_with(".zip")
                && matches!(
                    asset.content_type.as_str(),
                    "application/zip" | "application/x-zip-compressed" | "raw"
                )
            {
                assets.insert(asset.download_url.clone());
            }
        }
    }
    for reference in &repository.refs.nodes {
        let commit = reference.target.commit();
        if reference.name.starts_with('v') && commit.committed_date.as_str() >= FIRST_RELEASE_DATE {
            sources.insert(commit.zipball_url.clone());
        }
    }
    assets.into_iter().chain(sources).collect()
}

pub async fn load(options: &Options, offline: bool) -> Result<LoadedRepository> {
    let token = crate::config::Env::global()
        .github_token
        .clone()
        .ok_or_else(|| Error::Other("GitHub token required; set GITHUB_TOKEN".into()))?;
    let client = Client {
        http: http::metadata_client()?,
        base: crate::config::Env::global().github_api_url.trim_end_matches('/').into(),
        token,
        offline,
    };
    let extra = read_list(options.repositories_file.as_deref())?;
    let ignored = read_list(options.ignored_file.as_deref())?;
    let mut repositories = client.candidates().await?;
    repositories.extend(extra);
    repositories.retain(|repository| !ignored.contains(repository));
    let mut loaded = LoadedRepository::empty();
    let mut catalogue = ArchiveCatalogue::default();
    for name in repositories {
        let repository = match client.releases(&name).await {
            Ok(repository) => repository,
            Err(Error::NotFound(_)) => {
                loaded.notes.push(format!("GitHub repository {name} was not found"));
                continue;
            }
            Err(error) => return Err(error),
        };
        let host = format!("https://github.com/{name}");
        for url in archive_urls(&repository) {
            let bytes = client.archive(&url).await?;
            super::archive::add_bytes(&mut catalogue, &bytes, &url, Some(&host))?;
        }
    }
    loaded.snapshot.plugins = catalogue.into_plugins()?;
    Ok(loaded)
}
