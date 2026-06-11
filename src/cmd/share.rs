//! `hcli share` command group: put, get, list, delete.

use std::path::PathBuf;

use clap::{Args, Subcommand};

use crate::api::{ApiClient, Asset, PagedAssets};
use crate::error::Result;
use crate::util::{fmt, tui};

#[derive(Debug, Subcommand)]
pub enum ShareCommands {
    /// Upload a shared file
    Put(PutArgs),
    /// Download a shared file by shortcode
    Get(GetArgs),
    /// List your shared files
    List(ListArgs),
    /// Delete a shared file by shortcode
    Delete(DeleteArgs),
}

#[derive(Debug, Args)]
pub struct PutArgs {
    /// Path to the file to upload
    pub path: PathBuf,
    /// Access control level
    #[arg(short, long)]
    pub acl: Option<String>,
    /// Upload a new version for an existing code
    #[arg(short, long)]
    pub code: Option<String>,
    /// Force upload
    #[arg(short, long)]
    pub force: bool,
}

#[derive(Debug, Args)]
pub struct GetArgs {
    /// The shortcode of the shared file
    pub shortcode: String,
    /// Output directory
    #[arg(short, long, conflicts_with = "output_file")]
    pub output_dir: Option<PathBuf>,
    /// Output file path
    #[arg(short = 'O', long, conflicts_with = "output_dir")]
    pub output_file: Option<PathBuf>,
    /// Overwrite existing files
    #[arg(short, long)]
    pub force: bool,
}

#[derive(Debug, Args)]
pub struct ListArgs {
    /// Maximum number of files to display
    #[arg(long, default_value_t = 100)]
    pub limit: i64,
    /// Offset for pagination
    #[arg(long, default_value_t = 0)]
    pub offset: i64,
    /// Disable interactive mode
    #[arg(long)]
    pub no_interactive: bool,
}

#[derive(Debug, Args)]
pub struct DeleteArgs {
    /// Shortcode of the file to delete
    pub code: String,
    /// Skip confirmation
    #[arg(short, long)]
    pub force: bool,
}

pub async fn run(cmd: ShareCommands) -> Result<()> {
    match cmd {
        ShareCommands::Put(args) => run_put(args).await,
        ShareCommands::Get(args) => run_get(args).await,
        ShareCommands::List(args) => run_list(args).await,
        ShareCommands::Delete(args) => run_delete(args).await,
    }
}

async fn run_put(args: PutArgs) -> Result<()> {
    use sha2::{Digest, Sha256};

    let client = ApiClient::new()?;
    let path = args.path.canonicalize()?;

    if !path.is_file() {
        fmt::error(&format!("Not a file: {}", path.display()));
        return Ok(());
    }

    let meta = std::fs::metadata(&path)?;
    let filename = path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    // Determine ACL.
    let acl_type = match args.acl.as_deref() {
        Some(a) => a.to_string(),
        None => {
            let choices = vec![
                "private (just me)".to_string(),
                "domain (my organization)".to_string(),
                "authenticated (anyone with link)".to_string(),
            ];
            match tui::select("Access control", &choices, 0) {
                Some(1) => "domain".to_string(),
                Some(2) => "authenticated".to_string(),
                _ => "private".to_string(),
            }
        }
    };

    // Get the user's email for ACL computation.
    let user_email = {
        let auth = crate::auth::AuthService::global();
        auth.user_email().unwrap_or("").to_string()
    };
    let email_domain = user_email
        .split_once('@')
        .map(|(_, d)| format!("@{d}"))
        .unwrap_or_default();

    // Compute permissions from ACL type.
    let (allowed_segments, allowed_emails): (Vec<String>, Option<Vec<String>>) = match acl_type.as_str() {
        "private" => (
            vec!["authenticated".into()],
            Some(vec![user_email.clone()]),
        ),
        "domain" => (
            vec!["authenticated".into(), email_domain],
            None,
        ),
        _ => (
            vec!["authenticated".into()],
            None,
        ),
    };

    // SHA-256
    let bytes = std::fs::read(&path)?;
    let checksum = format!("{:x}", Sha256::digest(&bytes));

    let mut upload_data = serde_json::json!({
        "filename": filename,
        "size": meta.len(),
        "force": args.force,
        "status": "active",
        "checksum": checksum,
        "allowed_segments": allowed_segments,
        "metadata": { "acl_type": acl_type },
    });

    if let Some(ref emails) = allowed_emails {
        upload_data["allowed_emails"] = serde_json::json!(emails);
    }

    if let Some(ref code) = args.code {
        upload_data["code"] = serde_json::json!(code);
    }

    // Request upload URL.
    let resp: serde_json::Value = client
        .post_json("/api/assets/shared", &upload_data)
        .await?;
    let upload_url = resp.get("url").and_then(|v| v.as_str());
    let code = resp.get("code").and_then(|v| v.as_str());
    let key = resp.get("key").and_then(|v| v.as_str());
    let download_url = resp.get("download_url").and_then(|v| v.as_str());

    if let Some(url) = upload_url {
        client.put_file(url, &path).await?;
        // Confirm.
        if let Some(k) = key {
            let _: serde_json::Value = client
                .post_json(&format!("/api/assets/shared/{k}"), &serde_json::json!({}))
                .await?;
        }
    }

    if let Some(c) = code {
        let portal = crate::config::Env::global().portal_url.clone();
        fmt::success(&format!("Uploaded! Code: {c}"));
        eprintln!("  Share URL: {portal}/share/{c}");
        if let Some(dl) = download_url {
            eprintln!("  Download: {dl}");
        }
    } else {
        fmt::success("Upload complete.");
    }

    Ok(())
}

