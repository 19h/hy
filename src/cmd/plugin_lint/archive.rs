//! Discover descriptors before reporting file references and recommendations.

use std::io::{Read, Seek};

use crate::error::{Error, Result};
use crate::plugin::{PluginManifest, archive_paths, files::ArchiveReferences};
use crate::util::python_zip::Archive;

use super::report;

pub(super) fn lint(source: impl Read + Seek, source_name: &str) -> Result<super::LintResult> {
    let mut archive = Archive::new(source)?;
    let references = ArchiveReferences::read(&archive);
    let mut plugins = Vec::new();
    let mut findings = 0;
    let mut errors = 0;
    for index in 0..archive.len() {
        let path = archive.name_for_index(index).expect("central directory index");
        if !path.ends_with("ida-plugin.json") {
            continue;
        }
        let path = path.to_owned();
        let bytes = archive.read(&path)?;
        // Lint catches model errors, but decoding or reading a member is terminal.
        let text = std::str::from_utf8(&bytes)
            .map_err(|error| Error::Other(format!("{path}: {error}")))?;
        match serde_json::from_str::<PluginManifest>(text) {
            Ok(manifest) => plugins.push((path, manifest.metadata)),
            Err(error) => {
                println!(
                    "Error ({source_name}): {path}: ida-plugin.json validation failed: {error}"
                );
                findings += 1;
                errors += 1;
            }
        }
    }
    if plugins.is_empty() {
        println!("Error: No valid plugins found in archive {source_name}");
        return Ok(super::LintResult {
            findings: findings + 1,
            errors: errors + 1,
        });
    }
    let component_names: std::collections::HashSet<_> = plugins
        .iter()
        .flat_map(|(_, metadata)| crate::plugin::components::names(metadata))
        .collect();
    if plugins.len() > 1 {
        for (path, metadata) in &plugins {
            if !component_names.contains(&metadata.name)
                && path.split('/').filter(|part| !part.is_empty() && *part != ".").count() != 2
            {
                println!(
                    "Error ({source_name}): root manifest for '{}' is at '{}' but should be at the archive's top level (e.g., '{}/ida-plugin.json')",
                    metadata.name,
                    crate::plugin::archive_paths::display(path),
                    metadata.name
                );
                errors += 1;
                findings += 1;
            }
        }
    }
    for (raw_path, metadata) in &plugins {
        let prefix = archive_paths::descriptor_prefix(raw_path);
        let path = archive_paths::display(raw_path);
        if let Err(error) = references.validate(metadata, &prefix) {
            println!("Error: {path}: ida-plugin.json validation failed: {error}");
            findings += 1;
            errors += 1;
            continue;
        }
        let source = format!("{source_name}:{path}");
        findings += report::metadata(metadata, &source);
        let count = super::components::authored(metadata, &source);
        findings += count;
        errors += count;
        let names: Vec<_> = archive
            .file_names()
            .filter_map(|name| archive_paths::filename_in_prefix(name, &prefix))
            .collect();
        findings += report::readme(names.iter().map(String::as_str), &source);
        let (component_findings, component_errors) =
            super::components::archive(metadata, raw_path, &plugins, &source);
        findings += component_findings;
        errors += component_errors;
    }
    Ok(super::LintResult {
        findings,
        errors,
    })
}
