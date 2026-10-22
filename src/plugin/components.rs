//! Component trees share the archive and lifecycle of their owning plugin.

use std::collections::{HashMap, HashSet};
use std::io::{Read, Seek};
use std::path::Path;

use serde::{Deserialize, Serialize};

use super::{PluginManifest, PluginMetadata};
use crate::error::{Error, Result};
use crate::util::python_zip::Archive;

pub const MAX_DEPTH: usize = 10;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Component {
    Name(String),
    Inline(Box<PluginManifest>),
}

impl Component {
    pub fn name(&self) -> &str {
        match self {
            Self::Name(name) => name,
            Self::Inline(descriptor) => &descriptor.metadata.name,
        }
    }
}

pub fn validate_names(metadata: &PluginMetadata) -> Result<()> {
    let mut names = HashSet::new();
    for component in &metadata.components {
        super::validate_name(component.name())?;
        if !names.insert(component.name().to_lowercase()) {
            return Err(Error::PluginInstall(format!("duplicate component: {}", component.name())));
        }
    }
    let mut dependencies = HashSet::new();
    for dependency in &metadata.dependencies {
        if !dependencies.insert(dependency.name.to_lowercase()) {
            return Err(Error::PluginInstall(format!("duplicate dependency: {}", dependency.name)));
        }
    }
    Ok(())
}

pub fn archive_manifests<R: Read + Seek>(
    archive: &mut Archive<R>,
) -> Result<Vec<(String, PluginManifest)>> {
    let mut descriptors = Vec::new();
    for index in 0..archive.len() {
        if let Some(descriptor) = super::read_archived_manifest(archive, index)? {
            descriptors.push(descriptor);
        }
    }
    let by_name: HashMap<_, _> = descriptors
        .iter()
        .map(|(path, descriptor)| {
            (descriptor.metadata.name.clone(), (path.clone(), descriptor.clone()))
        })
        .collect();
    for (path, descriptor) in &mut descriptors {
        expand_archive(&mut descriptor.metadata, path, &by_name, 0)?;
    }
    Ok(descriptors)
}

pub(crate) fn expand_archived(
    metadata: &mut PluginMetadata,
    path: &str,
    descriptors: &[(String, PluginMetadata)],
) -> Result<()> {
    let by_name = descriptors
        .iter()
        .map(|(path, metadata)| {
            (
                metadata.name.clone(),
                (
                    path.clone(),
                    PluginManifest {
                        _schema: None,
                        descriptor_version: 1,
                        metadata: metadata.clone(),
                    },
                ),
            )
        })
        .collect();
    expand_archive(metadata, path, &by_name, 0)
}

fn expand_archive(
    metadata: &mut PluginMetadata,
    path: &str,
    by_name: &HashMap<String, (String, PluginManifest)>,
    depth: usize,
) -> Result<()> {
    check_depth(depth)?;
    let prefix = super::archive_paths::descriptor_prefix(path);
    for component in &mut metadata.components {
        let (path, mut descriptor) = match component {
            Component::Name(name) => by_name.get(name).cloned().ok_or_else(|| {
                Error::PluginInstall(format!(
                    "component '{name}' has no ida-plugin.json in archive"
                ))
            })?,
            Component::Inline(descriptor) => {
                let path = by_name
                    .get(&descriptor.metadata.name)
                    .map(|(path, _)| path.clone())
                    .unwrap_or_else(|| {
                        format!("{prefix}{}/ida-plugin.json", descriptor.metadata.name)
                    });
                (path, *descriptor.clone())
            }
        };
        expand_archive(&mut descriptor.metadata, &path, by_name, depth + 1)?;
        *component = Component::Inline(Box::new(descriptor));
    }
    Ok(())
}

fn check_depth(depth: usize) -> Result<()> {
    if depth >= MAX_DEPTH {
        return Err(Error::PluginInstall(format!(
            "component nesting exceeds maximum depth ({MAX_DEPTH})"
        )));
    }
    Ok(())
}

