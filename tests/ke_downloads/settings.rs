//! Unicode limits and retention conversion failures through the KE command.

use std::fs;

use super::fixture::{CONTENT, Fixture, assert_error, response};
use super::support::assert_success;

#[test]
fn unicode_download_limits_reject_oversized_content_and_preserve_the_cached_idb() {
    for limit in ["١", "\u{1c}+१\u{1f}"] {
        let fixture = Fixture::new(response(200, b"{}"), response(200, &vec![b'x'; 1_048_577]));
        fixture.seed();
        let output = fixture.command().env("HCLI_KE_MAX_DOWNLOAD_MB", limit).output().unwrap();
        assert_error(&output, "size limit");
        assert_eq!(fs::read(fixture.path()).unwrap(), b"old content");
        assert_eq!(fs::read(fixture.sidecar()).unwrap(), b"{}");
        fixture.assert_no_staging_files();
    }
}

#[test]
fn overflowing_retention_fails_only_when_the_download_root_already_exists() {
    let overflow = format!("1{}", "0".repeat(400));
    for existing in [false, true] {
        let fixture = Fixture::new(response(200, b"{}"), response(200, CONTENT));
        if existing {
            fixture.seed();
        }
        let output =
            fixture.command().env("HCLI_KE_DOWNLOADS_RETENTION_DAYS", &overflow).output().unwrap();
        if existing {
            assert_error(&output, "int too large to convert to float");
            assert_eq!(fs::read(fixture.path()).unwrap(), b"old content");
            assert_eq!(fs::read(fixture.sidecar()).unwrap(), br#"{"old":true}"#);
            assert!(fixture.server.requests().is_empty());
            #[cfg(unix)]
            assert!(!fixture.dialogs.directory.join("progress").exists());
        } else {
            assert_success(&output);
            assert_eq!(fs::read(fixture.path()).unwrap(), CONTENT);
        }
        fixture.assert_no_staging_files();
    }
}
