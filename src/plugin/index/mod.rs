//! Plugin repository models, loading, and compatible-version selection.

use std::path::{Path, PathBuf};

use crate::error::{Error, Result};

mod archive;
mod catalogue;
mod config;
pub mod github;
mod models;
mod reference;
mod selection;
mod snapshot;
mod transport;

#[cfg(all(test, unix))]
mod loading_tests;

pub(crate) use archive::add_bytes as index_archive;
pub(crate) use catalogue::ArchiveCatalogue;
pub use config::{RESERVED, Repository, repositories, save_repositories, valid_repo_name};
pub use models::{Location, Plugin, Snapshot};
pub(crate) use reference::is_direct_github;
pub use reference::{Reference, normalize_host, parse_reference};
pub use selection::{matches_reference, matching_plugins, select, select_for_platform};
pub use transport::{fetch, github_archive};

pub struct LoadedRepository {
    pub snapshot: Snapshot,
    pub notes: Vec<String>,
    bundle_reader: Option<crate::plugin::bundle::BundleReader>,
}

impl LoadedRepository {
    pub(crate) fn bundle_reader(&self) -> Option<&crate::plugin::bundle::BundleReader> {
        self.bundle_reader.as_ref()
    }

    fn empty() -> Self {
        Self {
            snapshot: Snapshot {
                version: 1,
                plugins: Vec::new(),
            },
            notes: Vec::new(),
            bundle_reader: None,
        }
    }

    pub async fn fetch_verified(&self, location: &Location) -> Result<Vec<u8>> {
        let url = location.url.to_utf8()?;
        let bytes = match (&self.bundle_reader, url.strip_prefix("hcli-bundle:")) {
            (Some(reader), Some(member)) => reader.read(member)?,
            _ => fetch(&url).await?,
        };
        transport::verify_checksum(location, &bytes)?;
        Ok(bytes)
    }
}

/// Named repositories other than hexrays cannot publish Hex-Rays plugin identities.
pub async fn load_named(
    name: Option<&str>,
    source: &str,
    offline: bool,
) -> Result<LoadedRepository> {
    let mut repository = load(source, offline).await.map_err(|error| match error {
        Error::PluginAccessDenied(mut denied) => {
            denied.repository = name.map(str::to_owned);
            Error::PluginAccessDenied(denied)
        }
        other => other,
    })?;
    if let Some(name) = name.filter(|name| *name != "hexrays") {
        let before = repository.snapshot.plugins.len();
        repository.snapshot.plugins.retain(|plugin| {
            !url::Url::parse(&plugin.host)
                .ok()
                .is_some_and(|url| url.host_str() == Some("plugins.hex-rays.com"))
        });
        let dropped = before - repository.snapshot.plugins.len();
        if dropped > 0 {
            repository
                .notes
                .push(format!("{name}: ignored {dropped} plugin(s) claiming Hex-Rays identities"));
        }
    }
    Ok(repository)
}

pub async fn load(source: &str, offline: bool) -> Result<LoadedRepository> {
    let local = if let Some(path) = transport::local_file_path(source)? {
        Some(path)
    } else if Path::new(source).exists() {
        Some(PathBuf::from(source))
    } else {
        None
    };
    let mut loaded = LoadedRepository::empty();
    if let Some(path) = local {
        if path.is_dir() {
            let mut catalogue = ArchiveCatalogue::default();
            archive::add_directory(&mut catalogue, &path)?;
            loaded.snapshot.plugins = catalogue.into_plugins()?;
        } else if crate::plugin::bundle::is_plugin_bundle_zip(&path) {
            let reader = crate::plugin::bundle::BundleReader::open(&path)?;
            loaded.snapshot.plugins = reader.plugins()?;
            loaded.bundle_reader = Some(reader);
        } else {
            loaded.snapshot = serde_json::from_slice(&std::fs::read(path)?)?;
        }
    } else {
        if offline {
            return Err(Error::Other(format!("offline: remote repository unavailable: {source}")));
        }
        loaded.snapshot = serde_json::from_slice(&fetch(source).await?)?;
    }
    Ok(loaded)
}
#[cfg(test)]
mod tests {
    use super::config::repositories_from_config;
    use super::transport::credential_host;
    use super::*;
    #[test]
    fn reference_scopes_and_versions() {
        let r = parse_reference("private/foo>=1.10@https://GitHub.com/Org/Repo/").unwrap();
        assert_eq!(r.repo.as_deref(), Some("private"));
        assert_eq!(r.host.as_deref(), Some("https://github.com/org/repo"));
        assert_eq!(r.spec, ">=1.10");
        for invalid in ["../x", "x=1", "x@invalid", "https://github.com/org/repo", ""] {
            assert!(parse_reference(invalid).is_err(), "{invalid}");
        }
    }

    #[test]
    fn identity_normalization_preserves_scheme_port_and_one_trailing_slash() {
        for (input, expected) in [
            ("https://GitHub.com/Org/Repo/", "https://github.com/org/repo"),
            ("https://example.test:443/Repo", "https://example.test:443/repo"),
            ("http://example.test/Repo//", "http://example.test/repo/"),
        ] {
            assert_eq!(normalize_host(input).unwrap(), expected);
        }
    }
    #[test]
    fn credential_scope_is_exact_and_https_only() {
        for host in ["https://plugins.hex-rays.com/a", "https://hexrays.plugins.hex-rays.com/a"] {
            assert!(credential_host(host).unwrap());
        }
        for host in [
            "http://plugins.hex-rays.com",
            "https://evilplugins.hex-rays.com",
            "https://plugins.hex-rays.com.evil.test",
            "https://github.com",
        ] {
            assert!(!credential_host(host).unwrap());
        }
    }
    #[test]
    fn reserved_urls_and_removed_repositories() {
        let (repos, _) = repositories_from_config(&serde_json::json!({
            "Settings": {
                "plugin-repositories": {
                    "hexrays": {"url": "https://evil.test"},
                },
                "default-plugin-repository": "hexrays",
            },
        }));
        assert_eq!(repos.len(), 1);
        assert_eq!(repos["hexrays"].url, RESERVED[1].1);
        let (repos, default) = repositories_from_config(
            &serde_json::json!({"Settings":{"plugin-repository":{"url":"file:///tmp/repo.json"}}}),
        );
        assert_eq!(default, "custom");
        assert_eq!(repos.len(), 3);
    }
}
