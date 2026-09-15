//! Retain one distribution snapshot between descriptor selection and staging.

use std::io::Cursor;
use std::path::{Path, PathBuf};

use crate::error::{Error, Result};
use crate::plugin::{PluginMetadata, read_metadata_from_directory, select_archived_plugin};
use crate::util::python_zip::Archive;

use super::{PreparedPlugin, directory, prepare_archive, prepare_editable};

pub enum InstallationSource {
    Editable(PathBuf),
    Archive(Archive<Cursor<Vec<u8>>>),
}

impl InstallationSource {
    pub fn read(path: &Path, editable: bool) -> Result<Self> {
        if editable {
            if !path.is_dir() {
                return Err(Error::PluginInstall("--editable requires a local directory".into()));
            }
            return Ok(Self::Editable(path.into()));
        }
        let bytes = if path.is_dir() {
            directory::pack(path)?
        } else {
            std::fs::read(path)?
        };
        Ok(Self::Archive(Archive::new(Cursor::new(bytes))?))
    }

    pub fn metadata(&mut self, name: Option<&str>) -> Result<PluginMetadata> {
        match self {
            Self::Editable(path) => read_metadata_from_directory(path),
            Self::Archive(archive) => Ok(select_archived_plugin(archive, name)?.metadata),
        }
    }

    pub fn prepare(
        mut self,
        ida_version: Option<&str>,
        replace: bool,
        expected: &PluginMetadata,
    ) -> Result<PreparedPlugin> {
        let prepared = match &mut self {
            Self::Editable(path) => prepare_editable(path, ida_version, replace)?,
            Self::Archive(archive) => {
                prepare_archive(archive, ida_version, replace, Some(&expected.name))?
            }
        };
        prepared.metadata.verify_prepared_identity(expected)?;
        Ok(prepared)
    }
}

#[cfg(test)]
mod tests;
