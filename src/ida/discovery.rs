//! Platform discovery in upstream source order, deduplicated by resolved path.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

#[cfg(any(target_os = "linux", test))]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(windows)]
pub(super) mod windows;

pub async fn find_standard_installations() -> Vec<PathBuf> {
    let mut paths: Vec<_> = super::current_install_dir().into_iter().collect();
    #[cfg(target_os = "macos")]
    paths.extend(macos::installations().await);
    #[cfg(target_os = "linux")]
    paths.extend(linux::installations());
    #[cfg(windows)]
    paths.extend(windows::installations());
    deduplicate(paths)
}

fn deduplicate(paths: Vec<PathBuf>) -> Vec<PathBuf> {
    let mut seen = HashSet::new();
    paths
        .into_iter()
        .filter(|path| seen.insert(path.canonicalize().unwrap_or_else(|_| path.clone())))
        .collect()
}

fn has_installer_name(path: &Path) -> bool {
    static PATTERN: LazyLock<regex::Regex> = LazyLock::new(|| {
        regex::Regex::new(concat!(
            r"^(?:IDA[- ](?:Professional|Classroom|Essential|Free|",
            r"Home[- ]\((?:ARM|MIPS|PC|PPC|RISC-V)\))[- ]|",
            r"ida-(?:pro|classroom|essential|free|home-(?:arm|mips|pc|ppc|riscv))-)",
            r"[0-9]+\.[0-9]+(?:sp[0-9]+)?(?:\.app)?$"
        ))
        .expect("static installation name pattern")
    });
    path.file_name().and_then(|name| name.to_str()).is_some_and(|name| PATTERN.is_match(name))
}

fn valid_installation(path: &Path) -> bool {
    path.is_dir() && has_installer_name(path) && super::ida_binary_path(path).is_some()
}

fn in_directory(directory: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(directory) else {
        return Vec::new();
    };
    entries.flatten().map(|entry| entry.path()).filter(|path| valid_installation(path)).collect()
}

/// Derive the registration name using the installer's display name when available.
pub fn generate_instance_name(path: &Path) -> String {
    #[cfg(windows)]
    if let Some(installation) = windows::for_directory(path) {
        return shorten_name(&installation.display_name);
    }
    shorten_name(&path.file_name().unwrap_or_default().to_string_lossy())
}

fn shorten_name(name: &str) -> String {
    name.strip_suffix(".app")
        .unwrap_or(name)
        .to_lowercase()
        .replace(' ', "-")
        .replace("ida-professional", "ida-pro")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn installation_names_cover_editions_and_service_packs() {
        for name in [
            "IDA Professional 9.10.app",
            "IDA Home (RISC-V) 9.2sp1.app",
            "ida-home-mips-9.4",
            "IDA-Home-(PC)-9.2",
            "IDA Classroom 9.4",
        ] {
            assert!(has_installer_name(Path::new(name)), "{name}");
        }
        for name in ["IDA.app", "idapro", "IDA Pro 9.4", "ida-pro-9.4-beta", "ida-foo-9.4"] {
            assert!(!has_installer_name(Path::new(name)), "{name}");
        }
    }

    #[test]
    fn resolved_duplicates_preserve_the_first_spelling_and_order() {
        let directory = tempfile::tempdir().unwrap();
        let first = directory.path().join("first");
        let second = directory.path().join("second");
        std::fs::create_dir(&first).unwrap();
        std::fs::create_dir(&second).unwrap();
        assert_eq!(
            deduplicate(vec![second.clone(), first.join("."), first.clone(), second.clone()]),
            vec![second, first.join(".")]
        );
    }
}
