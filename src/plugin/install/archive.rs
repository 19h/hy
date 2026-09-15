//! Validate the selected subtree, then extract named members in directory order.

use std::io::{Read, Seek};
use std::path::Path;

use crate::error::{Error, Result};
use crate::util::python_zip::Archive;

pub(super) fn extract<R: Read + Seek>(
    archive: &mut Archive<R>,
    prefix: &str,
    destination: &Path,
) -> Result<()> {
    let mut selected = Vec::new();
    for member in archive.members() {
        let Some(relative) =
            crate::plugin::archive_paths::extraction_relative(prefix, member.name)?
        else {
            continue;
        };
        if member.is_symlink() {
            return Err(Error::PluginInstall(format!(
                "archive symlink is not supported: {}",
                member.name
            )));
        }
        let target = destination.join(&relative);
        if relative.split('/').any(|part| part == "..") || !target.starts_with(destination) {
            return Err(Error::PluginInstall(format!(
                "archive contains an unsafe path: {}",
                member.name
            )));
        }
        selected.push((member.name.to_owned(), target, member.is_dir()));
    }
    std::fs::create_dir_all(destination)?;
    for (name, target, directory) in selected {
        if directory {
            std::fs::create_dir_all(target)?;
        } else {
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent)?;
            }
            archive.copy_to(&name, || std::fs::File::create(&target))?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
