//! Select descriptors before validating references or extracting archive content.

use std::io::{Read, Seek};

use crate::error::{Error, Result};
use crate::util::python_zip::Archive;

use super::ArchivedPlugin;

pub(crate) fn select_archived_plugin<R: Read + Seek>(
    archive: &mut Archive<R>,
    name: Option<&str>,
) -> Result<ArchivedPlugin> {
    if let Some(name) = name {
        for index in 0..archive.len() {
            if let Some((path, manifest)) = super::read_archived_manifest(archive, index)? {
                if manifest.metadata.name == name && manifest.metadata.components.is_empty() {
                    return Ok(ArchivedPlugin {
                        prefix: crate::plugin::archive_paths::descriptor_prefix(&path),
                        metadata: manifest.metadata,
                    });
                }
                if manifest.metadata.name == name {
                    break;
                }
            }
        }
    }
    let manifests = crate::plugin::components::archive_manifests(archive)?;
    let component_names: std::collections::HashSet<_> = manifests
        .iter()
        .flat_map(|(_, manifest)| crate::plugin::components::names(&manifest.metadata))
        .collect();
    let mut selected = None;
    let mut count = 0_usize;
    let mut root_names = Vec::new();
    for (path, manifest) in manifests {
        if name.is_some_and(|name| manifest.metadata.name != name) {
            continue;
        }
        if name.is_none() && component_names.contains(&manifest.metadata.name) {
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
        root_names.push(plugin.metadata.name.clone());
        selected = Some(plugin);
    }
    if count > 1 {
        return Err(Error::PluginInstall(format!(
            "archive contains multiple unrelated plugins ({}); specify which one to install",
            root_names.join(", ")
        )));
    }
    selected.ok_or_else(|| {
        Error::PluginInstall(match name {
            Some(name) => format!("plugin '{name}' not found in archive"),
            None => "no valid ida-plugin.json found in archive".into(),
        })
    })
}

#[cfg(test)]
mod tests;
