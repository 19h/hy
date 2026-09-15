//! XDG-compliant cache directory management.

use std::path::PathBuf;

/// Return the root hcli cache directory, respecting `$HCLI_CACHE_DIR` and
/// platform conventions (XDG on Linux, `~/Library/Caches` on macOS,
/// `%LOCALAPPDATA%` on Windows).
pub fn default_cache_dir() -> PathBuf {
    if let Ok(custom) = std::env::var("HCLI_CACHE_DIR")
        && !custom.is_empty()
    {
        return PathBuf::from(custom);
    }
    if cfg!(target_os = "linux")
        && let Ok(directory) = std::env::var("XDG_CACHE_HOME")
    {
        // Upstream uses an explicit XDG_CACHE_HOME directly, without appending
        // its vendor/application directories.
        return PathBuf::from(directory);
    }

    let base = if cfg!(target_os = "macos") {
        dirs::home_dir()
            .map(|h| h.join("Library").join("Caches"))
            .unwrap_or_else(|| PathBuf::from(".cache"))
    } else if cfg!(target_os = "windows") {
        dirs::cache_dir().unwrap_or_else(|| PathBuf::from(".cache"))
    } else {
        // Linux / other: $XDG_CACHE_HOME or ~/.cache
        std::env::var("XDG_CACHE_HOME").ok().map(PathBuf::from).unwrap_or_else(|| {
            dirs::home_dir().unwrap_or_else(|| PathBuf::from(".")).join(".cache")
        })
    };

    let directory = base.join("hex-rays").join("hcli");
    if cfg!(target_os = "windows") {
        directory.join("cache")
    } else {
        directory
    }
}

/// Get (or create) a sub-directory of the cache.
///
/// ```text
/// cache_dir("downloads") → <cache_root>/downloads/
/// ```
pub fn cache_dir(key: &str) -> PathBuf {
    let dir = default_cache_dir().join(key);
    let _ = std::fs::create_dir_all(&dir);
    dir
}
