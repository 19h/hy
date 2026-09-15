//! Classify direct-install inputs in upstream branch order before acquisition.

use std::path::{Path, PathBuf};

use crate::error::{Error, Result};
use crate::plugin::index;
use crate::util::python_path;

#[derive(Debug, PartialEq, Eq)]
pub(super) enum Source {
    Directory(PathBuf),
    Archive(PathBuf),
    GitHub,
    Download,
    Repository,
}

pub(super) fn classify(value: &str, editable: bool) -> Result<Source> {
    let expanded = PathBuf::from(python_path::expand_user(value)?);
    if editable {
        if !python_path::is_dir(&expanded)? {
            return Err(Error::PluginInstall(
                "--editable requires a local directory containing ida-plugin.json".into(),
            ));
        }
        return Ok(Source::Directory(expanded.canonicalize()?));
    }
    if python_path::is_dir(&expanded)? && python_path::is_file(&expanded.join("ida-plugin.json"))? {
        return Ok(Source::Directory(expanded.canonicalize()?));
    }
    // Unlike directory recognition, local archive recognition does not expand ~
    // or require a regular file. A directory named *.zip fails at the file read.
    if python_path::exists(Path::new(value))? && value.ends_with(".zip") {
        return Ok(Source::Archive(value.into()));
    }
    if value.starts_with("file://") {
        return Ok(Source::Download);
    }
    if index::is_direct_github(value) {
        return Ok(Source::GitHub);
    }
    if value.starts_with("https://") {
        return Ok(Source::Download);
    }
    Ok(Source::Repository)
}

#[cfg(all(test, unix))]
mod tests;
