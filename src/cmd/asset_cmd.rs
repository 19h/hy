//! `hy asset` command group (hidden): low-level bucket operations.
//!
//! Mirrors the hidden `hcli asset` group: upload an asset to an arbitrary
//! bucket with validated metadata, or delete an asset by key.

use std::collections::HashMap;
use std::path::PathBuf;

use clap::{Args, Subcommand};
use serde::Deserialize;

use crate::api::ApiClient;
use crate::error::Result;
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
    #[arg(short, long = "metadata")]
    pub metadata: Vec<String>,
    /// Comma-separated list of allowed segments (e.g. segment1,segment2)
    #[arg(long)]
    pub allowed_segments: Option<String>,
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
    #[serde(default, rename = "requiredMetadata")]
    required_metadata: HashMap<String, RequiredField>,
}

#[derive(Debug, Deserialize)]
struct RequiredField {
    #[serde(default)]
    description: String,
    #[serde(default)]
    example: String,
}

pub async fn run(cmd: AssetCommands) -> Result<()> {
    match cmd {
        AssetCommands::Put(args) => run_put(args).await,
        AssetCommands::Delete(args) => run_delete(args).await,
    }
}

async fn run_put(args: AssetPutArgs) -> Result<()> {
    use sha2::{Digest, Sha256};

    if !args.path.is_file() {
        fmt::error(&format!("Not a file: {}", args.path.display()));
        return Ok(());
    }

    let client = ApiClient::new()?;

    // Fetch the bucket schema and enforce required metadata.
    let bucket: Bucket = match client
        .get_json(&format!("/api/assets/buckets/{}", args.bucket))
        .await
    {
        Ok(b) => b,
        Err(_) => {
            fmt::error(&format!("Bucket '{}' does not exist.", args.bucket));
            return Ok(());
        }
    };

    let mut metadata = serde_json::Map::new();
    for item in &args.metadata {
        let Some((key, val)) = item.split_once('=') else {
            fmt::error(&format!("Metadata '{item}' is not in KEY=VALUE format."));
            return Ok(());
        };
        metadata.insert(key.to_owned(), serde_json::Value::String(val.to_owned()));
    }

    let missing: Vec<&String> = bucket
        .required_metadata
        .keys()
        .filter(|k| !metadata.contains_key(*k))
        .collect();
    if !missing.is_empty() {
        fmt::error(&format!(
            "Missing required metadata fields: {}",
            missing
                .iter()
                .map(|s| s.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ));
        fmt::warning("Required fields:");
        for (key, field) in &bucket.required_metadata {
            eprintln!("    - {key}: {} (example: {})", field.description, field.example);
        }
        return Ok(());
    }

    let meta = std::fs::metadata(&args.path)?;
    let filename = args
        .path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    let bytes = std::fs::read(&args.path)?;
    let checksum = format!("{:x}", Sha256::digest(&bytes));
    drop(bytes);

    let mut upload_data = serde_json::json!({
        "filename": filename,
        "size": meta.len(),
        "force": args.force,
        "status": "active",
        "checksum": checksum,
        "metadata": metadata,
    });
    if let Some(ref segments) = args.allowed_segments {
        upload_data["allowed_segments"] =
            serde_json::json!(segments.split(',').map(str::trim).collect::<Vec<_>>());
    }
    if let Some(ref emails) = args.allowed_emails {
        upload_data["allowed_emails"] =
            serde_json::json!(emails.split(',').map(str::trim).collect::<Vec<_>>());
    }

    // Request an upload URL, stream the file, then confirm.
    let resp: serde_json::Value = client
        .post_json(&format!("/api/assets/{}", args.bucket), &upload_data)
        .await?;
    let upload_url = resp.get("url").and_then(|v| v.as_str());
    let key = resp.get("key").and_then(|v| v.as_str()).unwrap_or_default();
    let version = resp.get("version").and_then(|v| v.as_i64()).unwrap_or(0);

    if let Some(url) = upload_url {
        client.put_file(url, &args.path).await?;
        let _: serde_json::Value = client
            .post_json(
                &format!("/api/assets/{}/{key}", args.bucket),
                &serde_json::json!({}),
            )
            .await?;
    }

    fmt::success("File uploaded successfully!");
    eprintln!("  Bucket:  {}", args.bucket);
    eprintln!("  Key:     {key}");
    eprintln!("  Version: {version}");
    Ok(())
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
        fmt::warning("Deletion cancelled.");
        return Ok(());
    }

    let client = ApiClient::new()?;
    let _: serde_json::Value = client
        .delete_json(&format!("/api/assets/{}/{}", args.bucket, args.key))
        .await?;

    fmt::success("Asset deleted successfully!");
    eprintln!("  Bucket: {}", args.bucket);
    eprintln!("  Key:    {}", args.key);
    Ok(())
}
