//! One command's repository snapshot, with archive ownership retained after merging.

use std::collections::{BTreeMap, HashMap};

use super::{PluginContext, index};
use crate::error::{Error, Result};
use crate::util::python_json::Text;

pub struct Repositories {
    pub snapshot: index::Snapshot,
    sources: Vec<index::LoadedRepository>,
    names: Vec<Option<String>>,
    owners: BTreeMap<(Text, String), usize>,
    archives: tokio::sync::Mutex<BTreeMap<(Text, String), Vec<u8>>>,
}

impl Repositories {
    pub fn empty() -> Self {
        Self {
            snapshot: index::Snapshot {
                version: 1,
                plugins: Vec::new(),
            },
            sources: Vec::new(),
            names: Vec::new(),
            owners: BTreeMap::new(),
            archives: tokio::sync::Mutex::new(BTreeMap::new()),
        }
    }

    /// Root scopes restrict those names only; dependencies retain all repositories.
    pub async fn load(
        context: &PluginContext,
        scopes: &HashMap<String, String>,
        override_source: Option<&str>,
    ) -> Result<Self> {
        let configured = match override_source {
            Some(source) => vec![(
                None,
                index::Repository {
                    url: source.into(),
                },
            )],
            None => context.repositories()?,
        };
        if (override_source.is_some() || context.repository.is_some()) && !scopes.is_empty() {
            return Err(Error::PluginInstall(
                "repository prefixes cannot be combined with --repo".into(),
            ));
        }
        for scope in scopes.values() {
            if !configured.iter().any(|(name, _)| name.as_ref() == Some(scope)) {
                return Err(Error::NotFound(format!("plugin repository '{scope}'")));
            }
        }
        let mut result = Self::empty();
        for (name, repository) in configured {
            let loaded =
                match context.load_repository(name.as_deref(), &repository.url, false).await {
                    Ok(loaded) => loaded,
                    Err(error)
                        if override_source.is_some()
                            || context.repository.is_some()
                            || name
                                .as_ref()
                                .is_some_and(|name| scopes.values().any(|scope| scope == name)) =>
                    {
                        return Err(error);
                    }
                    Err(error) => {
                        crate::util::fmt::warning(&format!(
                            "skipping plugin repository {}: {error}",
                            name.as_deref().unwrap_or("repository")
                        ));
                        continue;
                    }
                };
            for note in &loaded.notes {
                crate::util::fmt::warning(note);
            }
            result.add(loaded, |plugin| {
                scopes
                    .get(&plugin.name.to_lowercase())
                    .is_none_or(|scope| Some(scope) == name.as_ref())
            })?;
            *result.names.last_mut().expect("added repository") = name;
        }
        Ok(result)
    }

    pub fn add(
        &mut self,
        source: index::LoadedRepository,
        include: impl Fn(&index::Plugin) -> bool,
    ) -> Result<()> {
        let owner = self.sources.len();
        for plugin in &source.snapshot.plugins {
            if !include(plugin) {
                continue;
            }
            let host = index::normalize_host(&plugin.host)?;
            let position = self.snapshot.plugins.iter().position(|known| {
                known.name.eq_ignore_ascii_case(&plugin.name)
                    && index::normalize_host(&known.host).ok().as_deref() == Some(host.as_str())
            });
            let merged = match position {
                Some(position) => &mut self.snapshot.plugins[position],
                None => {
                    self.snapshot.plugins.push(index::Plugin {
                        name: plugin.name.clone(),
                        host,
                        versions: indexmap::IndexMap::new(),
                    });
                    self.snapshot.plugins.last_mut().expect("inserted plugin")
                }
            };
            for (version, locations) in &plugin.versions {
                let merged_locations = merged.versions.entry(version.clone()).or_default();
                for location in locations {
                    let key = (location.url.clone(), location.sha256.clone());
                    self.owners.entry(key).or_insert(owner);
                    if !merged_locations
                        .iter()
                        .any(|known| known.url == location.url && known.sha256 == location.sha256)
                    {
                        merged_locations.push(location.clone());
                    }
                }
            }
        }
        self.sources.push(source);
        self.names.push(None);
        Ok(())
    }

    pub fn source(&self, location: &index::Location) -> Result<&index::LoadedRepository> {
        let owner =
            self.owners.get(&(location.url.clone(), location.sha256.clone())).ok_or_else(|| {
                Error::PluginInstall("resolved archive has no repository owner".into())
            })?;
        Ok(&self.sources[*owner])
    }

    pub async fn fetch(&self, location: &index::Location) -> Result<Vec<u8>> {
        let key = (location.url.clone(), location.sha256.clone());
        let mut archives = self.archives.lock().await;
        if let Some(bytes) = archives.get(&key) {
            return Ok(bytes.clone());
        }
        let bytes = self.source(location)?.fetch_verified(location).await?;
        archives.insert(key, bytes.clone());
        Ok(bytes)
    }

    pub fn name_of(&self, location: &index::Location) -> Option<&str> {
        self.owners
            .get(&(location.url.clone(), location.sha256.clone()))
            .and_then(|owner| self.names[*owner].as_deref())
    }
}
