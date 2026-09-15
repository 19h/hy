//! Repository identity lookup precedes version ordering and location compatibility.

use super::{Location, Plugin, Reference, Snapshot, normalize_host};
use crate::error::{Error, Result};
use crate::plugin::{
    is_ida_version_compatible, is_platform_compatible, parse_version, valid_specification,
    version_matches,
};
use crate::util::python_repr::string_repr;

struct Identity {
    name: String,
    host: Option<String>,
}

impl Identity {
    fn new(reference: &Reference) -> Result<Self> {
        Ok(Self {
            name: reference.name.to_lowercase(),
            host: reference
                .host
                .as_deref()
                .filter(|host| !host.is_empty())
                .map(normalize_host)
                .transpose()?,
        })
    }

    fn matches(&self, plugin: &Plugin) -> Result<bool> {
        if plugin.name.to_lowercase() != self.name {
            return Ok(false);
        }
        match &self.host {
            Some(host) => Ok(normalize_host(&plugin.host)? == *host),
            None => Ok(true),
        }
    }
}

/// Catalogue scans retain their existing best-effort handling of malformed hosts.
pub fn matches_reference(plugin: &Plugin, reference: &Reference) -> bool {
    Identity::new(reference).and_then(|identity| identity.matches(plugin)).unwrap_or(false)
}

pub fn matching_plugins<'a>(
    snapshot: &'a Snapshot,
    reference: &Reference,
) -> Result<Vec<&'a Plugin>> {
    let identity = Identity::new(reference)?;
    let mut matches = Vec::new();
    for plugin in &snapshot.plugins {
        if identity.matches(plugin)? {
            matches.push(plugin);
        }
    }
    Ok(matches)
}

pub fn select<'a>(
    snapshot: &'a Snapshot,
    reference: &Reference,
    ida_version: Option<&str>,
) -> Result<&'a Location> {
    select_for_platform(snapshot, reference, &crate::ida::current_ida_platform()?, ida_version)
}

pub fn select_for_platform<'a>(
    snapshot: &'a Snapshot,
    reference: &Reference,
    platform: &str,
    ida_version: Option<&str>,
) -> Result<&'a Location> {
    let specification = if reference.spec.is_empty() {
        ">=0"
    } else {
        &reference.spec
    };
    if !valid_specification(specification) {
        return Err(Error::Other(format!("invalid plugin version spec: {specification}")));
    }
    let candidates = matching_plugins(snapshot, reference)?;
    let plugin = match candidates.as_slice() {
        [plugin] => *plugin,
        [] => return Err(Error::NotFound(reference.name.clone())),
        _ => {
            return Err(Error::Other(format!(
                "ambiguous plugin reference: {} matches {} plugins",
                string_repr(&reference.name),
                candidates.len(),
            )));
        }
    };
    let mut versions = Vec::new();
    for (version, locations) in &plugin.versions {
        let parsed = parse_version(version).ok_or_else(|| {
            Error::Other(format!("invalid plugin repository version: {}", string_repr(version)))
        })?;
        versions.push((parsed, version, locations));
    }
    versions.sort_by(|left, right| right.0.cmp_precedence(&left.0));
    for (_, version, locations) in versions {
        if !version_matches(version, specification) {
            continue;
        }
        for location in locations {
            let metadata = &location.descriptor.metadata;
            if is_platform_compatible(metadata, platform)
                && ida_version.is_none_or(|version| is_ida_version_compatible(metadata, version))
            {
                return Ok(location);
            }
        }
    }
    Err(Error::NotFound(format!("no compatible version of {}{}", reference.name, reference.spec)))
}

#[cfg(test)]
mod tests;
