//! Apply selected shared-file actions, continuing after individual failures.

use std::path::{Path, PathBuf};

use crate::api::{ApiClient, Asset, asset_path};
use crate::error::{Error, Result};
use crate::util::{fmt, tui};

pub(super) async fn delete(client: &ApiClient, files: &[&Asset]) {
    eprintln!("\nYou are about to delete {} file(s):", files.len());
    for file in files {
        eprintln!("  * {} ({})", file.filename, file.code.as_deref().unwrap_or("-"));
    }
    if !tui::confirm("Are you sure you want to delete these files?", false) {
        fmt::warning("Deletion cancelled.");
        return;
    }
    for file in files {
        match client.delete_json::<serde_json::Value>(&asset_path("shared", &file.key)).await {
            Ok(_) => fmt::success(&format!(
                "Deleted: {} ({})",
                file.filename,
                file.code.as_deref().unwrap_or("-")
            )),
            Err(error) => fmt::error(&format!("Failed to delete {}: {error}", file.filename)),
        }
    }
}

pub(super) async fn download(client: &ApiClient, files: &[&Asset]) -> Result<()> {
    let output = PathBuf::from(super::prompt::output_directory()?);
    std::fs::create_dir_all(&output)?;
    for file in files {
        fmt::info(&format!("Downloading {}...", file.filename));
        match download_selected(client, file, &output).await {
            Ok(path) => fmt::success(&format!("Downloaded: {}", path.display())),
            Err(error) => fmt::error(&format!("Failed to download {}: {error}", file.filename)),
        }
    }
    fmt::success(&format!("Download completed. Files saved to: {}", output.display()));
    Ok(())
}

async fn download_selected(client: &ApiClient, file: &Asset, output: &Path) -> Result<PathBuf> {
    let asset = client
        .shared_asset(file.code.as_deref().unwrap_or(""), &file.version)
        .await?
        .ok_or_else(|| Error::NotFound("download information unavailable".into()))?;
    let url = asset
        .url
        .filter(|url| !url.is_empty())
        .ok_or_else(|| Error::Other("no download URL available".into()))?;
    client.download_file(&url, output, Some(&file.filename), false, true, None).await
}
