//! Ordered catalogue acquisition and the logical identities of archive caches.

use std::collections::HashSet;
use std::path::PathBuf;

use crate::error::Result;
use crate::util::pydantic_integer::Integer;
use crate::util::python_path;

use super::{cache, models::Repository};

const FIRST_RELEASE_DATE: &str = "2025-09-01";
const MAX_ASSET_BYTES: i64 = 104_857_600;

#[derive(Default)]
pub(super) struct Plan {
    assets: Vec<Archive>,
    sources: Vec<Archive>,
}

pub(super) struct Archive {
    pub repository: String,
    pub url: String,
    kind: Kind,
}

enum Kind {
    Asset {
        tag: String,
        name: String,
        size: Integer,
    },
    Source {
        commit: String,
    },
}

impl Plan {
    pub fn from_repositories(mut repositories: Vec<(String, Repository)>) -> Self {
        // Source collection sorts (owner, repository) tuples. Sorting joined
        // names instead reverses prefix owners such as "a" and "a-b".
        repositories
            .sort_by(|(left, _), (right, _)| left.split_once('/').cmp(&right.split_once('/')));
        let mut plan = Self::default();
        for (name, repository) in repositories {
            plan.append(&name, repository);
        }
        plan
    }

    fn append(&mut self, name: &str, repository: Repository) {
        let mut seen_sources = HashSet::new();
        for release in repository.releases {
            if release.published_at.as_str() < FIRST_RELEASE_DATE {
                continue;
            }
            seen_sources.insert(release.zipball_url.clone());
            // Every release contributes its source, even when URLs repeat.
            self.sources.push(Archive::source(name, release.commit_hash, release.zipball_url));
            for asset in release.assets {
                if !matches!(
                    asset.content_type.as_str(),
                    "application/zip" | "application/x-zip-compressed" | "raw"
                ) || !asset.name.to_lowercase().ends_with(".zip")
                {
                    continue;
                }
                self.assets.push(Archive {
                    repository: name.into(),
                    url: asset.download_url,
                    kind: Kind::Asset {
                        tag: release.tag_name.clone(),
                        name: asset.name,
                        size: asset.size,
                    },
                });
            }
        }
        for tag in repository.tags {
            if tag.tag_name.starts_with('v')
                && tag.committed_date.as_str() >= FIRST_RELEASE_DATE
                && seen_sources.insert(tag.zipball_url.clone())
            {
                self.sources.push(Archive::source(name, tag.commit_hash, tag.zipball_url));
            }
        }
    }

    pub fn into_archives(self) -> impl Iterator<Item = Archive> {
        self.assets.into_iter().chain(self.sources)
    }
}

impl Archive {
    fn source(repository: &str, commit: String, url: String) -> Self {
        Self {
            repository: repository.into(),
            url,
            kind: Kind::Source {
                commit,
            },
        }
    }

    pub fn cache_path(&self) -> Result<PathBuf> {
        let (owner, repo) = super::discovery::parse_repository(&self.repository)?;
        match &self.kind {
            Kind::Asset {
                tag,
                name,
                ..
            } => Ok(python_path::join(
                &cache::directory(&[owner, repo, "release-assets", tag])?,
                name,
            )),
            Kind::Source {
                commit,
            } => {
                Ok(cache::directory(&[owner, repo, "source-archives", commit])?.join("source.zip"))
            }
        }
    }

    pub fn exceeds_download_limit(&self) -> bool {
        matches!(&self.kind, Kind::Asset { size, .. } if *size > Integer::from(MAX_ASSET_BYTES))
    }
}

#[cfg(test)]
mod tests;
