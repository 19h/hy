use std::io::Write;

use flate2::Compression;
use flate2::write::{DeflateEncoder, GzEncoder, ZlibEncoder};

use crate::support::http::Response;

pub(super) const ENCODINGS: [&str; 5] = ["gzip", "deflate", "raw", "gzip, deflate", "unknown"];

pub(super) fn encode(encoding: &str, bytes: &[u8]) -> Vec<u8> {
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
        "gzip, deflate" => encode("deflate", &encode("gzip", bytes)),
        _ => bytes.to_vec(),
    }
}

pub(super) fn response(
    encoding: &str,
    mut response: Response,
) -> (Response, Vec<(String, String)>) {
    response.body = encode(encoding, &response.body);
    let header = if encoding == "raw" {
        "deflate"
    } else {
        encoding
    };
    (response, vec![("Content-Encoding".into(), header.into())])
}
