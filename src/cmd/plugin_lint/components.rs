//! Authored component diagnostics are separate from expanded repository metadata.

use std::collections::HashSet;
use std::path::Path;

use crate::error::Result;
use crate::plugin::{PluginManifest, PluginMetadata, components::Component};

pub(super) fn authored(metadata: &PluginMetadata, source: &str) -> usize {
    let mut errors = 0;
    for (index, component) in metadata.components.iter().enumerate() {
        if let Component::Inline(descriptor) = component {
            println!(
                concat!(
                    "Error ({}): plugin.components[{}]: contains expanded metadata object for '{}'; ",
                    "use the string form in authored archives"
                ),
                source, index, descriptor.metadata.name
            );
            errors += 1;
        }
    }
    errors
}

pub(super) fn directory(metadata: &PluginMetadata, root: &Path) -> Result<(usize, usize)> {
    let mut seen = HashSet::from([metadata.name.clone()]);
    fn undeclared(metadata: &PluginMetadata, root: &Path, source: &str) -> Result<usize> {
        let mut findings = 0;
        let mut entries: Vec<_> = std::fs::read_dir(root)?.collect::<std::io::Result<_>>()?;
        entries.sort_by_key(|entry| entry.file_name());
        for entry in entries {
            let path = entry.path();
            if !path.is_dir() || !path.join("ida-plugin.json").is_file() {
                continue;
            }
            if let Some(Component::Inline(component)) = metadata
                .components
                .iter()
                .find(|component| entry.file_name().to_str() == Some(component.name()))
            {
                findings += undeclared(&component.metadata, &path, source)?;
            } else if let Ok(descriptor) = std::fs::read(path.join("ida-plugin.json"))
                .map_err(crate::error::Error::from)
                .and_then(|bytes| {
                    serde_json::from_slice::<PluginManifest>(&bytes).map_err(Into::into)
                })
            {
                println!(
                    concat!(
                        "Warning ({}): subdirectory '{}' contains ida-plugin.json (plugin '{}') ",
                        "but is not declared as a component"
                    ),
                    source,
                    entry.file_name().to_string_lossy(),
                    descriptor.metadata.name
                );
                findings += 1;
            }
        }
        Ok(findings)
    }
    fn visit(
        metadata: &PluginMetadata,
        root: &Path,
        seen: &mut HashSet<String>,
    ) -> Result<(usize, usize)> {
        let mut findings = 0;
        let mut errors = 0;
        for component in &metadata.components {
            let Component::Inline(expanded) = component else {
                continue;
            };
            let path = root.join(component.name());
            let source = path.display().to_string();
            let raw: PluginManifest =
                serde_json::from_slice(&std::fs::read(path.join("ida-plugin.json"))?)?;
            findings += super::report::metadata(&raw.metadata, &source);
            let count = authored(&raw.metadata, &source);
            findings += count;
            errors += count;
            if !seen.insert(component.name().to_owned()) {
                println!("Error ({source}): duplicate component name '{}'", component.name());
                errors += 1;
                findings += 1;
            }
            let (child_findings, child_errors) = visit(&expanded.metadata, &path, seen)?;
            findings += child_findings;
            errors += child_errors;
        }
        Ok((findings, errors))
    }
    let warnings = undeclared(metadata, root, &root.to_string_lossy())?;
    let (findings, errors) = visit(metadata, root, &mut seen)?;
    Ok((findings + warnings, errors))
}

pub(super) fn archive(
    metadata: &PluginMetadata,
    path: &str,
    descriptors: &[(String, PluginMetadata)],
    source: &str,
) -> (usize, usize) {
    let mut expanded = metadata.clone();
    let result = crate::plugin::components::expand_archived(&mut expanded, path, descriptors);
    let declared: HashSet<_> = crate::plugin::components::metadata_tree(&expanded)
        .into_iter()
        .map(|metadata| metadata.name.as_str())
        .collect();
    let mut findings = 0;
    let mut errors = 0;
    for (_, descriptor) in descriptors {
        if !declared.contains(descriptor.name.as_str()) {
            println!(
                "Warning ({source}): archive contains plugin '{}' that is not declared as a component",
                descriptor.name
            );
            findings += 1;
        }
    }
    if let Err(error) = result {
        println!("Error ({source}): component validation failed: {error}");
        return (findings + 1, 1);
    }
    let mut seen = HashSet::from([metadata.name.as_str()]);
    for component in crate::plugin::components::metadata_tree(&expanded).into_iter().skip(1) {
        let source = format!("{source}:{}", component.name);
        if !seen.insert(&component.name) {
            println!("Error ({source}): duplicate component name '{}'", component.name);
            errors += 1;
            findings += 1;
        }
        findings += super::report::metadata(component, &source);
        let count = authored(component, &source);
        findings += count;
        errors += count;
    }
    (findings, errors)
}
