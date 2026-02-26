//! `hy download` command.

use std::path::PathBuf;

use clap::Args;
use console::style;

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
        return run_list_tags(&client).await;
    }

    // Resolve the key.
    let mut key = args.key.clone();
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
                // Show tag suggestions.
                if let Ok(data) = client.get_json::<TagsResponse>("/api/assets/tags").await {
                    if !data.tags.is_empty() {
                        eprintln!("Available tags:");
                        for tag in data.tags.iter().take(10) {
                            eprintln!("  * {}", tag.tag);
                        }
                        if data.tags.len() > 10 {
                            eprintln!("  ... and {} more", data.tags.len() - 10);
                        }
                    }
                }
                return Err(crate::error::Error::Other(format!(
                    "Tag '{normalized}' not found"
                )));
            }
        }
    }

    let selected_keys: Vec<String> = if let Some(k) = key {
        vec![k]
    } else if mode == "direct" {
        run_direct_mode(&client, &args).await?
    } else {
        // Interactive tree navigation.
        run_interactive_mode(&client).await?
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
    Ok(())
}

// ── direct mode ────────────────────────────────────────────────────────

async fn run_direct_mode(client: &ApiClient, args: &DownloadArgs) -> Result<Vec<String>> {
    if let Some(ref pattern) = args.pattern {
        let data: crate::api::PagedAssets = client
            .get_json("/api/assets/installers?type=file&limit=1000&offset=0")
            .await?;
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

// ── interactive tree navigator ─────────────────────────────────────────

async fn run_interactive_mode(client: &ApiClient) -> Result<Vec<String>> {
    fmt::info("Fetching available downloads...");
    let tree: Vec<TreeNode> = client
        .get_json("/api/assets/installers?type=file&view=tree&limit=1000&offset=0")
        .await?;

    if tree.is_empty() {
        fmt::warning("No downloads available.");
        return Ok(Vec::new());
    }

    match select_asset(&tree)? {
        Some(asset) => Ok(vec![asset.key]),
        None => {
            fmt::warning("Download cancelled.");
            Ok(Vec::new())
        }
    }
}

/// Interactive tree navigation picker.
fn select_asset(root: &[TreeNode]) -> Result<Option<Asset>> {
    let mut path_stack: Vec<String> = Vec::new();

    loop {
        let current_nodes = nodes_at_path(root, &path_stack);
        if current_nodes.is_empty() {
            if path_stack.pop().is_some() {
                continue;
            }
            return Ok(None);
        }

        // Partition into folders and files.
        let mut folders: Vec<&TreeNode> = current_nodes
            .iter()
            .copied()
            .filter(|n| n.node_type == "folder" && n.children.as_ref().is_some_and(|c| !c.is_empty()))
            .collect();
        let files: Vec<&TreeNode> = current_nodes
            .iter()
            .copied()
            .filter(|n| n.node_type == "file")
            .collect();

        // Sort folders: version-like names descending, others alphabetically.
        folders.sort_by(|a, b| {
            match (parse_version_key(&a.name), parse_version_key(&b.name)) {
                (Some(va), Some(vb)) => vb.cmp(&va),
                _ => a.name.cmp(&b.name),
            }
        });

        // Build display items.
        let mut items: Vec<String> = Vec::new();
        let mut values: Vec<PickerItem> = Vec::new();

        if !path_stack.is_empty() {
            items.push(format!("{}", style("..").dim()));
            values.push(PickerItem::Back);
        }

        for folder in &folders {
            items.push(format!("{}/", folder.name));
            values.push(PickerItem::Folder(folder.name.clone()));
        }

        for file in &files {
            let display = file_display_name(file);
            items.push(format!("  {}", style(display).dim()));
            values.push(PickerItem::File(
                file.asset.as_ref().cloned().unwrap_or_else(|| Asset {
                    email: None,
                    filename: file.name.clone(),
                    size: 0,
                    key: String::new(),
                    code: None,
                    created_at: None,
                    expires_at: None,
                    url: None,
                    version: 0,
                    metadata: None,
                }),
            ));
        }

        // Prompt includes the current path.
        let prompt = if path_stack.is_empty() {
            "/".to_string()
        } else {
            format!("/{}", path_stack.join("/"))
        };

        let selection = dialoguer::FuzzySelect::new()
            .with_prompt(&prompt)
            .items(&items)
            .default(0)
            .highlight_matches(true)
            .interact_opt()
            .unwrap_or(None);

        let Some(idx) = selection else {
            return Ok(None);
        };

        match &values[idx] {
            PickerItem::Back => {
                path_stack.pop();
            }
            PickerItem::Folder(name) => {
                path_stack.push(name.clone());
            }
            PickerItem::File(asset) => {
                return Ok(Some(asset.clone()));
            }
        }
    }
}

#[derive(Clone)]
enum PickerItem {
    Back,
    Folder(String),
    File(Asset),
}

/// Walk the tree from the root following the given path stack.
fn nodes_at_path<'a>(root: &'a [TreeNode], path: &[String]) -> Vec<&'a TreeNode> {
    let mut current: Vec<&TreeNode> = root.iter().collect();
    for segment in path {
        let folder = current.iter().find(|n| n.node_type == "folder" && n.name == *segment);
        match folder {
            Some(f) => match &f.children {
                Some(children) => current = children.iter().collect(),
                None => return Vec::new(),
            },
            None => return Vec::new(),
        }
    }
    current
}

/// Build a human-friendly display name for a file node.
///
/// If the asset has metadata with a `"name"` key, use that plus the
/// filename in parentheses.  Otherwise, just use the node name.
fn file_display_name(node: &TreeNode) -> String {
    if let Some(ref asset) = node.asset {
        if let Some(ref meta) = asset.metadata {
            if let Some(name_val) = meta.get("name") {
                if let Some(name) = name_val.as_str() {
                    let filename = asset.key.rsplit('/').next().unwrap_or(&node.name);
                    return format!("{name} ({filename})");
                }
            }
        }
    }
    node.name.clone()
}

/// Try to parse a node name as a version-like key for sorting.
/// Returns a tuple of numeric parts for comparison.
fn parse_version_key(name: &str) -> Option<Vec<u32>> {
    // Handles names like "9.3", "9.0sp1", "8.4", "9.2-test"
    let cleaned = name.replace(|c: char| !c.is_ascii_digit() && c != '.', " ");
    let parts: Vec<u32> = cleaned
        .split(|c: char| !c.is_ascii_digit())
        .filter(|s| !s.is_empty())
        .filter_map(|s| s.parse().ok())
        .collect();
    if parts.is_empty() {
        None
    } else {
        Some(parts)
    }
}

// ── download execution ─────────────────────────────────────────────────

async fn download_keys(
    client: &ApiClient,
    keys: &[String],
    args: &DownloadArgs,
) -> Result<()> {
    let target_dir = PathBuf::from(&args.output_dir);
    let mut downloaded = 0usize;

    for selected_key in keys {
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
