//! Compare selection outcomes, request ordering and cache publication with source.

use serde_json::{Value, json};

use super::tests::{decode_fixture, utf8_names};
use super::*;

#[test]
fn candidate_roots_and_scalar_search_names_match_source_call_boundaries() {
    let mut cases = Vec::new();
    for root in [
        Value::Null,
        json!(false),
        json!(true),
        json!(0),
        json!(-1),
        json!(0.5),
        json!(""),
        json!("/"),
        json!("Aa/🧠"),
        json!([]),
        json!({}),
        json!(["OWNER/REPO", "owner/repo"]),
        json!([null]),
        json!([false]),
        json!([0]),
        json!([[]]),
        json!([{}]),
        json!({"Owner/Repo": null, "OTHER/REPO": ["unused value"]}),
        json!({"invalid": 1}),
        json!({"a/b/c": false}),
    ] {
        for ignored in [json!([]), json!(["a", "/", "🧠", "invalid", "a/b/c"])] {
            cases.push(json!({"kind": "cache", "root": root, "ignored": ignored}));
        }
    }
    for name in [
        Value::Null,
        json!(false),
        json!(true),
        json!(0),
        json!(-1),
        json!(0.5),
        json!("Owner/Repo"),
        json!("invalid"),
        json!([]),
        json!({}),
        json!(["Owner/Repo"]),
        json!({"key": 1}),
    ] {
        for count in [1, 100, 101] {
            for mixed in [false, true] {
                let mut items = vec![json!({"repository": {"full_name": name}}); count];
                if mixed {
                    items.push(json!({"repository": {"full_name": "Other/Repo"}}));
                }
                cases.push(json!({"kind": "pipeline", "response": {"items": items}}));
            }
        }
    }
    assert_eq!(cases.len(), 112);
    let expected: Vec<_> = cases.iter().map(observe).collect();
    super::tests::compare_source(&cases, &expected);
}

fn observe(case: &Value) -> Value {
    let mut urls = Vec::new();
    let mut published = None;
    let result = if case["kind"] == "cache" {
        values::cached(&decode_fixture(&case["root"])).and_then(|names| {
            select(names, Vec::new(), serde_json::from_value(case["ignored"].clone()).unwrap())
        })
    } else {
        discover(case, &mut urls).and_then(|names| {
            let names = lowercase(names);
            published = Some(utf8_names(names.clone()));
            select(names, Vec::new(), Vec::new())
        })
    };
    let mut outcome =
        result.map_or_else(|_| json!({"error": true}), |names| json!({"value": utf8_names(names)}));
    outcome["urls"] = json!(urls);
    outcome["published"] = json!(published);
    outcome
}

fn discover(case: &Value, urls: &mut Vec<String>) -> Result<BTreeSet<Text>> {
    let mut names = values::Search::default();
    for query in ENCODED_QUERIES {
        urls.push(search_url("https://api.github.com", query, 1));
        if names.append(&decode_fixture(&case["response"]))? >= PAGE_SIZE {
            urls.push(search_url("https://api.github.com", query, 2));
            names.append(&decode_fixture(&json!({})))?;
        }
    }
    names.finish()
}
