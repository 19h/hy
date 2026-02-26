//! IDA directory and binary path resolution.

use std::path::{Path, PathBuf};

use crate::config::Env;

/// Resolve the IDA user directory (`$IDAUSR` equivalent).
///
/// Priority: `$HCLI_IDAUSR` → `$IDAUSR` → platform default.
pub fn ida_user_dir() -> PathBuf {
    let env = Env::global();

    if let Some(ref d) = env.hcli_idausr {
        return PathBuf::from(d);
    }
    if let Some(ref d) = env.idausr {
        // IDAUSR can be a search path; take the first component.
        let sep = if cfg!(windows) { ';' } else { ':' };
        return PathBuf::from(d.split(sep).next().unwrap_or(d));
    }

    // Platform default
    if cfg!(target_os = "macos") {
        dirs::home_dir().unwrap_or_default().join(".idapro")
    } else if cfg!(target_os = "windows") {
        dirs::home_dir()
            .unwrap_or_default()
            .join("AppData")
            .join("Hex-Rays")
            .join("IDA Pro")
    } else {
        dirs::home_dir().unwrap_or_default().join(".idapro")
    }
}

/// Default IDA installation directory per platform.
pub fn default_install_dir() -> PathBuf {
    if cfg!(target_os = "macos") {
        PathBuf::from("/Applications")
    } else if cfg!(target_os = "windows") {
        PathBuf::from(r"C:\Program Files\IDA Pro")
    } else {
        dirs::home_dir().unwrap_or_default().join("ida")
    }
}

/// Locate IDA binary path inside an installation directory.
pub fn ida_binary_path(install_dir: &Path) -> Option<PathBuf> {
    if cfg!(target_os = "macos") {
        // Look for IDA*.app bundles.
        let candidates = ["ida64.app", "ida.app", "IDA Professional.app"];
        for name in &candidates {
            let p = install_dir.join(name);
            if p.exists() {
                return Some(p);
            }
        }
        None
    } else if cfg!(target_os = "windows") {
        let p = install_dir.join("ida64.exe");
        if p.exists() {
            Some(p)
        } else {
            let p = install_dir.join("ida.exe");
            p.exists().then_some(p)
        }
    } else {
        let p = install_dir.join("ida64");
        if p.exists() {
            Some(p)
        } else {
            let p = install_dir.join("ida");
            p.exists().then_some(p)
        }
    }
}

/// Locate the `idat` (headless / text-mode) binary.
#[allow(dead_code)]
pub fn idat_path(install_dir: &Path) -> Option<PathBuf> {
    let name = if cfg!(target_os = "windows") {
        "idat64.exe"
    } else {
        "idat64"
    };
    let p = install_dir.join(name);
    if p.exists() {
        Some(p)
    } else {
        let fallback = if cfg!(target_os = "windows") {
            "idat.exe"
        } else {
            "idat"
        };
        let p = install_dir.join(fallback);
        p.exists().then_some(p)
    }
}

/// Find IDA installations in standard locations for the current platform.
pub fn find_standard_installations() -> Vec<PathBuf> {
    let mut results = Vec::new();

    if cfg!(target_os = "macos") {
        // /Applications/IDA*
        if let Ok(entries) = std::fs::read_dir("/Applications") {
            for entry in entries.flatten() {
                let name = entry.file_name();
                let name_str = name.to_string_lossy();
                if name_str.starts_with("IDA")
                    && name_str.ends_with(".app")
                    && entry.path().is_dir()
                {
                    results.push(entry.path());
                }
            }
        }
    } else if cfg!(target_os = "windows") {
        for base in &[r"C:\Program Files", r"C:\Program Files (x86)"] {
            if let Ok(entries) = std::fs::read_dir(base) {
                for entry in entries.flatten() {
                    let name = entry.file_name();
                    if name.to_string_lossy().contains("IDA") && entry.path().is_dir() {
                        results.push(entry.path());
                    }
                }
            }
        }
    } else {
        // Linux: ~/ida*, /opt/ida*
        if let Some(home) = dirs::home_dir() {
            if let Ok(entries) = std::fs::read_dir(&home) {
                for entry in entries.flatten() {
                    let name = entry.file_name();
                    if name.to_string_lossy().starts_with("ida") && entry.path().is_dir() {
                        results.push(entry.path());
                    }
                }
            }
        }
        if let Ok(entries) = std::fs::read_dir("/opt") {
            for entry in entries.flatten() {
                let name = entry.file_name();
                if name.to_string_lossy().starts_with("ida") && entry.path().is_dir() {
                    results.push(entry.path());
                }
            }
        }
    }

    results.sort();
    results
}

/// Determine the current IDA installation directory from environment or config.
pub fn current_install_dir() -> Option<PathBuf> {
    let env = Env::global();
    if let Some(ref d) = env.current_ida_install_dir {
        return Some(PathBuf::from(d));
    }
    if let Some(ref d) = env.idadir {
        return Some(PathBuf::from(d));
    }

    // Try to read from ida-config.json
    let config_path = ida_user_dir().join("ida-config.json");
    if config_path.exists() {
        if let Ok(text) = std::fs::read_to_string(&config_path) {
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&text) {
                if let Some(dir) = val
                    .get("paths")
                    .and_then(|p| p.get("ida_install_dir"))
                    .and_then(|v| v.as_str())
                {
                    let path = PathBuf::from(dir);
                    if path.exists() {
                        return Some(path);
                    }
                }
            }
        }
    }

    None
}
