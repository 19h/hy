//! `hcli download` command.

use std::path::PathBuf;

use clap::Args;

use crate::api::{ApiClient, Asset, TagsResponse, TreeNode};
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
    let client = ApiClient::new()?;

    // --list-tags
    if args.list_tags {
        fmt::info("Fetching available tags...");
        let data: TagsResponse = client.get_json("/api/assets/tags").await?;
        let mut tags = data.tags;
        tags.sort_by(|a, b| a.tag.cmp(&b.tag));

        eprintln!("\nAvailable Download Tags ({} total):\n", tags.len());
        eprintln!(
            "{:<45} {:<40} {}",
            "Tag", "Name", "Asset Key"
        );
        eprintln!("{}", "-".repeat(130));
        for tag in &tags {
            eprintln!(
                "{:<45} {:<40} {}",
                tag.tag, tag.description, tag.key
            );
        }
        eprintln!("\nDetected platform: {}", tag_os());
        return Ok(());
    }

    // Resolve the key.
    let mut key = args.key;
    let mode = if args.pattern.is_some() {
        "direct"
    } else {
        &args.mode
    };

    if let Some(ref k) = key {
        if is_tag_format(k) {
            fmt::info(&format!("Resolving tag: {k}..."));
            let normalized = normalize_tag_with_os(k);
            if let Some(resolved) = resolve_tag(&client, &normalized).await? {
                fmt::success(&format!("Resolved to: {resolved}"));
                key = Some(resolved);
            } else {
                fmt::error(&format!("Tag '{normalized}' not found"));
                return Ok(());
            }
        }
    }

    let selected_keys: Vec<String> = if let Some(k) = key {
        vec![k]
    } else if mode == "direct" {
        if let Some(ref pattern) = args.pattern {
            let data: crate::api::PagedAssets =
                client.get_json("/api/assets/installers?type=file&limit=1000&offset=0").await?;
            let re = regex::Regex::new(pattern)
                .map_err(|e| crate::error::Error::Other(format!("Invalid regex: {e}")))?;
            let filtered: Vec<String> = data
                .items
                .into_iter()
                .filter(|a| re.is_match(&a.key))
                .map(|a| a.key)
                .collect();
            if filtered.is_empty() {
                fmt::error(&format!("No assets matching pattern: {pattern}"));
                return Ok(());
            }
            for k in &filtered {
                eprintln!("  * {k}");
            }
            filtered
        } else {
            fmt::error("--pattern is required in direct mode");
            return Ok(());
        }
    } else {
        // Interactive mode: tree navigation.
        let tree: Vec<TreeNode> = client
            .get_json("/api/assets/installers?type=file&view=tree&limit=1000&offset=0")
            .await?;
        let all_assets = collect_assets(&tree);
        if all_assets.is_empty() {
            fmt::warning("No downloads available.");
            return Ok(());
        }

        let items: Vec<String> = all_assets.iter().map(|a| a.key.clone()).collect();
        let selection = dialoguer::Select::new()
            .with_prompt("Select a download")
            .items(&items)
            .interact_opt()
            .unwrap_or(None);

        match selection {
            Some(idx) => vec![all_assets[idx].key.clone()],
            None => {
                fmt::warning("Download cancelled.");
                return Ok(());
            }
        }
    };

    // Download each key.
    let target_dir = PathBuf::from(&args.output_dir);
    let mut downloaded = 0usize;

    for selected_key in &selected_keys {
        fmt::info(&format!("Getting download URL for: {selected_key}"));
        let asset: Asset = match client
            .get_json(&format!("/api/assets/installers/{selected_key}"))
            .await
        {
            Ok(a) => a,
            Err(e) => {
                fmt::error(&format!("Failed to get URL for {selected_key}: {e}"));
                continue;
            }
        };

        let Some(ref url) = asset.url else {
            fmt::error(&format!("No download URL for {selected_key}"));
            continue;
        };

        match client
            .download_file(
                url,
                &target_dir,
                None,
                args.force,
                true,
                Some(selected_key),
            )
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
    } else {
        fmt::error("No files were downloaded");
    }

    Ok(())
}

// ── helpers ─────────────────────────────────────────────────────────────

fn is_tag_format(key: &str) -> bool {
    key.contains(':') && !key.contains('/')
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
    let data: TagsResponse = client.get_json("/api/assets/tags").await?;
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

fn collect_assets(nodes: &[TreeNode]) -> Vec<Asset> {
    let mut out = Vec::new();
    for node in nodes {
        if node.node_type == "file" {
            if let Some(ref a) = node.asset {
                out.push(a.clone());
            }
        }
        if let Some(ref children) = node.children {
            out.extend(collect_assets(children));
        }
    }
    out
}
