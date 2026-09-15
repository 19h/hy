//! Recognize bundles and inspect their embedded plugin archives in member order.

use std::path::Path;
use std::sync::Mutex;

use super::{BundleManifest, BundleTargetPlatformTag, manifest, validate_bundle_path};
use crate::error::{Error, Result};
use crate::plugin::index::{ArchiveCatalogue, Plugin, index_archive};
use crate::util::python_zip::Archive;

mod member;

/// Recognition checks the lowercase `.zip` suffix and manifest presence, not its contents.
pub fn is_plugin_bundle_zip(path: &Path) -> bool {
    if path.extension().is_none_or(|extension| extension != "zip") || !path.is_file() {
        return false;
    }
    let Ok(file) = std::fs::File::open(path) else {
        return false;
    };
    let Ok(archive) = Archive::new(file) else {
        return false;
    };
    archive.file_names().any(|name| name == "plugin-bundle.json")
}

/// Keep the same opened ZIP for indexing and later member fetches.
pub struct BundleReader {
    archive: Mutex<Archive<std::fs::File>>,
    manifest: BundleManifest,
}

impl BundleReader {
    pub fn open(path: &Path) -> Result<Self> {
        let mut archive = Archive::new(std::fs::File::open(path)?)?;
        let manifest = manifest::read_from_archive(&mut archive, path)?;
        Ok(Self {
            archive: Mutex::new(archive),
            manifest,
        })
    }

    pub fn manifest(&self) -> &BundleManifest {
        &self.manifest
    }

    pub fn plugins(&self) -> Result<Vec<Plugin>> {
        let mut archive =
            self.archive.lock().map_err(|_| Error::Other("bundle reader lock poisoned".into()))?;
        let mut catalogue = ArchiveCatalogue::default();
        let names: Vec<_> = archive
            .file_names()
            .filter(|name| name.starts_with("plugins/") && name.ends_with(".zip"))
            .map(str::to_owned)
            .collect();
        for name in names {
            validate_bundle_path(&name)?;
            let Some(bytes) = member::read(&mut archive, &name)? else {
                continue;
            };
            index_archive(&mut catalogue, &bytes, &format!("hcli-bundle:{name}"), None)?;
        }
        catalogue.into_plugins()
    }

    pub fn read(&self, name: &str) -> Result<Vec<u8>> {
        let mut archive =
            self.archive.lock().map_err(|_| Error::Other("bundle reader lock poisoned".into()))?;
        Ok(archive.read(name)?)
    }

    pub fn extract_wheelhouse(
        &self,
        target: &BundleTargetPlatformTag,
        destination: &Path,
    ) -> Result<()> {
        let mut archive =
            self.archive.lock().map_err(|_| Error::Other("bundle reader lock poisoned".into()))?;
        super::wheelhouse::extract(&mut archive, &target.wheelhouse, destination)
    }
}
