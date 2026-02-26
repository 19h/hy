//! `hy license` command group: list, get, install.

use std::path::PathBuf;

use clap::{Args, Subcommand};
use dialoguer::Select;
use owo_colors::OwoColorize;

use crate::api::{ApiClient, License, PagedLicenses};
use crate::error::Result;
use crate::util::fmt;

// ── CLI definitions ─────────────────────────────────────────────────────

#[derive(Debug, Subcommand)]
pub enum LicenseCommands {
    /// List licenses
    List(LicenseListArgs),
    /// Download license file(s)
    Get(LicenseGetArgs),
    /// Install a license file into an IDA directory
    Install(LicenseInstallArgs),
}

#[derive(Debug, Args)]
pub struct LicenseListArgs {
    /// Customer ID (auto-detected if omitted)
    pub customer_id: Option<String>,
    /// Filter by plan type
    #[arg(short, long)]
    pub plan: Option<String>,
}

#[derive(Debug, Args)]
pub struct LicenseGetArgs {
    /// Customer ID (auto-detected if omitted)
    #[arg(long)]
    pub customer_id: Option<String>,
    /// License ID (pubhash) to download
    #[arg(short, long)]
    pub id: Option<String>,
    /// Download all matching licenses
    #[arg(short, long)]
    pub all: bool,
    /// Output directory
    #[arg(long, default_value = "./")]
    pub output_dir: PathBuf,
}

#[derive(Debug, Args)]
pub struct LicenseInstallArgs {
    /// Path to the .hexlic license file
    pub file: PathBuf,
    /// Target IDA installation directory (interactive if omitted)
    pub ida_dir: Option<PathBuf>,
}

// ── dispatch ────────────────────────────────────────────────────────────

pub async fn run(cmd: LicenseCommands) -> Result<()> {
    match cmd {
        LicenseCommands::List(args) => run_list(args).await,
        LicenseCommands::Get(args) => run_get(args).await,
        LicenseCommands::Install(args) => run_install(args).await,
    }
}

// ── helpers ─────────────────────────────────────────────────────────────

/// Fetch all customers and let the user select one if multiple exist.
async fn select_customer(client: &ApiClient) -> Result<String> {
    let customers: Vec<crate::api::Customer> =
        client.get_json("/api/customers").await?;

    if customers.is_empty() {
        return Err(crate::error::Error::NotFound(
            "No customer accounts found.".into(),
        ));
    }

    if customers.len() == 1 {
        return Ok(customers[0]
            .id
            .map(|id| id.to_string())
            .unwrap_or_default());
    }

    // Multiple customers — let user pick.
    let items: Vec<String> = customers
        .iter()
        .map(|c| {
            format!(
                "{} ({})",
                c.display_name(),
                c.id.map(|id| id.to_string()).unwrap_or_default()
            )
        })
        .collect();

    let selection = Select::new()
        .with_prompt("Select customer account")
        .items(&items)
        .interact()
        .map_err(|_| crate::error::Error::Other("Cancelled".into()))?;

    Ok(customers[selection]
        .id
        .map(|id| id.to_string())
        .unwrap_or_default())
}

/// Format license status with color.
fn format_status(lic: &License) -> String {
    let status = lic.status.as_deref().unwrap_or("unknown");
    match status {
        "active" => "Active".green().to_string(),
        "expired" => "Expired".red().to_string(),
        "cancelled" => "Cancelled".yellow().to_string(),
        _ => status.to_string(),
    }
}

/// Format relative expiration.
fn format_expiry(end_date: Option<&str>) -> String {
    let Some(end) = end_date else {
        return "-".into();
    };
    let Ok(dt) = chrono::DateTime::parse_from_rfc3339(end)
        .or_else(|_| chrono::DateTime::parse_from_rfc3339(&end.replace('Z', "+00:00")))
    else {
        return end.chars().take(10).collect();
    };

    let now = chrono::Utc::now();
    let diff = dt.signed_duration_since(now);
    let days = diff.num_days();
    if days < 0 {
        let ago = -days;
        if ago > 365 {
            format!("{}y ago", ago / 365)
        } else if ago > 30 {
            format!("{}mo ago", ago / 30)
        } else {
            format!("{}d ago", ago)
        }
    } else if days > 365 {
        format!("{}y", days / 365)
    } else if days > 30 {
        format!("{}mo", days / 30)
    } else {
        format!("{}d", days)
    }
}

