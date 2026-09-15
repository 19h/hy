//! Non-strict Unix realpath, including missing tails and cached link resolution.

use std::collections::HashMap;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

enum Step {
    Component(OsString),
    FinishLink(PathBuf),
}

fn push_path(pending: &mut Vec<Step>, path: &Path) {
    pending.extend(
        path.components().rev().map(|component| Step::Component(component.as_os_str().to_owned())),
    );
}

pub fn resolve(path: &Path) -> std::io::Result<PathBuf> {
    let absolute = if path.is_absolute() {
        path.to_owned()
    } else {
        std::env::current_dir()?.join(path)
    };
    let mut pending = Vec::new();
    push_path(&mut pending, &absolute);
    let mut resolved = PathBuf::new();
    // None marks a link being expanded; Some caches its completed target.
    let mut links: HashMap<PathBuf, Option<PathBuf>> = HashMap::new();
    while let Some(step) = pending.pop() {
        let name = match step {
            Step::FinishLink(link) => {
                links.insert(link, Some(resolved.clone()));
                continue;
            }
            Step::Component(name) => name,
        };
        if name == "/" {
            resolved = PathBuf::from("/");
            continue;
        }
        if name == "." {
            continue;
        }
        if name == ".." {
            resolved.pop();
            continue;
        }
        let next = resolved.join(name);
        if std::fs::symlink_metadata(&next).is_ok_and(|metadata| metadata.is_symlink()) {
            if let Some(cached) = links.get(&next) {
                // CPython's non-strict resolver leaves a cyclic link unresolved.
                resolved = cached.as_ref().unwrap_or(&next).clone();
                continue;
            }
            if let Ok(target) = std::fs::read_link(&next) {
                links.insert(next.clone(), None);
                pending.push(Step::FinishLink(next));
                push_path(&mut pending, &target);
                continue;
            }
        }
        // Non-strict realpath retains missing or inaccessible components.
        resolved = next;
    }
    Ok(resolved)
}
