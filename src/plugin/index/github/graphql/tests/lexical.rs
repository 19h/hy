//! Raw GraphQL JSON: strict text decoding, Python truthiness and error repr.

use super::super::super::http;
use super::*;

#[tokio::test]
async fn raw_envelopes_match_source_errors_truthiness_and_model_publication() {
    let mut bodies = Vec::new();
    let repository = cases::repository().to_string();
    for value in [
        "NaN",
        "Infinity",
        "-Infinity",
        "1e999",
        "0e999",
        "-0.0",
        r#""\ud800""#,
        r#""\udc80""#,
        r#""'\udfff\"""#,
        r#"{"\ud800":[NaN,Infinity,-Infinity,"\udfff"]}"#,
        r#"[NaN,Infinity,-Infinity,"\ud800"]"#,
        "{}",
        "[]",
        "null",
        "false",
        "true",
    ] {
        for body in [
            format!(r#"{{"data":{{"repo0":{repository}}},"ignored":{value}}}"#),
            format!(r#"{{"data":{{"repo0":{value}}}}}"#),
            format!(r#"{{"data":{value}}}"#),
            format!(r#"{{"errors":{value},"data":{{}}}}"#),
            format!(r#"{{"errors":[{{"type":"NOT_FOUND","message":{value}}}],"data":{{}}}}"#),
            format!(r#"{{"errors":[{{"type":{value},"message":"test"}}],"data":null}}"#),
            format!(r#"{{"errors":[{{"type":"FORBIDDEN","message":{value}}}],"data":null}}"#),
        ] {
            bodies.push(body.into_bytes());
        }
    }
    for bytes in [b"\xef\xbb\xbf{}".as_slice(), b"{\0}\0", b"\0{\0}", b"\xff"] {
        bodies.push(bytes.to_vec());
    }
    let cases: Vec<_> = bodies
        .into_iter()
        .map(|body| json!({"repositories":["owner/repo"], "body":body}))
        .collect();
    assert_eq!(cases.len(), 116);
    let mut expected = Vec::new();
    for case in &cases {
        let body: Vec<u8> = serde_json::from_value(case["body"].clone()).unwrap();
        let response = hyper::Response::new(reqwest::Body::from(body)).into();
        let result = http::read_graphql_json(response)
            .await
            .and_then(|value| decode(&["owner/repo".into()], &value));
        expected.push(match result {
            Ok(repositories) => {
                let models: Vec<_> =
                    repositories.iter().map(|(_, model)| model.cache_text().unwrap()).collect();
                json!({"models": models})
            }
            Err(Error::Other(message)) if message.starts_with("GraphQL errors:") => {
                json!({"fatal": message})
            }
            Err(_) => json!({"error": true}),
        });
    }
    compare_source(&cases, &expected);
}
