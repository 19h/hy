//! Ten-repository cache warming, followed by ordinary per-repository lookups.

use crate::error::{Error, Result};

use super::{Client, METADATA_LIFETIME, cache, graphql, models::Repository};

const BATCH_SIZE: usize = 10;

impl Client {
    pub(super) async fn warm_releases(&self, repositories: &[String]) -> Result<()> {
        let mut missing = Vec::new();
        for name in repositories {
            if self.cached_releases(name)?.is_none() {
                missing.push(name.clone());
            }
        }
        for batch in missing.chunks(BATCH_SIZE) {
            let releases = self.query_releases(batch).await?;
            for (name, repository) in releases {
                self.store_releases(&name, &repository)?;
            }
        }
        Ok(())
    }

    pub(super) async fn releases(&self, name: &str) -> Result<Repository> {
        if let Some(repository) = self.cached_releases(name)? {
            return Ok(repository);
        }
        // Missing aliases are not negatively cached by the warming pass.
        let (_, repository) = self
            .query_releases(&[name.to_owned()])
            .await?
            .pop()
            .ok_or_else(|| Error::NotFound(format!("GitHub repository {name}")))?;
        self.store_releases(name, &repository)?;
        Ok(repository)
    }

    async fn query_releases(&self, repositories: &[String]) -> Result<Vec<(String, Repository)>> {
        if repositories.is_empty() {
            return Ok(Vec::new());
        }
        let body = graphql::request(repositories)?;
        let response =
            self.json(self.http.post(format!("{}/graphql", self.base)).json(&body)).await?;
        graphql::decode(repositories, response)
    }

    fn cached_releases(&self, name: &str) -> Result<Option<Repository>> {
        let key = self.cache_key(&format!("releases-v3/{name}"));
        cache::read(&key, Some(METADATA_LIFETIME))?
            .map(|bytes| serde_json::from_slice(&bytes).map_err(Into::into))
            .transpose()
    }

    fn store_releases(&self, name: &str, repository: &Repository) -> Result<()> {
        let key = self.cache_key(&format!("releases-v3/{name}"));
        cache::write(&key, &serde_json::to_vec(repository)?)
    }
}
