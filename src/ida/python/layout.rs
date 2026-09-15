//! Shared interpreter layout candidates and lexical absolute paths.

use std::path::{Component, Path, PathBuf};

pub(super) fn candidates(root: &Path, version: Option<&str>) -> Vec<PathBuf> {
    if cfg!(windows) {
        return vec![root.join("Scripts/python.exe"), root.join("python.exe")];
    }
    let mut paths = Vec::new();
    if let Some(version) = version.filter(|version| !version.is_empty()) {
        paths.push(root.join(format!("bin/python{version}")));
    }
    paths.extend([root.join("bin/python3"), root.join("bin/python")]);
    paths
}

pub(super) fn store_shim(path: &Path) -> bool {
    let path = path.to_string_lossy().to_lowercase();
    path.contains("microsoft/windowsapps") || path.contains("microsoft\\windowsapps")
}

pub(super) fn absolute(path: &Path) -> std::io::Result<PathBuf> {
    let mut result = PathBuf::new();
    for component in std::path::absolute(path)?.components() {
        match component {
            Component::CurDir => (),
            Component::ParentDir => {
                result.pop();
            }
            _ => result.push(component.as_os_str()),
        }
    }
    Ok(result)
}