pub fn expand_directory(
    metadata: &mut PluginMetadata,
    directory: &Path,
    depth: usize,
) -> Result<()> {
    check_depth(depth)?;
    for component in &mut metadata.components {
        let name = component.name().to_owned();
        let path = directory.join(&name);
        let bytes = std::fs::read(path.join("ida-plugin.json")).map_err(|error| {
            Error::PluginInstall(format!("component '{name}' in {}: {error}", directory.display()))
        })?;
        let mut descriptor: PluginManifest = serde_json::from_slice(&bytes)?;
        if descriptor.metadata.name != name {
            return Err(Error::PluginInstall(format!(
                "component directory '{name}' contains plugin '{}'",
                descriptor.metadata.name
            )));
        }
        super::validate_directory_files(&descriptor.metadata, &path)?;
        expand_directory(&mut descriptor.metadata, &path, depth + 1)?;
        *component = Component::Inline(Box::new(descriptor));
    }
    Ok(())
}

pub fn names(metadata: &PluginMetadata) -> Vec<String> {
    let mut names = Vec::new();
    for component in &metadata.components {
        names.push(component.name().into());
        if let Component::Inline(descriptor) = component {
            names.extend(self::names(&descriptor.metadata));
        }
    }
    names
}

pub fn metadata_tree(metadata: &PluginMetadata) -> Vec<&PluginMetadata> {
    let mut tree = vec![metadata];
    for component in &metadata.components {
        if let Component::Inline(descriptor) = component {
            tree.extend(metadata_tree(&descriptor.metadata));
        }
    }
    tree
}

pub fn resolve_inline_dependencies<R: Read + Seek>(
    metadata: &mut PluginMetadata,
    prefix: &str,
    archive: &mut Archive<R>,
) -> Result<()> {
    if matches!(&metadata.python_dependencies, super::PythonDependencies::Source(value) if value == "inline")
    {
        if !metadata.entry_point.ends_with(".py") {
            return Err(Error::PluginInstall(
                "inline dependencies require a Python entry point".into(),
            ));
        }
        let bytes =
            archive.read(&super::archive_paths::reference(prefix, &metadata.entry_point))?;
        let script =
            std::str::from_utf8(&bytes).map_err(|error| Error::PluginInstall(error.to_string()))?;
        metadata.python_dependencies =
            super::PythonDependencies::Requirements(super::parse_inline_dependencies(script)?);
    }
    for component in &mut metadata.components {
        if let Component::Inline(descriptor) = component {
            let prefix = format!("{prefix}{}/", descriptor.metadata.name);
            resolve_inline_dependencies(&mut descriptor.metadata, &prefix, archive)?;
        }
    }
    Ok(())
}

/// Inline component descriptors are materialized only in the private staging tree.
pub fn materialize(metadata: &PluginMetadata, directory: &Path) -> Result<()> {
    for component in &metadata.components {
        if let Component::Inline(descriptor) = component {
            let path = directory.join(component.name());
            std::fs::create_dir_all(&path)?;
            if !path.join("ida-plugin.json").exists() {
                std::fs::write(
                    path.join("ida-plugin.json"),
                    serde_json::to_vec_pretty(descriptor)?,
                )?;
            }
            super::validate_directory_files(&descriptor.metadata, &path)?;
            materialize(&descriptor.metadata, &path)?;
        }
    }
    Ok(())
}

pub fn python_dependencies(metadata: &PluginMetadata, directory: &Path) -> Result<Vec<String>> {
    let mut dependencies = super::dependencies_from_directory(metadata, directory)?;
    for component in &metadata.components {
        if let Component::Inline(descriptor) = component {
            dependencies.extend(python_dependencies(
                &descriptor.metadata,
                &directory.join(component.name()),
            )?);
        }
    }
    Ok(dependencies)
}

pub fn validate_collisions(metadata: &PluginMetadata, replace: bool) -> Result<()> {
    let incoming =
        std::iter::once(metadata.name.clone()).chain(names(metadata)).collect::<HashSet<_>>();
    for installed in super::installed_plugins()? {
        if replace && installed.metadata.name.eq_ignore_ascii_case(&metadata.name) {
            continue;
        }
        let existing =
            std::iter::once(installed.metadata.name.clone()).chain(names(&installed.metadata));
        for name in existing {
            if incoming.contains(&name) {
                return Err(Error::PluginInstall(format!(
                    "'{name}' collides with installed plugin or suite '{}'",
                    installed.metadata.name
                )));
            }
        }
    }
    Ok(())
}
