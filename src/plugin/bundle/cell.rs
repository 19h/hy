//! Isolate wheel-download attempts so rejected optional groups leave no files behind.

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::Path;

use super::ResolvedCell;
use crate::error::{Error, Result};
use crate::ida::python::PipOptions;
use crate::plugin::{dependencies_from_archive, requirements};

fn required_closure(
    cell: &ResolvedCell,
    roots: impl IntoIterator<Item = String>,
) -> HashSet<String> {
    let mut selected = HashSet::new();
    let mut pending: Vec<_> = roots.into_iter().map(|name| name.to_lowercase()).collect();
    while let Some(name) = pending.pop() {
        if !selected.insert(name.clone()) {
            continue;
        }
        if let Some((_, (_, metadata))) =
            cell.plugins.iter().find(|(candidate, _)| candidate.eq_ignore_ascii_case(&name))
        {
            pending.extend(
                requirements::collect(metadata)
                    .into_iter()
                    .filter(|requirement| requirement.required)
                    .map(|requirement| requirement.name.to_lowercase()),
            );
        }
    }
    selected
}

fn dependencies(
    cell: &ResolvedCell,
    names: &HashSet<String>,
    by_name: &HashMap<String, Vec<String>>,
) -> Vec<String> {
    let mut seen = HashSet::new();
    cell.plugins
        .keys()
        .filter(|name| names.contains(&name.to_lowercase()))
        .flat_map(|name| by_name[&name.to_lowercase()].iter())
        .filter(|dependency| seen.insert((*dependency).clone()))
        .cloned()
        .collect()
}

async fn download(
    cell: &ResolvedCell,
    names: &HashSet<String>,
    by_name: &HashMap<String, Vec<String>>,
    destination: &Path,
    options: &PipOptions,
) -> Result<()> {
    let dependencies = dependencies(cell, names, by_name);
    if !dependencies.is_empty() {
        super::download::wheelhouse(&dependencies, &cell.target, destination, options).await?;
    }
    Ok(())
}

pub(super) async fn prepare(
    cell: &ResolvedCell,
    destination: &Path,
    options: &PipOptions,
) -> Result<HashSet<String>> {
    let by_name: HashMap<_, _> = cell
        .plugins
        .iter()
        .map(|(name, (bytes, _))| Ok((name.to_lowercase(), dependencies_from_archive(bytes)?)))
        .collect::<Result<_>>()?;
    let all: HashSet<_> = cell.plugins.keys().map(|name| name.to_lowercase()).collect();
    let required = required_closure(cell, cell.roots.clone());
    match download(cell, &all, &by_name, destination, options).await {
        Ok(()) => return Ok(all),
        Err(error) if all == required => return Err(error),
        Err(error) if !matches!(error, Error::BundleDownload(_)) => return Err(error),
        Err(error) => tracing::debug!(%error, "retrying bundle wheels without optional plugins"),
    }
    let attempts = tempfile::tempdir()?;
    let mut accepted = required;
    let mut current = attempts.path().join("required");
    std::fs::create_dir(&current)?;
    download(cell, &accepted, &by_name, &current, options).await?;
    let mut queue: VecDeque<_> = cell
        .plugins
        .keys()
        .map(|name| name.to_lowercase())
        .filter(|name| accepted.contains(name))
        .collect();
    let mut visited = HashSet::new();
    let mut failed = HashSet::new();
    let mut attempt_index = 0;
    while let Some(parent) = queue.pop_front() {
        if !visited.insert(parent.clone()) {
            continue;
        }
        let Some((_, (_, metadata))) =
            cell.plugins.iter().find(|(name, _)| name.eq_ignore_ascii_case(&parent))
        else {
            continue;
        };
        for requirement in
            requirements::collect(metadata).into_iter().filter(|requirement| !requirement.required)
        {
            let name = requirement.name.to_lowercase();
            if accepted.contains(&name) || failed.contains(&name) || !all.contains(&name) {
                continue;
            }
            let group = required_closure(cell, [name.clone()]);
            let candidate: HashSet<_> = accepted.union(&group).cloned().collect();
            let ordered_group: Vec<_> = cell
                .plugins
                .keys()
                .map(|name| name.to_lowercase())
                .filter(|name| group.contains(name))
                .collect();
            let new_dependencies: HashSet<_> =
                dependencies(cell, &candidate, &by_name).into_iter().collect();
            let existing_dependencies: HashSet<_> =
                dependencies(cell, &accepted, &by_name).into_iter().collect();
            if new_dependencies == existing_dependencies {
                queue.extend(ordered_group);
                accepted = candidate;
                continue;
            }
            attempt_index += 1;
            let attempt = attempts.path().join(attempt_index.to_string());
            std::fs::create_dir(&attempt)?;
            match download(cell, &candidate, &by_name, &attempt, options).await {
                Ok(()) => {
                    queue.extend(ordered_group);
                    accepted = candidate;
                    current = attempt;
                }
                Err(error @ Error::BundleDownload(_)) => {
                    failed.insert(name);
                    crate::util::fmt::warning(&format!(
                        "{}: skipping optional dependency {requirement}: {error}",
                        cell.target.id()
                    ));
                }
                Err(error) => return Err(error),
            }
        }
    }
    std::fs::remove_dir_all(destination)?;
    std::fs::create_dir(destination)?;
    for entry in std::fs::read_dir(current)? {
        let entry = entry?;
        std::fs::copy(entry.path(), destination.join(entry.file_name()))?;
    }
    Ok(accepted)
}
