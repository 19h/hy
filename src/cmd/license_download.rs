//! License filtering, interactive selection, and per-asset download reporting.

use super::license::{LicenseGetArgs, customer_id};
use super::license_list::selection_label;
use crate::api::{ApiClient, License};
use crate::error::Result;
use crate::util::files::absolute_path;
use crate::util::{fmt, tui};

pub(super) async fn run(args: LicenseGetArgs) -> Result<()> {
    let client = ApiClient::new()?;
    let Some(customer) = customer_id(&client, args.customer_id.clone()).await? else {
        return Ok(());
    };
    let licenses = client.licenses(&customer).await?;
    let mut matching: Vec<&License> =
        licenses.iter().filter(|license| matches(license, &args)).collect();
    if !args.all && matching.len() > 1 {
        // The upstream picker groups known catalogues and starts with nothing checked.
        matching = ["legacy", "subscription"]
            .into_iter()
            .flat_map(|catalogue| {
                licenses
                    .iter()
                    .filter(move |license| license.product_catalog.as_deref() == Some(catalogue))
            })
            .filter(|license| matches(license, &args))
            .collect();
        let labels = matching.iter().map(|license| selection_label(license)).collect::<Vec<_>>();
        let selected = tui::multi_select("Select licenses", &labels).unwrap_or_default();
        matching = selected.into_iter().map(|index| matching[index]).collect();
    }
    if matching.is_empty() {
        fmt::warning("No licenses found matching criteria");
        return Ok(());
    }
    let output = absolute_path(&args.output_dir)?;
    std::fs::create_dir_all(&output)?;
    fmt::info(&format!("Downloading {} license(s) to {}", matching.len(), output.display()));
    for license in matching {
        let types = license.asset_types.as_deref().unwrap_or_default();
        if types.is_empty() {
            fmt::warning("This license has no assets to download.");
        }
        for asset_type in types {
            let Some(key) = license.license_key.as_deref().filter(|key| !key.is_empty()) else {
                fmt::error(&format!("License has no key for asset {asset_type}"));
                continue;
            };
            match client.download_license(&customer, key, asset_type, &output).await {
                Ok(Some(path)) => fmt::success(&format!(
                    "License {asset_type} for {} downloaded as: {}",
                    license.pubhash.as_deref().unwrap_or("None"),
                    path.display()
                )),
                Ok(None) => fmt::error("Failed to download license"),
                Err(error) => fmt::error(&format!("Error downloading license: {error}")),
            }
        }
    }
    // Individual asset failures are reported and do not abort subsequent downloads.
    fmt::success("Download completed");
    Ok(())
}

fn matches(license: &License, args: &LicenseGetArgs) -> bool {
    license.status.as_deref() == Some("active")
        && args
            .id
            .as_deref()
            .filter(|id| !id.is_empty())
            .is_none_or(|id| license.pubhash.as_deref() == Some(id))
        && args
            .product_type
            .as_deref()
            .filter(|kind| !kind.is_empty())
            .is_none_or(|kind| license.product_code.as_deref() == Some(kind))
        && args.plan.is_none_or(|plan| license.product_catalog.as_deref() == Some(plan.as_str()))
}
