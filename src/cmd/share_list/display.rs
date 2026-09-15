//! Shared-file table columns and date/name formatting.

use crate::api::Asset;
pub(super) use crate::cmd::share_display::size;
use crate::error::Result;
use crate::util::tui;

pub(super) fn table(files: &[Asset]) -> Result<()> {
    let mut table =
        tui::Table::new(&["Index", "Code", "Name", "Version", "Size", "Created", "ACL"]);
    for (index, file) in files.iter().enumerate() {
        let acl = file
            .metadata
            .as_ref()
            .and_then(|metadata| metadata.get("acl_type"))
            .and_then(serde_json::Value::as_str)
            .unwrap_or("");
        table.add_row(vec![
            (index + 1).to_string(),
            file.code.as_deref().unwrap_or("-").into(),
            display_name(&file.filename),
            format!("v{}", file.version),
            size(file)?,
            file.created_at.as_deref().map(format_created_at).unwrap_or_else(|| "N/A".into()),
            acl.into(),
        ]);
    }
    table.print();
    Ok(())
}

fn format_created_at(value: &str) -> String {
    crate::util::python_datetime::parse(&value.replace('Z', "+00:00"))
        .map(|date| date.local.format("%Y-%m-%d %H:%M:%S").to_string())
        .unwrap_or_else(|| value.into())
}

fn display_name(filename: &str) -> String {
    let name = if filename.is_empty() {
        "unnamed"
    } else {
        filename
    };
    if name.chars().count() > 30 {
        format!("{}...", name.chars().take(27).collect::<String>())
    } else {
        name.into()
    }
}

#[cfg(test)]
mod tests;
