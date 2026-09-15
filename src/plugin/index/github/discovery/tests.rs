use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use serde_json::{Value, json};

use super::*;

#[test]
fn lists_selection_and_search_pages_match_upstream() {
    let mut cases = Vec::new();
    for separator in [
        "\n", "\r", "\r\n", "\u{b}", "\u{c}", "\u{1c}", "\u{1d}", "\u{1e}", "\u{85}", "\u{2028}",
        "\u{2029}",
    ] {
        for line in
            ["", " ", "#comment", " #indented", " Owner/Repo ", "\u{1f}A/B\u{1f}", "\u{feff}A/B"]
        {
            cases.push(
                json!({"kind": "list", "text": format!("{line}{separator}Other/Repo{separator}")}),
            );
        }
    }
    for name in [
        "Owner/Repo",
        "A/R",
        "a-b/r",
        "invalid",
        "a/b/c",
        "/",
        "/r",
        "a/",
        "../r",
        "a/..",
        "a/space name",
        "a/quote\"name",
        "a/back\\slash",
        "Ä/R",
        "Σ/ΟΣ",
        "a/\t",
        "a/\n",
        "a/\0",
    ] {
        for ignored in [vec![], vec![name.to_owned()], vec![name.to_lowercase()]] {
            cases.push(json!({
                "kind": "select",
                "candidates": ["z/r", name],
                "extra": ["A/R", name],
                "ignored": ignored,
            }));
        }
        if name.matches('/').count() == 1 && !name.contains('\0') {
            cases.push(json!({"kind": "component", "name": name}));
        }
    }
    for response in [
        json!({}),
        json!({"items": null}),
        json!({"items": false}),
        json!({"items": 0}),
        json!({"items": ""}),
        json!({"items": {}}),
        json!({"items": []}),
        json!({"items": true}),
        json!({"items": [null]}),
        json!({"items": [{}]}),
        json!({"items": [{"repository": {}}]}),
        json!({"items": "invalid"}),
        Value::Null,
        json!([]),
    ] {
        cases.push(json!({"kind": "search", "response": response}));
    }
    for count in [1, 99, 100, 101, 200] {
        let items: Vec<_> = (0..count)
            .map(|index| json!({"repository": {"full_name": format!("Owner/Repo{}", index % 3)}}))
            .collect();
        cases.push(json!({"kind": "search", "response": {"items": items}}));
    }
    assert_eq!(cases.len(), 165);
    let expected: Vec<Value> = cases.iter().map(observe).collect();
    compare_source(&cases, &expected);
}

pub(super) fn compare_source(cases: &[Value], expected: &[Value]) {
    let Some(python) = std::env::var_os("HY_TEST_BUNDLE_ORACLE_PYTHON") else {
        return;
    };
    let source = std::env::var_os("HY_TEST_HCLI_SOURCE").map(PathBuf::from).unwrap_or_else(|| {
        Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().join("ida-hcli")
    });
    let mut child = Command::new(python)
        .args(["-I", "-B", "-c", include_str!("reference.py")])
        .arg(source)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(&serde_json::to_vec(cases).unwrap()).unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    let actual: Vec<Value> = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(actual.len(), expected.len());
    for (index, (actual, expected)) in actual.iter().zip(expected).enumerate() {
        assert_eq!(actual, expected, "case {index}: {}", cases[index]);
    }
    eprintln!("matched {} upstream discovery cases", cases.len());
}

fn observe(case: &Value) -> Value {
    match case["kind"].as_str().unwrap() {
        "list" => json!({"value": parse_list(case["text"].as_str().unwrap())}),
        "select" => {
            let result = select(
                serde_json::from_value(case["candidates"].clone()).unwrap(),
                serde_json::from_value(case["extra"].clone()).unwrap(),
                serde_json::from_value(case["ignored"].clone()).unwrap(),
            );
            result.map_or_else(|_| json!({"error": true}), |names| json!({"value": names}))
        }
        "component" => {
            json!({"valid": validate_cache_name(case["name"].as_str().unwrap()).is_ok()})
        }
        "search" => {
            let mut urls = Vec::new();
            let mut names = values::Search::default();
            for query in ENCODED_QUERIES {
                urls.push(search_url("https://api.github.com", query, 1));
                match names.append(&case["response"]) {
                    Ok(count) => {
                        if count >= PAGE_SIZE {
                            urls.push(search_url("https://api.github.com", query, 2));
                        }
                    }
                    Err(_) => return json!({"error": true, "urls": urls}),
                }
            }
            json!({"value": names.finish().unwrap(), "urls": urls})
        }
        _ => unreachable!(),
    }
}

#[test]
fn ignored_invalid_names_are_removed_before_repository_parsing() {
    let names = select(BTreeSet::from(["INVALID".into()]), vec![], vec!["invalid".into()]).unwrap();
    assert!(names.is_empty());
    assert_eq!(parse_list(" #indented\n \n#comment\n"), ["#indented", ""]);
}
