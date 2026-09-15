//! Prepare a KE cache entry and strip download parameters before navigation.

use super::download::{Payload, download};
use super::navigation::{LaunchOptions, Target, navigate_to_database};
use super::transport::Transport;
use super::uri::ParsedLink;
use crate::error::{Error, Result};

mod cache;
mod dialogs;
mod paths;
mod query;

pub(super) async fn open(
    original: &str,
    parsed: &ParsedLink<'_>,
    options: LaunchOptions,
) -> Result<()> {
    let result = prepare(original, parsed, options.no_launch).await;
    if let Err(error) = &result {
        dialogs::error(&error.to_string()).await;
    }
    let Some(target) = result? else {
        return Ok(());
    };
    let result = navigate_to_database(target, options).await;
    if let Err(error @ Error::IdaLaunch(_)) = &result {
        dialogs::error(&error.to_string()).await;
    }
    result
}

async fn prepare(
    original: &str,
    parsed: &ParsedLink<'_>,
    no_launch: bool,
) -> Result<Option<Target>> {
    let query::Request {
        name,
        content,
        sha,
        navigation,
    } = query::Request::parse(original, parsed)?;
    let settings = &crate::config::Env::global().ke;
    let root = settings
        .downloads_dir
        .clone()
        .unwrap_or_else(|| dirs::home_dir().unwrap_or_default().join(".ke/downloads"));
    let path = root.join(&sha).join(&name);
    paths::validate(&root, &path)?;
    if !no_launch
        && !settings.skip_confirm
        && !dialogs::confirm(
            &format!("{name} ({})", &sha[..8]),
            content.host_str().unwrap_or("an unknown host"),
        )
        .await
    {
        crate::util::fmt::warning("Cancelled — nothing downloaded.");
        crate::util::fmt::info("Set HCLI_KE_SKIP_CONFIRM=1 to skip this prompt.");
        return Ok(None);
    }
    let client = Transport::for_url(&content).await?;
    cache::cleanup(&root, &settings.retention_days)?;
    std::fs::create_dir_all(root.join(&sha))?;
    let progress = dialogs::Progress::show(&name);
    let result = transfer(&client, &content, &path, &name, &sha, settings.max_download_bytes).await;
    if result.is_ok() {
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    }
    progress.dismiss().await;
    result?;
    if no_launch {
        println!("{}", path.display());
        return Ok(None);
    }
    Ok(Some(Target {
        uri: navigation,
        name,
        path: Some(path),
        exact_path_match: true,
    }))
}

async fn transfer(
    client: &Transport,
    content: &url::Url,
    path: &std::path::Path,
    name: &str,
    sha: &str,
    limit: u64,
) -> Result<()> {
    let mut metadata = content.clone();
    metadata.set_path(content.path().trim_end_matches('/').trim_end_matches("/content"));
    let sidecar = path.with_file_name(format!("{name}.ke.json"));
    if let Err(error) = download(&metadata, &sidecar, Payload::Metadata, client).await {
        crate::util::fmt::warning(&format!("KE metadata fetch failed: {error}"));
    }
    download(
        content,
        path,
        Payload::Content {
            sha256: sha,
            limit,
        },
        client,
    )
    .await
}
