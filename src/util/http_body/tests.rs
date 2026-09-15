use std::io::Write;
use std::process::{Command, Stdio};

use flate2::Compression;
use flate2::write::{DeflateEncoder, GzEncoder, ZlibEncoder};
use reqwest::header::{CONTENT_ENCODING, HeaderMap, HeaderValue};
use serde::Serialize;

use super::Decoder;

fn encode(encoding: &str, bytes: &[u8]) -> Vec<u8> {
    match encoding {
        "gzip" => {
            let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
            encoder.write_all(bytes).unwrap();
            encoder.finish().unwrap()
        }
        "deflate" => {
            let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
            encoder.write_all(bytes).unwrap();
            encoder.finish().unwrap()
        }
        "raw" => {
            let mut encoder = DeflateEncoder::new(Vec::new(), Compression::default());
            encoder.write_all(bytes).unwrap();
            encoder.finish().unwrap()
        }
        _ => bytes.to_vec(),
    }
}

#[derive(Serialize)]
struct Case {
    headers: Vec<Vec<u8>>,
    chunks: Vec<Vec<u8>>,
    expected: Option<Vec<u8>>,
}

fn decoded(case: &Case, limit: u64) -> crate::error::Result<Vec<u8>> {
    let mut headers = HeaderMap::new();
    for value in &case.headers {
        headers.append(CONTENT_ENCODING, HeaderValue::from_bytes(value).unwrap());
    }
    let mut decoder = Decoder::new(&headers, limit);
    let mut output = Vec::new();
    for chunk in &case.chunks {
        output.extend_from_slice(&decoder.decode(chunk)?);
    }
    output.extend_from_slice(&decoder.finish()?);
    Ok(output)
}

fn assert_cases(cases: &[Case]) {
    for (index, case) in cases.iter().enumerate() {
        assert_eq!(decoded(case, u64::MAX).ok(), case.expected, "case {index}");
    }
    let Some(python) = std::env::var_os("HY_TEST_HTTPX_ORACLE_PYTHON") else {
        return;
    };
    const ORACLE: &str = r#"
import json, sys, httpx
class Chunks(httpx.SyncByteStream):
    def __init__(self, chunks):
        self.chunks = chunks
    def __iter__(self):
        yield from (bytes(chunk) for chunk in self.chunks)
results = []
for case in json.load(sys.stdin):
    headers = [(b'content-encoding', bytes(value)) for value in case['headers']]
    response = httpx.Response(200, headers=headers, stream=Chunks(case['chunks']))
    try:
        results.append(list(b''.join(response.iter_bytes())))
    except httpx.DecodingError:
        results.append(None)
json.dump(results, sys.stdout)
"#;
    let mut child = Command::new(python)
        .args(["-I", "-B", "-c", ORACLE])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(&serde_json::to_vec(cases).unwrap()).unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let actual: Vec<Option<Vec<u8>>> = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(actual.len(), cases.len());
    for (index, (case, actual)) in cases.iter().zip(actual).enumerate() {
        assert_eq!(actual, case.expected, "HTTPX case {index}");
    }
    eprintln!("HTTPX content-decoding oracle matched {} cases", cases.len());
}

#[test]
fn gzip_deflate_and_raw_deflate_match_httpx() {
    let payload = b"fixture payload: \x00\xff with binary bytes";
    let mut cases = Vec::new();
    for encoding in ["gzip", "deflate", "raw"] {
        let body = encode(encoding, payload);
        let header = if encoding == "raw" {
            "deflate"
        } else {
            encoding
        };
        for split in [body.len(), 2, 5, 13] {
            cases.push(Case {
                headers: vec![header.as_bytes().to_vec()],
                chunks: body.chunks(split).map(<[u8]>::to_vec).collect(),
                expected: Some(payload.to_vec()),
            });
        }
    }
    assert_cases(&cases);
}

