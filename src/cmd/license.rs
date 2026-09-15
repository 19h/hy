//! License command definitions and shared customer selection.

use std::path::PathBuf;

use clap::{Args, Subcommand, ValueEnum};

use crate::api::{ApiClient, Customer};
use crate::error::{Error, Result};
use crate::util::{fmt, tui};

#[derive(Debug, Subcommand)]
pub enum LicenseCommands {
    /// List licenses
    List(LicenseListArgs),
    /// Download license files
    Get(LicenseGetArgs),
    /// Install a license file into an IDA directory
    Install(LicenseInstallArgs),
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum Plan {
    Subscription,
    Legacy,
}

impl Plan {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Subscription => "subscription",
            Self::Legacy => "legacy",
        }
    }
}

#[derive(Debug, Args)]
pub struct LicenseListArgs {
    /// Customer ID (auto-detected if omitted; native CLI extension)
    pub customer_id: Option<String>,
    /// Filter by plan type
    #[arg(short, long, value_enum)]
    pub plan: Option<Plan>,
}

#[derive(Debug, Args)]
pub struct LicenseGetArgs {
    /// Customer ID (auto-detected if omitted; native CLI extension)
    #[arg(long)]
    pub customer_id: Option<String>,
    /// Exact public license ID to download
    #[arg(short, long)]
    pub id: Option<String>,
    /// Filter by plan type
    #[arg(short, long, value_enum)]
    pub plan: Option<Plan>,
    /// Product code, e.g. IDAPRO, IDAHOME, or LICENSE_SERVER
    #[arg(short = 't', long = "type")]
    pub product_type: Option<String>,
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
    /// Target directory (interactive if omitted)
    pub ida_dir: Option<PathBuf>,
}

pub async fn run(cmd: LicenseCommands) -> Result<()> {
    match cmd {
        LicenseCommands::List(args) => super::license_list::run(args).await,
        LicenseCommands::Get(args) => super::license_download::run(args).await,
        LicenseCommands::Install(args) => super::license_install::run(args).await,
    }
}

pub(super) async fn customer_id(
    client: &ApiClient,
    explicit: Option<String>,
) -> Result<Option<String>> {
    if let Some(id) = explicit {
        return Ok(Some(id));
    }
    let customers: Vec<Customer> = client.get_json("/api/customers").await?;
    if customers.is_empty() {
        return Err(Error::NotFound("No customers found".into()));
    }
    let selected = if customers.len() == 1 {
        0
    } else {
        let choices: Vec<String> = customers
            .iter()
            .map(|customer| {
                format!(
                    "[{}] {}",
                    customer.id.map(|id| id.to_string()).unwrap_or_default(),
                    customer.display_name()
                )
            })
            .collect();
        let Some(index) = tui::select("Select customer", &choices, 0) else {
            return Ok(None);
        };
        index
    };
    match customers[selected].id.filter(|id| *id != 0) {
        Some(id) => Ok(Some(id.to_string())),
        None => {
            fmt::error("Customer ID not available");
            Ok(None)
        }
    }
}
