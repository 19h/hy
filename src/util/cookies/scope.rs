use crate::util::python_integer;

use super::{Cookie, Version};

pub(super) struct Context {
    host: String,
    pub(super) effective_host: String,
    pub(super) path: String,
    pub(super) port: Option<String>,
    secure: bool,
}

impl Context {
    pub(super) fn new(url: &url::Url) -> Self {
        let host = url[url::Position::BeforeUsername..url::Position::AfterHost].to_lowercase();
        let effective_host = if host.contains('.') {
            host.clone()
        } else {
            format!("{host}.local")
        };
        let authority = &url[url::Position::BeforeUsername..url::Position::AfterPort];
        // urllib's cookie policy uses 80 when no explicit port appears, even
        // for HTTPS. A colon inside an IPv6 authority makes its port unknown.
        let port = match authority.split_once(':') {
            Some((_, value)) if value.parse::<i64>().is_ok() => Some(value.into()),
            Some(_) => None,
            None => Some("80".into()),
        };
        Self {
            host,
            effective_host,
            path: escape_path(url.path()),
            port,
            secure: url.scheme() == "https",
        }
    }
}

pub(super) fn accepts(cookie: &Cookie, context: &Context) -> bool {
    if cookie.version == Version::Unsupported {
        return false;
    }
    if cookie.domain_specified {
        let undotted = cookie.domain.strip_prefix('.').unwrap_or(&cookie.domain);
        if !undotted.contains('.') && !context.effective_host.ends_with(".local") {
            return false;
        }
        let host = &context.effective_host;
        if cookie.version == Version::Netscape
            && !host.ends_with(&cookie.domain)
            && !host.ends_with(&format!("{undotted}.local"))
            && (host.starts_with('.') || !format!(".{host}").ends_with(&cookie.domain))
        {
            return false;
        }
    }
    if cookie.port_specified {
        let port = context.port.as_deref().unwrap_or("80");
        for candidate in cookie.ports.as_deref().unwrap_or_default() {
            if python_integer::parse(candidate).is_none() {
                return false;
            }
            if candidate == port {
                return true;
            }
        }
        return false;
    }
    true
}

pub(super) fn returns(cookie: &Cookie, context: &Context) -> bool {
    let domain = if cookie.domain.starts_with('.') {
        cookie.domain.clone()
    } else {
        format!(".{}", cookie.domain)
    };
    let path_matches = context.path == cookie.path
        || (context.path.starts_with(&cookie.path)
            && (cookie.path.ends_with('/')
                || context.path.as_bytes().get(cookie.path.len()) == Some(&b'/')));
    let mut domain_matches = format!(".{}", context.effective_host).ends_with(&domain);
    if cookie.version == Version::Negative {
        let host = &context.host;
        let dotted = if host.starts_with('.') {
            host.clone()
        } else {
            format!(".{host}")
        };
        domain_matches |= dotted.ends_with(&domain);
    }
    domain_matches
        && path_matches
        && (!cookie.secure || context.secure)
        && cookie.ports.as_ref().is_none_or(|ports| {
            ports.iter().any(|port| port == context.port.as_deref().unwrap_or("80"))
        })
}

pub(super) fn escape_path(path: &str) -> String {
    const HEX: &[u8] = b"0123456789ABCDEF";
    let bytes = path.as_bytes();
    let mut escaped = String::new();
    let mut index = 0;
    while index < bytes.len() {
        let byte = bytes[index];
        if byte == b'%'
            && bytes
                .get(index + 1..index + 3)
                .is_some_and(|pair| pair.iter().all(u8::is_ascii_hexdigit))
        {
            escaped.push('%');
            escaped.push(char::from(bytes[index + 1].to_ascii_uppercase()));
            escaped.push(char::from(bytes[index + 2].to_ascii_uppercase()));
            index += 3;
            continue;
        }
        if byte.is_ascii_alphanumeric() || b"_-.%/;:@&=+$,!~*'()".contains(&byte) {
            escaped.push(char::from(byte));
        } else {
            escaped.push('%');
            escaped.push(char::from(HEX[usize::from(byte >> 4)]));
            escaped.push(char::from(HEX[usize::from(byte & 15)]));
        }
        index += 1;
    }
    escaped
}
