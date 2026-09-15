//! Compare resolved cache paths without requiring the downloaded file to exist.

use std::path::Path;

use crate::error::{Error, Result};

use crate::util::realpath::resolve;

#[cfg(all(test, unix))]
use crate::util::realpath as unix;

pub(super) fn validate(root: &Path, destination: &Path) -> Result<()> {
    let root = resolve(root)?;
    let destination = resolve(destination)?;
    if !destination.starts_with(root) {
        return Err(Error::Other("refusing to write outside the downloads directory".into()));
    }
    Ok(())
}

#[cfg(all(test, unix))]
mod tests;
