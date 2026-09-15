//! Atomic, content-keyed catalogue caches; metadata expires after one day.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use sha2::{Digest, Sha256};

use crate::error::Result;
use crate::util::python_path;

fn path(key: &str) -> PathBuf {
    crate::util::cache::default_cache_dir()
        .join("github-catalogue")
        .join(format!("{:x}", Sha256::digest(key.as_bytes())))
}

pub fn read(key: &str, lifetime: Option<Duration>) -> Result<Option<Vec<u8>>> {
    read_path(&path(key), lifetime, SystemTime::now)
}

fn read_path(
    path: &Path,
    lifetime: Option<Duration>,
    clock: impl FnOnce() -> SystemTime,
) -> Result<Option<Vec<u8>>> {
    if !python_path::exists(path)? {
        return Ok(None);
    }
    if let Some(lifetime) = lifetime {
        let now = clock();
        let modified = std::fs::metadata(path)?.modified()?;
        // Python subtracts two binary64 epoch timestamps, including future dates.
        let age = epoch_seconds(now) - epoch_seconds(modified);
        if age > lifetime.as_secs_f64() {
            std::fs::remove_file(path)?;
            return Ok(None);
        }
    }
    Ok(Some(std::fs::read(path)?))
}

fn epoch_seconds(time: SystemTime) -> f64 {
    match time.duration_since(UNIX_EPOCH) {
        Ok(duration) => duration.as_secs_f64(),
        Err(error) => -error.duration().as_secs_f64(),
    }
}

pub fn write(key: &str, bytes: &[u8]) -> Result<()> {
    let path = path(key);
    let parent = path.parent().expect("cache file has a parent");
    std::fs::create_dir_all(parent)?;
    let mut file = tempfile::NamedTempFile::new_in(parent)?;
    file.write_all(bytes)?;
    file.persist(path).map_err(|error| error.error)?;
    Ok(())
}

#[cfg(test)]
mod tests;
