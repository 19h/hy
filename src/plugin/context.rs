//! Per-command plugin options; no mutable process-wide installation state.

use crate::error::Result;
use crate::ida::python::PipOptions;

use super::index;

#[derive(Debug, Default)]
pub struct PluginContext {
    pub repository: Option<String>,
    pub pip: PipOptions,
    pub github: index::github::Options,
}

impl PluginContext {
    pub fn validate_pip_sources(&self) -> Result<()> {
        if !self.pip.no_index || !self.pip.find_links.is_empty() {
            return Ok(());
        }
        if self
            .repository
            .as_deref()
            .is_some_and(|source| super::bundle::is_plugin_bundle_zip(std::path::Path::new(source)))
        {
            return Ok(());
        }
        Err(crate::error::Error::Other(
            "--offline requires --pip-find-links or a plugin bundle repository".into(),
        ))
    }

    pub fn repositories(&self) -> Result<Vec<(Option<String>, index::Repository)>> {
        if let Some(url) = &self.repository {
            return Ok(vec![(
                None,
                index::Repository {
                    url: url.clone(),
                },
            )]);
        }
        let (repositories, _) = index::repositories()?;
        Ok(repositories.into_iter().map(|(name, repository)| (Some(name), repository)).collect())
    }

    pub fn repository_for(&self, reference: &index::Reference) -> Result<index::Repository> {
        if let Some(url) = &self.repository {
            return Ok(index::Repository {
                url: url.clone(),
            });
        }
        let (repositories, default) = index::repositories()?;
        let name = reference.repo.as_deref().unwrap_or(&default);
        repositories
            .get(name)
            .cloned()
            .ok_or_else(|| crate::error::Error::NotFound(format!("plugin repository '{name}'")))
    }

    pub async fn load_for(&self, reference: &index::Reference) -> Result<index::LoadedRepository> {
        if let Some(source) = &self.repository {
            if reference.repo.is_some() {
                return Err(crate::error::Error::Other(
                    "repository prefixes cannot be combined with --repo".into(),
                ));
            }
            return self.load_repository(None, source, false).await;
        }
        let repository = self.repository_for(reference)?;
        let (_, default) = index::repositories()?;
        let name = Some(reference.repo.as_deref().unwrap_or(&default));
        index::load_named(name, &repository.url, false).await
    }

    pub async fn load_repository(
        &self,
        name: Option<&str>,
        source: &str,
        offline: bool,
    ) -> Result<index::LoadedRepository> {
        if name.is_none() && source == "github" {
            index::github::load(&self.github, offline).await
        } else {
            index::load_named(name, source, offline).await
        }
    }
}
