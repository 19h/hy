//! Standalone shared-file downloads and deletion, including their report order.

use std::path::Path;

use super::share::{DeleteArgs, GetArgs, expanded_path};
use super::share_display::size;
use crate::api::{ApiClient, AssetInteger, asset_path};
use crate::error::{Error, Result};

mod confirmation;

pub(super) async fn get(args: GetArgs) -> Result<()> {
    if args.output_dir.is_some() && args.output_file.is_some() {
        println!("Error: --output-dir and --output-file cannot be used together");
        eprintln!("Aborted!");
        return Err(Error::ChildExit(1));
    }
    download(args).await.map_err(|error| abort("Error downloading file", error))
}

async fn download(args: GetArgs) -> Result<()> {
    let client = ApiClient::new()?;
    let Some(asset) = client.shared_asset(&args.shortcode, &AssetInteger::from(-1)).await? else {
        println!("Error: File with shortcode '{}' not found", args.shortcode);
        return Ok(());
    };
    let (directory, filename) = if let Some(output) = args.output_file {
        let output = crate::util::realpath::resolve(&expanded_path(&output)?)?;
        let filename = output
            .file_name()
            .ok_or_else(|| Error::Other("missing output filename".into()))?
            .to_string_lossy()
            .into_owned();
        (output.parent().unwrap_or(Path::new(".")).to_path_buf(), filename)
    } else {
        (
            crate::util::realpath::resolve(&expanded_path(
                args.output_dir.as_deref().unwrap_or(Path::new(".")),
            )?)?,
            asset.filename.clone(),
        )
    };
    let target = directory.join(&filename);
    if target.exists() && !args.force {
        println!("Warning: File already exists: {}", target.display());
        if !confirmation::overwrite()? {
            println!("Download cancelled");
            return Ok(());
        }
    }
    let Some(url) = asset.url.as_deref().filter(|url| !url.is_empty()) else {
        println!("Error: No download URL available for file");
        return Ok(());
    };
    let path =
        client.download_file(url, &directory, Some(&filename), args.force, true, None).await?;
    println!("✓ File downloaded successfully!");
    println!("File: {}", asset.filename);
    // Upstream formats after publishing the download: a reporting error leaves it intact.
    println!("Size: {}", size(&asset)?);
    println!("Saved to: {}", path.display());
    Ok(())
}

pub(super) async fn delete(args: DeleteArgs) -> Result<()> {
    remove(args).await.map_err(|error| abort("Error during deletion", error))
}

async fn remove(args: DeleteArgs) -> Result<()> {
    let client = ApiClient::new()?;
    let Some(asset) = client.shared_asset(&args.code, &AssetInteger::from(-1)).await? else {
        println!("File not found {}.", args.code);
        return Ok(());
    };
    println!("\nFile to delete:");
    println!("  Name: {}", asset.filename);
    println!("  Code: {}", asset.code.as_deref().unwrap_or("None"));
    // This report precedes both confirmation and a forced DELETE request.
    println!("  Size: {}", size(&asset)?);
    if !args.force && !confirmation::delete(&asset)? {
        println!("Deletion cancelled.");
        return Ok(());
    }
    let _: serde_json::Value = client.delete_json(&asset_path("shared", &asset.key)).await?;
    println!("✓ Deleted: {}", args.code);
    Ok(())
}

fn abort(context: &str, error: Error) -> Error {
    println!("{context}: {error}");
    eprintln!("Aborted!");
    Error::ChildExit(1)
}
