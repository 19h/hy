//! Extract the selected wheelhouse in directory order, flattening file names.

use std::collections::HashSet;
use std::io::{Read, Seek};
use std::path::Path;

use crate::error::{Error, Result};
use crate::util::python_zip::Archive;

pub(super) fn extract<R: Read + Seek>(
    archive: &mut Archive<R>,
    wheelhouse: &str,
    destination: &Path,
) -> Result<()> {
    std::fs::create_dir_all(destination)?;
    let prefix = format!("{}/", wheelhouse.trim_end_matches('/'));
    let members: Vec<_> = archive
        .members()
        .filter(|member| member.name.starts_with(&prefix) && !member.is_dir())
        .map(|member| (member.name.to_owned(), member.is_symlink()))
        .collect();
    let mut seen = HashSet::new();
    for (name, symlink) in members {
        super::validate_bundle_path(&name)?;
        if symlink {
            return Err(Error::PluginInstall(format!("symlink in wheelhouse: {name}")));
        }
        // PurePosixPath collapses empty and '.' components before taking .name.
        let basename = name[prefix.len()..]
            .split('/')
            .rfind(|component| !component.is_empty() && *component != ".")
            .unwrap_or("");
        if !seen.insert(basename.to_owned()) {
            return Err(Error::PluginInstall(format!(
                "duplicate filename in wheelhouse: {basename} (from {name})"
            )));
        }
        let target = destination.join(basename);
        if !target.starts_with(destination) {
            return Err(Error::PluginInstall(format!("invalid wheelhouse filename: {basename}")));
        }
        archive.copy_to(&name, || std::fs::File::create(&target))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests;
