//! Bundle-specific preprocessing before repository lookup and version matching.

use crate::error::{Error, Result};
use crate::plugin::index::{Reference, parse_reference};

pub(super) fn prepare(input: &str) -> Result<Reference> {
    let (clean, host) = match parse_reference(input) {
        Ok(reference) => (format!("{}{}", reference.name, reference.spec), reference.host),
        Err(_) => (input.into(), None),
    };
    if !clean.contains("==") {
        let qualifier = host.as_ref().map(|host| format!("@{host}")).unwrap_or_default();
        return Err(Error::Other(format!(
            "repository plugin specs must include exact version (e.g. {clean}==1.0.0{qualifier})",
        )));
    }
    // Upstream drops a parsed repository scope and lets repository lookup parse
    // the remaining version text. A failed reference parse retains the raw spec.
    let split = clean.find(['=', '<', '>', '!', '~']).unwrap_or(clean.len());
    Ok(Reference {
        name: clean[..split].into(),
        spec: clean[split..].into(),
        host,
        repo: None,
    })
}

#[cfg(test)]
mod tests;
