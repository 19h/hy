//! JSON values are decoded from bytes independently of declared HTTP charsets.

mod support;

use std::fs;

use serde_json::json;
use support::{
    auth::*,
    http::{Response, Server},
    *,
};

fn encodings(text: &str) -> Vec<Vec<u8>> {
    let utf16_le: Vec<_> = text.encode_utf16().flat_map(u16::to_le_bytes).collect();
    let utf16_be: Vec<_> = text.encode_utf16().flat_map(u16::to_be_bytes).collect();
    let utf32_le: Vec<_> = text.chars().flat_map(|value| u32::from(value).to_le_bytes()).collect();
    let utf32_be: Vec<_> = text.chars().flat_map(|value| u32::from(value).to_be_bytes()).collect();
    let mut variants = Vec::new();
    for (bom, bytes) in [
        (b"\xef\xbb\xbf".as_slice(), text.as_bytes()),
        (b"\xff\xfe".as_slice(), utf16_le.as_slice()),
        (b"\xfe\xff".as_slice(), utf16_be.as_slice()),
        (b"\xff\xfe\0\0".as_slice(), utf32_le.as_slice()),
        (b"\0\0\xfe\xff".as_slice(), utf32_be.as_slice()),
    ] {
        variants.push(bytes.to_vec());
        variants.push([bom, bytes].concat());
    }
    variants
}

fn encoded(body: Vec<u8>) -> Response {
    Response {
        status: 200,
        content_type: "application/json; charset=windows-1252",
        body,
    }
}

#[test]
fn get_responses_preserve_unicode_without_using_the_declared_charset() {
    let document = json!({"offset":0, "limit":100, "total":1, "items":[{
        "key":"fixture", "filename":"café-🦀.bin", "size":7,
    }]})
    .to_string();
    for (index, body) in encodings(&document).into_iter().enumerate() {
        let sandbox = Sandbox::new();
        write_config(&sandbox, &stored("key", "fixture-key"));
        let server = Server::start(move |_, _| encoded(body.clone()));
        let output =
            command(&sandbox, &server, &["share", "list", "--no-interactive"]).output().unwrap();
        assert_success(&output);
        let report = String::from_utf8_lossy(&output.stderr);
        assert!(report.contains("café-🦀.bin"), "encoding {index}: {report}");
        assert_eq!(server.requests().len(), 1);
    }
}

#[test]
fn post_and_delete_responses_use_the_same_byte_decoder() {
    for operation in ["put", "delete"] {
        let document =
            json!({"key":"fixture", "code":"CODE-é🦀", "version":"7.0", "url":null}).to_string();
        for body in encodings(&document) {
            let sandbox = Sandbox::new();
            write_config(&sandbox, &stored("key", "fixture-key"));
            let file = sandbox.path().join("payload");
            fs::write(&file, b"fixture").unwrap();
            let server = Server::start(move |request, _| {
                if request.method == "GET" {
                    return Response::json(
                        json!({"key":"fixture", "filename":"fixture", "code":"CODE-é🦀"}),
                    );
                }
                encoded(body.clone())
            });
            let args = if operation == "put" {
                vec!["share", "put", file.to_str().unwrap(), "--acl", "private"]
            } else {
                vec!["share", "delete", "fixture", "--force"]
            };
            let output = command(&sandbox, &server, &args).output().unwrap();
            assert_success(&output);
            assert!(String::from_utf8_lossy(&output.stdout).contains("CODE-é🦀"));
            assert_eq!(
                server.requests().len(),
                if operation == "put" {
                    1
                } else {
                    2
                }
            );
        }
    }
}

#[test]
fn api_error_messages_are_decoded_from_their_json_bytes() {
    for body in encodings(&json!({"message":"café-🦀 failure"}).to_string()) {
        let sandbox = Sandbox::new();
        write_config(&sandbox, &stored("key", "fixture-key"));
        let server = Server::start(move |_, _| Response {
            status: 500,
            ..encoded(body.clone())
        });
        let output = command(&sandbox, &server, &["share", "get", "fixture"]).output().unwrap();
        assert!(!output.status.success());
        let report = String::from_utf8_lossy(&output.stdout);
        assert!(report.contains("café-🦀 failure"), "{report}");
        assert_eq!(server.requests().len(), 1);
    }
}

#[test]
fn standalone_identity_uses_the_same_json_encoding_rules() {
    for body in encodings(&json!({"email":"café-🦀@example.test"}).to_string()) {
        let sandbox = Sandbox::new();
        let server = Server::start(move |_, _| encoded(body.clone()));
        let output = command(&sandbox, &server, &["whoami"])
            .env("HCLI_API_KEY", "fixture-key")
            .output()
            .unwrap();
        assert_success(&output);
        let report = format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(report.contains("café-🦀@example.test"), "{report}");
        assert_eq!(server.requests().len(), 1);
    }
}

#[test]
fn oversized_integers_in_unknown_fields_reject_the_entire_response() {
    let sandbox = Sandbox::new();
    write_config(&sandbox, &stored("key", "fixture-key"));
    let body = format!(r#"{{"key":"fixture","filename":"fixture","unused":{}}}"#, "1".repeat(4301))
        .into_bytes();
    let server = Server::start(move |_, _| encoded(body.clone()));
    let output = command(&sandbox, &server, &["share", "get", "fixture"])
        .current_dir(sandbox.path())
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert_eq!(server.requests().len(), 1);
    assert!(String::from_utf8_lossy(&output.stdout).contains("4300-digit"));
}

#[test]
fn malformed_error_json_uses_the_upstream_status_fallback() {
    for body in [
        b"not JSON".to_vec(),
        format!(r#"{{"message":"must not be used","unused":{}}}"#, "1".repeat(4301)).into_bytes(),
    ] {
        let sandbox = Sandbox::new();
        write_config(&sandbox, &stored("key", "fixture-key"));
        let server = Server::start(move |_, _| Response {
            status: 500,
            ..encoded(body.clone())
        });
        let output = command(&sandbox, &server, &["share", "get", "fixture"]).output().unwrap();
        assert!(!output.status.success());
        let report = String::from_utf8_lossy(&output.stdout);
        assert!(report.contains("API request failed: 500"), "{report}");
        assert!(!report.contains("must not be used"), "{report}");
    }
}
