//! Installer metadata from the Windows uninstall registry and Program Files.

use std::path::{Path, PathBuf};

use winreg::RegKey;
use winreg::enums::{HKEY_LOCAL_MACHINE, KEY_READ};

pub(crate) struct Installation {
    pub path: PathBuf,
    pub display_name: String,
    pub display_version: Option<String>,
}

pub(super) fn installations() -> Vec<PathBuf> {
    let mut paths: Vec<_> = registry().into_iter().map(|installation| installation.path).collect();
    let program_files =
        std::env::var_os("ProgramFiles").unwrap_or_else(|| r"C:\Program Files".into());
    paths.extend(super::in_directory(Path::new(&program_files)));
    paths
}

pub(crate) fn for_directory(directory: &Path) -> Option<Installation> {
    let resolved = directory.canonicalize().unwrap_or_else(|_| directory.to_owned());
    registry().into_iter().find(|installation| {
        installation.path.canonicalize().unwrap_or_else(|_| installation.path.clone()) == resolved
    })
}

fn registry() -> Vec<Installation> {
    let machine = RegKey::predef(HKEY_LOCAL_MACHINE);
    let Ok(root) = machine
        .open_subkey_with_flags(r"Software\Microsoft\Windows\CurrentVersion\Uninstall", KEY_READ)
    else {
        return Vec::new();
    };
    root.enum_keys()
        .map_while(Result::ok)
        .filter_map(|name| root.open_subkey_with_flags(name, KEY_READ).ok())
        .filter_map(|key| read_installation(&key))
        .collect()
}

fn read_installation(key: &RegKey) -> Option<Installation> {
    let display_name: String = key.get_value("DisplayName").ok()?;
    if !super::has_installer_name(Path::new(&display_name)) {
        return None;
    }
    let location: String = key.get_value("InstallLocation").ok()?;
    if location.is_empty() {
        return None;
    }
    let path = PathBuf::from(location);
    super::super::ida_binary_path(&path)?;
    Some(Installation {
        path,
        display_name,
        display_version: key.get_value("DisplayVersion").ok(),
    })
}
