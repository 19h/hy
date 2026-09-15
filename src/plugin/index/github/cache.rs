//! Atomic, content-keyed catalogue caches; metadata expires after one day.

use std::io::Write;
use std::path::PathBuf;
use std::time::Duration;

use sha2::{Digest, Sha256};

use crate::error::Result;

fn path(key: &str) -> PathBuf {
    crate::util::cache::default_cache_dir()
        .join("github-catalogue")
        .join(format!("{:x}", Sha256::digest(key.as_bytes())))
}

pub fn read(key: &str, lifetime: Option<Duration>) -> Option<Vec<u8>> {
    let path = path(key);
    if let Some(lifetime) = lifetime {
        let age = std::fs::metadata(&path).ok()?.modified().ok()?.elapsed().ok()?;
        if age > lifetime {
            return None;
        }
    }
    std::fs::read(path).ok()
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
