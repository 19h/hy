//! Resolve IDA links and dispatch navigation or content-addressed downloads.

use crate::error::{Error, Result};
use crate::ida::ipc;

mod download;
mod ke;
mod lookup;
mod metadata;
mod navigation;
mod pattern;
mod transport;
mod uri;
mod wait;

use lookup::find_database;
use navigation::{LaunchOptions, Target, navigate_relative, navigate_to_database};
use uri::{DefaultTarget, ParsedLink};

pub fn filename(segment: &str) -> Result<String> {
    let decoded = percent_encoding::percent_decode_str(segment).decode_utf8_lossy().into_owned();
    if decoded.is_empty()
        || decoded == "."
        || decoded == ".."
        || decoded.contains(['/', '\\'])
        || decoded.chars().any(|character| character < '\u{20}' || character == '\u{7f}')
    {
        return Err(Error::Other("link filename is not a safe basename".into()));
    }
    Ok(decoded)
}

pub fn content_sha(url: &url::Url) -> Result<String> {
    let segments: Vec<_> = url.path().split('/').filter(|segment| !segment.is_empty()).collect();
    if segments.len() < 3
        || segments[segments.len() - 1] != "content"
        || segments[segments.len() - 3] != "objects"
    {
        return Err(Error::Other("KE download URL must end in /objects/<sha256>/content".into()));
    }
    let hash = segments[segments.len() - 2];
    if hash.len() != 64 || !hash.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(Error::Other("invalid SHA-256 in KE URL".into()));
    }
    Ok(hash.to_lowercase())
}

pub async fn open(uri: &str, no_launch: bool, timeout: f64, skip_analysis: bool) -> Result<()> {
    let parsed = ParsedLink::parse(uri)?;
    let options = LaunchOptions {
        no_launch,
        timeout,
        skip_analysis,
    };
    let has_download =
        url::form_urlencoded::parse(parsed.query.as_bytes()).any(|(key, _)| key == "url");
    if parsed.source == "ke" && !parsed.segments.is_empty() && has_download {
        return ke::open(uri, &parsed, options).await;
    }
    let target = match parsed.default_target()? {
        DefaultTarget::Relative {
            uri,
        } => {
            return navigate_relative(&uri, ipc::discover().await).await;
        }
        DefaultTarget::Database {
            uri,
            name,
            source,
        } => {
            let path = find_database(&name, &source)?;
            Target {
                uri: Some(uri),
                name,
                path,
                exact_path_match: false,
            }
        }
    };
    navigate_to_database(target, options).await
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn filename_decoding_matches_the_pinned_python_helper() {
        for (encoded, expected) in [
            ("hello%20world.i64", "hello world.i64"),
            ("C%3Asample.i64", "C:sample.i64"),
            ("bad%FF.i64", "bad\u{fffd}.i64"),
            ("name%C2%85.i64", "name\u{85}.i64"),
            ("%ED%A0%80.i64", "\u{fffd}\u{fffd}\u{fffd}.i64"),
            ("a+b.i64", "a+b.i64"),
            ("bad%zz.i64", "bad%zz.i64"),
            ("%C3%28.i64", "\u{fffd}(.i64"),
        ] {
            assert_eq!(filename(encoded).unwrap(), expected);
        }
        for encoded in ["%00", "%1f", "%7f", "a%2fb", "a%5cb", "%2e%2e"] {
            assert!(filename(encoded).is_err(), "{encoded}");
        }
    }

    #[test]
    fn content_identity_and_names_are_strict() {
        for name in ["..", "%2Ftmp", "a%5Cb", "%00"] {
            assert!(filename(name).is_err());
        }
        assert_eq!(filename("hello%20world.i64").unwrap(), "hello world.i64");
        let hash = "a".repeat(64);
        assert_eq!(
            content_sha(
                &url::Url::parse(&format!(
                    "https://example.test/api/objects/{hash}/content?token=x"
                ))
                .unwrap()
            )
            .unwrap(),
            hash
        );
        assert!(
            content_sha(&url::Url::parse("https://example.test/objects/short/content").unwrap())
                .is_err()
        );
    }
}
