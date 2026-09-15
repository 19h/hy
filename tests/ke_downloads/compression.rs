//! Compressed KE transfers, decoded limits and publication failure boundaries.

use std::fs;
use std::io::Write;

use flate2::Compression;
use flate2::write::{DeflateEncoder, GzEncoder, ZlibEncoder};
use sha2::{Digest, Sha256};

use super::fixture::{CONTENT, Fixture, assert_error, response};
use super::support::assert_success;

fn compress(encoding: &str, bytes: &[u8]) -> Vec<u8> {
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
        "gzip, deflate" => compress("deflate", &compress("gzip", bytes)),
        _ => bytes.to_vec(),
    }
}

#[test]
fn compressed_metadata_and_idbs_are_decoded_before_validation_and_publication() {
    let metadata = b" {\"preserve\": [1, 2]} \n";
    for encoding in ["gzip", "deflate", "raw", "gzip, deflate", "unknown"] {
        let header = if encoding == "raw" {
            "deflate"
        } else {
            encoding
        };
        let fixture = Fixture::encoded(
            response(200, &compress(encoding, metadata)),
            response(200, &compress(encoding, CONTENT)),
            header,
        );
        assert_success(&fixture.run());
        assert_eq!(fs::read(fixture.sidecar()).unwrap(), metadata);
        assert_eq!(fs::read(fixture.path()).unwrap(), CONTENT);
        fixture.assert_request_order();
        fixture.assert_no_staging_files();
        for request in fixture.server.requests() {
            assert!(
                request.headers.to_ascii_lowercase().contains("accept-encoding: gzip, deflate\r\n")
            );
        }
    }
}

#[test]
fn decoded_size_limits_preserve_existing_content_and_sidecars() {
    let fixture = Fixture::encoded(
        response(200, &compress("gzip", b"{}")),
        response(200, &compress("gzip", &vec![0; 1024 * 1024 + 1])),
        "gzip",
    );
    fixture.seed();
    let output = fixture.command().env("HCLI_KE_MAX_DOWNLOAD_MB", "1").output().unwrap();
    assert_error(&output, "size limit");
    assert_eq!(fs::read(fixture.path()).unwrap(), b"old content");
    fixture.assert_no_staging_files();

    let metadata = format!("\"{}\"", "x".repeat(16 * 1024 * 1024));
    let fixture = Fixture::encoded(
        response(200, &compress("gzip", metadata.as_bytes())),
        response(200, &compress("gzip", CONTENT)),
        "gzip",
    );
    fixture.seed();
    assert_success(&fixture.run());
    assert_eq!(fs::read(fixture.sidecar()).unwrap(), br#"{"old":true}"#);
    assert_eq!(fs::read(fixture.path()).unwrap(), CONTENT);
    fixture.assert_no_staging_files();
}

#[test]
fn corrupt_metadata_is_advisory_but_corrupt_content_is_terminal() {
    let fixture = Fixture::encoded(
        response(200, b"invalid gzip"),
        response(200, &compress("gzip", CONTENT)),
        "gzip",
    );
    fixture.seed();
    let output = fixture.run();
    assert_success(&output);
    assert!(String::from_utf8_lossy(&output.stderr).contains("metadata fetch failed"));
    assert_eq!(fs::read(fixture.sidecar()).unwrap(), br#"{"old":true}"#);
    assert_eq!(fs::read(fixture.path()).unwrap(), CONTENT);
    fixture.assert_no_staging_files();

    let mut corrupt = compress("gzip", CONTENT);
    let checksum = corrupt.len() - 8;
    corrupt[checksum] ^= 1;
    let fixture =
        Fixture::encoded(response(200, &compress("gzip", b"{}")), response(200, &corrupt), "gzip");
    fixture.seed();
    assert_error(&fixture.run(), "response decoding failed");
    assert_eq!(fs::read(fixture.path()).unwrap(), b"old content");
    fixture.assert_no_staging_files();
}

#[test]
fn compressed_wire_length_does_not_override_the_decoded_content_limit() {
    let mut state = 0x1234_5678_u32;
    let content: Vec<_> = (0..1024 * 1024)
        .map(|_| {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            state as u8
        })
        .collect();
    let compressed = compress("gzip", &content);
    assert!(compressed.len() > content.len());
    let mut fixture = Fixture::encoded(
        response(200, &compress("gzip", b"{}")),
        response(200, &compressed),
        "gzip",
    );
    fixture.hash = format!("{:x}", Sha256::digest(&content));
    let output = fixture.command().env("HCLI_KE_MAX_DOWNLOAD_MB", "1").output().unwrap();
    assert_success(&output);
    assert_eq!(fs::read(fixture.path()).unwrap(), content);
    fixture.assert_request_order();
    fixture.assert_no_staging_files();
}