async fn run_get(args: GetArgs) -> Result<()> {
    let client = ApiClient::new()?;

    let asset: Asset = client
        .get_json(&format!("/api/assets/s/{}?version=-1", args.shortcode))
        .await?;

    let Some(ref url) = asset.url else {
        fmt::error("No download URL available.");
        return Ok(());
    };

    let (dir, filename) = if let Some(ref out) = args.output_file {
        (
            out.parent().unwrap_or(std::path::Path::new(".")).to_path_buf(),
            Some(out.file_name().unwrap_or_default().to_string_lossy().to_string()),
        )
    } else {
        (
            args.output_dir.unwrap_or_else(|| PathBuf::from(".")),
            None,
        )
    };

    let path = client
        .download_file(
            url,
            &dir,
            filename.as_deref(),
            args.force,
            true,
            None,
        )
        .await?;

    fmt::success(&format!("Saved to: {}", path.display()));
    Ok(())
}

async fn run_list(args: ListArgs) -> Result<()> {
    let client = ApiClient::new()?;

    fmt::info("Loading shared files...");
    let page: PagedAssets = client
        .get_json(&format!(
            "/api/assets/shared?type=file&limit={}&offset={}",
            args.limit, args.offset
        ))
        .await?;

    if page.items.is_empty() {
        fmt::warning("No shared files found.");
        return Ok(());
    }

    if args.no_interactive {
        let mut table = tui::Table::new(&["#", "Code", "Name", "Ver", "Size", "Created"]);
        for (i, file) in page.items.iter().enumerate() {
            table.add_row(vec![
                (i + 1).to_string(),
                file.code.as_deref().unwrap_or("-").to_string(),
                file.filename.clone(),
                format!("v{}", file.version),
                fmt::format_size(file.size),
                file.created_at
                    .as_deref()
                    .map(fmt::format_datetime)
                    .unwrap_or_else(|| "N/A".into()),
            ]);
        }
        table.print();
        return Ok(());
    }

    // Interactive mode
    let mut items = Vec::new();
    for file in &page.items {
        let code = file.code.as_deref().unwrap_or("-");
        let name = if file.filename.is_empty() { "unnamed" } else { &file.filename };
        items.push(format!("{} ({}) - {}", name, code, fmt::format_size(file.size)));
    }

    let selection = tui::multi_select("Select files to manage", &items);

    let Some(selected_indices) = selection else {
        fmt::warning("No files selected.");
        return Ok(());
    };

    if selected_indices.is_empty() {
        fmt::warning("No files selected.");
        return Ok(());
    }

    let selected_files: Vec<&Asset> = selected_indices.into_iter().map(|i| &page.items[i]).collect();
    let count = selected_files.len();

    let action_choices = vec![
        format!("Delete {count} file{}", if count > 1 { "s" } else { "" }),
        format!("Download {count} file{}", if count > 1 { "s" } else { "" }),
    ];

    let action_idx = tui::select("What would you like to do?", &action_choices, 0);

    let Some(idx) = action_idx else {
        return Ok(());
    };

    if idx == 0 {
        // Delete
        eprintln!("\nYou are about to delete {count} file(s):");
        for f in &selected_files {
            eprintln!("  * {} ({})", f.filename, f.code.as_deref().unwrap_or("-"));
        }

        let confirm = tui::confirm("Are you sure you want to delete these files?", false);

        if !confirm {
            fmt::warning("Deletion cancelled.");
            return Ok(());
        }

        for f in selected_files {
            match client.delete_json::<serde_json::Value>(&format!("/api/assets/shared/{}", f.key)).await {
                Ok(_) => fmt::success(&format!("Deleted: {} ({})", f.filename, f.code.as_deref().unwrap_or("-"))),
                Err(e) => fmt::error(&format!("Failed to delete {}: {}", f.filename, e)),
            }
        }
    } else if idx == 1 {
        // Download
        let output_dir = tui::input("Output directory", "./");
        let output_path = PathBuf::from(output_dir);

        for f in selected_files {
            fmt::info(&format!("Downloading {}...", f.filename));
            
            // Get download URL by code/version
            let code = f.code.as_deref().unwrap_or("");
            let url_path = format!("/api/assets/s/{}?version={}", code, f.version);
            
            let asset_info: Asset = match client.get_json(&url_path).await {
                Ok(a) => a,
                Err(e) => {
                    fmt::error(&format!("Failed to get download info for {}: {}", f.filename, e));
                    continue;
                }
            };

            let Some(url) = asset_info.url else {
                fmt::error(&format!("No download URL available for {}", f.filename));
                continue;
            };

            match client.download_file(
                &url,
                &output_path,
                Some(&f.filename),
                false,
                true,
                Some(&f.key),
            ).await {
                Ok(p) => fmt::success(&format!("Downloaded: {}", p.display())),
                Err(e) => fmt::error(&format!("Failed to download {}: {}", f.filename, e)),
            }
        }
    }

    Ok(())
}

async fn run_delete(args: DeleteArgs) -> Result<()> {
    let client = ApiClient::new()?;

    let asset: Asset = client
        .get_json(&format!("/api/assets/s/{}?version=-1", args.code))
        .await?;

    eprintln!("File: {} ({})", asset.filename, args.code);

    if !args.force {
        let confirm = tui::confirm("Delete this file?", false);
        if !confirm {
            fmt::warning("Deletion cancelled.");
            return Ok(());
        }
    }

    let _: serde_json::Value = client
        .delete_json(&format!("/api/assets/shared/{}", asset.key))
        .await?;

    fmt::success(&format!("Deleted: {}", args.code));
    Ok(())
}
