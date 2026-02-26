//! Filesystem, platform, and process helpers.

use std::path::{Path, PathBuf};

use crate::error::{Error, Result};

/// Check that `required` bytes of free space are available at `path`.
pub fn check_free_space(path: &Path, required: u64) -> Result<()> {
    let check = if path.exists() {
        path.to_path_buf()
    } else {
        // Walk up to the first existing ancestor.
        let mut p = path.to_path_buf();
        while !p.exists() {
            if !p.pop() {
                return Ok(()); // Can't check — skip.
            }
        }
        p
    };

    match fs2::available_space(&check) {
        Ok(available) if available < required => Err(Error::NoSpace {
            path: path.to_path_buf(),
            required: Some(required),
            available: Some(available),
        }),
        _ => Ok(()),
    }
}

/// Open a URL in the default browser.
pub fn open_url(url: &str) {
    let _ = open::that(url);
}

/// Normalised OS identifier: `"windows"`, `"linux"`, `"mac"`.
pub fn os_name() -> &'static str {
    if cfg!(target_os = "macos") {
        "mac"
    } else if cfg!(target_os = "windows") {
        "windows"
    } else {
        "linux"
    }
}

/// Normalised architecture: `"x86_64"` or `"arm64"`.
pub fn arch_name() -> &'static str {
    if cfg!(target_arch = "aarch64") {
        "arm64"
    } else {
        "x86_64"
    }
}

/// Tag-style OS string used by asset tags, e.g. `"armmac"` or `"x64linux"`.
pub fn tag_os() -> String {
    let arch_prefix = if cfg!(target_arch = "aarch64") {
        "arm"
    } else {
        "x64"
    };
    let os_suffix = if cfg!(target_os = "macos") {
        "mac"
    } else if cfg!(target_os = "windows") {
        "win"
    } else {
        "linux"
    };
    format!("{arch_prefix}{os_suffix}")
}

/// The path to this executable.
pub fn executable_path() -> PathBuf {
    std::env::current_exe().unwrap_or_else(|_| PathBuf::from("hcli"))
}

/// True if this process is a frozen / compiled binary (as opposed to
/// `cargo run`).
pub fn is_binary() -> bool {
    // In the Python version this checks `sys.frozen`.  For Rust we treat
    // release-profile builds without a `CARGO` env var as "binary".
    std::env::var("CARGO").is_err() && cfg!(not(debug_assertions))
}

/// Read a text file, handling BOM and encoding gracefully.
pub fn read_text_file(path: &Path) -> Result<String> {
    let bytes = std::fs::read(path)?;

    // UTF-16 LE BOM
    if bytes.starts_with(&[0xFF, 0xFE]) {
        let (cow, _enc, _had_errors) = encoding_rs::UTF_16LE.decode(&bytes[2..]);
        return Ok(cow.into_owned());
    }

    // Try UTF-8, then fall back to Latin-1.
    match std::str::from_utf8(&bytes) {
        Ok(s) => Ok(s.to_owned()),
        Err(_) => {
            let (cow, _enc, _had_errors) = encoding_rs::WINDOWS_1252.decode(&bytes);
            Ok(cow.into_owned())
        }
    }
}
