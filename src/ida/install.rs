//! IDA installation (macOS, Linux, Windows).

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::error::{Error, Result};
use crate::util::io::check_free_space;

/// Install IDA from a downloaded installer file.
///
/// Dispatches to the platform-appropriate installer logic.
pub async fn install_ida(
    installer_path: &Path,
    install_dir: &Path,
    accept_eula: bool,
) -> Result<PathBuf> {
    if !installer_path.exists() {
        return Err(Error::FileNotFound(installer_path.to_path_buf()));
    }

    std::fs::create_dir_all(install_dir)?;

    // Check free space (rough estimate: 2x installer size).
    let installer_size = std::fs::metadata(installer_path)?.len();
    check_free_space(install_dir, installer_size * 2)?;

    let ext = installer_path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");

    match ext {
        "zip" if cfg!(target_os = "macos") => install_ida_mac(installer_path, install_dir).await,
        "run" if cfg!(target_os = "linux") => {
            install_ida_unix(installer_path, install_dir, accept_eula).await
        }
        "exe" if cfg!(target_os = "windows") => {
            install_ida_windows(installer_path, install_dir, accept_eula).await
        }
        _ => Err(Error::IdaInstallFailed(format!(
            "Unsupported installer format: {ext}"
        ))),
    }
}

/// macOS: unzip .app.zip into the target directory.
async fn install_ida_mac(zip_path: &Path, install_dir: &Path) -> Result<PathBuf> {
    let status = Command::new("ditto")
        .args(["-xk", &zip_path.to_string_lossy(), &install_dir.to_string_lossy()])
        .status()?;

    if !status.success() {
        return Err(Error::IdaInstallFailed("ditto extraction failed".into()));
    }

    // Find the extracted .app bundle.
    for entry in std::fs::read_dir(install_dir)?.flatten() {
        let name = entry.file_name();
        if name.to_string_lossy().ends_with(".app") {
            return Ok(entry.path());
        }
    }

    Err(Error::IdaInstallFailed(
        "No .app bundle found after extraction".into(),
    ))
}

/// Linux: run the .run installer with --unattendedmodeui none.
async fn install_ida_unix(
    run_path: &Path,
    install_dir: &Path,
    accept_eula: bool,
) -> Result<PathBuf> {
    // Make executable.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(run_path)?.permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(run_path, perms)?;
    }

    let mut cmd = Command::new(run_path.as_os_str());
    cmd.arg("--unattendedmodeui")
        .arg("none")
        .arg("--mode")
        .arg("unattended")
        .arg("--prefix")
        .arg(install_dir.as_os_str());

    if accept_eula {
        cmd.arg("--installpassword").arg("");
    }

    let status = cmd.status()?;
    if !status.success() {
        return Err(Error::IdaInstallFailed(format!(
            "Installer exited with code {}",
            status.code().unwrap_or(-1)
        )));
    }

    Ok(install_dir.to_path_buf())
}

/// Windows: run the .exe installer silently.
async fn install_ida_windows(
    exe_path: &Path,
    install_dir: &Path,
    accept_eula: bool,
) -> Result<PathBuf> {
    let mut cmd = Command::new(exe_path.as_os_str());
    cmd.arg("--unattendedmodeui")
        .arg("none")
        .arg("--mode")
        .arg("unattended")
        .arg("--prefix")
        .arg(install_dir.as_os_str());

    if accept_eula {
        cmd.arg("--installpassword").arg("");
    }

    let status = cmd.status()?;
    if !status.success() {
        return Err(Error::IdaInstallFailed(format!(
            "Installer exited with code {}",
            status.code().unwrap_or(-1)
        )));
    }

    Ok(install_dir.to_path_buf())
}

/// Detect version from an IDA installation directory.
pub fn detect_ida_version(install_dir: &Path) -> Option<String> {
    // Try reading from python/ida_pro.py docstring.
    let sdk_file = install_dir.join("python").join("ida_pro.py");
    if sdk_file.exists()
        && let Ok(content) = std::fs::read_to_string(&sdk_file) {
            // Look for version pattern like "IDA SDK v9.2" in docstring.
            for line in content.lines().take(10) {
                if let Some(rest) = line.strip_prefix("IDA SDK v") {
                    let version = rest.trim().trim_matches('"');
                    if !version.is_empty() {
                        return Some(version.to_owned());
                    }
                }
            }
        }

    // Fallback: extract from directory name.
    install_dir
        .file_name()
        .and_then(|n| n.to_str())
        .and_then(|name| {
            // Extract version-like patterns: "IDA Pro 9.2", "ida-9.2", etc.
            let re = regex::Regex::new(r"(\d+\.\d+(?:\.\d+)?)").ok()?;
            re.find(name).map(|m| m.as_str().to_owned())
        })
}
