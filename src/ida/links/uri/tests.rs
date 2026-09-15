//! Expected targets captured from the pinned upstream DefaultURLHandler.

use super::*;

#[test]
fn database_targets_preserve_the_upstream_uri_and_raw_filename() {
    for (input, expected_uri, expected_name, expected_source) in [
        ("ida:///sample.i64", "ida:///sample.i64/functions", "sample.i64", ""),
        ("ida:///sample.i64?", "ida:///sample.i64?/functions", "sample.i64", ""),
        ("ida:///sample.i64#anchor", "ida:///sample.i64#anchor/functions", "sample.i64", ""),
        ("ida:///sample.i64///", "ida:///sample.i64/functions", "sample.i64", ""),
        (
            "ida://SRC:bad/a%20b.i64/functions?ea=1",
            "ida://SRC:bad/a%20b.i64/functions?ea=1",
            "a%20b.i64",
            "src",
        ),
        ("ida:///a/../b.i64", "ida:///a/../b.i64", "a", ""),
        ("ida:sample.i64", "ida:sample.i64/functions", "sample.i64", ""),
        (" ida:///a\tb.i64 ", " ida:///a\tb.i64 /functions", "ab.i64 ", ""),
        (
            "ida://user:pass@[FE80::1%Zone]/a.i64",
            "ida://user:pass@[FE80::1%Zone]/a.i64/functions",
            "a.i64",
            "fe80::1%Zone",
        ),
        ("ida://[v1.A]/a.i64", "ida://[v1.A]/a.i64/functions", "a.i64", "v1.a"),
    ] {
        let target = ParsedLink::parse(input).unwrap().default_target().unwrap();
        let DefaultTarget::Database {
            uri,
            name,
            source,
        } = target
        else {
            panic!("expected database target for {input:?}");
        };
        assert_eq!(
            (uri.as_str(), name.as_str(), source.as_str()),
            (expected_uri, expected_name, expected_source),
            "{input:?}"
        );
    }
}

#[test]
fn relative_queries_allow_an_empty_path_but_empty_queries_do_not() {
    for input in ["ida:///?rva=0", "ida:///functions?rva=1"] {
        let DefaultTarget::Relative {
            uri,
        } = ParsedLink::parse(input).unwrap().default_target().unwrap()
        else {
            panic!("expected relative target for {input}");
        };
        assert_eq!(uri, input);
    }
    for input in ["ida://", "ida:///?", "ida:///#fragment"] {
        assert!(ParsedLink::parse(input).unwrap().default_target().is_err(), "{input}");
    }
}

#[test]
fn malformed_schemes_and_bracketed_hosts_are_rejected() {
    for input in [
        "https://source/file",
        "file.i64",
        "ida://[missing/file",
        "ida://source]/file",
        "ida://[127.0.0.1]/file",
    ] {
        assert!(ParsedLink::parse(input).is_err(), "{input}");
    }
}
