//! Group archive locations using the source repository's identity and ordering rules.

use std::collections::{BTreeMap, BTreeSet};

use indexmap::IndexMap;

use super::{Location, Plugin};
use crate::error::{Error, Result};
use crate::plugin::{PluginManifest, PluginMetadata, parse_version};
use crate::util::python_sort;

mod equality;

#[derive(Eq, PartialEq, Hash)]
struct Compatibility {
    ida_versions: BTreeSet<String>,
    platforms: BTreeSet<String>,
}

impl Compatibility {
    fn from_metadata(metadata: &PluginMetadata) -> Self {
        Self {
            ida_versions: metadata.ida_versions.iter().cloned().collect(),
            platforms: metadata.platforms.iter().cloned().collect(),
        }
    }

    fn less_than(&self, other: &Self) -> bool {
        if self.ida_versions == other.ida_versions {
            self.platforms != other.platforms && self.platforms.is_subset(&other.platforms)
        } else {
            self.ida_versions.is_subset(&other.ida_versions)
        }
    }
}

type Variants = IndexMap<Compatibility, Vec<Location>>;
type Versions = IndexMap<String, Variants>;

#[derive(Default)]
pub(crate) struct ArchiveCatalogue {
    plugins: BTreeMap<(String, String), Versions>,
}

impl ArchiveCatalogue {
    pub fn add(&mut self, url: &str, sha256: &str, descriptor: PluginManifest) -> Result<()> {
        let metadata = &descriptor.metadata;
        let identity = (metadata.name.to_lowercase(), metadata.normalized_host()?);
        let variants =
            self.plugins.entry(identity).or_default().entry(metadata.version.clone()).or_default();
        let compatibility = Compatibility::from_metadata(metadata);
        variants.entry(compatibility).or_default().push(Location {
            url: url.into(),
            sha256: sha256.into(),
            descriptor,
        });
        Ok(())
    }

    pub fn into_plugins(self) -> Result<Vec<Plugin>> {
        let mut plugins = Vec::with_capacity(self.plugins.len());
        for ((mut name, host), versions) in self.plugins {
            let mut ordered = Vec::with_capacity(versions.len());
            for (version, variants) in versions {
                let parsed = parse_version(&version).ok_or_else(|| {
                    Error::Other(format!("invalid plugin repository version: {version}"))
                })?;
                ordered.push((parsed, version, variants));
            }
            ordered.sort_by(|left, right| left.0.cmp_precedence(&right.0));
            let mut versions = IndexMap::new();
            for (_, version, variants) in ordered {
                let mut groups: Vec<_> = variants.iter().collect();
                python_sort::sort_by(&mut groups, |left, right| left.0.less_than(right.0));
                let mut locations = Vec::new();
                for (_, group) in groups {
                    let mut group: Vec<_> = group.iter().collect();
                    group.sort_by(|left, right| {
                        (&left.url, &left.sha256).cmp(&(&right.url, &right.sha256))
                    });
                    reject_metadata_comparisons(&group)?;
                    for location in group {
                        name.clone_from(&location.descriptor.metadata.name);
                        locations.push(location.clone());
                    }
                }
                versions.insert(version, locations);
            }
            plugins.push(Plugin {
                name,
                host,
                versions,
            });
        }
        Ok(plugins)
    }
}

fn reject_metadata_comparisons(locations: &[&Location]) -> Result<()> {
    for pair in locations.windows(2) {
        let (left, right) = (pair[0], pair[1]);
        if left.url == right.url
            && left.sha256 == right.sha256
            && (left.descriptor._schema != right.descriptor._schema
                || !equality::equal(
                    &serde_json::to_value(&left.descriptor)?,
                    &serde_json::to_value(&right.descriptor)?,
                ))
        {
            // Python tuple sorting reaches the unordered Pydantic descriptor here.
            return Err(Error::Other(
                "cannot order distinct plugin descriptors with the same archive URL and hash"
                    .into(),
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
