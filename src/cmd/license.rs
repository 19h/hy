//! `hcli license` command group: list, get, install.

use clap::{Args, Subcommand};

use crate::api::{ApiClient, PagedLicenses};
use crate::error::Result;
use crate::util::fmt;

#[derive(Debug, Subcommand)]
pub enum LicenseCommands {
    /// List licenses for a customer
    List(LicenseListArgs),
    /// Get license details
    Get(LicenseGetArgs),
    /// Install a license file
    Install(LicenseInstallArgs),
}

#[derive(Debug, Args)]
pub struct LicenseListArgs {
    /// Customer ID
    pub customer_id: String,
}

#[derive(Debug, Args)]
pub struct LicenseGetArgs {
    /// Customer ID
    pub customer_id: String,
    /// License ID
    pub license_id: String,
}

#[derive(Debug, Args)]
pub struct LicenseInstallArgs {
    /// Customer ID
    pub customer_id: String,
    /// License ID
    pub license_id: String,
    /// Asset type to download
    #[arg(long, default_value = "ida")]
    pub asset_type: String,
    /// Output directory
    #[arg(long, default_value = "./")]
    pub output_dir: String,
}

pub async fn run(cmd: LicenseCommands) -> Result<()> {
    match cmd {
        LicenseCommands::List(args) => run_list(args).await,
        LicenseCommands::Get(args) => run_get(args).await,
        LicenseCommands::Install(args) => run_install(args).await,
    }
}

async fn run_list(args: LicenseListArgs) -> Result<()> {
    let client = ApiClient::new()?;
    let data: PagedLicenses = client
        .get_json(&format!(
            "/api/licenses/{}?page=1&limit=100",
            args.customer_id
        ))
        .await?;

    if data.items.is_empty() {
        fmt::warning("No licenses found.");
        return Ok(());
    }

    eprintln!(
        "{:<8} {:<20} {:<15} {:<15} {:<12} {}",
        "ID", "Product", "Start", "End", "Status", "Seats"
    );
    eprintln!("{}", "-".repeat(90));

    for lic in &data.items {
        eprintln!(
            "{:<8} {:<20} {:<15} {:<15} {:<12} {}",
            lic.id.unwrap_or(0),
            lic.product_code.as_deref().unwrap_or("-"),
            lic.start_date.as_deref().unwrap_or("-"),
            lic.end_date.as_deref().unwrap_or("-"),
            lic.status.as_deref().unwrap_or("-"),
            lic.seats.unwrap_or(0),
        );
    }

    Ok(())
}

async fn run_get(args: LicenseGetArgs) -> Result<()> {
    let client = ApiClient::new()?;
    let data: PagedLicenses = client
        .get_json(&format!(
            "/api/licenses/{}?page=1&limit=100",
            args.customer_id
        ))
        .await?;

    let license = data
        .items
        .iter()
        .find(|l| l.id.map(|id| id.to_string()) == Some(args.license_id.clone()));

    match license {
        Some(lic) => {
            let json = serde_json::to_string_pretty(lic)?;
            eprintln!("{json}");
        }
        None => {
            fmt::error("License not found.");
        }
    }

    Ok(())
}

async fn run_install(args: LicenseInstallArgs) -> Result<()> {
    let client = ApiClient::new()?;

    let url: String = client
        .get_json(&format!(
            "/api/licenses/{}/download/{}/{}",
            args.customer_id, args.asset_type, args.license_id
        ))
        .await?;

    let path = client
        .download_file(
            &url,
            std::path::Path::new(&args.output_dir),
            None,
            false,
            false,
            None,
        )
        .await?;

    fmt::success(&format!("License installed to: {}", path.display()));
    Ok(())
}
