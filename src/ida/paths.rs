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
///
/// On macOS this resolves `.app` bundles to the executable inside
/// `Contents/MacOS`, and also supports non-bundle installations where the
/// `ida` binary sits directly in the directory.
pub fn ida_binary_path(install_dir: &Path) -> Option<PathBuf> {
    if cfg!(target_os = "macos") {
        // The install dir may itself be a bundle (`.../IDA Professional.app`).
        if install_dir.extension().is_some_and(|e| e == "app") {
            return macos_bundle_binary(install_dir);
        }

        // Look for IDA*.app bundles inside the directory.
        let candidates = ["ida64.app", "ida.app", "IDA Professional.app"];
        for name in &candidates {
            let p = install_dir.join(name);
            if p.exists() {
                return macos_bundle_binary(&p);
            }
        }

        // Non-bundle layout: plain binaries, or a raw Contents/MacOS tree
        // without the .app suffix.
        for name in &["ida64", "ida"] {
            let p = install_dir.join(name);
            if p.is_file() {
                return Some(p);
            }
            let p = install_dir.join("Contents").join("MacOS").join(name);
            if p.is_file() {
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

/// Resolve the executable inside a macOS app bundle. Falls back to the
/// bundle path itself if no binary is found (so callers can still report
/// a sensible path).
fn macos_bundle_binary(bundle: &Path) -> Option<PathBuf> {
    let macos_dir = bundle.join("Contents").join("MacOS");
    for name in &["ida", "ida64", "ida32"] {
        let p = macos_dir.join(name);
        if p.is_file() {
            return Some(p);
        }
    }
    // Any executable in Contents/MacOS as a last resort.
    if let Ok(entries) = std::fs::read_dir(&macos_dir) {
        for entry in entries.flatten() {
            if entry.path().is_file() {
                return Some(entry.path());
            }
        }
    }
    bundle.exists().then(|| bundle.to_path_buf())
}

/// Launch IDA with an optional file argument.
///
/// On macOS, app bundles are launched through LaunchServices (`open -n -a`)
/// which escapes sandbox restrictions inherited from protocol handlers —
/// but only when the bundle path actually ends in `.app`. Non-bundle
/// installations invoke the binary directly.
pub fn launch_ida(ida_bin: &Path, file: Option<&Path>) -> std::io::Result<()> {
    let bin_str = ida_bin.to_string_lossy();

    if cfg!(target_os = "macos") {
        let app_bundle = if let Some(pos) = bin_str.find("/Contents/MacOS/") {
            Some(&bin_str[..pos])
        } else if bin_str.ends_with(".app") {
            Some(bin_str.as_ref())
        } else {
            None
        };

        if let Some(bundle) = app_bundle
            && bundle.ends_with(".app") {
                let mut cmd = std::process::Command::new("open");
                cmd.arg("-n").arg("-a").arg(bundle);
                if let Some(f) = file {
                    cmd.arg("--args").arg(f);
                }
                cmd.spawn()?;
                return Ok(());
            }
    }

    let mut cmd = std::process::Command::new(ida_bin);
    if let Some(f) = file {
        cmd.arg(f);
    }
    cmd.spawn()?;
    Ok(())
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
        if let Some(home) = dirs::home_dir()
            && let Ok(entries) = std::fs::read_dir(&home) {
                for entry in entries.flatten() {
                    let name = entry.file_name();
                    if name.to_string_lossy().starts_with("ida") && entry.path().is_dir() {
                        results.push(entry.path());
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
    if config_path.exists()
        && let Ok(text) = std::fs::read_to_string(&config_path)
            && let Ok(val) = serde_json::from_str::<serde_json::Value>(&text)
                && let Some(dir) = val
                    .get("paths")
                    .and_then(|p| p.get("ida_install_dir"))
                    .and_then(|v| v.as_str())
                {
                    let path = PathBuf::from(dir);
                    if path.exists() {
                        return Some(path);
                    }
                }

    None
}