/// Get the edition display name for a license.
fn edition_name(lic: &License) -> String {
    lic.edition
        .as_ref()
        .and_then(|e| e.edition_name.clone())
        .or_else(|| lic.product_code.clone())
        .unwrap_or_else(|| "-".into())
}

/// Count addon decompilers.
fn addon_summary(lic: &License) -> String {
    let Some(ref addons) = lic.addons else {
        return "-".into();
    };
    if addons.is_empty() {
        return "-".into();
    }
    let decompiler_count = addons
        .iter()
        .filter(|a| {
            a.product
                .as_ref()
                .map(|p| p.product_type == "decompiler" || p.code.contains("hex"))
                .unwrap_or(false)
        })
        .count();
    let other_count = addons.len() - decompiler_count;
    let mut parts = Vec::new();
    if decompiler_count > 0 {
        parts.push(format!("{}x decompiler", decompiler_count));
    }
    if other_count > 0 {
        parts.push(format!("+{} other", other_count));
    }
    if parts.is_empty() {
        format!("{} addon(s)", addons.len())
    } else {
        parts.join(", ")
    }
}

// ── license list ────────────────────────────────────────────────────────

async fn run_list(args: LicenseListArgs) -> Result<()> {
    let client = ApiClient::new()?;
    let customer_id = match args.customer_id {
        Some(id) => id,
        None => select_customer(&client).await?,
    };

    let data: PagedLicenses = client
        .get_json(&format!(
            "/api/licenses/{}?page=1&limit=100",
            customer_id
        ))
        .await?;

    if data.items.is_empty() {
        fmt::warning("No licenses found.");
        return Ok(());
    }

    // Optional plan filter.
    let items: Vec<&License> = if let Some(ref plan) = args.plan {
        data.items
            .iter()
            .filter(|l| {
                l.product_catalog
                    .as_deref()
                    .map(|c| c.to_lowercase().contains(&plan.to_lowercase()))
                    .unwrap_or(false)
            })
            .collect()
    } else {
        data.items.iter().collect()
    };

    if items.is_empty() {
        fmt::warning(&format!(
            "No licenses matching plan '{}'.",
            args.plan.as_deref().unwrap_or("?")
        ));
        return Ok(());
    }

    eprintln!(
        "  {:<12} {:<22} {:<12} {:<10} {:<10} {}",
        "ID", "Edition", "Type", "Status", "Expires", "Addons"
    );
    eprintln!("  {}", "-".repeat(85));

    for lic in &items {
        let pubhash = lic
            .pubhash
            .as_deref()
            .map(|h| &h[..h.len().min(10)])
            .unwrap_or("-");
        let lic_type = lic.license_type.as_deref().unwrap_or("-");
        let expiry = format_expiry(lic.end_date.as_deref());

        eprintln!(
            "  {:<12} {:<22} {:<12} {:<10} {:<10} {}",
            pubhash,
            edition_name(lic),
            lic_type,
            format_status(lic),
            expiry,
            addon_summary(lic).dimmed(),
        );
    }

    eprintln!();
    eprintln!("  {} license(s).", items.len());

    Ok(())
}

// ── license get (download) ──────────────────────────────────────────────

