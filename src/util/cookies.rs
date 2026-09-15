//! In-memory HTTPX cookie sessions with Python's default Netscape policy.

use indexmap::IndexMap;
use num_bigint::BigInt;
use reqwest::header::{HeaderMap, HeaderValue};

use crate::error::{Error, Result};

mod dates;
mod extract;
mod parse;
mod scope;

type Names = IndexMap<String, Cookie>;
type Paths = IndexMap<String, Names>;

#[derive(Default)]
pub(crate) struct Jar {
    domains: IndexMap<String, Paths>,
}

struct Cookie {
    name: String,
    value: Option<String>,
    domain: String,
    domain_specified: bool,
    path: String,
    ports: Option<Vec<String>>,
    port_specified: bool,
    secure: bool,
    expires: Option<BigInt>,
    version: Version,
}

#[derive(PartialEq)]
enum Version {
    Negative,
    Netscape,
    Unsupported,
}

impl Jar {
    pub(crate) fn store(&mut self, headers: &HeaderMap, url: &url::Url) {
        self.store_at(headers, url, chrono::Utc::now().timestamp());
    }

    fn store_at(&mut self, headers: &HeaderMap, url: &url::Url, now: i64) {
        self.extract(headers, url, now);
    }

    pub(crate) fn header(&mut self, url: &url::Url) -> Result<Option<HeaderValue>> {
        self.header_at(url, chrono::Utc::now().timestamp())
    }

    fn header_at(&mut self, url: &url::Url, now: i64) -> Result<Option<HeaderValue>> {
        let context = scope::Context::new(url);
        let now = BigInt::from(now);
        let mut matching = Vec::new();
        for paths in self.domains.values_mut() {
            for names in paths.values_mut() {
                names.retain(|_, cookie| {
                    cookie.expires.as_ref().is_none_or(|expires| expires > &now)
                });
                matching.extend(names.values().filter(|cookie| scope::returns(cookie, &context)));
            }
        }
        // Python traverses domain/path/name insertion order, then stably sorts
        // by descending path length. Updating a cookie keeps its position.
        matching.sort_by_key(|cookie| std::cmp::Reverse(cookie.path.len()));
        let pairs: Vec<_> = matching
            .into_iter()
            .map(|cookie| match &cookie.value {
                Some(value) => format!("{}={value}", cookie.name),
                None => cookie.name.clone(),
            })
            .collect();
        if pairs.is_empty() {
            return Ok(None);
        }
        let text = pairs.join("; ");
        if !text.is_ascii() {
            return Err(Error::Other("cookie header contains non-ASCII text".into()));
        }
        HeaderValue::from_str(&text)
            .map(Some)
            .map_err(|error| Error::Other(format!("invalid cookie header: {error}")))
    }
}

#[cfg(test)]
mod tests;
