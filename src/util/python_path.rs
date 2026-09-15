//! CPython pathlib spelling and home expansion, without filesystem canonicalization.

use crate::error::{Error, Result};

mod exists;
mod home;
pub(crate) use exists::exists;
pub(crate) mod path;

#[cfg(test)]
mod tests;

use path::{Flavor, ParsedPath};

/// Preserve pathlib's lexical normalization before a native filesystem lookup.
pub(crate) fn join(base: &std::path::Path, relative: &str) -> std::path::PathBuf {
    if cfg!(windows) {
        ParsedPath::joined(&base.to_string_lossy(), relative, Flavor::Windows)
            .render(Flavor::Windows)
            .into()
    } else if let Some(base) = base.to_str() {
        ParsedPath::joined(base, relative, Flavor::Posix).render(Flavor::Posix).into()
    } else {
        let relative = ParsedPath::parse(relative, Flavor::Posix);
        if relative.root.is_empty() && relative.tail.is_empty() {
            base.to_path_buf()
        } else {
            base.join(relative.render(Flavor::Posix))
        }
    }
}

/// Normalize lexical path spelling and expand an initial home component.
pub(crate) fn expand_user(value: &str) -> Result<String> {
    let flavor = if cfg!(windows) {
        Flavor::Windows
    } else {
        Flavor::Posix
    };
    expand_with(value, flavor, home::resolve)
}

/// Apply the plugin find-links callback's additional URL-preservation rule.
pub(crate) fn expand_user_unless_url(value: &str) -> Result<String> {
    unless_url(value, || expand_user(value))
}

fn unless_url(value: &str, expand: impl FnOnce() -> Result<String>) -> Result<String> {
    // The source find-links callback uses a substring check, not URL parsing.
    if value.contains("://") {
        Ok(value.into())
    } else {
        expand()
    }
}

fn expand_with(
    value: &str,
    flavor: Flavor,
    home: impl FnOnce(&str) -> Result<Option<String>>,
) -> Result<String> {
    let mut path = ParsedPath::parse(value, flavor);
    if path.drive.is_empty()
        && path.root.is_empty()
        && let Some(first) = path.tail.first().filter(|part| part.starts_with('~'))
    {
        let home = home(&first[1..])?
            .filter(|home| !home.starts_with('~'))
            .ok_or_else(|| Error::Other("Could not determine home directory.".into()))?;
        let mut expanded = ParsedPath::parse(&home, flavor);
        expanded.tail.extend(path.tail.drain(1..));
        path = expanded;
    }
    Ok(path.render(flavor))
}
