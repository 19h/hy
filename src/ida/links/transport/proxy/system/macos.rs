//! SystemConfiguration snapshot; PAC and bypass settings are not HTTPX mounts.

use core_foundation::base::{CFType, TCFType};
use core_foundation::dictionary::CFDictionary;
use core_foundation::number::CFNumber;
use core_foundation::string::CFString;
use system_configuration_sys::dynamic_store_copy_specific::SCDynamicStoreCopyProxies;

use super::ProxyMap;

type Settings = CFDictionary<CFString, CFType>;

pub(super) fn discover() -> ProxyMap {
    // SAFETY: a null session requests the current snapshot. A non-null result
    // follows the Core Foundation Create Rule and is released by CFDictionary.
    let settings = unsafe {
        let raw = SCDynamicStoreCopyProxies(std::ptr::null());
        if raw.is_null() {
            return ProxyMap::new();
        }
        Settings::wrap_under_create_rule(raw)
    };
    from_settings(&settings)
}

fn from_settings(settings: &Settings) -> ProxyMap {
    let mut proxies = ProxyMap::new();
    for (scheme, prefix) in [
        ("http", "HTTP"),
        ("https", "HTTPS"),
        ("ftp", "FTP"),
        ("gopher", "Gopher"),
        ("socks", "SOCKS"),
    ] {
        if number(settings, &format!("{prefix}Enable")).is_none_or(|value| value == 0) {
            continue;
        }
        let Some(host) = settings
            .find(CFString::new(&format!("{prefix}Proxy")))
            .and_then(|value| value.downcast::<CFString>())
        else {
            continue;
        };
        // CPython _scproxy uses HTTP URLs for every scheme, including HTTPS.
        let mut address = format!("http://{host}");
        if let Some(port) = number(settings, &format!("{prefix}Port")) {
            address.push_str(&format!(":{port}"));
        }
        proxies.insert(scheme.into(), address);
    }
    proxies
}

fn number(settings: &Settings, key: &str) -> Option<i32> {
    settings.find(CFString::new(key))?.downcast::<CFNumber>()?.to_i32()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enabled_protocols_preserve_hosts_and_optional_ports() {
        let pairs = [
            ("HTTPEnable", CFNumber::from(1).as_CFType()),
            ("HTTPProxy", CFString::new("proxy.test").as_CFType()),
            ("HTTPPort", CFNumber::from(8080).as_CFType()),
            ("HTTPSEnable", CFNumber::from(-1).as_CFType()),
            ("HTTPSProxy", CFString::new("secure.test").as_CFType()),
            ("FTPEnable", CFNumber::from(0).as_CFType()),
            ("FTPProxy", CFString::new("disabled.test").as_CFType()),
            ("GopherEnable", CFNumber::from(1).as_CFType()),
            ("SOCKSEnable", CFNumber::from(1).as_CFType()),
            ("SOCKSProxy", CFString::new("socks.test").as_CFType()),
            ("SOCKSPort", CFNumber::from(0).as_CFType()),
        ];
        let pairs: Vec<_> =
            pairs.into_iter().map(|(key, value)| (CFString::new(key), value)).collect();
        assert_eq!(
            from_settings(&Settings::from_CFType_pairs(&pairs)),
            ProxyMap::from([
                ("http".into(), "http://proxy.test:8080".into()),
                ("https".into(), "http://secure.test".into()),
                ("socks".into(), "http://socks.test:0".into()),
            ])
        );
    }

    #[test]
    fn current_snapshot_matches_cpython_when_oracle_is_available() {
        let Some(python) = std::env::var_os("HY_TEST_HTTPX_ORACLE_PYTHON") else {
            return;
        };
        let output = std::process::Command::new(python)
            .args([
                "-I",
                "-B",
                "-c",
                "import _scproxy,json; print(json.dumps(_scproxy._get_proxies()))",
            ])
            .output()
            .expect("read CPython system proxy snapshot");
        assert!(output.status.success());
        let expected: ProxyMap = serde_json::from_slice(&output.stdout).expect("proxy map");
        assert_eq!(discover(), expected);
    }
}
