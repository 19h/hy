//! Model acceptance and exact canonical JSON publication through actual source models.

use super::*;
use crate::util::python_json;

#[test]
fn raw_model_values_and_ascii_cache_text_match_upstream() {
    let mut cases = Vec::new();
    for (kind, baseline) in [("graphql", cases::graphql()), ("cache", cases::cached())] {
        let mut paths = Vec::new();
        field_paths(&baseline, "", &mut paths);
        for path in paths {
            for replacement in [
                "NaN",
                "Infinity",
                "-Infinity",
                "1e999",
                "0e999",
                "1.0",
                "-0.0",
                r#""\ud800""#,
                r#""\udc80""#,
                r#""\udfff""#,
                r#""A\ud800𐀀\udfffZ""#,
                r#""$serde_json::private::RawValue""#,
                r#"[NaN,"\ud800"]"#,
                r#"{"ignored":NaN,"\ud800":"\udfff"}"#,
            ] {
                let mut value = baseline.clone();
                *value.pointer_mut(&path).unwrap() = json!("__RAW_VALUE__");
                let body = value.to_string().replace("\"__RAW_VALUE__\"", replacement);
                cases.push(json!({"kind": kind, "body": body}));
            }
        }
        for replacement in [
            "NaN".to_owned(),
            "Infinity".to_owned(),
            r#""\ud800""#.to_owned(),
            format!("{}NaN{}", "[".repeat(512), "]".repeat(512)),
            "9".repeat(4300),
            "9".repeat(4301),
        ] {
            let mut value = baseline.clone();
            value["ignored"] = json!("__RAW_VALUE__");
            cases.push(json!({"kind":kind, "body": value.to_string().replace("\"__RAW_VALUE__\"", &replacement)}));
        }
        let surrogates: String =
            (0xd800..=0xdfff).map(|point| format!("\\u{point:04x}X")).collect();
        let body = baseline.to_string().replace("\"release\"", &format!("\"{surrogates}\""));
        cases.push(json!({"kind": kind, "body": body}));
    }
    assert_eq!(cases.len(), 882);
    let expected: Vec<_> = cases
        .iter()
        .map(|case| {
            let result = python_json::parse(case["body"].as_str().unwrap())
                .and_then(|value| {
                    if case["kind"] == "graphql" {
                        Repository::from_graphql(&value)
                    } else {
                        Repository::from_cached(&value)
                    }
                })
                .and_then(|model| model.cache_text());
            result.map_or_else(|_| json!({"error":true}), |text| json!({"text":text}))
        })
        .collect();
    compare_source(&cases, &expected);
}

fn field_paths(value: &Value, prefix: &str, paths: &mut Vec<String>) {
    match value {
        Value::Object(fields) => {
            for (key, value) in fields {
                let path = format!("{prefix}/{key}");
                paths.push(path.clone());
                field_paths(value, &path, paths);
            }
        }
        Value::Array(values) => {
            for (index, value) in values.iter().enumerate() {
                let path = format!("{prefix}/{index}");
                paths.push(path.clone());
                field_paths(value, &path, paths);
            }
        }
        _ => {}
    }
}
