//! GitHub release discovery with preserved tags and upstream selection order.

use reqwest::blocking::Client;
use semver::Version;
use serde::Deserialize;

use crate::config::Env;
use crate::error::{Error, Result};

#[derive(Debug, Clone, Deserialize)]
pub struct ReleaseAsset {
    pub id: u64,
    pub name: String,
    pub size: u64,
}

impl ReleaseAsset {
    pub fn is_valid(&self) -> bool {
        self.id > 0 && self.size > 0 && !self.name.trim().is_empty()
    }
}

#[derive(Debug, Deserialize)]
struct Release {
    tag_name: Option<String>,
}

#[derive(Deserialize)]
struct ReleaseAssets {
    assets: Option<Vec<serde_json::Value>>,
}

pub struct ReleaseVersion {
    pub version: Version,
    pub tag: String,
}

#[derive(Debug)]
pub struct GitHubRepo {
    owner: String,
    repo: String,
}

impl GitHubRepo {
    pub fn from_url(source: &str) -> Result<Self> {
        let path = if let Some(path) = source.strip_prefix("git@github.com:") {
            path.to_owned()
        } else {
            let url = url::Url::parse(source)
                .map_err(|error| Error::UpdateFailed(format!("invalid GitHub URL: {error}")))?;
            if url.scheme() != "https" || url.host_str() != Some("github.com") {
                return Err(Error::UpdateFailed("expected an HTTPS or SSH GitHub URL".into()));
            }
            url.path().trim_matches('/').to_owned()
        };
        let path = path.strip_suffix(".git").unwrap_or(&path);
        let (owner, repo) = path
            .split_once('/')
            .ok_or_else(|| Error::UpdateFailed("GitHub URL requires owner/repository".into()))?;
        let valid = |part: &str| {
            !part.is_empty()
                && part != "."
                && part != ".."
                && part.bytes().all(|byte| byte.is_ascii_alphanumeric() || b"-_.".contains(&byte))
        };
        if !valid(owner) || !valid(repo) {
            return Err(Error::UpdateFailed("invalid GitHub owner/repository".into()));
        }
        Ok(Self {
            owner: owner.into(),
            repo: repo.into(),
        })
    }

    pub(super) fn api_url(&self, path: &str) -> String {
        format!(
            "{}/repos/{}/{}{path}",
            Env::global().github_api_url.trim_end_matches('/'),
            self.owner,
            self.repo
        )
    }

    pub(super) fn client(&self) -> Result<Client> {
        Ok(Client::builder()
            .user_agent(concat!("hy/", env!("CARGO_PKG_VERSION")))
            .timeout(std::time::Duration::from_secs(30))
            .build()?)
    }

    pub(super) fn get(&self, client: &Client, path: &str) -> reqwest::blocking::RequestBuilder {
        let request = client.get(self.api_url(path));
        match &Env::global().github_token {
            Some(token) => request.bearer_auth(token),
            None => request,
        }
    }
}

pub fn compatible_version(
    repo: &GitHubRepo,
    requirement: &str,
    include_dev: bool,
) -> Result<Option<ReleaseVersion>> {
    if !crate::plugin::valid_specification(requirement) {
        return Err(Error::UpdateFailed(format!("invalid version requirement: {requirement}")));
    }
    let client = repo.client()?;
    let mut latest: Option<ReleaseVersion> = None;
    for page in 1.. {
        let data: serde_json::Value =
            repo.get(&client, &format!("/releases?per_page=100&page={page}")).send()?.json()?;
        let Some(items) = data.as_array() else {
            break;
        };
        if items.contains(&serde_json::Value::String("message".into())) {
            break;
        }
        let releases: Vec<Release> = serde_json::from_value(data)?;
        let last_page = releases.len() < 100;
        for release in releases {
            consider_release(&mut latest, release, requirement, include_dev);
        }
        if last_page {
            break;
        }
    }
    Ok(latest)
}

fn consider_release(
    latest: &mut Option<ReleaseVersion>,
    release: Release,
    requirement: &str,
    include_dev: bool,
) {
    let Some(tag) = release.tag_name else {
        return;
    };
    let Some(version) = super::version::parse_version(&tag) else {
        return;
    };
    if (!include_dev && super::version::is_dev_tag(&tag))
        || !crate::plugin::version_matches(&version.to_string(), requirement)
    {
        return;
    }
    // Upstream's stable sort selects the last equal-precedence release.
    if latest.as_ref().is_none_or(|previous| !version.cmp_precedence(&previous.version).is_lt()) {
        *latest = Some(ReleaseVersion {
            version,
            tag,
        });
    }
}

pub fn get_assets(repo: &GitHubRepo, tag: &str, mask: &regex::Regex) -> Result<Vec<ReleaseAsset>> {
    let client = repo.client()?;
    const TAG_ESCAPE: &percent_encoding::AsciiSet =
        &percent_encoding::CONTROLS.add(b' ').add(b'/').add(b'?').add(b'#').add(b'%');
    let encoded_tag = percent_encoding::utf8_percent_encode(tag, TAG_ESCAPE);
    let data: serde_json::Value =
        repo.get(&client, &format!("/releases/tags/{encoded_tag}")).send()?.json()?;
    if data.get("message").is_some() {
        return Ok(Vec::new());
    }
    let release: ReleaseAssets = serde_json::from_value(data)?;
    Ok(release
        .assets
        .unwrap_or_default()
        .into_iter()
        .filter_map(|value| serde_json::from_value::<ReleaseAsset>(value).ok())
        .filter(|asset| asset.is_valid() && mask.is_match(&asset.name))
        .collect())
}

#[cfg(test)]
mod tests;
