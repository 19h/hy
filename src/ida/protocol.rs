//! Platform-specific ida:// protocol registration and launcher construction.

#[cfg(unix)]
use std::path::PathBuf;
#[cfg(unix)]
use std::process::Command;

use crate::error::{Error, Result};

#[cfg(any(target_os = "linux", test))]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(windows)]
mod windows;

#[cfg(any(unix, test))]
const PYTHON_ENVIRONMENT_RESET: &str =
    "env -u PYTHONHOME -u PYTHONPATH -u PYTHONEXECUTABLE -u PYTHONSTARTUP";

#[cfg(unix)]
fn home() -> Result<PathBuf> {
    dirs::home_dir().ok_or_else(|| Error::Other("home directory is unknown".into()))
}

#[cfg(unix)]
fn checked(command: &mut Command) -> Result<()> {
    let output = command.output()?;
    if !output.status.success() {
        return Err(Error::Other(format!(
            "{} failed ({}): {}",
            command.get_program().to_string_lossy(),
            output.status,
            String::from_utf8_lossy(&output.stderr).trim(),
        )));
    }
    Ok(())
}

pub fn register_protocol_handler(binary_path: &str) -> Result<()> {
    if binary_path.chars().any(char::is_control) {
        return Err(Error::Other("protocol launcher path contains a control character".into()));
    }
    #[cfg(target_os = "macos")]
    return macos::register(binary_path);
    #[cfg(target_os = "linux")]
    return linux::register(binary_path);
    #[cfg(windows)]
    return windows::register(binary_path);
    #[cfg(not(any(target_os = "macos", target_os = "linux", windows)))]
    Err(Error::Other("protocol registration is unsupported on this platform".into()))
}

pub fn unregister_protocol_handler() -> Result<()> {
    #[cfg(target_os = "macos")]
    return macos::unregister();
    #[cfg(target_os = "linux")]
    return linux::unregister();
    #[cfg(windows)]
    return windows::unregister();
    #[cfg(not(any(target_os = "macos", target_os = "linux", windows)))]
    Err(Error::Other("protocol registration is unsupported on this platform".into()))
}
