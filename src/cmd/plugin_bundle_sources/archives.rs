//! Preserve source resolution order while grouping byte-identical distributions.

use indexmap::{IndexMap, map::Entry};
use sha2::{Digest, Sha256};

use crate::error::Result;
use crate::plugin::bundle::{ResolvedPluginArchive, archive_version};

pub(super) struct Archive {
    pub name: String,
    pub bytes: Vec<u8>,
    pub platforms: Vec<String>,
}

impl Archive {
    pub fn new(name: String, bytes: Vec<u8>) -> Self {
        Self {
            name,
            bytes,
            platforms: Vec::new(),
        }
    }

    pub fn resolve(self) -> Result<ResolvedPluginArchive> {
        Ok(ResolvedPluginArchive {
            version: archive_version(&self.bytes, &self.name)?,
            name: self.name,
            bytes: self.bytes,
            platforms: self.platforms,
        })
    }
}

#[derive(Default)]
pub(super) struct Archives(IndexMap<[u8; 32], Archive>);

impl Archives {
    pub fn insert(&mut self, archive: Archive, platform: &str) {
        let hash = Sha256::digest(&archive.bytes).into();
        let entry = match self.0.entry(hash) {
            Entry::Occupied(entry) => {
                let existing = entry.into_mut();
                existing.name = archive.name;
                existing.bytes = archive.bytes;
                existing
            }
            Entry::Vacant(entry) => entry.insert(archive),
        };
        entry.platforms.push(platform.into());
    }

    pub fn finish(self) -> Vec<Archive> {
        let mut archives: Vec<_> = self.0.into_values().collect();
        if let [archive] = archives.as_mut_slice() {
            archive.platforms.clear();
        }
        archives
    }
}
