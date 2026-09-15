//! `hy download` command.

use std::path::PathBuf;

use clap::Args;

use crate::api::{ApiClient, Asset, TagsResponse, asset_path};
use crate::error::Result;
use crate::util::fmt;
use crate::util::io::tag_os;

#[derive(Debug, Args)]
pub struct DownloadArgs {
    /// Asset key or tag for direct download (e.g. `ida-pro:latest`)
    pub key: Option<String>,

    /// Skip cache
    #[arg(short, long)]
    pub force: bool,

    /// Output directory
    #[arg(long, default_value = "./")]
    pub output_dir: String,

    /// Download mode: interactive or direct
    #[arg(long, default_value = "interactive")]
    pub mode: String,

    /// Pattern to search for assets (required in direct mode)
    #[arg(long)]
    pub pattern: Option<String>,

    /// List all available download tags and exit
    #[arg(long)]
    pub list_tags: bool,
}

pub async fn run(args: DownloadArgs) -> Result<()> {
    if args.mode == "direct" && args.pattern.as_deref().is_none_or(str::is_empty) {
        return Err(crate::error::Error::Other("--pattern is required in direct mode".into()));
    }
    let client = ApiClient::new()?;

    // --list-tags
    if args.list_tags {
        return run_list_tags(&client).await;
    }

    // Resolve the key.
    let mut key = args.key.clone().filter(|key| !key.is_empty());
    let mode = if args.pattern.as_deref().is_some_and(|pattern| !pattern.is_empty()) {
        "direct"
    } else {
        &args.mode
    };

    if let Some(ref k) = key
        && is_tag_format(k)
    {
        fmt::info(&format!("Resolving tag: {k}..."));
        let normalized = normalize_tag_with_os(k);
        if let Some(resolved) = resolve_tag(&client, &normalized).await? {
            fmt::success(&format!("Resolved to: {resolved}"));
            key = Some(resolved);
        } else {
            fmt::error(&format!("Tag '{normalized}' not found"));
            // Show tag suggestions.
            let data: TagsResponse = client.get_json("/api/assets/tags").await?;
            if !data.tags.is_empty() {
                eprintln!("Available tags:");
                for tag in data.tags.iter().take(10) {
                    eprintln!("  * {}", tag.tag);
                }
                if data.tags.len() > 10 {
                    eprintln!("  ... and {} more", data.tags.len() - 10);
                }
            }
            return Ok(());
        }
    }

    let selected_keys: Vec<String> = if let Some(k) = key {
        vec![k]
    } else if mode == "direct" {
        run_direct_mode(&client, &args).await?
    } else {
        // Interactive tree navigation.
        super::download_picker::run(&client).await?
    };

    if selected_keys.is_empty() {
        // Empty selection from interactive cancel is not an error.
        return Ok(());
    }

    // Download each selected key.
    download_keys(&client, &selected_keys, &args).await
}

// ── list-tags ──────────────────────────────────────────────────────────

async fn run_list_tags(client: &ApiClient) -> Result<()> {
    fmt::info("Fetching available tags...");
    let data: TagsResponse = client.get_json("/api/assets/tags").await?;
    let mut tags = data.tags;
    if tags.is_empty() {
        fmt::warning("No tags available");
        return Ok(());
    }
    tags.sort_by(|a, b| a.tag.cmp(&b.tag));

    eprintln!("\nAvailable Download Tags ({} total):\n", tags.len());
    eprintln!("{:<45} {:<40} Asset Key", "Tag", "Name");
    eprintln!("{}", "-".repeat(130));
    for tag in &tags {
        eprintln!("{:<45} {:<40} {}", tag.tag, tag.description, tag.key);
    }
    eprintln!("\nDetected platform: {}", tag_os());
    Ok(())
}

// ── direct mode ────────────────────────────────────────────────────────

