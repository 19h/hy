//! Ordered catalogue acquisition and the logical identities of archive caches.

use std::collections::HashSet;
use std::path::PathBuf;

use crate::error::Result;
use crate::util::pydantic_integer::Integer;
use crate::util::{python_json::Text, python_path};

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
    pub url: Text,
    kind: Kind,
}

enum Kind {
    Asset {
        tag: Text,
        name: Text,
        size: Integer,
    },
    Source {
        commit: Text,
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
            if release.published_at.compare(FIRST_RELEASE_DATE).is_lt() {
                continue;
            }
            seen_sources.insert(release.zipball_url.clone());
            // Every release contributes its source, even when URLs repeat.
            self.sources.push(Archive::source(name, release.commit_hash, release.zipball_url));
            for asset in release.assets {
                if !["application/zip", "application/x-zip-compressed", "raw"]
                    .iter()
                    .any(|kind| asset.content_type.equals(kind))
                    || !asset.name.has_zip_suffix()
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
                && !tag.committed_date.compare(FIRST_RELEASE_DATE).is_lt()
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
    fn source(repository: &str, commit: Text, url: Text) -> Self {
        Self {
            repository: repository.into(),
            url,
            kind: Kind::Source {
                commit,
            },
        }
    }

    /// None means the filename cannot be encoded by the native filesystem.
    /// Python treats that existence probe as a cache miss, but publication fails.
    pub fn cache_path(&self) -> Result<Option<PathBuf>> {
        let (owner, repo) = super::discovery::parse_repository(&self.repository)?;
        let owner = Text::from(owner);
        let repo = Text::from(repo);
        match &self.kind {
            Kind::Asset {
                tag,
                name,
                ..
            } => {
                let directory =
                    cache::directory_text(&[&owner, &repo, &"release-assets".into(), tag])?;
                Ok(python_path::join_text(&directory, name))
            }
            Kind::Source {
                commit,
            } => {
                let directory =
                    cache::directory_text(&[&owner, &repo, &"source-archives".into(), commit])?;
                Ok(Some(directory.join("source.zip")))
            }
        }
    }

    pub fn exceeds_download_limit(&self) -> bool {
        matches!(&self.kind, Kind::Asset { size, .. } if *size > Integer::from(MAX_ASSET_BYTES))
    }
}

#[cfg(test)]
mod tests;
