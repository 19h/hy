//! Named repository configuration and legacy single-repository migration.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::error::Result;

pub const RESERVED: &[(&str, &str)] = &[
    ("community", "https://community.plugins.hex-rays.com/plugin-repository.json"),
    ("hexrays", "https://hexrays.plugins.hex-rays.com/plugin-repository.json"),
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Repository {
    pub url: String,
}
pub fn valid_repo_name(name: &str) -> bool {
    !name.is_empty()
        && name.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

pub fn repositories() -> Result<(BTreeMap<String, Repository>, String)> {
    let config = crate::plugin::read_ida_config()?;
    Ok(repositories_from_config(&config))
}

pub fn repositories_from_config(
    config: &serde_json::Value,
) -> (BTreeMap<String, Repository>, String) {
    let settings = &config["Settings"];
    let mut repos: BTreeMap<String, Repository> =
        serde_json::from_value(settings["plugin-repositories"].clone()).unwrap_or_default();
    let mut default =
        settings["default-plugin-repository"].as_str().unwrap_or("community").to_owned();
    if repos.is_empty() && settings["default-plugin-repository"].is_null() {
        for &(name, url) in RESERVED {
            repos.insert(
                name.into(),
                Repository {
                    url: url.into(),
                },
            );
        }
        if let Some(url) = settings["plugin-repository"]["url"].as_str().filter(|u| !u.is_empty())
            && url
                != "https://raw.githubusercontent.com/HexRaysSA/plugin-repository/refs/heads/v1/plugin-repository.json"
        {
            repos.insert(
                "custom".into(),
                Repository {
                    url: url.into(),
                },
            );
            default = "custom".into();
        }
    }
    repos.retain(|name, entry| valid_repo_name(name) && !entry.url.is_empty());
    for &(name, url) in RESERVED {
        if let Some(entry) = repos.get_mut(name) {
            entry.url = url.into();
        }
    }
    (repos, default)
}

pub fn save_repositories(repos: &BTreeMap<String, Repository>, default: &str) -> Result<()> {
    let mut config = crate::plugin::read_ida_config()?;
    if !config["Settings"].is_object() {
        config["Settings"] = serde_json::json!({});
    }
    config["Settings"]["plugin-repositories"] = serde_json::to_value(repos)?;
    config["Settings"]["default-plugin-repository"] = serde_json::json!(default);
    config["Settings"].as_object_mut().unwrap().remove("plugin-repository");
    crate::plugin::write_ida_config(&config)
}
