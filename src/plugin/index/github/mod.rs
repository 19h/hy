//! Discover GitHub plugin releases and tags, then index their distribution and source archives.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::de::DeserializeOwned;

use super::{ArchiveCatalogue, LoadedRepository};
use crate::error::{Error, Result};

mod acquisition;
mod cache;
mod graphql;
mod http;
mod metadata;
mod models;
mod retry;

use models::SearchResponse;

const METADATA_LIFETIME: Duration = Duration::from_secs(86_400);

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
        if let Some(bytes) = cache::read(&key, Some(METADATA_LIFETIME))? {
            return Ok(serde_json::from_slice(&bytes)?);
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

    async fn archive(&self, archive: &acquisition::Archive) -> Result<Option<Vec<u8>>> {
        let key = self.cache_key(&archive.cache_resource());
        if let Some(bytes) = cache::read(&key, None)? {
            return Ok(Some(bytes));
        }
        // Upstream consults its cache before download_release_asset checks size.
        if archive.exceeds_download_limit() {
            return Ok(None);
        }
        let url = &archive.url;
        if self.offline {
            return Err(Error::Other(format!("archive unavailable offline: {url}")));
        }
        let result = if url.starts_with("file://") {
            super::fetch(url).await
        } else {
            http::download(url).await
        };
        let bytes = match result {
            Ok(bytes) => bytes,
            Err(Error::GitHubValue(_)) => return Ok(None),
            Err(error) => return Err(error),
        };
        cache::write(&key, &bytes)?;
        Ok(Some(bytes))
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
    let mut repositories: Vec<_> = repositories.into_iter().collect();
    client.warm_releases(&repositories).await?;
    repositories.sort_by(|left, right| left.split_once('/').cmp(&right.split_once('/')));
    let mut loaded = LoadedRepository::empty();
    let mut metadata = Vec::new();
    for name in repositories {
        let repository = match client.releases(&name).await {
            Ok(repository) => repository,
            Err(Error::NotFound(_)) => {
                loaded.notes.push(format!("GitHub repository {name} was not found"));
                continue;
            }
            Err(error) => return Err(error),
        };
        metadata.push((name, repository));
    }
    let mut catalogue = ArchiveCatalogue::default();
    for archive in acquisition::Plan::from_repositories(metadata).into_archives() {
        if let Some(bytes) = client.archive(&archive).await? {
            let host = format!("https://github.com/{}", archive.repository);
            super::archive::add_bytes(&mut catalogue, &bytes, &archive.url, Some(&host))?;
        }
    }
    loaded.snapshot.plugins = catalogue.into_plugins()?;
    Ok(loaded)
}
