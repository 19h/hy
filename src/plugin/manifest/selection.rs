//! Select descriptors before validating references or extracting archive content.

use std::io::{Read, Seek};

use crate::error::{Error, Result};
use crate::util::python_zip::Archive;

use super::{ArchivedPlugin, read_archived_manifest};

pub(crate) fn select_archived_plugin<R: Read + Seek>(
    archive: &mut Archive<R>,
    name: Option<&str>,
) -> Result<ArchivedPlugin> {
    let mut selected = None;
    let mut count = 0_usize;
    for index in 0..archive.len() {
        let Some((path, manifest)) = read_archived_manifest(archive, index)? else {
            continue;
        };
        if name.is_some_and(|name| manifest.metadata.name != name) {
            continue;
        }
        let plugin = ArchivedPlugin {
            prefix: crate::plugin::archive_paths::descriptor_prefix(&path),
            metadata: manifest.metadata,
        };
        if name.is_some() {
            return Ok(plugin);
        }
        count += 1;
        selected = Some(plugin);
    }
    if count > 1 {
        return Err(Error::PluginInstall(
            "direct installation requires an archive containing one plugin".into(),
        ));
    }
    selected.ok_or_else(|| {
        Error::PluginInstall(match name {
            Some(name) => format!("plugin {name} not found in archive"),
            None => "ida-plugin.json not found in archive".into(),
        })
    })
}

#[cfg(test)]
mod tests;
