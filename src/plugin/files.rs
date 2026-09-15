//! Validate referenced files before dependency resolution or catalogue publication.

use std::collections::HashSet;
use std::io::{Read, Seek};
use std::path::Path;

use super::PluginMetadata;
use crate::error::{Error, Result};

const NATIVE_EXTENSIONS: [&str; 3] = ["so", "dll", "dylib"];

#[cfg(test)]
#[path = "files/parity.rs"]
mod parity;

/// Metadata references use exact central-directory names without opening members.
pub(crate) struct ArchiveReferences(HashSet<String>);

impl ArchiveReferences {
    pub fn read<R: Read + Seek>(archive: &crate::util::python_zip::Archive<R>) -> Self {
        Self(archive.file_names().map(str::to_owned).collect())
    }

    pub fn validate(&self, metadata: &PluginMetadata, prefix: &str) -> Result<()> {
        validate_distribution(metadata, |relative| {
            Ok(self.0.contains(&super::archive_paths::reference(prefix, relative)))
        })
    }
}

/// Upstream validates references as POSIX paths, even on Windows.
fn validate_path(path: &str, field: &str) -> Result<()> {
    if path.is_ascii() && !path.starts_with('/') && !path.split('/').any(|part| part == "..") {
        return Ok(());
    }
    Err(Error::PluginInstall(format!("unsafe {field} path: {path}")))
}

fn validate_paths(metadata: &PluginMetadata) -> Result<()> {
    validate_path(&metadata.entry_point, "entry point")?;
    if let Some(logo) = metadata.logo_path.as_deref().filter(|path| !path.is_empty()) {
        validate_path(logo, "logo")?;
    }
    Ok(())
}

fn require_file(path: &str, field: &str, exists: &impl Fn(&str) -> Result<bool>) -> Result<()> {
    if !exists(path)? {
        return Err(Error::PluginInstall(format!("{field} file not found: {path}")));
    }
    Ok(())
}

fn validate_logo(metadata: &PluginMetadata, exists: &impl Fn(&str) -> Result<bool>) -> Result<()> {
    if let Some(logo) = metadata.logo_path.as_deref().filter(|path| !path.is_empty()) {
        require_file(logo, "logo", exists)?;
    }
    Ok(())
}

pub fn validate_directory_files(metadata: &PluginMetadata, root: &Path) -> Result<()> {
    validate_paths(metadata)?;
    let exists = |path: &str| {
        Ok(crate::util::python_path::exists(&crate::util::python_path::join(root, path))?)
    };
    let entry = &metadata.entry_point;
    if !exists(entry)? && (entry.ends_with(".py") || !has_native_file(entry, &exists)?) {
        return Err(Error::PluginInstall(format!("entry point file not found: {entry}")));
    }
    validate_logo(metadata, &exists)
}

pub(super) fn validate_distribution_directory(
    metadata: &PluginMetadata,
    root: &Path,
) -> Result<()> {
    // Directory packaging emits files, not directory records.
    validate_distribution(metadata, |path| Ok(crate::util::python_path::join(root, path).is_file()))
}

pub(super) fn validate_distribution(
    metadata: &PluginMetadata,
    exists: impl Fn(&str) -> Result<bool>,
) -> Result<()> {
    validate_paths(metadata)?;
    let entry = &metadata.entry_point;
    if entry.ends_with(".py") {
        require_file(entry, "entry point", &exists)?;
    } else {
        // Upstream archive validation requires an appended extension, even when
        // an exact native filename exists. Directory validation permits both.
        if !has_native_file(entry, &exists)? {
            return Err(Error::PluginInstall(format!(
                "native entry point requires an appended .so, .dll or .dylib file: {entry}"
            )));
        }
        for platform in &metadata.platforms {
            let extension = match platform.split_once('-').map(|(os, _)| os) {
                Some("linux") => "so",
                Some("windows") => "dll",
                Some("macos") => "dylib",
                _ => unreachable!("platforms are validated by PluginMetadata"),
            };
            require_file(&format!("{entry}.{extension}"), "native entry point", &exists)?;
        }
    }
    validate_logo(metadata, &exists)
}

fn has_native_file(entry: &str, exists: &impl Fn(&str) -> Result<bool>) -> Result<bool> {
    for extension in NATIVE_EXTENSIONS {
        if exists(&format!("{entry}.{extension}"))? {
            return Ok(true);
        }
    }
    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Deserialize)]
    struct Case {
        name: String,
        entry: String,
        platforms: Vec<String>,
        files: Vec<String>,
        logo: Option<String>,
        archive: bool,
        directory: bool,
    }

    #[derive(Deserialize)]
    struct Oracle {
        cases: Vec<Case>,
    }

    #[test]
    fn archive_and_directory_rules_match_pinned_upstream() {
        let oracle: Oracle =
            serde_json::from_str(include_str!("../../tests/fixtures/plugin-files.json")).unwrap();
        for case in oracle.cases {
            let metadata: PluginMetadata = serde_json::from_value(serde_json::json!({
                "name": "example", "version": "1", "entryPoint": case.entry,
                "platforms": case.platforms, "logoPath": case.logo,
                "urls": {"repository": "https://github.com/example/files"},
                "authors": [{"email": "author@example.test"}],
            }))
            .unwrap();
            for prefix in ["", "repository/nested/"] {
                let files = ArchiveReferences(
                    case.files.iter().map(|file| format!("{prefix}{file}")).collect(),
                );
                assert_eq!(
                    files.validate(&metadata, prefix).is_ok(),
                    case.archive,
                    "{}: {prefix}",
                    case.name
                );
            }
            let directory = tempfile::tempdir().unwrap();
            for file in &case.files {
                // Invalid paths are rejected before lookup; never materialize them.
                let relative = Path::new(file);
                if relative.components().all(|part| {
                    matches!(part, std::path::Component::Normal(_) | std::path::Component::CurDir)
                }) {
                    let path = directory.path().join(relative);
                    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
                    std::fs::write(path, b"fixture").unwrap();
                }
            }
            assert_eq!(
                validate_directory_files(&metadata, directory.path()).is_ok(),
                case.directory,
                "{}",
                case.name
            );
        }
    }
}
