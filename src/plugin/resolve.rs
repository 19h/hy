//! Resolve complete plugin dependency graphs before downloading or publishing files.
//!
//! Candidate pruning reaches a fixed point before the bounded, newest-first search.
//! Repository access and Python probing belong to the caller, keeping this deterministic.

use std::collections::{HashMap, HashSet, VecDeque};

use indexmap::IndexMap;

use super::PluginMetadata;
use super::index::{Location, Snapshot};
use super::requirements::{self, Requirement};

const MAX_STEPS: usize = 10_000;
const MAX_SELECTIONS: usize = 256;

#[derive(Debug, Clone)]
pub struct Cell {
    pub platform: String,
    pub ida_version: Option<String>,
    pub python_version: Option<String>,
    pub check_python: bool,
}

impl Cell {
    pub fn incompatibility(&self, metadata: &PluginMetadata) -> Option<String> {
        if !super::is_platform_compatible(metadata, &self.platform) {
            return Some(format!("does not support {}", self.platform));
        }
        if let Some(version) = &self.ida_version
            && !super::is_ida_version_compatible(metadata, version)
        {
            return Some(format!("does not support IDA {version}"));
        }
        if self.check_python
            && let Some(requirement) = &metadata.requires_python
        {
            match &self.python_version {
                Some(version) if requirements::python_matches(version, requirement) => (),
                Some(version) => {
                    return Some(format!("requires Python {requirement}, found {version}"));
                }
                None => {
                    return Some(format!(
                        "requires Python {requirement}, and IDA's Python cannot be detected"
                    ));
                }
            }
        }
        None
    }
}

#[derive(Debug)]
pub struct ResolutionError {
    pub message: String,
    pub fatal: bool,
}

impl std::fmt::Display for ResolutionError {
    fn fmt(&self, output: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        output.write_str(&self.message)
    }
}

impl std::error::Error for ResolutionError {}

impl From<ResolutionError> for crate::error::Error {
    fn from(error: ResolutionError) -> Self {
        Self::PluginInstall(error.message)
    }
}

#[derive(Debug, Default)]
pub struct Resolution {
    pub selected: IndexMap<String, Location>,
    pub order: Vec<String>,
    pub skipped: Vec<String>,
    pub warnings: Vec<String>,
}

#[derive(Clone)]
struct Pending {
    requirement: Requirement,
    chain: Vec<String>,
}

