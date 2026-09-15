//! Filesystem observations for diagnostics; no interpreter or startup script execution.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::ida::python::env_var;
use crate::util::{realpath, strings::python_trim};

pub(in crate::ida::python) fn venv_root(executable: &Path) -> Option<PathBuf> {
    if !executable.file_name()?.to_string_lossy().to_lowercase().contains("python") {
        return None;
    }
    let parent = executable.parent()?;
    if !matches!(parent.file_name()?.to_str(), Some("bin" | "Scripts")) {
        return None;
    }
    let root = parent.parent()?;
    let root = if root.as_os_str().is_empty() {
        Path::new(".")
    } else {
        root
    };
    root.join("pyvenv.cfg").exists().then(|| root.into())
}

pub(in crate::ida::python) fn config(root: &Path) -> HashMap<String, String> {
    let Ok(bytes) = std::fs::read(root.join("pyvenv.cfg")) else {
        return HashMap::new();
    };
    String::from_utf8_lossy(&bytes)
        .split(|c| {
            matches!(
                c,
                '\n' | '\r' | '\x0b' | '\x0c' | '\x1c'
                    ..='\x1e' | '\u{85}' | '\u{2028}' | '\u{2029}'
            )
        })
        .filter_map(|line| line.split_once('='))
        .map(|(key, value)| (python_trim(key).to_lowercase(), python_trim(value).into()))
        .collect()
}

pub(in crate::ida::python) fn normalized(path: &Path) -> Option<PathBuf> {
    let path = if path.as_os_str().is_empty() {
        Path::new(".")
    } else {
        path
    };
    let mut result = super::super::layout::absolute(path).ok()?;
    if cfg!(windows) {
        result = result.to_string_lossy().to_lowercase().into();
    }
    Some(result)
}

pub(super) fn variable_selects(executable: Option<&Path>, root: Option<&Path>) -> bool {
    let (Some(executable), Some(root)) = (executable, root) else {
        return false;
    };
    let requested =
        venv_root(executable).or_else(|| executable.parent()?.parent().map(Path::to_owned));
    requested.and_then(|path| normalized(&path)).is_some_and(|path| Some(path) == normalized(root))
}

pub(super) fn homebrew(path: &Path) -> bool {
    [Some(path.to_owned()), realpath::resolve(path).ok()].into_iter().flatten().any(|path| {
        let text = path.to_string_lossy();
        let text = if cfg!(windows) {
            text.replace('\\', "/")
        } else {
            text.into_owned()
        };
        ["/opt/homebrew", "/usr/local/Cellar", "/home/linuxbrew/.linuxbrew"]
            .iter()
            .any(|prefix| text.starts_with(prefix))
    })
}

pub(in crate::ida::python) fn uv_ephemeral(path: &Path) -> bool {
    let resolved = realpath::resolve(path).unwrap_or_else(|_| path.into());
    let mut caches = Vec::new();
    if let Some(path) = env_var("UV_CACHE_DIR") {
        caches.push(PathBuf::from(path));
    }
    if let Some(home) = dirs::home_dir() {
        if cfg!(target_os = "macos") {
            caches.extend([home.join("Library/Caches/uv"), home.join(".cache/uv")]);
        } else if cfg!(windows) {
            if let Some(local) = env_var("LOCALAPPDATA") {
                caches.push(Path::new(&local).join("uv/cache"));
            }
        } else {
            if let Some(xdg) = env_var("XDG_CACHE_HOME") {
                caches.push(Path::new(&xdg).join("uv"));
            }
            caches.push(home.join(".cache/uv"));
        }
    }
    caches
        .iter()
        .filter_map(|cache| realpath::resolve(cache).ok())
        .any(|cache| resolved.starts_with(cache))
        || resolved.ancestors().skip(1).any(|parent| {
            matches!(
                parent.file_name().and_then(|name| name.to_str()),
                Some("archive-v0" | "builds-v0")
            )
        })
        || config(path).contains_key("extends-environment")
}

pub(in crate::ida::python) fn shell_venv() -> Option<PathBuf> {
    let current = PathBuf::from(env_var("VIRTUAL_ENV")?);
    // A native executable has no sys.prefix virtualenv to exclude.
    if !uv_ephemeral(&current) {
        return Some(current);
    }
    for entry in std::env::split_paths(&std::env::var_os("PATH")?) {
        if !matches!(entry.file_name().and_then(|name| name.to_str()), Some("bin" | "Scripts")) {
            continue;
        }
        if let Some(root) = entry.parent()
            && root.join("pyvenv.cfg").is_file()
            && !uv_ephemeral(root)
        {
            return Some(root.into());
        }
    }
    None
}
