use serde_json::{Value, json};

pub(super) fn documents() -> Vec<Vec<u8>> {
    let mut values = vec![json!(null), json!(false), json!(1), json!(""), json!([]), json!({})];
    for assets in [
        json!(null),
        json!(true),
        json!(1),
        json!(""),
        json!("x"),
        json!({}),
        json!({"a": 1}),
        json!([]),
    ] {
        values.push(json!({"assets": assets}));
    }
    for name in [
        None,
        Some(json!(null)),
        Some(json!(1)),
        Some(json!(false)),
        Some(json!([])),
        Some(json!({})),
        Some(json!("")),
        Some(json!("readme")),
        Some(json!("plugin.zip")),
        Some(json!("plugin.ZIP")),
        Some(json!("x.zip\n")),
        Some(json!("é.ZiP")),
    ] {
        for size in [
            None,
            Some(json!(null)),
            Some(json!(false)),
            Some(json!(true)),
            Some(json!(-1)),
            Some(json!(0)),
            Some(json!(0.5)),
            Some(json!(104857600)),
            Some(json!(104857601)),
            Some(json!(104857600.00000001)),
            Some(json!("100")),
            Some(json!([])),
            Some(json!({})),
        ] {
            for download in
                [None, Some(json!(null)), Some(json!(42)), Some(json!("https://mirror.test/a"))]
            {
                let mut asset = serde_json::Map::new();
                for (key, value) in
                    [("name", &name), ("size", &size), ("browser_download_url", &download)]
                {
                    if let Some(value) = value {
                        asset.insert(key.into(), value.clone());
                    }
                }
                values.push(json!({"assets": [{"name": "README"}, Value::Object(asset), {}]}));
            }
        }
    }
    for first in [json!({"name": "one.zip"}), json!({"name": "other"}), json!(null), json!(1)] {
        for second in [json!({"name": "two.ZIP"}), json!({"name": "other"}), json!(null), json!([])]
        {
            values.push(json!({"assets": [first, second]}));
        }
    }
    let mut documents: Vec<_> =
        values.iter().map(|value| serde_json::to_vec(value).unwrap()).collect();
    for size in ["1e309", "-1e309", "104857600.0000000001", "-0", "18446744073709551616"] {
        documents.push(with_size(size));
    }
    for count in [4300, 4301] {
        documents.push(with_size(&"9".repeat(count)));
    }
    let text = r#"{"assets":[{"name":"é.ZIP","browser_download_url":"https://mirror.test/a"}]}"#;
    documents.push([b"\xef\xbb\xbf".as_slice(), text.as_bytes()].concat());
    documents.push(text.encode_utf16().flat_map(u16::to_le_bytes).collect());
    documents.push(text.encode_utf16().flat_map(u16::to_be_bytes).collect());
    documents.push(text.chars().flat_map(|ch| u32::from(ch).to_le_bytes()).collect());
    documents.push(text.chars().flat_map(|ch| u32::from(ch).to_be_bytes()).collect());
    documents
}

fn with_size(size: &str) -> Vec<u8> {
    format!(
        r#"{{"assets":[{{"name":"a.zip","size":{size},"browser_download_url":"{url}"}}]}}"#,
        url = "https://mirror.test/a",
    )
    .into_bytes()
}
