//! `hy asset` command group (hidden): low-level bucket operations.
//!
//! Mirrors the hidden `hcli asset` group: upload an asset to an arbitrary
//! bucket with validated metadata, or delete an asset by key.

use std::path::PathBuf;

use clap::{Args, Subcommand};
use indexmap::IndexMap;
use serde::Deserialize;

use crate::api::{ApiClient, UploadOptions, asset_path};
use crate::error::{Error, Result};
use crate::util::{fmt, tui};

#[derive(Debug, Subcommand)]
pub enum AssetCommands {
    /// Upload an asset to a bucket
    Put(AssetPutArgs),
    /// Delete an asset by key
    Delete(AssetDeleteArgs),
}

#[derive(Debug, Args)]
pub struct AssetPutArgs {
    /// Path to the file to upload
    pub path: PathBuf,
    /// Bucket to upload to
    #[arg(short, long)]
    pub bucket: String,
    /// Attach metadata as KEY=VALUE (repeatable), e.g. -m version=9.2 -m category=ida-free
    #[arg(short, long = "metadata", required = true)]
    pub metadata: Vec<String>,
    /// Comma-separated list of allowed segments (e.g. segment1,segment2)
    #[arg(long)]
    pub allowed_segments: Option<String>,
    /// Comma-separated licence editions, addon codes, or any_edition
    #[arg(long)]
    pub allowed_editions: Option<String>,
    /// Comma-separated list of allowed email addresses
    #[arg(long)]
    pub allowed_emails: Option<String>,
    /// Upload a new version or overwrite the asset if it exists
    #[arg(short, long)]
    pub force: bool,
}

#[derive(Debug, Args)]
pub struct AssetDeleteArgs {
    /// Asset key
    pub key: String,
    /// Bucket to delete from
    #[arg(short, long)]
    pub bucket: String,
    /// Skip confirmation prompt
    #[arg(short = 'y', long)]
    pub yes: bool,
}

/// Bucket configuration with required-metadata schema.
#[derive(Debug, Deserialize)]
struct Bucket {
    #[serde(rename = "filename")]
    _filename: String,
    #[serde(rename = "metadata")]
    _metadata: BucketMetadata,
    #[serde(rename = "requiredMetadata")]
    required_metadata: IndexMap<String, RequiredField>,
}

#[derive(Debug, Deserialize)]
struct BucketMetadata {
    #[serde(rename = "name")]
    _name: String,
}

#[derive(Debug, Deserialize)]
struct RequiredField {
    description: String,
    example: String,
}

pub async fn run(cmd: AssetCommands) -> Result<()> {
    match cmd {
        AssetCommands::Put(args) => run_put(args).await,
        AssetCommands::Delete(args) => run_delete(args).await,
    }
}

async fn run_put(args: AssetPutArgs) -> Result<()> {
    if !args.path.is_file() {
        return Err(Error::Other(format!("not a file: {}", args.path.display())));
    }
    let client = ApiClient::new()?;
    let bucket: Bucket = client.get_json(&format!("/api/assets/buckets/{}", args.bucket)).await?;
    let metadata = parse_metadata(&args.metadata, &bucket)?;
    let options = UploadOptions {
        force: args.force,
        metadata: Some(metadata),
        allowed_segments: permission_list(args.allowed_segments),
        allowed_emails: permission_list(args.allowed_emails),
        allowed_editions: permission_list(args.allowed_editions),
        ..Default::default()
    };
    let uploaded = client.upload_asset(&args.bucket, &args.path, options).await?;
    fmt::success("File uploaded successfully!");
    eprintln!("  Bucket:  {}", args.bucket);
    eprintln!("  Key:     {}", uploaded.key);
    eprintln!("  Version: {}", uploaded.version);
    Ok(())
}

fn parse_metadata(
    items: &[String],
    bucket: &Bucket,
) -> Result<serde_json::Map<String, serde_json::Value>> {
    let mut metadata = serde_json::Map::new();
    for item in items {
        let (key, value) = item.split_once('=').ok_or_else(|| {
            Error::Other(format!("Metadata '{item}' is not in KEY=VALUE format."))
        })?;
        metadata.insert(key.to_owned(), serde_json::Value::String(value.to_owned()));
    }
    let missing: Vec<&str> = bucket
        .required_metadata
        .keys()
        .filter(|key| !metadata.contains_key(*key))
        .map(String::as_str)
        .collect();
    if !missing.is_empty() {
        for (key, field) in &bucket.required_metadata {
            eprintln!("    - {key}: {} (example: {})", field.description, field.example);
        }
        return Err(Error::Other(format!(
            "Missing required metadata fields: {}",
            missing.join(", ")
        )));
    }
    Ok(metadata)
}

/// Match upstream's CSV handling: empty input is absent; individual values are preserved.
fn permission_list(value: Option<String>) -> Option<Vec<String>> {
    value
        .filter(|value| !value.is_empty())
        .map(|value| value.split(',').map(String::from).collect())
}

async fn run_delete(args: AssetDeleteArgs) -> Result<()> {
    if !args.yes
        && !tui::confirm(
            &format!(
                "Are you sure you want to delete '{}' from bucket '{}'?",
                args.key, args.bucket
            ),
            false,
        )
    {
        return Err(Error::Other("Deletion cancelled.".into()));
    }

    let client = ApiClient::new()?;
    let _: serde_json::Value = client.delete_json(&asset_path(&args.bucket, &args.key)).await?;

    fmt::success("Asset deleted successfully!");
    eprintln!("  Bucket: {}", args.bucket);
    eprintln!("  Key:    {}", args.key);
    Ok(())
}
