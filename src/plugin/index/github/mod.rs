//! Discover GitHub plugin releases and tags, then index their distribution and source archives.

use std::path::PathBuf;
use std::time::Duration;

use super::{ArchiveCatalogue, LoadedRepository};
use crate::error::{Error, Result};

mod acquisition;
mod cache;
mod discovery;
mod graphql;
mod http;
mod metadata;
mod models;
mod redirect;
mod retry;

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
    async fn response(&self, request: reqwest::RequestBuilder) -> Result<reqwest::Response> {
        if self.offline {
            return Err(Error::Other("GitHub metadata is unavailable in the local cache".into()));
        }
        let request = request.bearer_auth(&self.token).build()?;
        retry::send(&self.http, request).await
    }

    async fn archive(&self, archive: &acquisition::Archive) -> Result<Option<Vec<u8>>> {
        match self.acquire_archive(archive).await {
            Err(Error::GitHubValue(_)) => Ok(None),
            result => result,
        }
    }

    async fn acquire_archive(&self, archive: &acquisition::Archive) -> Result<Option<Vec<u8>>> {
        if let Some(bytes) = cache::read(&archive.cache_path()?, None)? {
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
        let bytes = result?;
        cache::write(&archive.cache_path()?, &bytes)?;
        Ok(Some(bytes))
    }
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
    let extra = discovery::read_list(options.repositories_file.as_deref())?;
    let ignored = discovery::read_list(options.ignored_file.as_deref())?;
    let names = discovery::select(client.candidates().await?, extra, ignored)?;
    let mut repositories = client.warm_releases(&names).await?;
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
