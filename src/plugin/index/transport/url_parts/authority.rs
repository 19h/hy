//! urllib authority validation before transport-specific normalization.

use crate::error::{Error, Result};

pub(in crate::plugin::index::transport) fn validate(authority: &str) -> Result<()> {
    if authority.contains('[') != authority.contains(']') {
        return Err(invalid());
    }
    if authority.contains('[') {
        let host = authority.rsplit('@').next().unwrap_or("");
        let hostname = if let Some((before, rest)) = host.split_once('[') {
            if !before.is_empty() {
                return Err(invalid());
            }
            let (hostname, port) = rest.split_once(']').unwrap_or((rest, ""));
            if !port.is_empty() && !port.starts_with(':') {
                return Err(invalid());
            }
            hostname
        } else {
            host.split_once(':').map_or(host, |(hostname, _)| hostname)
        };
        validate_ip(hostname)?;
    }
    if authority.chars().any(nfkc_delimiter) {
        return Err(Error::Other("URL authority contains an NFKC delimiter".into()));
    }
    Ok(())
}

fn invalid() -> Error {
    Error::Other("invalid bracketed URL authority".into())
}

fn validate_ip(hostname: &str) -> Result<()> {
    if let Some(rest) = hostname.strip_prefix(['v', 'V']) {
        let valid = rest.split_once('.').is_some_and(|(version, address)| {
            !version.is_empty()
                && version.bytes().all(|byte| byte.is_ascii_hexdigit())
                && !address.is_empty()
                && !address.contains('\n')
        });
        return if valid {
            Ok(())
        } else {
            Err(invalid())
        };
    }
    let address = if let Some((address, scope)) = hostname.split_once('%') {
        if scope.is_empty() || scope.contains('%') {
            return Err(invalid());
        }
        address
    } else {
        hostname
    };
    address.parse::<std::net::Ipv6Addr>().map(|_| ()).map_err(|_| invalid())
}

/// Unicode 15.1 scalars whose NFKC decomposition introduces / ? # @ or :.
/// The source comparison enumerates every scalar and tests combining contexts.
pub(in crate::plugin::index::transport) fn nfkc_delimiter(character: char) -> bool {
    matches!(
        character,
        '\u{2047}'
            | '\u{2048}'
            | '\u{2049}'
            | '\u{2100}'
            | '\u{2101}'
            | '\u{2105}'
            | '\u{2106}'
            | '\u{2a74}'
            | '\u{fe13}'
            | '\u{fe16}'
            | '\u{fe55}'
            | '\u{fe56}'
            | '\u{fe5f}'
            | '\u{fe6b}'
            | '\u{ff03}'
            | '\u{ff0f}'
            | '\u{ff1a}'
            | '\u{ff1f}'
            | '\u{ff20}'
    )
}
