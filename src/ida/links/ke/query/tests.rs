//! Cases evaluated against the pinned upstream helpers with CPython 3.13.15.

use super::*;

#[test]
fn raw_query_removal_and_navigation_detection_match_the_upstream_oracle() {
    let cases = [
        (
            "ida://ke/file.i64?url=x&rva=0x10&name=a%20b&view=raw+view",
            "ida://ke/file.i64?rva=0x10&name=a%20b&view=raw+view",
            true,
        ),
        (
            "IDA://Ke:invalid/file.i64?url=x&&rva=&flag&url=y&#part",
            "ida://Ke:invalid/file.i64?rva=&flag#part",
            true,
        ),
        ("ida://ke/file.i64?%75rl=x&rva=1&url=y", "ida://ke/file.i64?%75rl=x&rva=1", true),
        ("ida://ke/file.i64?URL=x&url=y&name=a%2fb", "ida://ke/file.i64?URL=x&name=a%2fb", true),
        ("ida://ke/file.i64?url=x#", "ida://ke/file.i64", false),
        ("ida://ke/file.i64?url=x&&", "ida://ke/file.i64", false),
        ("ida://ke/file.i64?url=x#url=keep", "ida://ke/file.i64#url=keep", false),
        ("\0 \tIDA://KE/f\tile.i64?u\rrl=x&rv\na=1#f\trag", "ida://KE/file.i64?rva=1#frag", true),
        (
            "ida://ke/file.i64?url=x&%72va=&keep=%2f%2F",
            "ida://ke/file.i64?%72va=&keep=%2f%2F",
            true,
        ),
        ("ida://ke/file.i64?url=x&EA=1", "ida://ke/file.i64?EA=1", false),
        ("ida://ke/file.i64?url=x&other=a+b", "ida://ke/file.i64?other=a+b", false),
        ("ida://ke/file.i64?url=x&view", "ida://ke/file.i64?view", true),
    ];
    let content = format!("https%3A%2F%2Fexample.test%2Fobjects%2F{}%2Fcontent", "a".repeat(64));
    for (original, stripped, navigate) in cases {
        assert_eq!(strip_download_parameter(original), stripped);
        let original =
            original.replace("=x", &format!("={content}")).replace("=y", &format!("={content}"));
        let parsed = ParsedLink::parse(&original).unwrap();
        let request = Request::parse(&original, &parsed).unwrap();
        assert_eq!(request.navigation.is_some(), navigate, "{original}");
    }
}

#[test]
fn the_last_download_parameter_wins_and_empty_values_precede_filename_errors() {
    let content = format!("https://example.test/objects/{}/content", "a".repeat(64));
    let original = format!("ida://ke/file.i64?url=invalid&url={content}");
    let parsed = ParsedLink::parse(&original).unwrap();
    assert_eq!(Request::parse(&original, &parsed).unwrap().content.as_str(), content);
    for original in [
        format!("ida://ke/file.i64?url={content}&url="),
        format!("ida://ke/file.i64/extra?url={content}&url="),
    ] {
        let parsed = ParsedLink::parse(&original).unwrap();
        let error = Request::parse(&original, &parsed).err().unwrap();
        assert!(error.to_string().contains("missing the 'url'"), "{error}");
    }
}

#[test]
fn repeated_route_separators_do_not_change_content_identity() {
    let hash = "a".repeat(64);
    let original =
        format!("ida://ke/file.i64?url=https://example.test/api//objects//{hash}//content///");
    let parsed = ParsedLink::parse(&original).unwrap();
    assert_eq!(Request::parse(&original, &parsed).unwrap().sha, hash);
}
