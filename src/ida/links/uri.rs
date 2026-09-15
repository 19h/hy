//! Preserve urllib's raw path and query spelling for ordinary IDA navigation.

use crate::error::{Error, Result};

pub(super) struct ParsedLink<'a> {
    original: &'a str,
    pub source: String,
    pub segments: Vec<String>,
    pub query: String,
}

pub(super) enum DefaultTarget {
    Relative {
        uri: String,
    },
    Database {
        uri: String,
        name: String,
        source: String,
    },
}

impl<'a> ParsedLink<'a> {
    pub fn parse(original: &'a str) -> Result<Self> {
        // urllib removes leading C0/space and embedded tab/CR/LF for parsing.
        // The original URI is still what upstream forwards to IDA.
        let cleaned = normalize_controls(original);
        let (_, remainder) = cleaned
            .split_once(':')
            .filter(|(scheme, _)| scheme.eq_ignore_ascii_case("ida"))
            .ok_or_else(|| Error::Other("expected an ida:// link".into()))?;
        let (source, remainder) = if let Some(authority) = remainder.strip_prefix("//") {
            let end = authority.find(['/', '?', '#']).unwrap_or(authority.len());
            (hostname(&authority[..end])?, &authority[end..])
        } else {
            (String::new(), remainder)
        };
        let without_fragment = remainder.split_once('#').map_or(remainder, |(path, _)| path);
        let (path, query) = without_fragment.split_once('?').unwrap_or((without_fragment, ""));
        Ok(Self {
            original,
            source,
            segments: path
                .split('/')
                .filter(|segment| !segment.is_empty())
                .map(str::to_owned)
                .collect(),
            query: query.into(),
        })
    }

    pub fn default_target(self) -> Result<DefaultTarget> {
        let mut uri = self.original.to_owned();
        if self.segments.len() == 1 && self.query.is_empty() {
            // Upstream appends to the original string, including any fragment or
            // empty-query delimiter. Reconstructing a URL changes this behavior.
            uri = format!("{}/functions", uri.trim_end_matches('/'));
        } else if self.segments.len() <= 1 {
            if self.query.is_empty() {
                return Err(Error::Other(format!("Unsupported ida:// URL: {}", self.original)));
            }
            return Ok(DefaultTarget::Relative {
                uri,
            });
        }
        Ok(DefaultTarget::Database {
            uri,
            name: self.segments.into_iter().next().expect("database target has a path segment"),
            source: self.source,
        })
    }
}

pub(super) fn normalize_controls(original: &str) -> String {
    original
        .trim_start_matches(|character: char| character <= '\u{20}')
        .chars()
        .filter(|character| !matches!(character, '\t' | '\r' | '\n'))
        .collect()
}

fn hostname(authority: &str) -> Result<String> {
    let host = authority.rsplit('@').next().unwrap_or("");
    let host = if host.contains(['[', ']']) {
        let bracketed = host.strip_prefix('[').and_then(|host| host.split_once(']'));
        let Some((address, _port)) = bracketed else {
            return Err(Error::Other("Invalid bracketed host in IDA link".into()));
        };
        let address_without_zone = address.split('%').next().unwrap_or(address);
        let ipv_future = address_without_zone.strip_prefix('v').is_some_and(|address| {
            address.split_once('.').is_some_and(|(version, address)| {
                !version.is_empty()
                    && version.bytes().all(|byte| byte.is_ascii_hexdigit())
                    && !address.is_empty()
            })
        });
        if !ipv_future && address_without_zone.parse::<std::net::Ipv6Addr>().is_err() {
            return Err(Error::Other("Invalid IP address in IDA link".into()));
        }
        address
    } else {
        host.split(':').next().unwrap_or("")
    };
    // urllib lowercases hostnames but preserves a scoped IPv6 zone identifier.
    Ok(match host.split_once('%') {
        Some((address, zone)) => format!("{}%{zone}", address.to_lowercase()),
        None => host.to_lowercase(),
    })
}

#[cfg(test)]
mod tests;
