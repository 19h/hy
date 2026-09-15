//! IDA directory and binary path resolution.

use std::path::{Path, PathBuf};

use crate::config::Env;

/// Resolve the IDA user directory (`$IDAUSR` equivalent).
///
/// Priority: `$HCLI_IDAUSR` → `$IDAUSR` → platform default.
pub fn ida_user_dir() -> PathBuf {
    let env = Env::global();

    if let Some(ref d) = env.hcli_idausr {
        return std::env::split_paths(d).next().unwrap_or_default();
    }
    if let Some(ref d) = env.idausr {
        // IDAUSR can be a search path; take the first component.
        let sep = if cfg!(windows) {
            ';'
        } else {
            ':'
        };
        return PathBuf::from(d.split(sep).next().unwrap_or(d));
    }

    // Platform default
    if cfg!(target_os = "macos") {
        dirs::home_dir().unwrap_or_default().join(".idapro")
    } else if cfg!(target_os = "windows") {
        dirs::home_dir().unwrap_or_default().join("AppData").join("Hex-Rays").join("IDA Pro")
    } else {
        dirs::home_dir().unwrap_or_default().join(".idapro")
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

/// Resolve the executable inside a macOS app bundle.
fn macos_bundle_binary(bundle: &Path) -> Option<PathBuf> {
    let macos_dir = bundle.join("Contents").join("MacOS");
    for name in &["ida", "ida64", "ida32"] {
        let p = macos_dir.join(name);
        if p.is_file() {
            return Some(p);
        }
    }
    None
}

/// Locate the `idat` (headless / text-mode) binary.
#[allow(dead_code)]
pub fn idat_path(install_dir: &Path) -> Option<PathBuf> {
    let install_dir = executable_dir(install_dir);
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

/// Installation path together with the source that selected it.
#[derive(Debug)]
pub struct ResolvedInstallDir {
    pub path: PathBuf,
    pub source: String,
}

pub fn resolve_install_dir() -> crate::error::Result<ResolvedInstallDir> {
    let environment = Env::global();
    for (value, source) in [
        (&environment.current_ida_install_dir, "$HCLI_CURRENT_IDA_INSTALL_DIR"),
        (&environment.idadir, "$IDADIR"),
    ] {
        if let Some(path) = value {
            return Ok(ResolvedInstallDir {
                path: normalize_install_dir(Path::new(path)),
                source: source.into(),
            });
        }
    }
    {
        let store = crate::config::ConfigStore::global();
        if let Some(name) = store.get_nested_str("ida.default")
            && let Some(path) = store.get_string_map("ida.instances").get(name)
        {
            let path = normalize_install_dir(Path::new(path));
            if ida_binary_path(&path).is_some() {
                return Ok(ResolvedInstallDir {
                    path,
                    source: format!("hcli default instance '{name}'"),
                });
            }
        }
    }
    let config = crate::plugin::read_ida_config()?;
    let configured = config["Paths"]["ida-install-dir"]
        .as_str()
        .or_else(|| config["paths"]["ida_install_dir"].as_str())
        .ok_or(crate::error::Error::IdaNotFound)?;
    let path = normalize_install_dir(Path::new(configured));
    if !path.is_dir() {
        return Err(crate::error::Error::Other(
            "ida-config.json: ida-install-dir does not exist".into(),
        ));
    }
    Ok(ResolvedInstallDir {
        path,
        source: ida_user_dir().join("ida-config.json").display().to_string(),
    })
}

/// Convenience lookup for callers that treat an unconfigured installation as absent.
pub fn current_install_dir() -> Option<PathBuf> {
    resolve_install_dir().ok().map(|resolved| resolved.path)
}

pub fn normalize_install_dir(path: &Path) -> PathBuf {
    let mut path = path.to_path_buf();
    if path.is_file() {
        path.pop();
    }
    if path.ends_with("Contents/MacOS") {
        path.pop();
        path.pop();
    }
    path
}

pub fn executable_dir(path: &Path) -> PathBuf {
    let normalized = normalize_install_dir(path);
    let bundle = normalized.join("Contents/MacOS");
    if bundle.is_dir() {
        bundle
    } else {
        normalized
    }
}
