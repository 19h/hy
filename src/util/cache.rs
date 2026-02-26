//! XDG-compliant cache directory management.

use std::path::PathBuf;

/// Return the root hcli cache directory, respecting `$HCLI_CACHE_DIR` and
/// platform conventions (XDG on Linux, `~/Library/Caches` on macOS,
/// `%LOCALAPPDATA%` on Windows).
pub fn default_cache_dir() -> PathBuf {
    if let Ok(custom) = std::env::var("HCLI_CACHE_DIR") {
        return PathBuf::from(custom);
    }

    let base = if cfg!(target_os = "macos") {
        dirs::home_dir()
            .map(|h| h.join("Library").join("Caches"))
            .unwrap_or_else(|| PathBuf::from(".cache"))
    } else if cfg!(target_os = "windows") {
        dirs::cache_dir().unwrap_or_else(|| PathBuf::from(".cache"))
    } else {
        // Linux / other: $XDG_CACHE_HOME or ~/.cache
        std::env::var("XDG_CACHE_HOME")
            .ok()
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                dirs::home_dir()
                    .unwrap_or_else(|| PathBuf::from("."))
                    .join(".cache")
            })
    };

    base.join("hex-rays").join("hcli")
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