async fn run_get(args: LicenseGetArgs) -> Result<()> {
    let client = ApiClient::new()?;
    let customer_id = match args.customer_id {
        Some(id) => id,
        None => select_customer(&client).await?,
    };

    let data: PagedLicenses = client
        .get_json(&format!(
            "/api/licenses/{}?page=1&limit=100",
            customer_id
        ))
        .await?;

    // Filter to active licenses.
    let active: Vec<&License> = data
        .items
        .iter()
        .filter(|l| l.status.as_deref() == Some("active"))
        .collect();

    if active.is_empty() {
        fmt::warning("No active licenses found.");
        return Ok(());
    }

    // Select which licenses to download.
    let to_download: Vec<&License> = if let Some(ref id) = args.id {
        // Filter by pubhash.
        let matched: Vec<_> = active
            .iter()
            .filter(|l| {
                l.pubhash
                    .as_deref()
                    .map(|h| h.starts_with(id.as_str()))
                    .unwrap_or(false)
            })
            .copied()
            .collect();
        if matched.is_empty() {
            fmt::error(&format!("No license matching ID '{id}'."));
            return Ok(());
        }
        matched
    } else if args.all {
        active
    } else if active.len() == 1 {
        active
    } else {
        // Interactive selection.
        let items: Vec<String> = active
            .iter()
            .map(|l| {
                format!(
                    "{} - {} ({})",
                    l.pubhash.as_deref().unwrap_or("?"),
                    edition_name(l),
                    l.license_type.as_deref().unwrap_or("?"),
                )
            })
            .collect();

        let selections = dialoguer::MultiSelect::new()
            .with_prompt("Select licenses to download")
            .items(&items)
            .defaults(&vec![true; items.len()])
            .interact()
            .map_err(|_| crate::error::Error::Other("Cancelled".into()))?;

        if selections.is_empty() {
            fmt::info("No licenses selected.");
            return Ok(());
        }

        selections.iter().map(|&i| active[i]).collect()
    };

    std::fs::create_dir_all(&args.output_dir)?;

    let mut count = 0;
    for lic in &to_download {
        let Some(ref pubhash) = lic.pubhash else {
            continue;
        };
        let asset_types = lic
            .asset_types
            .as_deref()
            .unwrap_or(&[]);

        if asset_types.is_empty() {
            fmt::warning(&format!(
                "License {} has no downloadable asset types.",
                pubhash
            ));
            continue;
        }

        for asset_type in asset_types {
            fmt::info(&format!(
                "Downloading {asset_type} for {}...",
                pubhash
            ));

            let url_result: std::result::Result<String, _> = client
                .get_json(&format!(
                    "/api/licenses/{}/download/{}/{}",
                    customer_id, asset_type, pubhash
                ))
                .await;

            match url_result {
                Ok(url) => {
                    match client
                        .download_file(
                            &url,
                            &args.output_dir,
                            None,
                            false,
                            false,
                            None,
                        )
                        .await
                    {
                        Ok(path) => {
                            fmt::success(&format!("  Saved: {}", path.display()));
                            count += 1;
                        }
                        Err(e) => {
                            fmt::error(&format!(
                                "  Failed to download {asset_type} for {pubhash}: {e}"
                            ));
                        }
                    }
                }
                Err(e) => {
                    fmt::error(&format!(
                        "  Failed to get download URL for {asset_type}/{pubhash}: {e}"
                    ));
                }
            }
        }
    }

    if count > 0 {
        fmt::success(&format!("Downloaded {count} file(s)."));
    } else {
        fmt::warning("No files were downloaded.");
    }

    Ok(())
}

// ── license install ─────────────────────────────────────────────────────

async fn run_install(args: LicenseInstallArgs) -> Result<()> {
    if !args.file.exists() {
        fmt::error(&format!("File not found: {}", args.file.display()));
        return Ok(());
    }

    // Determine target directory.
    let target = match args.ida_dir {
        Some(dir) => dir,
        None => {
            // Build a list of candidate directories.
            let mut candidates: Vec<(String, PathBuf)> = Vec::new();

            // IDA user directory.
            let user_dir = crate::ida::ida_user_dir();
            candidates.push(("IDA user directory".into(), user_dir));

            // Standard installations.
            for p in crate::ida::find_standard_installations() {
                let label = format!("Installation: {}", p.display());
                candidates.push((label, p));
            }

            // Current default installation.
            if let Some(d) = crate::ida::current_install_dir() {
                let label = format!("Default: {}", d.display());
                if !candidates.iter().any(|(_, p)| p == &d) {
                    candidates.push((label, d));
                }
            }

            if candidates.is_empty() {
                fmt::error("No IDA installation directories found.");
                fmt::info("Specify a target directory: hy license install <file> <ida_dir>");
                return Ok(());
            }

            if candidates.len() == 1 {
                candidates[0].1.clone()
            } else {
                let items: Vec<&str> = candidates.iter().map(|(s, _)| s.as_str()).collect();
                let selection = Select::new()
                    .with_prompt("Install license to")
                    .items(&items)
                    .interact()
                    .map_err(|_| crate::error::Error::Other("Cancelled".into()))?;
                candidates[selection].1.clone()
            }
        }
    };

    // Ensure target exists.
    if !target.exists() {
        let create = dialoguer::Confirm::new()
            .with_prompt(format!(
                "Directory {} does not exist. Create it?",
                target.display()
            ))
            .default(true)
            .interact()
            .unwrap_or(false);
        if create {
            std::fs::create_dir_all(&target)?;
        } else {
            return Ok(());
        }
    }

    // Copy the license file.
    let filename = args.file.file_name().unwrap_or_default();
    let dest = target.join(filename);

    std::fs::copy(&args.file, &dest)?;
    fmt::success(&format!("License installed to: {}", dest.display()));

    Ok(())
}
