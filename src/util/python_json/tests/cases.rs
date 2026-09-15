pub(super) fn documents() -> Vec<Vec<u8>> {
    let mut documents = Vec::new();
    for token in [
        "null",
        "true",
        "false",
        "NaN",
        "Infinity",
        "-Infinity",
        "nan",
        "inf",
        "+Infinity",
        "-NaN",
        "",
        "nul",
        "True",
    ] {
        for prefix in ["", " ", "\r\n\t", "\u{a0}"] {
            for suffix in ["", " ", ",", "x"] {
                documents.push(format!("{prefix}{token}{suffix}").into_bytes());
            }
        }
    }
    for sign in ["", "-", "+"] {
        for integer in ["", "0", "00", "01", "1", "123", "١", "０"] {
            for fraction in ["", ".", ".0", ".123", ".١"] {
                for exponent in ["", "e", "e0", "E+1", "e-1", "e9999", "e-9999", "e١"] {
                    documents.push(format!("{sign}{integer}{fraction}{exponent}").into_bytes());
                }
            }
        }
    }
    for token in [
        "0",
        "-0",
        "1e309",
        "-1e309",
        "1e-9999",
        "-1e-9999",
        "0.10000000000000001",
        "9007199254740993",
        "104857600.0000000001",
    ] {
        documents.push(token.as_bytes().to_vec());
    }
    for length in [4299, 4300, 4301] {
        for prefix in ["", "-"] {
            for suffix in ["", ".0", "e0"] {
                documents.push(format!("{prefix}{}{suffix}", "9".repeat(length)).into_bytes());
            }
        }
    }
    for text in [
        r#""plain""#,
        r#""\"\\\/\b\f\n\r\t""#,
        r#""\u0000""#,
        r#""\ud800""#,
        r#""\udfff""#,
        r#""\ud800\udc00""#,
        r#""\ud800\ud800\udc00""#,
        r#""\udc00\ud800""#,
        r#""\ud800\u0041""#,
        r#""\ud800\uZZZZ""#,
        r#""\U0001F600""#,
        r#""\x41""#,
        r#""\u١٢٣٤""#,
        r#""\u12""#,
        "\"é𐀀😀\"",
        "\"raw\nnewline\"",
        "\"\u{1f}\"",
        "\"unterminated",
        "[]",
        "{}",
        "[1,]",
        "{\"a\":1,}",
        "[NaN,Infinity,-Infinity]",
        r#"{"a":1,"b":2,"\u0061":3}"#,
        r#"{"$serde_json::private::Number":"NaN"}"#,
        r#"{"\ud800":1,"\ud800":2,"𐀀":3,"\ud800\udc00":4}"#,
    ] {
        for bytes in encoded(text) {
            documents.push(bytes);
        }
    }
    for point in [0xd7ff_u32, 0xd800, 0xdbff, 0xdc00, 0xdfff, 0xe000, 0x10ffff, 0x110000] {
        for little in [false, true] {
            let mut bytes = Vec::new();
            for point in [u32::from(b'"'), point, u32::from(b'"')] {
                bytes.extend(if little {
                    point.to_le_bytes()
                } else {
                    point.to_be_bytes()
                });
            }
            documents.push(bytes);
        }
    }
    for middle in [
        vec![0xed, 0xa0, 0x80],
        vec![0xed, 0xbf, 0xbf],
        vec![0xed, 0xa0, 0x80, 0xed, 0xb0, 0x80],
        vec![0xff],
        vec![0xc0, 0x80],
    ] {
        documents.push([b"\"".as_slice(), &middle, b"\""].concat());
    }
    for depth in [0, 1, 127, 128, 129, 256, 512, 900, 1100] {
        documents.push(format!("{}0{}", "[".repeat(depth), "]".repeat(depth)).into_bytes());
        documents.push(format!("{}0{}", "{\"x\":".repeat(depth), "}".repeat(depth)).into_bytes());
    }
    // Deterministic insertion/deletion mutations cover delimiters and quoted text.
    for text in [r#"{"a":[true,null,1.25,"x"]}"#, r#"[NaN,{"b":"\ud800"}]"#] {
        for offset in 0..=text.len() {
            for byte in b"\"\\,:[]{} 01n" {
                let mut bytes = text.as_bytes().to_vec();
                bytes.insert(offset, *byte);
                documents.push(bytes);
            }
            if offset < text.len() {
                let mut bytes = text.as_bytes().to_vec();
                bytes.remove(offset);
                documents.push(bytes);
            }
        }
    }
    documents
}

fn encoded(text: &str) -> Vec<Vec<u8>> {
    vec![
        text.as_bytes().to_vec(),
        [b"\xef\xbb\xbf".as_slice(), text.as_bytes()].concat(),
        text.encode_utf16().flat_map(u16::to_le_bytes).collect(),
        text.encode_utf16().flat_map(u16::to_be_bytes).collect(),
        text.chars().flat_map(|ch| u32::from(ch).to_le_bytes()).collect(),
        text.chars().flat_map(|ch| u32::from(ch).to_be_bytes()).collect(),
    ]
}
