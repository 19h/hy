//! Stage and verify an update on the executable's filesystem before publication.

use std::io::{Read, Write};
use std::path::Path;

use super::{GitHubRepo, ReleaseAsset};
use crate::error::{Error, Result};

pub fn update_binary(asset: &ReleaseAsset, repo: &GitHubRepo, binary: &Path) -> Result<()> {
    if !asset.is_valid() {
        return Err(Error::UpdateFailed("invalid release asset".into()));
    }
    let binary = binary.canonicalize()?;
    let parent =
        binary.parent().ok_or_else(|| Error::UpdateFailed("binary has no parent".into()))?;
    let permissions = std::fs::metadata(&binary)?.permissions();
    crate::util::io::check_free_space(parent, asset.size)?;

    // The asset's untrusted name is display metadata, never an output path.
    let mut staged = tempfile::NamedTempFile::new_in(parent)?;
    let client = repo.client()?;
    let response = repo
        .get(&client, &format!("/releases/assets/{}", asset.id))
        .header(reqwest::header::ACCEPT, "application/octet-stream")
        .send()?;
    if response.status() != reqwest::StatusCode::OK {
        return Err(Error::UpdateFailed(format!(
            "unexpected HTTP status {} downloading {}",
            response.status(),
            asset.name
        )));
    }
    let mut limited = response.take(asset.size.saturating_add(1));
    let size = std::io::copy(&mut limited, &mut staged)?;
    if size != asset.size {
        return Err(Error::UpdateFailed(format!(
            "downloaded {} bytes, expected {} bytes",
            size, asset.size
        )));
    }
    staged.flush()?;
    staged.as_file().set_permissions(permissions)?;
    staged.as_file().sync_all()?;
    publish(staged, &binary)
}

#[cfg(not(windows))]
fn publish(staged: tempfile::NamedTempFile, binary: &Path) -> Result<()> {
    staged.persist(binary).map_err(|error| error.error)?;
    Ok(())
}

#[cfg(windows)]
fn publish(staged: tempfile::NamedTempFile, binary: &Path) -> Result<()> {
    let backup = tempfile::Builder::new()
        .prefix(".hy-update-backup-")
        .tempdir_in(binary.parent().unwrap())?;
    let previous = backup.path().join("hy.exe");
    std::fs::rename(binary, &previous)?;
    if let Err(error) = staged.persist(binary) {
        if let Err(rollback) = std::fs::rename(&previous, binary) {
            let retained = backup.keep();
            return Err(Error::UpdateFailed(format!(
                "publication failed: {}; rollback failed: {rollback}; original retained at {}",
                error.error,
                retained.display()
            )));
        }
        return Err(error.error.into());
    }
    // Windows may keep the running executable open after successful replacement.
    if std::fs::remove_file(&previous).is_err() {
        let _retained = backup.keep();
    }
    Ok(())
}
