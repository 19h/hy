//! Ordered HTTPX proxy and bypass mounts, independent of their source.

use std::collections::HashMap;

use indexmap::IndexMap;
use reqwest::header::HeaderValue;

use crate::error::{Error, Result};
use crate::util::http_headers::trim;

use super::rules::Pattern;

#[derive(Clone, Default)]
pub(super) struct Config {
    mounts: Vec<Mount>,
}

#[derive(Clone)]
struct Mount {
    pattern: Pattern,
    proxy: Option<Proxy>,
}

#[derive(Clone)]
pub(super) struct Proxy {
    pub(super) url: url::Url,
    pub(super) authorization: Option<HeaderValue>,
}

impl Config {
    pub(super) fn from_environment() -> Result<Self> {
        let entries = std::env::vars_os()
            .filter_map(|(name, value)| Some((name.into_string().ok()?, value.into_string().ok()?)))
            .collect();
        Self::from_values(super::environment::discover(entries, super::system::discover))
    }

    #[cfg(test)]
    pub(super) fn from_entries(entries: Vec<(String, String)>) -> Result<Self> {
        Self::from_values(super::environment::discover(entries, HashMap::new))
    }

    fn from_values(values: HashMap<String, String>) -> Result<Self> {
        let exclusions: Vec<_> = values
            .get("no")
            .map(String::as_str)
            .unwrap_or("")
            .split(',')
            .map(trim)
            .filter(|value| !value.is_empty())
            .collect();
        if exclusions.contains(&"*") {
            return Ok(Self::default());
        }
        let mut mounts = IndexMap::new();
        for scheme in ["http", "https", "all"] {
            if let Some(value) = values.get(scheme) {
                let address = if value.contains("://") {
                    value.clone()
                } else {
                    format!("http://{value}")
                };
                mounts.insert(format!("{scheme}://"), Some(address));
            }
        }
        for value in exclusions {
            let address = value.split('/').next().unwrap_or(value);
            let pattern = if value.contains("://") {
                value.into()
            } else if address.parse::<std::net::Ipv4Addr>().is_ok()
                || value.eq_ignore_ascii_case("localhost")
            {
                format!("all://{value}")
            } else if address.parse::<std::net::Ipv6Addr>().is_ok() {
                format!("all://[{value}]")
            } else {
                format!("all://*{value}")
            };
            mounts.insert(pattern, None);
        }
        let mut parsed = Vec::new();
        for (pattern, proxy) in mounts {
            parsed.push(Mount {
                pattern: Pattern::parse(&pattern)?,
                proxy: proxy.map(|value| Proxy::parse(&value)).transpose()?,
            });
        }
        parsed.sort_by_key(|mount| mount.pattern.priority());
        Ok(Self {
            mounts: parsed,
        })
    }

    pub(super) fn select(&self, url: &url::Url) -> Option<&Proxy> {
        self.mounts
            .iter()
            .find(|mount| mount.pattern.matches(url))
            .and_then(|mount| mount.proxy.as_ref())
    }
}

impl Proxy {
    fn parse(value: &str) -> Result<Self> {
        let mut url =
            url::Url::parse(value).map_err(|_| Error::Other("invalid KE proxy URL".into()))?;
        if !matches!(url.scheme(), "http" | "https") || url.host().is_none() {
            return Err(Error::Other("KE proxy requires an HTTP(S) URL".into()));
        }
        let authorization = authorization(&url);
        url.set_username("").expect("HTTP proxy URL");
        url.set_password(None).expect("HTTP proxy URL");
        Ok(Self {
            url,
            authorization,
        })
    }
}

pub(super) fn authorization(url: &url::Url) -> Option<HeaderValue> {
    use base64::Engine;
    if url.username().is_empty() && url.password().is_none_or(str::is_empty) {
        return None;
    }
    let username = percent_encoding::percent_decode_str(url.username()).decode_utf8_lossy();
    let password =
        percent_encoding::percent_decode_str(url.password().unwrap_or("")).decode_utf8_lossy();
    let encoded =
        base64::engine::general_purpose::STANDARD.encode(format!("{username}:{password}"));
    Some(HeaderValue::from_str(&format!("Basic {encoded}")).expect("base64 authorization"))
}
