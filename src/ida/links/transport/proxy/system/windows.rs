//! Internet Settings registry discovery and urllib's ProxyServer grammar.

use super::ProxyMap;

#[cfg(windows)]
pub(super) fn discover() -> ProxyMap {
    use winreg::RegKey;
    use winreg::enums::HKEY_CURRENT_USER;

    let settings = RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey(r"Software\Microsoft\Windows\CurrentVersion\Internet Settings");
    let Ok(settings) = settings else {
        return ProxyMap::new();
    };
    if settings.get_value::<u32, _>("ProxyEnable").unwrap_or(0) == 0 {
        return ProxyMap::new();
    }
    settings.get_value::<String, _>("ProxyServer").map(|value| parse(&value)).unwrap_or_default()
}

fn parse(server: &str) -> ProxyMap {
    let mut proxies = ProxyMap::new();
    let expanded;
    let server = if !server.contains(['=', ';']) {
        expanded = format!("http={server};https={server};ftp={server}");
        &expanded
    } else {
        server
    };
    for entry in server.split(';') {
        let Some((protocol, address)) = entry.split_once('=') else {
            // urllib catches ValueError around the whole loop, retaining any
            // earlier entries and skipping SOCKS fallback on malformed input.
            return proxies;
        };
        let address = if has_scheme(address) {
            address.to_owned()
        } else {
            match protocol {
                "http" | "https" | "ftp" => format!("http://{address}"),
                "socks" => format!("socks://{address}"),
                _ => address.to_owned(),
            }
        };
        proxies.insert(protocol.into(), address);
    }
    if let Some(address) = proxies.get("socks").filter(|value| !value.is_empty()) {
        let address = address
            .strip_prefix("socks://")
            .map(|host| format!("socks4://{host}"))
            .unwrap_or_else(|| address.clone());
        for scheme in ["http", "https"] {
            let value = proxies.entry(scheme.into()).or_default();
            if value.is_empty() {
                *value = address.clone();
            }
        }
    }
    proxies
}

fn has_scheme(address: &str) -> bool {
    address
        .split_once("://")
        .is_some_and(|(prefix, _)| !prefix.is_empty() && !prefix.contains(['/', ':']))
}

#[cfg(test)]
mod tests;
