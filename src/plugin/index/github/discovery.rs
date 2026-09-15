//! Search pagination, repository lists and selection before metadata warming.

use std::collections::BTreeSet;
use std::path::Path;

use reqwest::header;

use crate::error::{Error, Result};
use crate::util::{python_path, strings::python_trim};

use super::{Client, METADATA_LIFETIME, cache};

mod values;

const ENCODED_QUERIES: [&str; 2] =
    ["filename%3Aida-plugin.json", "filename%3Aida-plugin.json%20fork%3Atrue"];
const PAGE_SIZE: usize = 100;

impl Client {
    pub(super) async fn candidates(&self) -> Result<BTreeSet<String>> {
        let path = cache::candidates_path()?;
        if let Some(bytes) = cache::read(&path, Some(METADATA_LIFETIME))? {
            let value = serde_json::from_slice(&bytes)?;
            return Ok(lowercase(values::cached(&value)?));
        }
        let mut repositories = values::Search::default();
        for query in ENCODED_QUERIES {
            for page in 1_u64.. {
                let request = self
                    .http
                    .get(search_url(&self.base, query, page))
                    .header(header::ACCEPT, "application/vnd.github.v3+json")
                    .header(header::USER_AGENT, "ida-hcli");
                let response = self.json(request).await?;
                let count = repositories.append(&response)?;
                if count < PAGE_SIZE {
                    break;
                }
            }
        }
        let repositories = lowercase(repositories.finish()?);
        // Upstream publishes discovery before selected names are parsed.
        cache::write_json(&cache::candidates_path()?, &repositories)?;
        Ok(repositories)
    }
}

fn search_url(base: &str, query: &str, page: u64) -> String {
    format!("{base}/search/code?q={query}&per_page={PAGE_SIZE}&page={page}")
}

pub(super) fn read_list(path: Option<&Path>) -> Result<Vec<String>> {
    let Some(path) = path else {
        return Ok(Vec::new());
    };
    if !python_path::exists(path)? {
        return Err(Error::GitHubValue(format!("file doesn't exist: {}", path.display())));
    }
    Ok(parse_list(&std::fs::read_to_string(path)?))
}

fn parse_list(text: &str) -> Vec<String> {
    text.split(|character| {
        matches!(
            character,
            '\n' | '\r'
                | '\u{b}'
                | '\u{c}'
                | '\u{1c}'
                | '\u{1d}'
                | '\u{1e}'
                | '\u{85}'
                | '\u{2028}'
                | '\u{2029}'
        )
    })
    .filter(|line| !line.is_empty() && !line.starts_with('#'))
    .map(|line| python_trim(line).to_owned())
    .collect()
}

pub(super) fn select(
    candidates: BTreeSet<String>,
    extra: Vec<String>,
    ignored: Vec<String>,
) -> Result<Vec<String>> {
    let mut names = lowercase(candidates);
    names.extend(lowercase(extra));
    let ignored = lowercase(ignored);
    names.retain(|name| !ignored.contains(name));
    for name in &names {
        parse_repository(name)?;
    }
    Ok(names.into_iter().collect())
}

fn lowercase(names: impl IntoIterator<Item = String>) -> BTreeSet<String> {
    names.into_iter().map(|name| name.to_lowercase()).collect()
}

pub(super) fn parse_repository(name: &str) -> Result<(&str, &str)> {
    name.split_once('/').filter(|(_, repo)| !repo.contains('/')).ok_or_else(|| {
        Error::GitHubValue(format!(
            "invalid repository format: {name}. Expected format: owner/repo"
        ))
    })
}

#[cfg(test)]
fn validate_cache_name(name: &str) -> Result<()> {
    let (owner, repo) = parse_repository(name)?;
    for part in [owner, repo] {
        cache::validate_component(part)?;
    }
    if name.contains('\0') {
        return Err(Error::GitHubValue("embedded null byte".into()));
    }
    Ok(())
}

#[cfg(test)]
mod tests;
#[cfg(test)]
mod value_tests;
