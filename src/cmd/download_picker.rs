//! Hierarchical installer selection, preserving API folder and file order.

use crate::api::{ApiClient, Asset, TreeNode};
use crate::error::Result;
use crate::util::fmt;
use console::style;

pub(super) async fn run(client: &ApiClient) -> Result<Vec<String>> {
    fmt::info("Fetching available downloads...");
    let tree: Vec<TreeNode> =
        client.get_json("/api/assets/installers?type=file&view=tree&limit=1000&offset=0").await?;

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
        let folders: Vec<&TreeNode> = current_nodes
            .iter()
            .copied()
            .filter(|n| {
                n.node_type == "folder" && n.children.as_ref().is_some_and(|c| !c.is_empty())
            })
            .collect();
        let files: Vec<&TreeNode> =
            current_nodes.iter().copied().filter(|n| n.node_type == "file").collect();

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
            values.push(PickerItem::File(file.asset.as_ref().cloned().map(Box::new)));
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
                return Ok(asset.as_deref().cloned());
            }
        }
    }
}

#[derive(Clone)]
enum PickerItem {
    Back,
    Folder(String),
    File(Option<Box<Asset>>),
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
    if let Some(ref asset) = node.asset
        && let Some(ref meta) = asset.metadata
        && meta.contains_key("operating_system")
    {
        let filename = asset.key.rsplit('/').next().unwrap_or(&node.name);
        let name = meta.get("name").and_then(serde_json::Value::as_str).unwrap_or(&node.name);
        return format!("{name} ({filename})");
    }
    node.name.clone()
}
