//! Pinned upstream IP classification, including CPython 3.13 registry exceptions.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};

use crate::error::{Error, Result};

pub(super) fn validate(addresses: Vec<SocketAddr>) -> Result<Vec<SocketAddr>> {
    let mut unique = Vec::new();
    for address in addresses {
        if blocked(address.ip()) {
            return Err(Error::Other("KE content host resolves to a non-public address".into()));
        }
        if !unique.contains(&address) {
            unique.push(address);
        }
    }
    if unique.is_empty() {
        return Err(Error::Other("KE host did not resolve to any address".into()));
    }
    Ok(unique)
}

pub(super) fn blocked(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => {
            let [a, b, c, d] = ip.octets();
            ip.is_private()
                || ip.is_loopback()
                || ip.is_link_local()
                || ip.is_multicast()
                || ip.is_unspecified()
                || ip.is_broadcast()
                || ip.is_documentation()
                || a == 0
                || a >= 240
                || (a == 100 && (64..128).contains(&b))
                || (a == 198 && (b == 18 || b == 19))
                || (a == 192 && b == 0 && c == 0 && !matches!(d, 9 | 10))
        }
        IpAddr::V6(ip) => blocked_v6(ip),
    }
}

fn blocked_v6(ip: Ipv6Addr) -> bool {
    if let Some(v4) = ip.to_ipv4_mapped() {
        return blocked(v4.into());
    }
    let segments = ip.segments();
    let [first, second, third, ..] = segments;
    if first == 0x2002 {
        let embedded = Ipv4Addr::from((u32::from(second) << 16) | u32::from(third));
        return blocked(embedded.into());
    }
    // CPython's predicates block all other ranges outside 2000::/3 except
    // fec0::/10 (deprecated site-local). Preserve that upstream exception.
    if first & 0xe000 != 0x2000 {
        return first & 0xffc0 != 0xfec0;
    }
    if first == 0x2001 && second < 0x0200 {
        return !private_2001_exception(segments);
    }
    (first == 0x2001 && second == 0x0db8) || (first == 0x3fff && second & 0xf000 == 0)
}

fn private_2001_exception(segments: [u16; 8]) -> bool {
    match segments {
        [0x2001, 1, 0, 0, 0, 0, 0, 1 | 2] => true,
        [0x2001, 3, ..] => true,
        [0x2001, 4, 0x112, ..] => true,
        [0x2001, second, ..] => matches!(second & 0xfff0, 0x20 | 0x30),
        _ => false,
    }
}
