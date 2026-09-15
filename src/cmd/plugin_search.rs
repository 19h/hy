//! Repository search contracts and presentation.

use serde::Serialize;
use serde_json::Value;

use super::plugin_cmd::PluginSearchArgs;
use crate::error::{Error, Result};
use crate::plugin::{self, PluginMetadata, index};

#[derive(Serialize)]
struct KeywordMatch {
    name: String,
    version: String,
    repository: Option<String>,
    repo: Option<String>,
    compatible: bool,
    installed: bool,
    installed_version: Option<String>,
    upgradable: bool,
}

#[derive(Serialize)]
struct KeywordReport {
    query: Option<String>,
    results: Vec<KeywordMatch>,
    repository_notes: Vec<String>,
}

#[derive(Serialize)]
struct VersionEntry {
    version: String,
    compatible: bool,
    currently_installed: bool,
    upgradable: bool,
}

#[derive(Serialize)]
struct VersionsReport {
    plugin: Value,
    installed_version: Option<String>,
    versions: Vec<VersionEntry>,
}

#[derive(Serialize)]
struct DownloadLocation {
    ida_versions: String,
    platforms: String,
    url: String,
}

#[derive(Serialize)]
struct ExactVersionReport {
    plugin: Value,
    download_locations: Vec<DownloadLocation>,
}

#[derive(Serialize)]
#[serde(untagged)]
enum PluginQueryReport {
    Versions(VersionsReport),
    Exact(ExactVersionReport),
}

impl PluginQueryReport {
    fn print_text(&self) {
        let metadata = match self {
            Self::Versions(report) => &report.plugin,
            Self::Exact(report) => &report.plugin,
        };
        if let Some(fields) = metadata.as_object() {
            for (key, value) in fields {
                println!("{key}: {value}");
            }
        }
        match self {
            Self::Versions(report) => {
                println!("\navailable versions:");
                for version in &report.versions {
                    let status = if version.currently_installed {
                        "currently installed"
                    } else if version.upgradable {
                        "upgradable"
                    } else if !version.compatible && report.installed_version.is_none() {
                        "incompatible"
                    } else {
                        ""
                    };
                    println!("{}\t{status}", version.version);
                }
            }
            Self::Exact(report) => {
                println!("\ndownload locations:");
                for location in &report.download_locations {
                    println!(
                        "IDA: {}\tplatforms: {}\t{}",
                        location.ida_versions, location.platforms, location.url
                    );
                }
            }
        }
    }
}

#[derive(Serialize)]
struct AmbiguityReport {
    error: &'static str,
    name: String,
    candidates: Vec<String>,
}

struct Catalogue {
    repositories: Vec<(Option<String>, index::LoadedRepository)>,
    notes: Vec<String>,
    installed: Vec<PluginMetadata>,
    ida_version: Option<String>,
    platform: String,
}

impl Catalogue {
    async fn load(context: &plugin::PluginContext, offline: bool) -> Result<Self> {
        let platform = crate::ida::current_ida_platform()?;
        let configured = context.repositories()?;
        let mut repositories = Vec::new();
        let mut notes = Vec::new();
        for (name, repository) in configured {
            match context.load_repository(name.as_deref(), &repository.url, offline).await {
                Ok(repository) => {
                    notes.extend(repository.notes.iter().cloned());
                    repositories.push((name, repository));
                }
                Err(error) => {
                    if context.repository.is_some() {
                        return Err(error);
                    }
                    notes.push(format!("{}: {error}", name.as_deref().unwrap_or("repository")))
                }
            }
        }
        let installed =
            plugin::installed_plugins()?.into_iter().map(|entry| entry.metadata).collect();
        let ida_version = crate::config::Env::global()
            .current_ida_version
            .clone()
            .or_else(plugin::detect_current_ida_version);
        Ok(Self {
            repositories,
            notes,
            installed,
            ida_version,
            platform,
        })
    }

    fn plugins(&self) -> impl Iterator<Item = (Option<&str>, &index::Plugin)> {
        self.repositories.iter().flat_map(|(name, repository)| {
            repository.snapshot.plugins.iter().map(move |plugin| (name.as_deref(), plugin))
        })
    }

    fn compatible(&self, metadata: &PluginMetadata) -> bool {
        plugin::is_platform_compatible(metadata, &self.platform)
            && self
                .ida_version
                .as_deref()
                .is_none_or(|version| plugin::is_ida_version_compatible(metadata, version))
    }