async fn run_direct_mode(client: &ApiClient, args: &DownloadArgs) -> Result<Vec<String>> {
    if let Some(ref pattern) = args.pattern {
        let data: crate::api::PagedAssets =
            client.get_json("/api/assets/installers?type=file&limit=1000&offset=0").await?;
        let re = match fancy_regex::RegexBuilder::new(pattern).case_insensitive(true).build() {
            Ok(pattern) => pattern,
            Err(error) => {
                fmt::error(&format!("Invalid regex pattern: {error}"));
                return Ok(Vec::new());
            }
        };
        let mut filtered = Vec::new();
        for asset in data.items {
            if re.is_match(&asset.key).map_err(|error| {
                crate::error::Error::Other(format!("Pattern evaluation failed: {error}"))
            })? {
                filtered.push(asset.key);
            }
        }
        if filtered.is_empty() {
            fmt::error(&format!("No assets matching pattern: {pattern}"));
            return Ok(Vec::new());
        }
        for k in &filtered {
            eprintln!("  * {k}");
        }
        Ok(filtered)
    } else {
        fmt::error("--pattern is required in direct mode");
        Ok(Vec::new())
    }
}

// ── download execution ─────────────────────────────────────────────────

async fn download_keys(client: &ApiClient, keys: &[String], args: &DownloadArgs) -> Result<()> {
    let target_dir = PathBuf::from(&args.output_dir);
    let mut downloaded = 0usize;

    for selected_key in keys {
        fmt::info(&format!("Getting download URL for: {selected_key}"));
        let asset: Asset = match client.get_json(&asset_path("installers", selected_key)).await {
            Ok(a) => a,
            Err(e) => {
                fmt::error(&format!("Failed to get URL for {selected_key}: {e}"));
                continue;
            }
        };

        let Some(url) = asset.url.as_deref().filter(|url| !url.is_empty()) else {
            fmt::error(&format!("No download URL for {selected_key}"));
            continue;
        };

        match client
            .download_file(url, &target_dir, None, args.force, true, Some(selected_key))
            .await
        {
            Ok(path) => {
                fmt::success(&format!("Saved to: {}", path.display()));
                downloaded += 1;
            }
            Err(e) => {
                fmt::error(&format!("Download failed for {selected_key}: {e}"));
            }
        }
    }

    if downloaded > 0 {
        fmt::success(&format!("Downloaded {downloaded} file(s)"));
        Ok(())
    } else {
        fmt::error("No files were downloaded");
        Err(crate::error::Error::Other("No files were downloaded".into()))
    }
}

// ── tag helpers ─────────────────────────────────────────────────────────

fn is_tag_format(key: &str) -> bool {
    key.contains(':') && !key.contains('/')
}

/// Resolve one installer reference and retain its actual downloaded path.
pub async fn download_installer(reference: &str, destination: &std::path::Path) -> Result<PathBuf> {
    let client = ApiClient::new()?;
    let key = if is_tag_format(reference) {
        let tag = normalize_tag_with_os(reference);
        resolve_tag(&client, &tag)
            .await?
            .ok_or_else(|| crate::error::Error::NotFound(format!("installer tag {tag}")))?
    } else {
        reference.into()
    };
    let asset: Asset = client.get_json(&asset_path("installers", &key)).await?;
    let url = asset
        .url
        .ok_or_else(|| crate::error::Error::NotFound(format!("download URL for {key}")))?;
    client.download_file(&url, destination, None, false, true, Some(&key)).await
}

fn normalize_tag_with_os(tag: &str) -> String {
    let parts: Vec<&str> = tag.split(':').collect();
    if parts.len() >= 3 {
        tag.to_owned()
    } else if parts.len() == 2 {
        format!("{tag}:{}", tag_os())
    } else {
        tag.to_owned()
    }
}

async fn resolve_tag(client: &ApiClient, tag: &str) -> Result<Option<String>> {
    let data: TagsResponse = match client.get_json("/api/assets/tags").await {
        Ok(data) => data,
        Err(error) => {
            fmt::warning(&format!("Failed to resolve tag: {error}"));
            return Ok(None);
        }
    };
    // Exact match first.
    if let Some(t) = data.tags.iter().find(|t| t.tag == tag) {
        return Ok(Some(t.key.clone()));
    }
    // Case-insensitive.
    let lower = tag.to_lowercase();
    if let Some(t) = data.tags.iter().find(|t| t.tag.to_lowercase() == lower) {
        return Ok(Some(t.key.clone()));
    }
    Ok(None)
}
