//! Resolve local bundle archives and platform-specific repository distributions.

use crate::error::{Error, Result};
use crate::plugin::{self, PluginContext, bundle::ResolvedPluginArchive, index};

mod archives;
mod local;
mod reference;

use archives::{Archive, Archives};

pub struct Sources<'a> {
    context: &'a PluginContext,
    repository_override: Option<&'a str>,
    repositories: Option<Vec<(Option<String>, index::LoadedRepository)>>,
}

impl<'a> Sources<'a> {
    pub fn new(context: &'a PluginContext, repository_override: Option<&'a str>) -> Self {
        Self {
            context,
            repository_override,
            repositories: None,
        }
    }

    async fn load(&mut self) -> Result<()> {
        if self.repositories.is_some() {
            return Ok(());
        }
        let configured = match self.repository_override {
            Some(source) => vec![(
                None,
                index::Repository {
                    url: source.into(),
                },
            )],
            None => self.context.repositories()?,
        };
        let explicit = self.repository_override.is_some() || self.context.repository.is_some();
        let mut repositories = Vec::new();
        for (name, repository) in configured {
            match self.context.load_repository(name.as_deref(), &repository.url, false).await {
                Ok(repository) => repositories.push((name, repository)),
                Err(error) if explicit => return Err(error),
                Err(error) => {
                    tracing::warn!(repository = ?name, %error, "bundle repository unavailable")
                }
            }
        }
        self.repositories = Some(repositories);
        Ok(())
    }

    pub async fn resolve(
        &mut self,
        spec: &str,
        platforms: &[String],
    ) -> Result<Vec<ResolvedPluginArchive>> {
        if let Some(archives) = local::resolve(spec, platforms)? {
            return Ok(archives);
        }
        let reference = reference::prepare(spec)?;
        if reference.spec.chars().nth(1) != Some('=')
            || reference.spec.get(2..).and_then(plugin::parse_version).is_none()
        {
            return Err(Error::Other(format!(
                "invalid plugin version spec: {}{}",
                reference.name, reference.spec,
            )));
        }
        self.load().await?;
        let mut candidates = Vec::new();
        for (_, repository) in self.repositories.as_ref().expect("loaded above") {
            for _ in index::matching_plugins(&repository.snapshot, &reference)? {
                candidates.push(repository);
            }
        }
        let repository = match candidates.as_slice() {
            [repository] => *repository,
            [] => return Err(Error::NotFound(format!("plugin {spec} in configured repositories"))),
            _ => {
                return Err(Error::Other(format!(
                    "ambiguous plugin {spec}; qualify its repository and host"
                )));
            }
        };
        let mut archives = Archives::default();
        for platform in platforms {
            let location =
                index::select_for_platform(&repository.snapshot, &reference, platform, None)?;
            let bytes = repository.fetch_verified(location).await?;
            archives
                .insert(Archive::new(location.descriptor.metadata.name.clone(), bytes), platform);
        }
        archives.finish().into_iter().map(Archive::resolve).collect()
    }
}
