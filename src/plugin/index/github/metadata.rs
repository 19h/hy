//! Ten-repository cache warming, followed by ordinary per-repository lookups.

use crate::error::{Error, Result};
use crate::util::python_json::Text;

use super::{Client, METADATA_LIFETIME, cache, graphql, http, models::Repository};

const BATCH_SIZE: usize = 10;

impl Client {
    pub(super) async fn warm_releases(&self, repositories: &[Text]) -> Result<Vec<String>> {
        let mut missing = Vec::new();
        let mut names = Vec::with_capacity(repositories.len());
        for name in repositories {
            // Cache component validation happens after candidate publication and
            // after any earlier repository probes, even for unpaired surrogates.
            let path = cache::candidate_metadata_path(name)?;
            let cached = read_releases(&path)?;
            let name = name.to_utf8()?;
            if cached.is_none() {
                missing.push(name.clone());
            }
            names.push(name);
        }
        for batch in missing.chunks(BATCH_SIZE) {
            let releases = self.query_releases(batch).await?;
            for (name, repository) in releases {
                self.store_releases(&name, &repository)?;
            }
        }
        Ok(names)
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
        let request = self.http.post(format!("{}/graphql", self.base)).json(&body);
        let response = http::read_graphql_json(self.response(request).await?).await?;
        graphql::decode(repositories, response)
    }

    fn cached_releases(&self, name: &str) -> Result<Option<Repository>> {
        let path = cache::metadata_path(name)?;
        read_releases(&path)
    }

    fn store_releases(&self, name: &str, repository: &Repository) -> Result<()> {
        cache::write_json(&cache::metadata_path(name)?, repository)
    }
}

fn read_releases(path: &std::path::Path) -> Result<Option<Repository>> {
    cache::read(path, Some(METADATA_LIFETIME))?
        .map(|bytes| serde_json::from_slice(&bytes).map_err(Into::into))
        .transpose()
}
