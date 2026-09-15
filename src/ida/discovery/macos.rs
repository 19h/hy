//! Spotlight locations followed by system and per-user Applications directories.

use std::path::{Path, PathBuf};
use std::time::Duration;

pub(super) async fn installations() -> Vec<PathBuf> {
    let mut paths = spotlight().await;
    paths.extend(super::in_directory(Path::new("/Applications")));
    if let Some(home) = dirs::home_dir() {
        paths.extend(super::in_directory(&home.join("Applications")));
    }
    paths
}

async fn spotlight() -> Vec<PathBuf> {
    let mut command = tokio::process::Command::new("mdfind");
    command.arg("kMDItemCFBundleIdentifier == 'com.hexrays.ida'").kill_on_drop(true);
    let Ok(Ok(output)) = tokio::time::timeout(Duration::from_secs(10), command.output()).await
    else {
        return Vec::new();
    };
    if !output.status.success() {
        return Vec::new();
    }
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(PathBuf::from)
        .filter(|path| super::valid_installation(path))
        .collect()
}
