//! Typed GitHub responses used by catalogue discovery.

use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct Connection<T> {
    pub nodes: Vec<T>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Repository {
    pub default_branch_ref: Option<Reference>,
    pub releases: Connection<Release>,
    pub refs: Connection<Reference>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Release {
    pub published_at: Option<String>,
    pub release_assets: Connection<Asset>,
    pub tag: Option<Reference>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Asset {
    pub name: String,
    pub download_url: String,
    pub size: u64,
    pub content_type: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Reference {
    #[serde(default)]
    pub name: String,
    pub target: Target,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum Target {
    Commit(Commit),
    Annotated {
        target: Box<Target>,
    },
}

impl Target {
    pub fn commit(&self) -> &Commit {
        match self {
            Self::Commit(commit) => commit,
            Self::Annotated {
                target,
            } => target.commit(),
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Commit {
    pub oid: String,
    pub zipball_url: String,
    pub committed_date: String,
}

#[derive(Deserialize)]
pub struct SearchResponse {
    pub items: Vec<SearchItem>,
}

#[derive(Deserialize)]
pub struct SearchItem {
    pub repository: SearchRepository,
}

#[derive(Deserialize)]
pub struct SearchRepository {
    pub full_name: String,
}

#[derive(Deserialize)]
pub struct GraphResponse {
    pub data: Option<GraphData>,
    #[serde(default)]
    pub errors: Vec<GraphError>,
}

#[derive(Deserialize)]
pub struct GraphData {
    pub repository: Option<Repository>,
}

#[derive(Deserialize)]
pub struct GraphError {
    #[serde(rename = "type", default)]
    pub kind: String,
    pub message: String,
}
