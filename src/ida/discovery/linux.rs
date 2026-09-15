//! Installer desktop entries followed by the standard installation directories.

use std::path::{Path, PathBuf};

#[cfg(target_os = "linux")]
pub(super) fn installations() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Some(data_home) = std::env::var_os("XDG_DATA_HOME").filter(|value| !value.is_empty()) {
        paths.extend(desktop_entries(&PathBuf::from(data_home).join("applications")));
    }
    let home = dirs::home_dir();
    if let Some(home) = &home {
        paths.extend(desktop_entries(&home.join(".local/share/applications")));
    }
    paths.extend(desktop_entries(Path::new("/usr/share/applications")));
    let mut directories = Vec::new();
    directories.extend(home.iter().cloned());
    directories.extend([PathBuf::from("/opt"), PathBuf::from("/usr/local")]);
    if let Some(home) = home {
        directories.push(home.join(".local/share/applications"));
    }
    for directory in directories {
        paths.extend(super::in_directory(&directory));
    }
    paths
}

fn desktop_entries(directory: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(directory) else {
        return Vec::new();
    };
    entries
        .flatten()
        .filter(|entry| {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            name.starts_with("com.hex_rays.IDA.") && name.ends_with(".desktop")
        })
        .filter_map(|entry| std::fs::read(entry.path()).ok())
        .filter_map(|bytes| desktop_installation(&String::from_utf8_lossy(&bytes)))
        .collect()
}

fn desktop_installation(contents: &str) -> Option<PathBuf> {
    let command = contents.lines().find_map(|line| line.strip_prefix("Exec="))?;
    // Match upstream's first-token rule, including its limitation with quoted spaces.
    let executable = command.split_whitespace().next()?;
    let directory = Path::new(executable).parent()?;
    super::super::ida_binary_path(directory).map(|_| directory.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn desktop_entries_use_the_first_exec_field_and_allow_custom_directory_names() {
        let root = tempfile::tempdir().unwrap();
        let installation = root.path().join("custom");
        std::fs::create_dir(&installation).unwrap();
        let binary = installation.join(if cfg!(windows) {
            "ida.exe"
        } else {
            "ida"
        });
        std::fs::write(&binary, b"fixture").unwrap();
        let command = format!("[Desktop Entry]\nExec={} %f\n", binary.display());
        std::fs::write(root.path().join("com.hex_rays.IDA.fixture.desktop"), &command).unwrap();
        std::fs::write(root.path().join("unrelated.desktop"), &command).unwrap();
        assert_eq!(desktop_entries(root.path()), vec![installation]);
        assert!(desktop_installation(&format!("Exec=\n{command}")).is_none());
    }
}