    fn installed_version(&self, candidate: &index::Plugin) -> Option<&str> {
        let candidate_host = index::normalize_host(&candidate.host).ok();
        self.installed
            .iter()
            .find(|installed| {
                let installed_host = index::normalize_host(&installed.urls.repository).ok();
                installed.name.eq_ignore_ascii_case(&candidate.name)
                    && installed_host == candidate_host
            })
            .map(|installed| installed.version.as_str())
    }
}

fn ordered_versions(plugin: &index::Plugin) -> Vec<(&str, &[index::Location])> {
    let mut versions: Vec<_> = plugin
        .versions
        .iter()
        .filter(|(_, locations)| !locations.is_empty())
        .map(|(version, locations)| (version.as_str(), locations.as_slice()))
        .collect();
    versions.sort_by_key(|(version, _)| std::cmp::Reverse(plugin::version_precedence(version)));
    versions
}

fn matches_keyword(plugin: &index::Plugin, query: &str) -> bool {
    if plugin.name.to_lowercase().contains(query) {
        return true;
    }
    plugin.versions.values().flatten().any(|location| {
        let metadata = &location.descriptor.metadata;
        metadata.description.as_deref().unwrap_or_default().to_lowercase().contains(query)
            || metadata
                .categories
                .iter()
                .chain(metadata.keywords.iter())
                .any(|value| value.to_lowercase().contains(query))
            || metadata
                .authors
                .iter()
                .chain(metadata.maintainers.iter())
                .filter_map(|contact| contact.name.as_deref())
                .any(|name| name.to_lowercase().contains(query))
    })
}

fn keyword_report(catalogue: &Catalogue, query: Option<String>) -> KeywordReport {
    let keyword = query.as_deref().unwrap_or("").to_lowercase();
    let mut results = Vec::new();
    for (repository, candidate) in catalogue.plugins() {
        if !matches_keyword(candidate, &keyword) {
            continue;
        }
        let versions = ordered_versions(candidate);
        let Some((version, locations)) = versions.first() else {
            continue;
        };
        let latest_compatible = versions.iter().find(|(_, locations)| {
            locations.iter().any(|location| catalogue.compatible(&location.descriptor.metadata))
        });
        let installed = latest_compatible.and_then(|_| catalogue.installed_version(candidate));
        let upgradable =
            installed.zip(latest_compatible).is_some_and(|(installed, (version, _))| {
                plugin::version_precedence(version) > plugin::version_precedence(installed)
            });
        results.push(KeywordMatch {
            name: candidate.name.clone(),
            version: (*version).to_owned(),
            repository: Some(locations[0].descriptor.metadata.urls.repository.clone()),
            repo: repository.map(str::to_owned),
            compatible: latest_compatible.is_some(),
            installed: installed.is_some(),
            installed_version: installed.map(str::to_owned),
            upgradable,
        });
    }
    results.sort_by_key(|entry| entry.name.to_lowercase());
    KeywordReport {
        query,
        results,
        repository_notes: catalogue.notes.clone(),
    }
}

fn display_ida_versions(versions: &[String]) -> String {
    if plugin::all_ida_versions().iter().all(|version| versions.contains(version)) {
        return "all".to_owned();
    }
    if versions.is_empty() {
        return "none".to_owned();
    }
    let mut versions = versions.to_vec();
    versions.sort_by_key(|version| plugin::parse_ida_version(version));
    if versions.len() == 1 {
        versions[0].clone()
    } else {
        format!("{}-{}", versions[0], versions[versions.len() - 1])
    }
}

fn report_metadata(metadata: &PluginMetadata) -> Result<Value> {
    // Upstream intentionally exposes extensible metadata here.
    let mut value = serde_json::to_value(metadata)?;
    if let Some(object) = value.as_object_mut() {
        object.remove("platforms");
        object.insert(
            "idaVersions".into(),
            Value::String(display_ida_versions(&metadata.ida_versions)),
        );
    }
    Ok(value)
}

