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
    repo: Option<String>,
    components: Option<Vec<ComponentInfo>>,
}

#[derive(Serialize)]
struct ComponentInfo {
    name: String,
    version: String,
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
    fn print_text(&self, show_components: bool, default: &str) {
        if self.plugins.is_empty() {
            println!("No plugins installed.");
        }
        let mut entries: Vec<_> = self.plugins.iter().collect();
        entries.sort_by_key(|entry| {
            let repo = match entry {
                StatusEntry::Installed(plugin) => plugin.repo.as_deref(),
                _ => None,
            };
            super::plugin_listing::repository_order(repo)
        });
        let mut group = None;
        for entry in entries {
            let repo = match entry {
                StatusEntry::Installed(plugin) => plugin.repo.as_deref(),
                _ => None,
            };
            if group.is_some() && group != Some(repo) {
                println!();
            }
            group = Some(repo);
            match entry {
                StatusEntry::Installed(plugin) => {
                    let reference = super::plugin_listing::install_reference(
                        &plugin.name,
                        plugin.repo.as_deref(),
                        default,
                    );
                    println!(
                        "{}\t{}\t{}",
                        reference,
                        plugin.version,
                        plugin.upgradable_to.as_deref().unwrap_or("")
                    );
                    if show_components {
                        for component in plugin.components.iter().flatten() {
                            println!("    {}\t{}", component.name, component.version);
                        }
                    }
                }
                StatusEntry::Legacy(plugin) => println!(
                    "{}\t{}\t{}",
                    plugin.name,
                    plugin.version.as_deref().unwrap_or("unknown"),
                    plugin.kind
                ),
                StatusEntry::Missing(plugin) => println!("{}\tnot installed", plugin.name),
            }
        }
    }
}

fn installed_status(
    metadata: plugin::PluginMetadata,
    repositories: &plugin::repositories::Repositories,
    ida_version: Option<&str>,
    skip_upgrade_check: bool,
) -> InstalledStatus {
    let reference = index::Reference {
        name: metadata.name.clone(),
        spec: String::new(),
        host: index::normalize_host(&metadata.urls.repository).ok(),
        repo: None,
    };
    let latest = index::select(&repositories.snapshot, &reference, ida_version).ok();
    let repo = latest.and_then(|location| repositories.name_of(location)).map(str::to_owned);
    let components = (!metadata.components.is_empty()).then(|| {
        plugin::components::metadata_tree(&metadata)
            .into_iter()
            .skip(1)
            .map(|component| ComponentInfo {
                name: component.name.clone(),
                version: component.version.clone(),
            })
            .collect()
    });
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
        repo,
        components,
    }
}

pub async fn run(args: PluginStatusArgs, context: &plugin::PluginContext) -> Result<()> {
    let installed = plugin::installed_plugins()?;
    let repositories = if args.offline || installed.is_empty() {
        plugin::repositories::Repositories::empty()
    } else {
        crate::ida::current_ida_platform()?;
        plugin::repositories::Repositories::load(context, &std::collections::HashMap::new(), None)
            .await?
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
        for reference in &args.names {
            let parsed = index::parse_reference(reference)?;
            let name = &parsed.name;
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
    report.plugins.sort_by(|left, right| match (left, right) {
        (StatusEntry::Installed(left), StatusEntry::Installed(right)) => {
            super::plugin_listing::repository_order(left.repo.as_deref())
                .cmp(&super::plugin_listing::repository_order(right.repo.as_deref()))
                .then_with(|| left.name.to_lowercase().cmp(&right.name.to_lowercase()))
        }
        (StatusEntry::Installed(_), _) => std::cmp::Ordering::Less,
        (_, StatusEntry::Installed(_)) => std::cmp::Ordering::Greater,
        _ => std::cmp::Ordering::Equal,
    });
    if args.json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        let (_, default) = index::repositories()?;
        report.print_text(args.show_components, &default);
    }
    if missing {
        Err(Error::ChildExit(1))
    } else {
        Ok(())
    }
}
