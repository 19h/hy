//! Collect installed-plugin status independently from its text or JSON presentation.

use serde::Serialize;

use super::plugin_cmd::PluginStatusArgs;
use crate::error::{Error, Result};
use crate::plugin::{self, index};

#[derive(Serialize)]
struct StatusReport {
    plugins: Vec<StatusEntry>,
}

#[derive(Serialize)]
#[serde(untagged)]
enum StatusEntry {
    Installed(InstalledStatus),
    Legacy(LegacyStatus),
    Missing(MissingStatus),
}

#[derive(Serialize)]
struct InstalledStatus {
    name: String,
    version: String,
    installed: bool,
    kind: &'static str,
    upgrade_checked: bool,
    in_repository: Option<bool>,
    upgradable_to: Option<String>,
}

#[derive(Serialize)]
struct LegacyStatus {
    name: String,
    version: Option<String>,
    installed: bool,
    kind: &'static str,
    path: String,
}

#[derive(Serialize)]
struct MissingStatus {
    name: String,
    installed: bool,
}

impl StatusReport {
    fn print_text(&self) {
        if self.plugins.is_empty() {
            println!("No plugins installed.");
        }
        for entry in &self.plugins {
            match entry {
                StatusEntry::Installed(plugin) => println!(
                    "{}\t{}\t{}",
                    plugin.name,
                    plugin.version,
                    plugin.upgradable_to.as_deref().unwrap_or(""),
                ),
                StatusEntry::Legacy(plugin) => println!(
                    "{}\t{}\t{}",
                    plugin.name,
                    plugin.version.as_deref().unwrap_or("unknown"),
                    plugin.kind,
                ),
                StatusEntry::Missing(plugin) => println!("{}\tnot installed", plugin.name),
            }
        }
    }
}

async fn load_repositories(
    context: &plugin::PluginContext,
) -> Result<Vec<index::LoadedRepository>> {
    let repositories = context.repositories()?;
    let mut loaded = Vec::new();
    for (name, repository) in repositories {
        match context.load_repository(name.as_deref(), &repository.url, false).await {
            Ok(repository) => loaded.push(repository),
            Err(error) if context.repository.is_some() => return Err(error),
            Err(error) => tracing::debug!(repository = ?name, %error, "repository unavailable"),
        }
    }
    Ok(loaded)
}

fn installed_status(
    metadata: plugin::PluginMetadata,
    repositories: &[index::LoadedRepository],
    ida_version: Option<&str>,
    skip_upgrade_check: bool,
) -> InstalledStatus {
    let reference = index::Reference {
        name: metadata.name.clone(),
        spec: String::new(),
        host: index::normalize_host(&metadata.urls.repository).ok(),
        repo: None,
    };
    let mut matches = repositories.iter().flat_map(|repository| {
        repository
            .snapshot
            .plugins
            .iter()
            .filter(|plugin| index::matches_reference(plugin, &reference))
            .map(move |_| repository)
    });
    let first = matches.next();
    // An identity appearing more than once is ambiguous even when both records
    // advertise the same host. Status must agree with upgrade resolution.
    let latest = if matches.next().is_none() {
        first.and_then(|repository| {
            index::select(&repository.snapshot, &reference, ida_version).ok()
        })
    } else {
        None
    };
    let upgradable_to = latest
        .filter(|location| {
            plugin::version_precedence(&location.descriptor.metadata.version)
                > plugin::version_precedence(&metadata.version)
        })
        .map(|location| location.descriptor.metadata.version.clone());
    InstalledStatus {
        name: metadata.name,
        version: metadata.version,
        installed: true,
        kind: "installed",
        upgrade_checked: !skip_upgrade_check,
        in_repository: (!skip_upgrade_check).then_some(latest.is_some()),
        upgradable_to,
    }
}

pub async fn run(args: PluginStatusArgs, context: &plugin::PluginContext) -> Result<()> {
    let installed = plugin::installed_plugins()?;
    let repositories = if args.offline || installed.is_empty() {
        Vec::new()
    } else {
        crate::ida::current_ida_platform()?;
        load_repositories(context).await?
    };
    let ida_version = crate::config::Env::global()
        .current_ida_version
        .clone()
        .or_else(plugin::detect_current_ida_version);
    let mut report = StatusReport {
        plugins: Vec::new(),
    };
    let mut missing = false;
    if args.names.is_empty() {
        for entry in &installed {
            report.plugins.push(StatusEntry::Installed(installed_status(
                entry.metadata.clone(),
                &repositories,
                ida_version.as_deref(),
                args.offline,
            )));
        }
        for entry in plugin::unmanaged_plugins()? {
            report.plugins.push(StatusEntry::Legacy(LegacyStatus {
                name: entry.name,
                version: entry.version,
                installed: true,
                kind: match entry.kind {
                    plugin::UnmanagedKind::Incompatible => "incompatible",
                    plugin::UnmanagedKind::Legacy => "legacy",
                },
                path: entry.path,
            }));
        }
    } else {
        for name in &args.names {
            if let Some(entry) =
                installed.iter().find(|entry| entry.metadata.name.eq_ignore_ascii_case(name))
            {
                report.plugins.push(StatusEntry::Installed(installed_status(
                    entry.metadata.clone(),
                    &repositories,
                    ida_version.as_deref(),
                    args.offline,
                )));
            } else {
                missing = true;
                report.plugins.push(StatusEntry::Missing(MissingStatus {
                    name: name.clone(),
                    installed: false,
                }));
            }
        }
    }
    if args.json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        report.print_text();
    }
    if missing {
        Err(Error::ChildExit(1))
    } else {
        Ok(())
    }
}