fn version_report(
    catalogue: &Catalogue,
    candidate: &index::Plugin,
    spec: &str,
) -> Result<VersionsReport> {
    let versions = ordered_versions(candidate);
    let (_, latest) =
        versions.first().ok_or_else(|| Error::NotFound("plugin has no versions".into()))?;
    let installed = catalogue.installed_version(candidate);
    let entries = versions
        .iter()
        .filter(|(version, _)| plugin::version_matches(version, spec))
        .map(|(version, locations)| {
            let compatible = locations
                .iter()
                .any(|location| catalogue.compatible(&location.descriptor.metadata));
            VersionEntry {
                version: (*version).to_owned(),
                compatible,
                currently_installed: installed.is_some_and(|installed| {
                    plugin::parse_version(installed) == plugin::parse_version(version)
                }),
                upgradable: compatible
                    && installed.is_some_and(|installed| {
                        plugin::version_precedence(installed) < plugin::version_precedence(version)
                    }),
            }
        })
        .collect();
    Ok(VersionsReport {
        plugin: report_metadata(&latest[0].descriptor.metadata)?,
        installed_version: installed.map(str::to_owned),
        versions: entries,
    })
}

fn exact_report(candidate: &index::Plugin, version: &str) -> Result<ExactVersionReport> {
    let locations =
        candidate.versions.get(version).filter(|locations| !locations.is_empty()).ok_or_else(
            || Error::NotFound(format!("version {version} for plugin {}", candidate.name)),
        )?;
    let download_locations = locations
        .iter()
        .map(|location| {
            let mut platforms = location.descriptor.metadata.platforms.clone();
            platforms.sort();
            DownloadLocation {
                ida_versions: display_ida_versions(&location.descriptor.metadata.ida_versions),
                platforms: if plugin::all_platforms()
                    .iter()
                    .all(|platform| platforms.contains(platform))
                {
                    "all".into()
                } else {
                    platforms.join(", ")
                },
                url: location.url.clone(),
            }
        })
        .collect();
    Ok(ExactVersionReport {
        plugin: report_metadata(&locations[0].descriptor.metadata)?,
        download_locations,
    })
}

pub async fn run(args: PluginSearchArgs, context: &plugin::PluginContext) -> Result<()> {
    let catalogue = Catalogue::load(context, args.offline).await?;
    let reference =
        args.query.as_deref().and_then(|query| index::parse_reference(query).ok()).filter(
            |reference| {
                reference.host.is_some()
                    || catalogue
                        .plugins()
                        .any(|(_, plugin)| plugin.name.eq_ignore_ascii_case(&reference.name))
            },
        );
    let Some(reference) = reference else {
        let report = keyword_report(&catalogue, args.query);
        if args.json {
            println!("{}", serde_json::to_string_pretty(&report)?);
        } else {
            for result in report.results {
                println!(
                    "{}/{}\t{}\t{}",
                    result.repo.as_deref().unwrap_or(""),
                    result.name,
                    result.version,
                    result.repository.as_deref().unwrap_or("")
                );
            }
            for note in report.repository_notes {
                eprintln!("repository {note}");
            }
        }
        return Ok(());
    };
    let candidates: Vec<_> = catalogue
        .plugins()
        .filter(|(repository, plugin)| {
            plugin.name.eq_ignore_ascii_case(&reference.name)
                && reference.repo.as_deref().is_none_or(|name| Some(name) == *repository)
                && reference.host.as_ref().is_none_or(|host| {
                    index::normalize_host(&plugin.host).ok().as_ref() == Some(host)
                })
        })
        .collect();
    if candidates.len() > 1 {
        let report = AmbiguityReport {
            error: "ambiguous plugin reference",
            name: reference.name,
            candidates: candidates
                .iter()
                .map(|(repo, plugin)| {
                    let prefix = repo.map(|repo| format!("{repo}/")).unwrap_or_default();
                    format!("{prefix}{}{}@{}", plugin.name, reference.spec, plugin.host)
                })
                .collect(),
        };
        if args.json {
            println!("{}", serde_json::to_string_pretty(&report)?);
        } else {
            eprintln!("{}: {}", report.error, report.name);
            for candidate in report.candidates {
                eprintln!("  {candidate}");
            }
        }
        return Err(Error::ChildExit(1));
    }
    let result = match candidates.first() {
        None => Err(Error::NotFound(reference.name)),
        Some((_, candidate)) => {
            match reference.spec.strip_prefix("==").filter(|spec| !spec.contains(',')) {
                Some(version) => exact_report(candidate, version).map(PluginQueryReport::Exact),
                None => version_report(&catalogue, candidate, &reference.spec)
                    .map(PluginQueryReport::Versions),
            }
        }
    };
    match result {
        Ok(report) => {
            if args.json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                report.print_text();
            }
            Ok(())
        }
        Err(error) if args.json => {
            println!(
                "{}",
                serde_json::to_string(&serde_json::json!({"error": error.to_string()}))?
            );
            Err(Error::ChildExit(1))
        }
        Err(error) => Err(error),
    }
}
