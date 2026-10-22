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
        let mut repositories: Vec<_> = repositories.into_iter().collect();
        repositories.sort_by_key(|(name, _)| (name == "community", name.clone()));
        Ok(repositories.into_iter().map(|(name, repository)| (Some(name), repository)).collect())
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