#[test]
fn stacked_repeated_and_unknown_encodings_match_httpx() {
    let payload = b"decoded object";
    let cases = [
        Case {
            headers: vec![],
            chunks: vec![payload.to_vec()],
            expected: Some(payload.to_vec()),
        },
        Case {
            headers: vec![b"unknown, identity, br, zstd".to_vec()],
            chunks: vec![payload.to_vec()],
            expected: Some(payload.to_vec()),
        },
        Case {
            headers: vec![b"gzip, deflate".to_vec()],
            chunks: vec![encode("deflate", &encode("gzip", payload))],
            expected: Some(payload.to_vec()),
        },
        Case {
            headers: vec![b"gzip".to_vec(), b"gzip".to_vec()],
            chunks: vec![encode("gzip", &encode("gzip", payload))],
            expected: Some(payload.to_vec()),
        },
        Case {
            headers: vec![b" unknown , GZip , identity ".to_vec()],
            chunks: vec![encode("gzip", payload)],
            expected: Some(payload.to_vec()),
        },
        Case {
            headers: vec![b"\xa0gzip\xa0".to_vec()],
            chunks: vec![encode("gzip", payload)],
            expected: Some(payload.to_vec()),
        },
    ];
    assert_cases(&cases);
}

#[test]
fn incomplete_trailing_and_concatenated_streams_match_httpx() {
    let payload = b"first member";
    let gzip = encode("gzip", payload);
    let zlib = encode("deflate", payload);
    let mut corrupt = gzip.clone();
    let trailer = corrupt.len() - 8;
    corrupt[trailer] ^= 1;
    let cases = [
        Case {
            headers: vec![b"gzip".to_vec()],
            chunks: vec![gzip[..gzip.len() - 8].to_vec()],
            expected: Some(payload.to_vec()),
        },
        Case {
            headers: vec![b"deflate".to_vec()],
            chunks: vec![zlib[..zlib.len() - 4].to_vec()],
            expected: Some(payload.to_vec()),
        },
        Case {
            headers: vec![b"gzip".to_vec()],
            chunks: vec![b"\x1f\x8b".to_vec()],
            expected: Some(Vec::new()),
        },
        Case {
            headers: vec![b"gzip".to_vec()],
            chunks: vec![Vec::new()],
            expected: Some(Vec::new()),
        },
        Case {
            headers: vec![b"gzip".to_vec()],
            chunks: vec![[gzip.as_slice(), b"trailing"].concat()],
            expected: Some(payload.to_vec()),
        },
        Case {
            headers: vec![b"gzip".to_vec()],
            chunks: vec![[gzip.as_slice(), encode("gzip", b"second").as_slice()].concat()],
            expected: Some(payload.to_vec()),
        },
        Case {
            headers: vec![b"gzip".to_vec()],
            chunks: vec![corrupt],
            expected: None,
        },
        Case {
            headers: vec![b"gzip".to_vec()],
            chunks: vec![b"bad stream".to_vec()],
            expected: None,
        },
        Case {
            headers: vec![b"deflate".to_vec()],
            chunks: vec![b"bad stream".to_vec()],
            expected: None,
        },
    ];
    assert_cases(&cases);
}

#[test]
fn deflate_fallback_is_limited_to_the_first_input_call() {
    let payload = b"raw payload";
    let body = encode("raw", payload);
    let cases = [
        Case {
            headers: vec![b"deflate".to_vec()],
            chunks: vec![body.clone()],
            expected: Some(payload.to_vec()),
        },
        Case {
            headers: vec![b"deflate".to_vec()],
            chunks: vec![body[..1].to_vec(), body[1..].to_vec()],
            expected: None,
        },
        Case {
            headers: vec![b"deflate".to_vec()],
            chunks: vec![Vec::new(), body.clone()],
            expected: Some(payload.to_vec()),
        },
        Case {
            headers: vec![b"deflate, gzip".to_vec()],
            chunks: encode("gzip", &body).chunks(1).map(<[u8]>::to_vec).collect(),
            expected: None,
        },
    ];
    assert_cases(&cases);
}

#[test]
fn decoded_limits_span_chunks_and_ignore_wire_size() {
    let payload = b"limit";
    for encoding in ["gzip", "deflate"] {
        let body = encode(encoding, payload);
        assert!(body.len() > payload.len());
        let case = Case {
            headers: vec![encoding.as_bytes().to_vec()],
            chunks: body.chunks(2).map(<[u8]>::to_vec).collect(),
            expected: Some(payload.to_vec()),
        };
        assert_eq!(decoded(&case, payload.len() as u64).unwrap(), payload);
        assert!(
            decoded(&case, payload.len() as u64 - 1)
                .unwrap_err()
                .to_string()
                .contains("size limit")
        );
    }
    let expanded = vec![b'x'; 2 * 1024 * 1024];
    let case = Case {
        headers: vec![b"gzip".to_vec()],
        chunks: vec![encode("gzip", &expanded)],
        expected: None,
    };
    assert!(decoded(&case, 1024).unwrap_err().to_string().contains("size limit"));
}
