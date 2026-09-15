//! Attribute precedence and defaults, matching CookieJar's normalization stage.

use std::collections::HashSet;

use num_bigint::BigInt;
use num_traits::{Signed, ToPrimitive, Zero};

use crate::util::http_headers::whitespace;
use crate::util::python_integer;

use super::super::{Cookie, Version, scope};
use super::{Attribute, Format};

enum Expiry {
    Seconds(BigInt),
    Unconverted,
}

#[derive(Default)]
pub(crate) struct Normalized {
    name: String,
    value: Option<String>,
    version: Option<String>,
    domain: Option<String>,
    path: Option<String>,
    port: Option<Option<String>>,
    secure: bool,
    expires: Option<Expiry>,
}

impl Normalized {
    pub(crate) fn parse(
        attributes: Vec<Attribute>,
        now: i64,
        format: Format,
    ) -> Result<Option<Self>, ()> {
        let mut attributes = attributes.into_iter();
        let Some(identity) = attributes.next() else {
            return Ok(None);
        };
        let mut cookie = Self {
            name: identity.key,
            value: identity.value,
            version: (format == Format::Netscape).then(|| "0".into()),
            ..Self::default()
        };
        let mut seen = HashSet::new();
        let mut max_age_set = false;
        for attribute in attributes {
            let key = attribute.key.to_ascii_lowercase();
            if key != "max-age" && seen.contains(&key) {
                continue;
            }
            let value = attribute.value;
            match key.as_str() {
                "domain" | "path" | "version" if value.is_none() => return Ok(None),
                "domain" => cookie.domain = value.map(|domain| domain.to_lowercase()),
                "path" => cookie.path = value,
                "version" => cookie.version = value,
                "secure" => cookie.secure = value.is_none_or(|value| !value.is_empty()),
                "port" => cookie.port = Some(value),
                "max-age" => {
                    // int(None) raises TypeError, which cancels normalization of
                    // the response. A malformed string rejects only this cookie.
                    let value = value.ok_or(())?;
                    let Some(age) = python_integer::parse(&value) else {
                        return Ok(None);
                    };
                    max_age_set = true;
                    cookie.expires = Some(Expiry::Seconds(age + now));
                }
                "expires" => {
                    if max_age_set {
                        continue;
                    }
                    let expiry = match format {
                        Format::Netscape => {
                            attribute.expiry.map(|seconds| Expiry::Seconds(seconds.into()))
                        }
                        Format::Rfc2965 => value.map(|_| Expiry::Unconverted),
                    };
                    let Some(expiry) = expiry else {
                        continue;
                    };
                    cookie.expires = Some(expiry);
                }
                _ => {}
            }
            seen.insert(key);
        }
        Ok(Some(cookie))
    }

    /// Version conversion occurs after normalization of the complete response.
    pub(crate) fn into_cookie(
        self,
        context: &scope::Context,
        format: Format,
    ) -> Result<Option<Cookie>, ()> {
        let number = match self.version {
            Some(value) => {
                let Some(number) = python_integer::parse(&value) else {
                    return Ok(None);
                };
                Some(number)
            }
            None => None,
        };
        let path = match self.path.filter(|path| !path.is_empty()) {
            Some(path) => scope::escape_path(&path),
            None => {
                let boundary = context.path.rfind('/').unwrap_or(0)
                    + usize::from(!number.as_ref().is_some_and(Zero::is_zero));
                let prefix = &context.path[..boundary.min(context.path.len())];
                if prefix.is_empty() {
                    "/".into()
                } else {
                    prefix.into()
                }
            }
        };
        let version = if number.as_ref().is_some_and(Signed::is_negative) {
            Version::Negative
        } else if number
            .as_ref()
            .and_then(ToPrimitive::to_u8)
            .is_some_and(|version| version == 0 || (version == 1 && format == Format::Netscape))
        {
            // RFC 2109 version 1 is downgraded after computing its default path.
            Version::Netscape
        } else {
            Version::Unsupported
        };
        let domain_specified = self.domain.is_some();
        let domain = match self.domain {
            Some(domain) if domain.starts_with('.') => domain,
            Some(domain) => format!(".{domain}"),
            None => context.effective_host.clone(),
        };
        let port_specified = matches!(self.port, Some(Some(_)));
        let ports = match self.port {
            Some(Some(value)) => Some(
                value
                    .chars()
                    .filter(|c| !whitespace(*c))
                    .collect::<String>()
                    .split(',')
                    .map(str::to_owned)
                    .collect(),
            ),
            Some(None) => context.port.as_ref().map(|port| vec![port.clone()]),
            None => None,
        };
        let expires = match self.expires {
            Some(Expiry::Seconds(seconds)) => Some(seconds),
            // Set-Cookie2 Expires values remain strings in Python; comparing
            // one with the integer clock aborts this construction batch.
            Some(Expiry::Unconverted) => return Err(()),
            None => None,
        };
        Ok(Some(Cookie {
            name: self.name,
            value: self.value,
            domain,
            domain_specified,
            path,
            ports,
            port_specified,
            secure: self.secure,
            expires,
            version,
        }))
    }
}
