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
    documents.extend(extended_values());
    documents
}

fn extended_values() -> Vec<Vec<u8>> {
    let mut documents = Vec::new();
    for field in ["extra", "name", "browser_download_url", "size"] {
        for token in
            ["NaN", "Infinity", "-Infinity", r#""\ud800""#, r#""\udfff""#, r#""\ud800\udc00""#]
        {
            documents.push(raw_field(field, token));
        }
    }
    for name in [r#""\ud800.zip""#, r#""\udfff.ZIP""#, r#""\ud800\udc00.zip""#] {
        for size in ["0", "104857601"] {
            let document = String::from_utf8(raw_field("name", name)).unwrap();
            documents
                .push(document.replace("\"size\":0", &format!("\"size\":{size}")).into_bytes());
        }
    }
    for text in [
        r#"{"$serde_json::private::Number":"NaN","assets":[{"name":"a.zip","browser_download_url":"https://mirror.test/a","size":NaN}]}"#,
        r#"{"assets":[{"name":"a.zip","browser_download_url":"https://mirror.test/a"}],"unused":["\ud800",Infinity]}"#,
        r#"{"assets":[{"name":"\ud800.zip"},{"name":"other.zip"}]}"#,
    ] {
        documents.push(text.as_bytes().to_vec());
    }
    for depth in [128, 512, 1100] {
        let nested = format!("{}0{}", "[".repeat(depth), "]".repeat(depth));
        documents.push(raw_field("extra", &nested));
    }
    for point in [0xd800_u32, 0xdc00, 0xdfff] {
        for oversized in [false, true] {
            let text = String::from_utf8(raw_field(
                "size",
                if oversized {
                    "104857601"
                } else {
                    "0"
                },
            ))
            .unwrap();
            let points: Vec<_> = text.chars().map(u32::from).collect();
            let name_start = text.find("a.zip").unwrap();
            let mut points = points;
            points[name_start] = point;
            for encoding in 0..5 {
                let mut bytes = Vec::new();
                for &point in &points {
                    match encoding {
                        0 if (0xd800..=0xdfff).contains(&point) => bytes.extend([
                            0xe0 | (point >> 12) as u8,
                            0x80 | ((point >> 6) & 0x3f) as u8,
                            0x80 | (point & 0x3f) as u8,
                        ]),
                        0 => bytes.push(u8::try_from(point).unwrap()),
                        1 => bytes.extend((point as u16).to_le_bytes()),
                        2 => bytes.extend((point as u16).to_be_bytes()),
                        3 => bytes.extend(point.to_le_bytes()),
                        _ => bytes.extend(point.to_be_bytes()),
                    }
                }
                documents.push(bytes);
            }
        }
    }
    documents
}

fn raw_field(field: &str, token: &str) -> Vec<u8> {
    let fields = [
        ("name", r#""a.zip""#),
        ("browser_download_url", r#""https://mirror.test/a""#),
        ("size", "0"),
        ("extra", "null"),
    ]
    .into_iter()
    .map(|(name, value)| {
        let value = if name == field {
            token
        } else {
            value
        };
        format!("\"{name}\":{value}")
    })
    .collect::<Vec<_>>()
    .join(",");
    format!("{{\"assets\":[{{{fields}}}]}}").into_bytes()
}

fn with_size(size: &str) -> Vec<u8> {
    format!(
        r#"{{"assets":[{{"name":"a.zip","size":{size},"browser_download_url":"{url}"}}]}}"#,
        url = "https://mirror.test/a",
    )
    .into_bytes()
}
