//! Resolve every bundle target independently, retaining archive ownership and local pins.

use std::collections::HashMap;
use std::io::Cursor;
use std::path::Path;

use indexmap::IndexMap;

use super::plugin_cmd::BundleCreateArgs;
use crate::error::{Error, Result};
use crate::plugin::repositories::Repositories;
use crate::plugin::requirements::Requirement;
use crate::plugin::{self, PluginContext, bundle, index, requirements, resolve};
use crate::util::python_zip::Archive;

pub(super) async fn resolve(
    args: &BundleCreateArgs,
    context: &PluginContext,
    targets: &[bundle::PipTarget],
) -> Result<Vec<bundle::ResolvedCell>> {
    let mut local = IndexMap::new();
    let mut roots = Vec::new();
    let mut scopes = HashMap::new();
    for spec in &args.plugin_specs {
        let expanded = crate::util::python_path::expand_user(spec)?;
        let path = Path::new(&expanded);
        if path.is_dir() && path.join("ida-plugin.json").is_file()
            || path.exists() && spec.ends_with(".zip")
        {
            let bytes = if path.is_dir() {
                plugin::pack_directory(&path.canonicalize()?)?
            } else {
                std::fs::read(path)?
            };
            let mut archive = Archive::new(Cursor::new(&bytes))?;
            let manifests = plugin::components::archive_manifests(&mut archive)?;
            let references = plugin::files::ArchiveReferences::read(&archive);
            for (path, manifest) in &manifests {
                references.validate(
                    &manifest.metadata,
                    &plugin::archive_paths::descriptor_prefix(path),
                )?;
            }
            let metadata = plugin::select_archived_plugin(&mut archive, None)?.metadata;
            local.insert(metadata.name.clone(), (bytes, metadata));
        } else {
            let reference = index::parse_reference(spec)?;
            if let Some(repo) = reference.repo {
                scopes.insert(reference.name.to_lowercase(), repo);
            }
            roots.push(Requirement::parse(spec)?);
        }
    }
    let fixed: Vec<_> = local.values().map(|(_, metadata)| metadata.clone()).collect();
    let pool = if roots.is_empty()
        && fixed.iter().all(|metadata| requirements::collect(metadata).is_empty())
    {
        Repositories::empty()
    } else {
        Repositories::load(context, &scopes, args.repo.as_deref()).await?
    };
    let mut cached: HashMap<(_, _), Vec<u8>> = HashMap::new();
    let mut cells = Vec::new();
    for target in targets {
        let cell = resolve::Cell {
            platform: target.ida_platform.clone(),
            ida_version: None,
            python_version: Some(target.python_version.clone()),
            check_python: true,
        };
        for metadata in &fixed {
            // Local roots may intentionally package several platforms, but their
            // Python constraint must admit every requested target.
            if let Some(requirement) = &metadata.requires_python
                && !requirements::python_matches(&target.python_version, requirement)
            {
                return Err(Error::PluginInstall(format!(
                    "{} {} requires Python {requirement}, which excludes target {}",
                    metadata.name,
                    metadata.version,
                    target.id()
                )));
            }
        }
        let resolution = resolve::resolve(&pool.snapshot, &roots, &cell, &fixed, &[])?;
        for warning in resolution.warnings.iter().chain(&resolution.skipped) {
            crate::util::fmt::warning(&format!("{}: {warning}", target.id()));
        }
        let mut plugins = IndexMap::new();
        for (name, (bytes, metadata)) in &local {
            plugins.insert(name.clone(), (bytes.clone(), metadata.clone()));
        }
        for (name, location) in resolution.selected {
            let key = (location.url.clone(), location.sha256.clone());
            let bytes = match cached.get(&key) {
                Some(bytes) => bytes.clone(),
                None => {
                    let bytes = pool.fetch(&location).await?;
                    cached.insert(key, bytes.clone());
                    bytes
                }
            };
            plugins.insert(name, (bytes, location.descriptor.metadata));
        }
        let mut root_names: Vec<_> = fixed.iter().map(|metadata| metadata.name.clone()).collect();
        root_names.extend(roots.iter().map(|requirement| requirement.name.clone()));
        cells.push(bundle::ResolvedCell {
            target: target.clone(),
            plugins,
            roots: root_names,
        });
    }
    Ok(cells)
}
