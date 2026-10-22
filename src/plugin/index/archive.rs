//! Shared archive indexing and filesystem repository acquisition.

use std::io::Cursor;
use std::path::Path;

use sha2::{Digest, Sha256};

use super::{ArchiveCatalogue, normalize_host};
use crate::error::{Error, Result};
use crate::plugin::files;
use crate::util::python_json::Text;
use crate::util::python_zip::Archive;

pub(crate) fn add_bytes(
    catalogue: &mut ArchiveCatalogue,
    bytes: &[u8],
    url: &str,
    expected_host: Option<&str>,
) -> Result<()> {
    add_text_bytes(catalogue, bytes, &Text::from(url), expected_host)
}

pub(super) fn add_text_bytes(
    catalogue: &mut ArchiveCatalogue,
    bytes: &[u8],
    url: &Text,
    expected_host: Option<&str>,
) -> Result<()> {
    let mut archive = Archive::new(Cursor::new(bytes))?;
    let references = files::ArchiveReferences::read(&archive);
    let checksum = format!("{:x}", Sha256::digest(bytes));
    let manifests = crate::plugin::components::archive_manifests(&mut archive)?;
    let components: std::collections::HashSet<_> = manifests
        .iter()
        .flat_map(|(_, manifest)| crate::plugin::components::names(&manifest.metadata))
        .collect();
    for (path, manifest) in &manifests {
        let prefix = crate::plugin::archive_paths::descriptor_prefix(path);
        if let Err(error) = references.validate(&manifest.metadata, &prefix) {
            tracing::debug!(%path, %error, "stopping plugin archive indexing");
            return Ok(());
        }
    }
    for (path, mut manifest) in manifests {
        if components.contains(&manifest.metadata.name) {
            continue;
        }
        crate::plugin::components::resolve_inline_dependencies(
            &mut manifest.metadata,
            &crate::plugin::archive_paths::descriptor_prefix(&path),
            &mut archive,
        )?;
        if let Some(expected) = expected_host.filter(|host| !host.is_empty())
            && manifest.metadata.normalized_host()? != normalize_host(expected)?
        {
            continue;
        }
        catalogue.add(url, &checksum, manifest)?;
    }
    Ok(())
}

pub(super) fn add_directory(catalogue: &mut ArchiveCatalogue, directory: &Path) -> Result<()> {
    // os.walk ignores directory-enumeration failures, yields files before children,
    // and retains filesystem order within each list.
    let entries = match std::fs::read_dir(directory)
        .and_then(|entries| entries.collect::<std::io::Result<Vec<_>>>())
    {
        Ok(entries) => entries,
        Err(error) => {
            tracing::debug!(path = ?directory, %error, "skipping unreadable repository directory");
            return Ok(());
        }
    };
    let mut directories = Vec::new();
    for entry in entries {
        let path = entry.path();
        if path.is_dir() {
            directories.push(path);
        } else if entry.file_name().to_string_lossy().ends_with(".zip") {
            let url = url::Url::from_file_path(std::path::absolute(&path)?)
                .map_err(|_| Error::Other("invalid archive path".into()))?;
            add_bytes(catalogue, &std::fs::read(path)?, url.as_str(), None)?;
        }
    }
    for directory in directories {
        if !directory.is_symlink() {
            add_directory(catalogue, &directory)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
