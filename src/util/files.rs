//! Local path resolution and metadata-preserving file copies.

use crate::error::{Error, Result};
use std::path::{Path, PathBuf};

/// Expand a leading `~` and make the path absolute without requiring it to exist.
pub fn absolute_path(path: &Path) -> Result<PathBuf> {
    let path = match path.strip_prefix("~") {
        Ok(suffix) => dirs::home_dir()
            .ok_or_else(|| Error::Other("home directory unavailable".into()))?
            .join(suffix),
        Err(_) => path.to_path_buf(),
    };
    Ok(std::path::absolute(path)?)
}

pub fn copy_metadata(source: &Path, destination: &Path) -> Result<()> {
    let metadata = std::fs::metadata(source)?;
    if same_file(source, destination, &metadata)? {
        return Err(Error::Other("source and destination are the same file".into()));
    }
    let mut input = std::fs::File::open(source)?;
    let mut output = std::fs::File::create(destination)?;
    std::io::copy(&mut input, &mut output)?;
    let times = std::fs::FileTimes::new()
        .set_accessed(metadata.accessed()?)
        .set_modified(metadata.modified()?);
    output.set_times(times)?;
    output.set_permissions(metadata.permissions())?;
    Ok(())
}

fn same_file(
    source: &Path,
    destination: &Path,
    source_metadata: &std::fs::Metadata,
) -> Result<bool> {
    let destination_metadata = match std::fs::metadata(destination) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error.into()),
    };
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if source_metadata.dev() == destination_metadata.dev()
            && source_metadata.ino() == destination_metadata.ino()
        {
            return Ok(true);
        }
    }
    #[cfg(not(unix))]
    let _ = (source_metadata, destination_metadata);
    Ok(source.canonicalize()? == destination.canonicalize()?)
}
