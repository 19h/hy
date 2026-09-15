//! Grouped license tables and selection labels.

use super::license::{LicenseListArgs, customer_id};
use crate::api::{ApiClient, License};
use crate::error::Result;
use crate::util::{fmt, tui};

mod expiration;

pub(super) async fn run(args: LicenseListArgs) -> Result<()> {
    let client = ApiClient::new()?;
    let Some(customer) = customer_id(&client, args.customer_id).await? else {
        return Ok(());
    };
    let licenses = client.licenses(&customer).await?;
    let matching: Vec<&License> = licenses
        .iter()
        .filter(|license| {
            args.plan.is_none_or(|plan| license.product_catalog.as_deref() == Some(plan.as_str()))
        })
        .collect();
    if matching.is_empty() {
        fmt::warning("No licenses found");
        return Ok(());
    }
    eprintln!("Licenses for customer [{customer}]:");
    for (catalogue, title) in
        [("legacy", "Perpetual Licenses"), ("subscription", "Subscription Licenses")]
    {
        let group: Vec<_> = matching
            .iter()
            .copied()
            .filter(|license| license.product_catalog.as_deref() == Some(catalogue))
            .collect();
        if !group.is_empty() {
            eprintln!("\n{title} ({}):", group.len());
            display_table(&group);
        }
    }
    eprintln!("\nTotal: {} license(s)", matching.len());
    Ok(())
}

fn display_table(licenses: &[&License]) {
    let mut table = tui::Table::new(&["ID", "Edition", "Type", "Status", "Expiration", "Addons"]);
    for license in licenses {
        let status = license.status.as_deref().unwrap_or("Unknown");
        let mut characters = status.chars();
        let status = characters
            .next()
            .map(|first| first.to_uppercase().to_string() + characters.as_str())
            .unwrap_or_default();
        let (decompilers, other) = addons(license);
        let mut summary = Vec::new();
        if decompilers > 0 {
            summary.push(format!("{decompilers} decompiler(s)"));
        }
        if !other.is_empty() {
            summary.push(other.join(", "));
        }
        table.add_row(vec![
            license.pubhash.as_deref().unwrap_or("None").into(),
            edition(license).into(),
            license.license_type.as_deref().unwrap_or("None").into(),
            status,
            expiration::format(license.end_date.as_deref(), false),
            if summary.is_empty() {
                "None".into()
            } else {
                summary.join(" + ")
            },
        ]);
    }
    table.print();
}

pub(super) fn selection_label(license: &License) -> String {
    let (decompilers, other) = addons(license);
    let mut suffix = String::new();
    if decompilers > 0 {
        suffix.push_str(&format!(
            "{decompilers} decompiler{} ",
            if decompilers == 1 {
                ""
            } else {
                "s"
            }
        ));
    }
    if !other.is_empty() {
        suffix.push_str(&format!("[{}]", other.join(", ")));
    }
    format!(
        "{} {} [{}] {} {suffix}",
        license.pubhash.as_deref().unwrap_or("None"),
        edition(license),
        license.license_type.as_deref().unwrap_or("None"),
        expiration::format(license.end_date.as_deref(), true),
    )
    .trim_end()
    .into()
}

fn edition(license: &License) -> &str {
    license
        .edition
        .as_ref()
        .map_or("Unknown", |edition| edition.edition_name.as_deref().unwrap_or("None"))
}

fn addons(license: &License) -> (usize, Vec<&str>) {
    let mut decompilers = 0;
    let mut other = Vec::new();
    for product in license.addons.iter().flatten().filter_map(|addon| addon.product.as_ref()) {
        if product.product_subtype.as_deref() == Some("DECOMPILER") {
            decompilers += 1;
        } else {
            other.push(product.code.as_str());
        }
    }
    (decompilers, other)
}
