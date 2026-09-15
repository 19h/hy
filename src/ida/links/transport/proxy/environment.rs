//! urllib's two-pass environment precedence and lazy system fallback.

use super::system::ProxyMap;

pub(super) fn discover(
    entries: Vec<(String, String)>,
    system: impl FnOnce() -> ProxyMap,
) -> ProxyMap {
    let mut values = ProxyMap::new();
    for (name, value) in &entries {
        if let Some(scheme) = name.to_lowercase().strip_suffix("_proxy")
            && !value.is_empty()
        {
            values.insert(scheme.to_owned(), value.clone());
        }
    }
    // CGI request headers must not supply the uppercase HTTP_PROXY setting.
    if entries.iter().any(|(name, _)| name == "REQUEST_METHOD") {
        values.remove("http");
    }
    for (name, value) in entries {
        if let Some(scheme) = name.strip_suffix("_proxy") {
            if value.is_empty() {
                values.remove(&scheme.to_lowercase());
            } else {
                values.insert(scheme.to_lowercase(), value);
            }
        }
    }
    // Unknown schemes also suppress fallback; filtering happens in Config.
    if values.is_empty() {
        system()
    } else {
        values
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn system_fallback_depends_on_the_complete_environment_map() {
        let cases: &[(&[(&str, &str)], bool)] = &[
            (&[], true),
            (&[("HTTP_PROXY", "")], true),
            (&[("HTTP_PROXY", "proxy"), ("http_proxy", "")], true),
            (&[("HTTP_PROXY", "proxy"), ("REQUEST_METHOD", "GET")], true),
            (&[("NO_PROXY", "*")], false),
            (&[("FTP_PROXY", "proxy")], false),
            (&[("UNUSED_PROXY", "proxy")], false),
            (&[("HTTP_PROXY", "proxy")], false),
            (&[("HTTP_proxy", "proxy"), ("REQUEST_METHOD", "GET")], false),
        ];
        for (entries, expected_fallback) in cases {
            let mut called = false;
            let values = discover(
                entries.iter().map(|(key, value)| ((*key).into(), (*value).into())).collect(),
                || {
                    called = true;
                    ProxyMap::from([("http".into(), "http://system.test".into())])
                },
            );
            assert_eq!(called, *expected_fallback, "{entries:?}");
            if called {
                assert_eq!(values["http"], "http://system.test");
            }
        }
    }
}
