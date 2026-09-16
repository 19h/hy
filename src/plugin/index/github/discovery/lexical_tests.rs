//! Raw response/cache text compared with the pinned source decoding pipeline.

use serde_json::{Value, json};

use super::*;

#[tokio::test]
async fn raw_json_preserves_python_values_and_publication_boundaries() {
    let cases = cases();
    assert_eq!(cases.len(), 276);
    let mut expected = Vec::with_capacity(cases.len());
    for case in &cases {
        expected.push(observe(case).await);
    }
    super::tests::compare_source(&cases, &expected);
}

fn cases() -> Vec<Value> {
    let mut documents = vec![
        "{}".to_owned(),
        "[]".to_owned(),
        "null".to_owned(),
        r#""A/\ud800""#.to_owned(),
        r#"{"Owner/Repo":NaN,"Other/Repo":"\udfff"}"#.to_owned(),
        r#"["Owner/Repo","\ud800/REPO","Ω/ΟΣ","Ω/ΟΣ\udfffΑ"]"#.to_owned(),
        r#"{"items":[],"ignored":[NaN,Infinity,-Infinity,"\ud800"]}"#.to_owned(),
        r#"{"items":[{"repository":{"full_name":"Owner/Repo"}}],"ignored":1e999}"#.to_owned(),
        r#"{"items":NaN,"items":[]}"#.to_owned(),
    ];
    for value in [
        "NaN",
        "Infinity",
        "-Infinity",
        "1e999",
        "-1e999",
        "0e999",
        "-0.0",
        "nan",
        "-NaN",
        "01",
        "+1",
        "1.",
        "1e",
        r#""\ud800/REPO""#,
        r#""OWNER/\udfff""#,
        r#""\ud800""#,
        r#""Ο/ΟΣ\ud800Α""#,
        r#""Ο/Α\udfffΟΣ""#,
        r#""\ud83e\udde0/Repo""#,
        r#"{"\ud800":NaN}"#,
        "[NaN]",
    ] {
        documents.push(value.to_owned());
        documents.push(format!("[{value}]"));
        documents.push(format!(r#"{{"items":{value}}}"#));
        for count in [1, PAGE_SIZE] {
            let item = format!(r#"{{"repository":{{"full_name":{value}}}}}"#);
            documents.push(format!(r#"{{"items":[{}]}}"#, vec![item; count].join(",")));
        }
    }
    for depth in [128, 512] {
        documents.push(format!(
            r#"{{"items":[],"ignored":{}NaN{}}}"#,
            "[".repeat(depth),
            "]".repeat(depth)
        ));
    }
    for digits in [4300, 4301] {
        documents.push(format!(r#"{{"items":[],"ignored":{}}}"#, "9".repeat(digits)));
    }
    let surrogates: String = (0xd800..=0xdfff).map(|point| format!("\\u{point:04x}AΣ")).collect();
    documents.push(format!(r#"["Owner/{surrogates}"]"#));
    documents
        .push(format!(r#"{{"items":[{{"repository":{{"full_name":"Owner/{surrogates}"}}}}]}}"#));
    let mut bodies: Vec<Vec<u8>> = documents.into_iter().map(String::into_bytes).collect();
    for body in [b"{}".as_slice(), br#"{"items":[]}"#, br#"["Owner/Repo"]"#] {
        bodies.push([b"\xef\xbb\xbf".as_slice(), body].concat());
        bodies.push(body.iter().flat_map(|&byte| [byte, 0]).collect());
        bodies.push(body.iter().flat_map(|&byte| [0, byte]).collect());
        bodies.push(body.iter().flat_map(|&byte| [byte, 0, 0, 0]).collect());
        bodies.push(body.iter().flat_map(|&byte| [0, 0, 0, byte]).collect());
    }
    for invalid in [b"\xed\xa0\x80".as_slice(), b"\xff", b"\xe2\x82"] {
        bodies.push([br#"{"ignored":""#.as_slice(), invalid, br#"","items":[]}"#].concat());
    }
    bodies
        .into_iter()
        .flat_map(|body| {
            ["raw_cache", "raw_pipeline"].map(|kind| json!({"kind": kind, "body": body}))
        })
        .collect()
}

async fn observe(case: &Value) -> Value {
    let body: Vec<u8> = serde_json::from_value(case["body"].clone()).unwrap();
    let mut urls = Vec::new();
    let mut published = None;
    let result = if case["kind"] == "raw_cache" {
        python_utf8::decode(&body)
            .and_then(python_json::parse)
            .and_then(|value| values::cached(&value))
    } else {
        discover(&body, &mut urls).await.map(|names| {
            let names = lowercase(names);
            published = Some(values::encode(&names));
            names
        })
    };
    let mut outcome = result.and_then(|names| select(names, vec![], vec![])).map_or_else(
        |_| json!({"error": true}),
        |names| {
            let points: Vec<Vec<_>> =
                names.iter().map(|name| name.codepoints().collect()).collect();
            json!({"value": points})
        },
    );
    outcome["urls"] = json!(urls);
    outcome["published"] = json!(published);
    outcome
}

async fn discover(body: &[u8], urls: &mut Vec<String>) -> Result<BTreeSet<Text>> {
    let mut names = values::Search::default();
    for query in ENCODED_QUERIES {
        urls.push(search_url("https://api.github.com", query, 1));
        let response = hyper::Response::new(reqwest::Body::from(body.to_vec())).into();
        let value = http::read_search_json(response).await?;
        if names.append(&value)? >= PAGE_SIZE {
            urls.push(search_url("https://api.github.com", query, 2));
            names.append(&python_json::parse("{}")?)?;
        }
    }
    names.finish()
}
