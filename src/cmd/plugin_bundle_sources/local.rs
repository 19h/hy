//! Local bundle paths and the source's once-versus-per-platform read policy.

use std::path::Path;

use super::archives::{Archive, Archives};
use crate::error::Result;
use crate::plugin::bundle::{ResolvedPluginArchive, local_archive_metadata};
use crate::util::python_path;

pub(super) fn resolve(
    spec: &str,
    platforms: &[String],
) -> Result<Option<Vec<ResolvedPluginArchive>>> {
    // The source chooses its read count before expanding the home directory.
    let read_once = Path::new(spec).exists() && spec.ends_with(".zip");
    let expanded = python_path::expand_user(spec)?;
    let path = Path::new(&expanded);
    if !path.exists() || !spec.ends_with(".zip") {
        return Ok(None);
    }
    let archives = collect(platforms, read_once, |_| {
        let bytes = std::fs::read(path)?;
        // Diagnostics retain the user's spec, including its tilde spelling.
        let metadata = local_archive_metadata(&bytes, Path::new(spec))?;
        Ok(Archive::new(metadata.name, bytes))
    })?;
    archives.into_iter().map(Archive::resolve).collect::<Result<Vec<_>>>().map(Some)
}

fn collect(
    platforms: &[String],
    read_once: bool,
    mut read: impl FnMut(Option<&str>) -> Result<Archive>,
) -> Result<Vec<Archive>> {
    if read_once {
        return Ok(vec![read(None)?]);
    }
    let mut archives = Archives::default();
    for platform in platforms {
        let archive = read(Some(platform))?;
        archives.insert(archive, platform);
    }
    Ok(archives.finish())
}

#[cfg(test)]
mod tests;
