//! Shared file listing and interactive batch operations.

use super::share::ListArgs;
use crate::api::{ApiClient, Asset, PagedAssets};
use crate::error::Result;
use crate::util::fmt;

mod actions;
mod display;
mod prompt;

pub(super) async fn run(args: ListArgs) -> Result<()> {
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
    } else if args.no_interactive {
        display::table(&page.items)?;
    } else {
        manage_files(&client, &page.items).await?;
    }
    Ok(())
}

async fn manage_files(client: &ApiClient, files: &[Asset]) -> Result<()> {
    let choices: Vec<String> = files
        .iter()
        .map(|file| {
            let name = if file.filename.is_empty() {
                "unnamed"
            } else {
                &file.filename
            };
            Ok(format!(
                "{} ({}) - {}",
                name,
                file.code.as_deref().unwrap_or("None"),
                display::size(file)?
            ))
        })
        .collect::<Result<_>>()?;
    let indices = prompt::files(&choices, files)?;
    if indices.is_empty() {
        fmt::warning("No files selected.");
        return Ok(());
    }
    let selected: Vec<&Asset> = indices.into_iter().map(|index| &files[index]).collect();
    let count = selected.len();
    let suffix = if count == 1 {
        ""
    } else {
        "s"
    };
    let actions =
        vec![format!("Delete {count} file{suffix}"), format!("Download {count} file{suffix}")];
    if prompt::action(&actions)? == 0 {
        actions::delete(client, &selected).await;
    } else {
        actions::download(client, &selected).await?;
    }
    Ok(())
}
