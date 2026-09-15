//! Lexical paths used by Python's archive metadata and subtree operations.

use crate::error::{Error, Result};
use crate::util::python_path::path::{Flavor, ParsedPath};

fn native_flavor() -> Flavor {
    if cfg!(windows) {
        Flavor::Windows
    } else {
        Flavor::Posix
    }
}

pub(crate) fn descriptor_prefix(path: &str) -> String {
    let flavor = native_flavor();
    let mut parent = ParsedPath::parse(path, flavor);
    parent.tail.pop();
    let parent = parent.render(flavor);
    if parent == "." {
        String::new()
    } else if cfg!(windows) {
        format!("{}/", parent.replace('\\', "/"))
    } else {
        format!("{parent}/")
    }
}

/// Render a descriptor location as the host's lexical pathlib path.
pub(crate) fn display(path: &str) -> String {
    ParsedPath::parse(path, native_flavor()).render(native_flavor())
}

/// README discovery compares lexical parents, including directory entries.
pub(crate) fn filename_in_prefix(path: &str, prefix: &str) -> Option<String> {
    let parent = descriptor_prefix(path);
    let same_parent = if cfg!(windows) {
        parent.to_lowercase() == prefix.to_lowercase()
    } else {
        parent == prefix
    };
    if !same_parent {
        return None;
    }
    ParsedPath::parse(path, native_flavor()).tail.pop()
}

pub(super) fn reference(prefix: &str, relative: &str) -> String {
    let flavor = native_flavor();
    let root = ParsedPath::parse(prefix.strip_suffix('/').unwrap_or(prefix), flavor).render(flavor);
    let root = ParsedPath::joined(&root, relative, flavor);
    let path = root.render(flavor);
    if cfg!(windows) {
        path.replace('\\', "/")
    } else {
        path
    }
}

/// Apply the source's raw-prefix filter before POSIX relative-path conversion.
pub(super) fn extraction_relative(prefix: &str, name: &str) -> Result<Option<String>> {
    if !name.starts_with(prefix) || name == prefix || name.starts_with(&format!("{prefix}.git/")) {
        return Ok(None);
    }
    let path = ParsedPath::parse(name, Flavor::Posix);
    let root = ParsedPath::parse(prefix.trim_end_matches('/'), Flavor::Posix);
    if path.root != root.root || !path.tail.starts_with(&root.tail) {
        return Err(Error::PluginInstall(format!("archive path is outside plugin root: {name}")));
    }
    let relative = path.tail[root.tail.len()..].join("/");
    if relative.is_empty() {
        Ok(None)
    } else {
        Ok(Some(relative))
    }
}
