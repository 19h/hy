//! Ordered catalogue acquisition and the logical identities of archive caches.

use std::collections::HashSet;

use super::models::Repository;

const FIRST_RELEASE_DATE: &str = "2025-09-01";
const MAX_ASSET_BYTES: u64 = 104_857_600;

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
        size: u64,
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
        for release in repository.releases.nodes {
            if release.published_at.as_deref().is_none_or(|date| date < FIRST_RELEASE_DATE) {
                continue;
            }
            if let Some(tag) = release.tag {
                let commit = tag.target.commit();
                seen_sources.insert(commit.zipball_url.clone());
                // Every release contributes its source, even when URLs repeat.
                self.sources.push(Archive::source(name, commit));
            }
            for asset in release.release_assets.nodes {
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
        for reference in repository.refs.nodes {
            let commit = reference.target.commit();
            if reference.name.starts_with('v')
                && commit.committed_date.as_str() >= FIRST_RELEASE_DATE
                && seen_sources.insert(commit.zipball_url.clone())
            {
                self.sources.push(Archive::source(name, commit));
            }
        }
    }

    pub fn into_archives(self) -> impl Iterator<Item = Archive> {
        self.assets.into_iter().chain(self.sources)
    }
}

impl Archive {
    fn source(repository: &str, commit: &super::models::Commit) -> Self {
        Self {
            repository: repository.into(),
            url: commit.zipball_url.clone(),
            kind: Kind::Source {
                commit: commit.oid.clone(),
            },
        }
    }

    pub fn cache_resource(&self) -> String {
        let parts = match &self.kind {
            Kind::Asset {
                tag,
                name,
                ..
            } => vec!["asset", &self.repository, tag, name],
            Kind::Source {
                commit,
            } => vec!["source", &self.repository, commit],
        };
        // Delimiter-bearing names remain distinct; old URL-only entries cannot
        // establish which release asset or commit supplied their bytes.
        format!("archive-v2/{}", serde_json::to_string(&parts).expect("string array serializes"))
    }

    pub fn exceeds_download_limit(&self) -> bool {
        matches!(&self.kind, Kind::Asset { size, .. } if *size > MAX_ASSET_BYTES)
    }
}

#[cfg(test)]
mod tests;
