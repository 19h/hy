//! Numeric grammar, deferred conversion and failure-stage ordering.

use super::{Case, NOW, Store, assert_cases};

#[test]
fn unicode_and_large_cookie_attributes_match_httpx() {
    let url = "https://example.test/";
    let mut cases = Vec::new();
    for (attribute, expected) in [
        ("Max-Age=١_۰۰۰".into(), Some("sid=one")),
        ("Max-Age=+９９９".into(), Some("sid=one")),
        ("Max-Age=²".into(), None),
        ("Max-Age=9223372036854775808".into(), Some("sid=one")),
        (format!("Max-Age={}", "9".repeat(308)), Some("sid=one")),
        (format!("Max-Age={}", "9".repeat(309)), None),
        (format!("Max-Age=-{}", "9".repeat(4300)), None),
        (format!("Max-Age={}", "0".repeat(4301)), None),
        ("Version=１".into(), Some("sid=one")),
        ("Version=\"１".into(), Some("sid=one")),
        ("Version=１\"".into(), Some("sid=one")),
        ("Version=\"\"１\"\"".into(), None),
        ("Version=-٠".into(), Some("sid=one")),
        (format!("Version=-{}", "9".repeat(4300)), Some("sid=one")),
        (format!("Version={}", "9".repeat(4300)), None),
        (format!("Version=-{}", "9".repeat(4301)), None),
        ("Port=８０".into(), None),
        ("Port=８０,80".into(), Some("sid=one")),
        ("Port=8_0,80".into(), Some("sid=one")),
        (format!("Port={},80", "9".repeat(4300)), Some("sid=one")),
        (format!("Port={},80", "9".repeat(4301)), None),
    ] {
        cases.push(Case::new(url, &[&format!("sid=one; {attribute}")], url, expected));
    }
    // Negative versions skip the source-domain suffix check in Python's policy.
    cases.push(Case::new(
        url,
        &["sid=one; Version=-１; Domain=other.test"],
        "https://other.test/",
        Some("sid=one"),
    ));
    cases.push(Case::new(
        "https://localhost/",
        &["sid=one; Version=-1; Domain=localhost"],
        "https://localhost/",
        Some("sid=one"),
    ));
    assert_cases(&cases);
}

#[test]
fn normalization_and_construction_failures_have_distinct_batch_effects() {
    let url = "https://example.test/";
    let mut cases = Vec::new();
    for (attribute, expected) in [
        ("Max-Age".into(), Some("sid=original")),
        ("Version=invalid; Max-Age".into(), Some("sid=original")),
        ("Version; Max-Age".into(), Some("new=one")),
        ("Domain; Max-Age".into(), Some("new=one")),
        ("Max-Age; Domain".into(), Some("sid=original")),
        ("Max-Age=invalid; Max-Age".into(), Some("new=one")),
        (format!("Max-Age={}", "9".repeat(309)), None),
        (format!("Max-Age={}", "9".repeat(4300)), None),
        (format!("Max-Age={}", "9".repeat(4301)), Some("new=one")),
        (format!("Version=2; Max-Age={}", "9".repeat(309)), None),
        (format!("Version=invalid; Max-Age={}", "9".repeat(309)), Some("new=one")),
        (format!("Max-Age=-{}", "9".repeat(4300)), Some("new=one")),
    ] {
        let mut case = Case::new(url, &["sid=original"], url, expected);
        case.stores.push(Store {
            source: url.into(),
            cookie2: Vec::new(),
            headers: vec![
                "sid=deleted; Max-Age=0".into(),
                "new=one".into(),
                format!("invalid=value; {attribute}"),
            ],
        });
        cases.push(case);
    }
    assert_cases(&cases);
}

#[test]
fn expiry_storage_rounds_through_binary64_like_python() {
    let url = "https://example.test/";
    let boundary = 1_i64 << 53;
    let mut cases = Vec::new();
    for (extra, elapsed, expected) in [
        (1, boundary - NOW - 1, Some("sid=one")),
        (1, boundary - NOW, None),
        (3, boundary - NOW + 3, Some("sid=one")),
        (3, boundary - NOW + 4, None),
    ] {
        let age = boundary - NOW + extra;
        let mut case = Case::new(url, &[&format!("sid=one; Max-Age={age}")], url, expected);
        case.elapsed = elapsed;
        cases.push(case);
    }
    let largest = num_bigint::BigInt::from(2).pow(1024) - num_bigint::BigInt::from(2).pow(970);
    for (offset, expected) in [(-1, Some("sid=one")), (0, None), (1, None)] {
        let age = &largest - NOW + offset;
        cases.push(Case::new(url, &[&format!("sid=one; Max-Age={age}")], url, expected));
    }
    assert_cases(&cases);
}
