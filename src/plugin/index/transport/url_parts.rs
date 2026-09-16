//! urllib URL pieces used before HTTP or filesystem-specific normalization.
//!
//! The adaptation and license notice are in util/python_path/LICENSE.

use crate::error::Result;

pub(in crate::plugin::index) mod authority;

pub(in crate::plugin::index) struct Parts {
    pub scheme: String,
    pub authority: String,
    pub path: String,
}

impl Parts {
    pub fn parse(value: &str) -> Result<Self> {
        let cleaned: String = value
            .trim_start_matches(|character: char| character <= '\u{20}')
            .chars()
            .filter(|character| !matches!(character, '\t' | '\r' | '\n'))
            .collect();
        let (scheme, mut remainder) = match cleaned.split_once(':') {
            Some((scheme, rest))
                if scheme.as_bytes().first().is_some_and(u8::is_ascii_alphabetic)
                    && scheme
                        .bytes()
                        .all(|byte| byte.is_ascii_alphanumeric() || b"+.-".contains(&byte)) =>
            {
                (scheme.to_ascii_lowercase(), rest)
            }
            _ => (String::new(), cleaned.as_str()),
        };
        let authority = if let Some(netloc) = remainder.strip_prefix("//") {
            let end = netloc.find(['/', '?', '#']).unwrap_or(netloc.len());
            authority::validate(&netloc[..end])?;
            remainder = &netloc[end..];
            netloc[..end].to_owned()
        } else {
            String::new()
        };
        let end = remainder.find(['?', '#']).unwrap_or(remainder.len());
        Ok(Self {
            scheme,
            authority,
            path: remainder[..end].into(),
        })
    }

    /// urllib's hostname accessor does not percent-decode or IDNA-normalize.
    pub fn hostname(&self) -> String {
        let host = self.authority.rsplit('@').next().unwrap_or("");
        let host = if let Some((_, bracketed)) = host.split_once('[') {
            bracketed.split_once(']').map_or(bracketed, |(host, _)| host)
        } else {
            host.split_once(':').map_or(host, |(host, _)| host)
        };
        host.to_lowercase()
    }
}