impl Pending {
    fn failure(&self, reason: impl std::fmt::Display) -> ResolutionError {
        let mut chain = self.chain.clone();
        chain.push(self.requirement.to_string());
        ResolutionError {
            message: format!("cannot resolve {}: {reason}", chain.join(" -> ")),
            fatal: false,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Source {
    Fixed,
    Installed,
    Repository,
}

#[derive(Clone)]
struct Identity {
    name: String,
    version: String,
    host: String,
}

impl From<&PluginMetadata> for Identity {
    fn from(metadata: &PluginMetadata) -> Self {
        Self {
            name: metadata.name.clone(),
            version: metadata.version.clone(),
            host: metadata.urls.repository.clone(),
        }
    }
}

#[derive(Clone)]
struct Candidate {
    identity: Identity,
    location: Location,
}

#[derive(Clone)]
struct Choice {
    identity: Identity,
    metadata: PluginMetadata,
    source: Source,
    location: Option<Location>,
    constraints: Vec<Requirement>,
}

#[derive(Clone, Default)]
struct State {
    chosen: IndexMap<String, Choice>,
    skipped: Vec<String>,
    warnings: Vec<String>,
}

struct Resolver<'a> {
    snapshot: &'a Snapshot,
    cell: &'a Cell,
    installed: &'a IndexMap<String, PluginMetadata>,
    viable: HashMap<usize, Vec<Candidate>>,
    steps: usize,
}

impl Resolver<'_> {
    fn satisfies(&self, identity: &Identity, requirement: &Requirement) -> (bool, Option<String>) {
        if requirement.host.as_ref().is_some_and(|host| {
            super::index::normalize_host(&identity.host).ok().as_ref() != Some(host)
        }) {
            return (false, None);
        }
        if requirement.matches(&identity.version) {
            return (true, None);
        }
        if let Some(installed) = self.installed.get(&requirement.name.to_lowercase())
            && let Some(pin) = requirement.spec.strip_prefix("==").and_then(super::parse_version)
        {
            let old = super::version_precedence(&installed.version);
            if Some(pin) < old && old <= super::version_precedence(&identity.version) {
                return (
                    true,
                    Some(format!(
                        "{} {} is installed, which is newer than {requirement}; not downgrading",
                        installed.name, installed.version
                    )),
                );
            }
        }
        (false, None)
    }

    fn lookup(&self, pending: &Pending) -> Result<Option<usize>, ResolutionError> {
        let requirement = &pending.requirement;
        let installed = self.installed.get(&requirement.name.to_lowercase());
        let host = installed
            .and_then(|metadata| metadata.normalized_host().ok())
            .or(requirement.host.clone());
        if let (Some(installed), Some(required)) = (installed, &requirement.host)
            && installed.normalized_host().ok().as_ref() != Some(required)
        {
            return Ok(None);
        }
        let matches: Vec<_> = self
            .snapshot
            .plugins
            .iter()
            .enumerate()
            .filter(|(_, plugin)| {
                plugin.name.eq_ignore_ascii_case(&requirement.name)
                    && host.as_ref().is_none_or(|host| {
                        super::index::normalize_host(&plugin.host).ok().as_ref() == Some(host)
                    })
            })
            .collect();
        match matches.as_slice() {
            [] => Ok(None),
            [(index, _)] => Ok(Some(*index)),
            _ => Err(ResolutionError {
                fatal: true,
                message: format!(
                    "ambiguous plugin {requirement}; use one of: {}",
                    matches
                        .iter()
                        .map(|(_, plugin)| format!("{}@{}", plugin.name, plugin.host))
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
            }),
        }
    }

    fn reachable(
        &mut self,
        pending: &[Pending],
        fixed: &IndexMap<String, Choice>,
    ) -> Result<(), ResolutionError> {
        let mut queue = VecDeque::from(pending.to_vec());
        while let Some(item) = queue.pop_front() {
            let key = item.requirement.name.to_lowercase();
            if fixed.contains_key(&key) {
                continue;
            }
            if self.installed.get(&key).is_some_and(|metadata| {
                self.satisfies(&Identity::from(metadata), &item.requirement).0
            }) {
                continue;
            }
            let Some(index) = self.lookup(&item)? else {
                continue;
            };
            if self.viable.contains_key(&index) {
                continue;
            }
            let plugin = &self.snapshot.plugins[index];
            for version in plugin.versions.keys() {
                if super::parse_version(version).is_none() {
                    return Err(ResolutionError {
                        fatal: true,
                        message: format!("invalid plugin repository version: {version}"),
                    });
                }
            }
            let mut versions: Vec<_> = plugin.versions.iter().collect();
            versions.sort_by(|(left, _), (right, _)| {
                super::version_precedence(right).cmp(&super::version_precedence(left))
            });
            let mut locations = Vec::new();
            for (version, candidates) in versions {
                if let Some(location) = candidates.iter().find(|location| {
                    self.cell.incompatibility(&location.descriptor.metadata).is_none()
                }) {
                    let mut chain = item.chain.clone();
                    chain.push(format!("{} {}", plugin.name, location.descriptor.metadata.version));
                    queue.extend(
                        requirements::collect(&location.descriptor.metadata).into_iter().map(
                            |requirement| Pending {
                                requirement,
                                chain: chain.clone(),
                            },
                        ),
                    );
                    locations.push(Candidate {
                        identity: Identity {
                            name: plugin.name.clone(),
                            version: version.clone(),
                            host: plugin.host.clone(),
                        },
                        location: location.clone(),
                    });
                }
            }
            self.viable.insert(index, locations);
        }
        // Each pass removes at least one candidate or terminates. Cycles with a
        // consistent version remain candidates and are handled by the search.
        loop {
            let mut removals = Vec::new();
            for (&index, locations) in &self.viable {
                for (position, location) in locations.iter().enumerate() {
                    for requirement in requirements::collect(&location.location.descriptor.metadata)
                        .into_iter()
                        .filter(|requirement| requirement.required)
                    {
                        if !self.has_candidate(&requirement, fixed)? {
                            removals.push((index, position));
                            break;
                        }
                    }
                }
            }
            if removals.is_empty() {
                break;
            }
            removals.sort_unstable();
            for (index, position) in removals.into_iter().rev() {
                self.viable.get_mut(&index).expect("known candidate").remove(position);
            }
        }
        Ok(())
    }

    fn has_candidate(
        &self,
        requirement: &Requirement,
        fixed: &IndexMap<String, Choice>,
    ) -> Result<bool, ResolutionError> {
        let key = requirement.name.to_lowercase();
        if let Some(choice) = fixed.get(&key) {
            return Ok(self.satisfies(&choice.identity, requirement).0);
        }
        if self
            .installed
            .get(&key)
            .is_some_and(|metadata| self.satisfies(&Identity::from(metadata), requirement).0)
        {
            return Ok(true);
        }
        let pending = Pending {
            requirement: requirement.clone(),
            chain: Vec::new(),
        };
        Ok(self.lookup(&pending)?.and_then(|index| self.viable.get(&index)).is_some_and(
            |locations| {
                locations.iter().any(|candidate| requirement.matches(&candidate.identity.version))
            },
        ))
    }

    fn search(
        &mut self,
        pending: Vec<Pending>,
        mut state: State,
        depth: usize,
    ) -> Result<State, ResolutionError> {
        for (position, item) in pending.iter().enumerate() {
            self.steps += 1;
            if self.steps > MAX_STEPS || depth > MAX_SELECTIONS {
                return Err(ResolutionError {
                    message: format!(
                        "plugin resolution exceeded {MAX_STEPS} steps or {MAX_SELECTIONS} selections"
                    ),
                    fatal: true,
                });
            }
            let key = item.requirement.name.to_lowercase();
            let existing = state.chosen.get(&key).cloned().or_else(|| {
                self.installed.get(&key).map(|metadata| Choice {
                    identity: Identity::from(metadata),
                    metadata: metadata.clone(),
                    source: Source::Installed,
                    location: None,
                    constraints: Vec::new(),
                })
            });
            if let Some(mut choice) = existing.clone() {
                let (satisfied, warning) = self.satisfies(&choice.identity, &item.requirement);
                if satisfied {
                    choice.constraints.push(item.requirement.clone());
                    state.chosen.insert(key, choice);
                    if let Some(warning) = warning {
                        state.warnings.push(warning);
                    }
                    continue;
                }
                if choice.source != Source::Installed {
                    let error = item.failure(format!(
                        "{} {} is already selected",
                        choice.identity.name, choice.identity.version
                    ));
                    if item.requirement.required {
                        return Err(error);
                    }
                    state.skipped.push(error.message);
                    continue;
                }
            }
            let mut constraints =
                existing.as_ref().map(|choice| choice.constraints.clone()).unwrap_or_default();
            constraints.push(item.requirement.clone());
            let index = self.lookup(item)?;
            let candidates =
                index.and_then(|index| self.viable.get(&index)).cloned().unwrap_or_default();
            let mut failure = item.failure(self.reasons(index, &item.requirement));
            for candidate in candidates {
                let location = candidate.location;
                let metadata = &location.descriptor.metadata;
                if !constraints
                    .iter()
                    .all(|constraint| constraint.matches(&candidate.identity.version))
                {
                    continue;
                }
                let mut branch = state.clone();
                branch.chosen.insert(
                    key.clone(),
                    Choice {
                        identity: candidate.identity.clone(),
                        metadata: metadata.clone(),
                        source: Source::Repository,
                        location: Some(location.clone()),
                        constraints: constraints.clone(),
                    },
                );
                let mut chain = item.chain.clone();
                chain.push(format!("{} {}", candidate.identity.name, candidate.identity.version));
                let dependencies = requirements::collect(metadata)
                    .into_iter()
                    .map(|requirement| Pending {
                        requirement,
                        chain: chain.clone(),
                    })
                    .collect();
                match self.search(
                    schedule(dependencies, &pending[position + 1..]),
                    branch,
                    depth + 1,
                ) {
                    Ok(solution) => return Ok(solution),
                    Err(error) if error.fatal => return Err(error),
                    Err(error) => failure = error,
                }
            }
            if item.requirement.required {
                return Err(failure);
            }
            state.skipped.push(failure.message);
        }
        Ok(state)
    }

    fn reasons(&self, index: Option<usize>, requirement: &Requirement) -> String {
        let Some(index) = index else {
            return format!(
                "{} is not in the repository, or conflicts with its installed host",
                requirement.name
            );
        };
        let plugin = &self.snapshot.plugins[index];
        let mut groups: IndexMap<String, Vec<String>> = IndexMap::new();
        let mut versions: Vec<_> =
            plugin.versions.iter().filter(|(version, _)| requirement.matches(version)).collect();
        versions.sort_by(|(left, _), (right, _)| {
            super::version_precedence(right).cmp(&super::version_precedence(left))
        });
        for (version, locations) in versions {
            let compatible = locations
                .iter()
                .any(|location| self.cell.incompatibility(&location.descriptor.metadata).is_none());
            let reason = if compatible {
                "has unsatisfied or conflicting required dependencies".into()
            } else {
                locations
                    .first()
                    .and_then(|location| self.cell.incompatibility(&location.descriptor.metadata))
                    .unwrap_or_else(|| "has no download locations".into())
            };
            groups.entry(reason).or_default().push(version.clone());
        }
        if groups.is_empty() {
            return format!("no {} version matches {}", plugin.name, requirement.spec);
        }
        groups
            .into_iter()
            .map(|(reason, versions)| {
                if versions.len() == 1 {
                    format!("{} {} {reason}", plugin.name, versions[0])
                } else {
                    format!(
                        "{} {} and {} older version(s) {reason}",
                        plugin.name,
                        versions[0],
                        versions.len() - 1
                    )
                }
            })
            .collect::<Vec<_>>()
            .join("; ")
    }
}

fn schedule(dependencies: Vec<Pending>, rest: &[Pending]) -> Vec<Pending> {
    dependencies
        .iter()
        .chain(rest)
        .filter(|item| item.requirement.required)
        .cloned()
        .chain(rest.iter().chain(&dependencies).filter(|item| !item.requirement.required).cloned())
        .collect()
}

pub fn resolve(
    snapshot: &Snapshot,
    roots: &[Requirement],
    cell: &Cell,
    fixed: &[PluginMetadata],
    installed: &[PluginMetadata],
) -> Result<Resolution, ResolutionError> {
    let installed =
        installed.iter().map(|metadata| (metadata.name.to_lowercase(), metadata.clone())).collect();
    let mut resolver = Resolver {
        snapshot,
        cell,
        installed: &installed,
        viable: HashMap::new(),
        steps: 0,
    };
    let mut state = State::default();
    let mut pending = Vec::new();
    for metadata in fixed {
        let key = metadata.name.to_lowercase();
        pending.extend(requirements::collect(metadata).into_iter().map(|requirement| Pending {
            requirement,
            chain: vec![format!("{} {}", metadata.name, metadata.version)],
        }));
        state.chosen.insert(
            key,
            Choice {
                identity: Identity::from(metadata),
                metadata: metadata.clone(),
                source: Source::Fixed,
                location: None,
                constraints: Vec::new(),
            },
        );
    }
    pending.extend(roots.iter().map(|requirement| Pending {
        requirement: requirement.clone(),
        chain: Vec::new(),
    }));
    let pending = schedule(Vec::new(), &pending);
    resolver.reachable(&pending, &state.chosen)?;
    let state = resolver.search(pending, state, 0)?;
    let mut order = Vec::new();
    let mut visited = HashSet::new();
    fn visit(key: &str, state: &State, visited: &mut HashSet<String>, order: &mut Vec<String>) {
        if !visited.insert(key.into()) {
            return;
        }
        let Some(choice) = state.chosen.get(key) else {
            return;
        };
        if choice.source != Source::Installed {
            for requirement in requirements::collect(&choice.metadata) {
                visit(&requirement.name.to_lowercase(), state, visited, order);
            }
        }
        if choice.source == Source::Repository {
            order.push(choice.identity.name.clone());
        }
    }
    for key in state.chosen.keys() {
        visit(key, &state, &mut visited, &mut order);
    }
    let selected = state
        .chosen
        .into_values()
        .filter_map(|choice| choice.location.map(|location| (choice.identity.name, location)))
        .collect();
    let mut warnings = state.warnings;
    let mut unique = HashSet::new();
    warnings.retain(|warning| unique.insert(warning.clone()));
    Ok(Resolution {
        selected,
        order,
        skipped: state.skipped,
        warnings,
    })
}

/// Determine whether reachable, platform-compatible candidates need a Python probe.
pub fn needs_python(
    snapshot: &Snapshot,
    roots: &[Requirement],
    fixed: &[PluginMetadata],
    cell: &Cell,
) -> bool {
    if fixed.iter().any(|metadata| metadata.requires_python.is_some()) {
        return true;
    }
    let mut pending: VecDeque<_> = roots.to_vec().into();
    pending.extend(fixed.iter().flat_map(requirements::collect));
    let mut visited = HashSet::new();
    while let Some(requirement) = pending.pop_front() {
        if !visited.insert((requirement.name.to_lowercase(), requirement.host.clone())) {
            continue;
        }
        for plugin in &snapshot.plugins {
            if !plugin.name.eq_ignore_ascii_case(&requirement.name) {
                continue;
            }
            for location in plugin.versions.values().flatten() {
                let metadata = &location.descriptor.metadata;
                if cell.incompatibility(metadata).is_some() {
                    continue;
                }
                if metadata.requires_python.is_some() {
                    return true;
                }
                pending.extend(requirements::collect(metadata));
            }
        }
    }
    false
}

#[cfg(test)]
mod tests;
