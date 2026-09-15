//! Pack source trees in pathlib order before entering archive installation.

use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use chrono::{Datelike, Local, Timelike, Utc};
use zip::write::SimpleFileOptions;

use crate::error::{Error, Result};

const EXCLUDED: &[&str] = &[".git", ".hg", ".svn", "__pycache__", ".DS_Store"];

pub(super) fn pack(root: &Path) -> Result<Vec<u8>> {
    let mut paths = collect(root);
    paths.sort_by_cached_key(|path| {
        path.components()
            .map(|part| {
                let part = part.as_os_str().to_string_lossy();
                if cfg!(windows) {
                    part.to_lowercase()
                } else {
                    part.into_owned()
                }
            })
            .collect::<Vec<_>>()
    });
    let mut archive = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for path in paths {
        // The source checks directory type before applying exclusion filters.
        if crate::util::python_path::is_dir(&path)? {
            continue;
        }
        let relative = path.strip_prefix(root).expect("collected path under source root");
        if relative.components().any(|part| EXCLUDED.iter().any(|name| part.as_os_str() == *name)) {
            continue;
        }
        let name = relative.to_str().ok_or_else(|| {
            Error::PluginInstall("source filename cannot be encoded as UTF-8".into())
        })?;
        let name = if cfg!(windows) {
            name.replace('\\', "/")
        } else {
            name.to_owned()
        };
        let metadata = fs::metadata(&path)?;
        let mut options = SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)
            .large_file(metadata.len() as f64 * 1.05 > f64::from(i32::MAX))
            .last_modified_time(timestamp(&metadata)?);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            // The writer keeps rwx bits only; installation creates fresh files
            // and does not restore the source's special permission bits.
            options = options.unix_permissions(metadata.permissions().mode());
        }
        #[cfg(windows)]
        {
            options = options.unix_permissions(if metadata.permissions().readonly() {
                0o444
            } else {
                0o666
            });
        }
        archive.start_file(name, options)?;
        std::io::copy(&mut fs::File::open(&path)?, &mut archive)?;
    }
    Ok(archive.finish()?.into_inner())
}

fn entries(path: &Path) -> Vec<fs::DirEntry> {
    fs::read_dir(path)
        .and_then(|entries| entries.collect::<std::io::Result<Vec<_>>>())
        .unwrap_or_default()
}

/// Path.rglob("*") does not recurse into links and suppresses scandir errors.
fn collect(root: &Path) -> Vec<PathBuf> {
    let mut paths: Vec<_> = entries(root).into_iter().map(|entry| entry.path()).collect();
    let mut pending = vec![root.to_path_buf()];
    while let Some(directory) = pending.pop() {
        for entry in entries(&directory) {
            if entry.file_type().is_ok_and(|kind| kind.is_dir()) {
                let path = entry.path();
                paths.extend(entries(&path).into_iter().map(|entry| entry.path()));
                pending.push(path);
            }
        }
    }
    paths
}

fn timestamp(metadata: &fs::Metadata) -> Result<zip::DateTime> {
    let seconds = match metadata.modified()?.duration_since(UNIX_EPOCH) {
        Ok(duration) => duration.as_secs_f64(),
        Err(error) => -error.duration().as_secs_f64(),
    };
    // ZipInfo.from_file passes the float st_mtime through time.localtime.
    let time = chrono::DateTime::<Utc>::from_timestamp(seconds.floor() as i64, 0)
        .ok_or_else(|| {
            Error::PluginInstall("source timestamp is outside the ZIP date range".into())
        })?
        .with_timezone(&Local);
    let year = u16::try_from(time.year()).map_err(|_| {
        Error::PluginInstall("source timestamp is outside the ZIP date range".into())
    })?;
    zip::DateTime::from_date_and_time(
        year,
        time.month() as u8,
        time.day() as u8,
        time.hour() as u8,
        time.minute() as u8,
        time.second() as u8,
    )
    .map_err(|_| Error::PluginInstall("source timestamp is outside the ZIP date range".into()))
}

#[cfg(all(test, unix))]
mod tests;
