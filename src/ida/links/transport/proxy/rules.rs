//! HTTPX URLPattern matching for NO_PROXY, including scheme and port priority.

use crate::error::{Error, Result};

#[derive(Clone)]
pub(super) struct Pattern {
    scheme: String,
    host: String,
    kind: HostMatch,
    port: Option<u16>,
    host_length: usize,
}

#[derive(Clone)]
enum HostMatch {
    Any,
    Exact,
    Suffix,
    Subdomains,
}

impl Pattern {
    pub(super) fn parse(value: &str) -> Result<Self> {
        let invalid = || Error::Other("invalid KE proxy bypass pattern".into());
        let (scheme, remaining) = value.split_once("://").ok_or_else(invalid)?;
        let authority = remaining.split(['/', '?', '#']).next().unwrap_or("");
        let authority = authority.rsplit('@').next().unwrap_or("");
        let (host, port) = if authority.starts_with('[') {
            let end = authority.find(']').ok_or_else(invalid)?;
            (&authority[..=end], authority[end + 1..].strip_prefix(':'))
        } else {
            authority.rsplit_once(':').map_or((authority, None), |(host, port)| (host, Some(port)))
        };
        let port = port
            .filter(|port| !port.is_empty())
            .map(|port| port.parse::<u16>().map_err(|_| invalid()))
            .transpose()?;
        // HTTPX normalizes default ports before lowercasing the scheme.
        let port = match (scheme, port) {
            ("http", Some(80)) | ("https", Some(443)) => None,
            _ => port,
        };
        let (kind, bare, prefix) = if host.is_empty() || host == "*" {
            (HostMatch::Any, "", 0)
        } else if let Some(host) = host.strip_prefix("*.") {
            (HostMatch::Subdomains, host, 2)
        } else if let Some(host) = host.strip_prefix('*') {
            (HostMatch::Suffix, host, 1)
        } else {
            (HostMatch::Exact, host, 0)
        };
        let host = if bare.is_empty() {
            String::new()
        } else {
            match url::Host::parse(bare).map_err(|_| invalid())? {
                url::Host::Domain(host) => host,
                url::Host::Ipv4(ip) => ip.to_string(),
                url::Host::Ipv6(ip) => ip.to_string(),
            }
        };
        Ok(Self {
            scheme: if scheme == "all" {
                String::new()
            } else {
                scheme.to_lowercase()
            },
            host_length: host.chars().count() + prefix,
            host,
            kind,
            port,
        })
    }

    pub(super) fn priority(&self) -> (bool, std::cmp::Reverse<usize>, std::cmp::Reverse<usize>) {
        (
            self.port.is_none(),
            std::cmp::Reverse(self.host_length),
            std::cmp::Reverse(self.scheme.len()),
        )
    }

    pub(super) fn matches(&self, url: &url::Url) -> bool {
        let host = match url.host() {
            Some(url::Host::Domain(host)) => host.to_owned(),
            Some(url::Host::Ipv4(ip)) => ip.to_string(),
            Some(url::Host::Ipv6(ip)) => ip.to_string(),
            None => String::new(),
        };
        let host_matches = match self.kind {
            HostMatch::Any => true,
            HostMatch::Exact => host == self.host,
            HostMatch::Suffix => host == self.host || host.ends_with(&format!(".{}", self.host)),
            HostMatch::Subdomains => {
                host.len() > self.host.len() + 1 && host.ends_with(&format!(".{}", self.host))
            }
        };
        (self.scheme.is_empty() || self.scheme == url.scheme())
            && host_matches
            && self.port.is_none_or(|port| Some(port) == url.port())
    }
}
