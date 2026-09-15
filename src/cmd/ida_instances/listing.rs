//! Version-ordered inventory and diagnostics for registered IDA installations.

use std::path::Path;

use super::{Instances, Version};
use crate::config::Env;
use crate::ida;
use crate::util::{fmt, tui};

#[derive(Eq, PartialEq)]
enum Status {
    Valid,
    Invalid,
    Missing,
}

impl Status {
    fn detect(path: &Path) -> Self {
        if !path.exists() {
            Self::Missing
        } else if ida::ida_binary_path(path).is_some() {
            Self::Valid
        } else {
            Self::Invalid
        }
    }

    fn label(&self) -> &'static str {
        match self {
            Self::Valid => "Valid",
            Self::Invalid => "Invalid",
            Self::Missing => "Missing",
        }
    }
}

struct Row<'a> {
    name: &'a str,
    path: &'a str,
    version: Version,
    status: Status,
}

pub(super) fn print(instances: &Instances, default: &str) {
    let mut rows: Vec<_> = instances
        .iter()
        .map(|(name, path)| Row {
            name,
            path,
            version: Version::detect(name, path).unwrap_or_default(),
            status: Status::detect(Path::new(path)),
        })
        .collect();
    // Listing uses ascending names for ties; default selection uses descending names.
    rows.sort_by(|left, right| {
        right.version.cmp(&left.version).then_with(|| left.name.cmp(right.name))
    });

    let mut table = tui::Table::new(&["Name", "Path", "Status"]);
    for row in &rows {
        let name = if row.name == default {
            format!("{} (default)", row.name)
        } else {
            row.name.to_owned()
        };
        table.add_row(vec![name, row.path.into(), row.status.label().into()]);
    }
    table.print();
    let valid = rows.iter().filter(|row| row.status == Status::Valid).count();
    eprintln!("Summary: {valid}/{} instances are valid", rows.len());
    print_default_status(instances, default, &rows);
}

fn print_default_status(instances: &Instances, default: &str, rows: &[Row<'_>]) {
    let binary = &Env::global().binary_name;
    if default.is_empty() {
        fmt::warning(&format!("No default instance set. Use '{binary} ida switch' to set one."));
    } else if !instances.contains_key(default) {
        fmt::warning(&format!("Default instance '{default}' no longer exists!"));
    } else {
        eprintln!("Default instance: {default}");
        if let Some(latest) = rows.iter().find(|row| row.status == Status::Valid)
            && latest.name != default
        {
            fmt::warning(&format!(
                "Latest IDA version is not the default. Use '{binary} ida switch {}' to update it.",
                latest.name
            ));
        }
    }
}
