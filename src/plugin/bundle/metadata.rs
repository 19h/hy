//! Descriptor discovery shared by bundle construction and archive inspection.

use std::io::{Cursor, Read, Seek};
use std::path::Path;

use crate::error::{Error, Result};
use crate::plugin::PluginMetadata;
use crate::util::python_zip::Archive;

pub(super) fn descriptors(bytes: &[u8]) -> Result<Vec<PluginMetadata>> {
    let mut archive = Archive::new(Cursor::new(bytes))?;
    let mut descriptors = Vec::new();
    for index in 0..archive.len() {
        if let Some((_, metadata)) = read_descriptor(&mut archive, index)? {
            descriptors.push(metadata);
        }
    }
    Ok(descriptors)
}

/// Invalid descriptor data is skipped; archive read errors propagate to the caller.
pub(super) fn read_descriptor<R: Read + Seek>(
    archive: &mut Archive<R>,
    index: usize,
) -> Result<Option<(String, PluginMetadata)>> {
    Ok(crate::plugin::read_archived_manifest(archive, index)?
        .map(|(path, manifest)| (path, manifest.metadata)))
}

pub(crate) fn single(bytes: &[u8], path: &Path) -> Result<PluginMetadata> {
    let mut descriptors = descriptors(bytes)?;
    match descriptors.len() {
        0 => Err(Error::Other(format!("no ida-plugin.json found in {}", path.display()))),
        1 => Ok(descriptors.pop().expect("one descriptor")),
        _ => Err(Error::Other(format!(
            "plugin archive must contain a single plugin, found: {}",
            descriptors
                .iter()
                .map(|metadata| metadata.name.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ))),
    }
}

/// Bundle naming uses the first descriptor with the exact requested name.
pub(crate) fn version(bytes: &[u8], name: &str) -> Result<String> {
    let mut archive = Archive::new(Cursor::new(bytes))?;
    for index in 0..archive.len() {
        if let Some((_, metadata)) = read_descriptor(&mut archive, index)?
            && metadata.name == name
        {
            return Ok(metadata.version);
        }
    }
    Err(Error::Other(format!("plugin '{name}' not found in archive")))
}
