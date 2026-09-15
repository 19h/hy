//! Ordered source lookup using raw filename patterns, independently of IPC names.

use std::path::{Path, PathBuf};

use indexmap::IndexMap;
use serde_json::json;

use crate::config::ConfigStore;
use crate::error::Result;

use super::pattern::Pattern;

pub(super) fn find_database(name: &str, source: &str) -> Result<Option<PathBuf>> {
    let sources = sources(&mut ConfigStore::global())?;
    for (source_name, root) in sources {
        if !source.is_empty() && source != "localhost" && source != source_name {
            continue;
        }
        if let Some(path) = find_in_directory(Path::new(&root), name) {
            return Ok(Some(path));
        }
    }
    Ok(None)
}

fn sources(store: &mut ConfigStore) -> Result<IndexMap<String, String>> {
    let mut sources: IndexMap<String, String> = store
        .get_value("idb.sources")
        .filter(|value| !value.is_null())
        .cloned()
        .map(serde_json::from_value)
        .transpose()?
        .unwrap_or_default();
    if sources.is_empty()
        && let Some(legacy) = store.get_value("idb.search-paths").filter(|value| !value.is_null())
    {
        let paths: Vec<String> = serde_json::from_value(legacy.clone())?;
        for (index, path) in paths.into_iter().enumerate() {
            sources.insert(format!("source-{}", index + 1), path);
        }
        if !sources.is_empty() {
            store.commit_changes([
                ("idb.sources".into(), Some(json!(sources))),
                ("idb.search-paths".into(), None),
            ])?;
        }
    }
    Ok(sources)
}

fn find_in_directory(root: &Path, name: &str) -> Option<PathBuf> {
    let literal = !name.contains(['*', '?', '[']);
    let pattern = Pattern::new(name);
    let mut directories = vec![root.to_owned()];
    while let Some(directory) = directories.pop() {
        if literal {
            let candidate = directory.join(name);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
        let Ok(entries) = std::fs::read_dir(directory) else {
            // pathlib suppresses scanning errors and continues with other sources.
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if !literal && pattern.matches(&entry.file_name().to_string_lossy()) && path.is_file() {
                return Some(path);
            }
            if entry.file_type().is_ok_and(|kind| kind.is_dir()) {
                directories.push(path);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests;
