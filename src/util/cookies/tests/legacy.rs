//! Set-Cookie2 participates only alongside Netscape headers under default policy.

use super::{Case, Store, assert_cases};

const URL: &str = "https://example.test/dir/page";

fn mixed(netscape: &[&str], legacy: &[&str], expected: Option<&str>) -> Case {
    let mut case = Case::new(URL, netscape, URL, expected);
    case.stores[0].cookie2 = legacy.iter().map(|value| (*value).into()).collect();
    case
}

#[test]
fn legacy_selection_quoting_versions_and_precedence_match_httpx() {
    let cases = [
        mixed(&[], &["b=two; Version=0"], None),
        mixed(&[""], &["b=two; Version=0"], Some("b=two")),
        mixed(&["=invalid"], &["b=two; Version=0"], Some("b=two")),
        mixed(&["a=one"], &["b=two; Version=0"], Some("b=two; a=one")),
        mixed(&["a=one"], &["b=two; Version=0; Path=/"], Some("a=one; b=two")),
        mixed(&["a=one"], &["b=two"], Some("a=one")),
        mixed(&["a=one"], &["b=two; Version=1"], Some("a=one")),
        mixed(&["a=one"], &["b=two; Version=-1"], Some("b=two; a=one")),
        mixed(&["a=one"], &["b=two; Version=\"0\"; Port=\"80,81\""], Some("b=two; a=one")),
        mixed(&["a=one"], &["b=two; Version=\"0"], Some("a=one")),
        mixed(&["a=one"], &["b=two; Version=0, c=three; Version=0"], Some("b=two; c=three; a=one")),
        mixed(&["a=one"], &[r#"b="one,two;three"; Version=0"#], Some("b=one,two;three; a=one")),
        mixed(&["a=one"], &[r#"b="one\"two"; Version=0"#], Some("b=one\"two; a=one")),
        mixed(
            &["a=netscape"],
            &["a=legacy; Version=0, b=two; Version=0"],
            Some("a=netscape; b=two"),
        ),
        mixed(&["a=one"], &["b=two; Version=0; Expires=0"], Some("a=one")),
        mixed(&["a=one"], &["b=two; Version=0; Expires"], Some("b=two; a=one")),
        mixed(&["a=one"], &["b=two; Version=0; Max-Age=100; Expires=0"], Some("b=two; a=one")),
        mixed(&["a=one"], &["b=two; Version=0; Expires=0; Max-Age=100"], Some("b=two; a=one")),
        mixed(&["a=one"], &["b=two; Version=invalid; Expires=0"], Some("a=one")),
        mixed(&["a=one; Max-Age"], &["b=two; Version=0"], Some("b=two")),
        mixed(
            &["a=one; Expires=Wed, 01 Jax 2038 00:00:00 GMT"],
            &["b=two; Version=0"],
            Some("b=two"),
        ),
    ];
    assert_cases(&cases);
}

#[test]
fn legacy_and_netscape_batches_mutate_existing_cookies_before_insertion() {
    let overflow = format!("bad=overflow; Version=0; Max-Age={}", "9".repeat(309));
    let mut cases = Vec::new();
    for (netscape, legacy, expected) in [
        (vec!["sid=deleted; Max-Age=0", "a=one"], vec!["sid=new; Version=0"], "sid=new; a=one"),
        (vec!["sid=new"], vec!["sid=deleted; Version=0; Max-Age=0"], "sid=new"),
        (vec!["a=one"], vec!["b=pending; Version=0", overflow.as_str()], "sid=original; a=one"),
        (
            vec!["a=one"],
            vec!["b=pending; Version=0", "bad=date; Version=0; Expires=0"],
            "sid=original; a=one",
        ),
        (vec!["a=one"], vec!["sid=deleted; Version=0; Max-Age=0", overflow.as_str()], "a=one"),
        (
            vec!["a=one"],
            vec!["sid=deleted; Version=0; Max-Age=0", "bad=value; Max-Age"],
            "sid=original; a=one",
        ),
        (vec!["a=one"], vec!["sid=deleted; Max-Age=0; Path=/dir"], "a=one"),
        (vec!["a=one"], vec!["sid=deleted; Version=1; Max-Age=0; Path=/dir"], "a=one"),
    ] {
        let mut case = Case::new(URL, &["sid=original"], URL, Some(expected));
        case.stores.push(Store {
            source: URL.into(),
            headers: netscape.into_iter().map(str::to_owned).collect(),
            cookie2: legacy.into_iter().map(str::to_owned).collect(),
        });
        cases.push(case);
    }
    assert_cases(&cases);
}
