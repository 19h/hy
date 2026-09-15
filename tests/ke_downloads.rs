//! KE sidecars, cache policy and native dialog orchestration through the CLI.

#[path = "ke_downloads/compression.rs"]
mod compression_tests;
#[path = "ke_downloads/cookies.rs"]
mod cookie_tests;
#[path = "ke_downloads/fixture.rs"]
mod fixture;
#[cfg(unix)]
#[path = "ke_downloads/navigation.rs"]
mod navigation_tests;
#[path = "ke_downloads/proxies.rs"]
mod proxy_tests;
#[path = "ke_downloads/settings.rs"]
mod settings_tests;
mod support;
#[path = "ke_downloads/tls.rs"]
mod tls_tests;

use fixture::*;
use std::fs;
use support::assert_success;

#[test]
fn metadata_is_fetched_first_and_valid_json_is_preserved_verbatim() {
    for metadata in [br#" { "object": 1 } "#.as_slice(), b"null\n", b"[true, 3]"] {
        let fixture = Fixture::new(response(200, metadata), response(200, CONTENT));
        assert_success(&fixture.run());
        assert_eq!(fs::read(fixture.path()).unwrap(), CONTENT);
        assert_eq!(fs::read(fixture.sidecar()).unwrap(), metadata);
        fixture.assert_request_order();
        fixture.assert_no_staging_files();
    }
}

#[test]
fn python_metadata_encodings_and_values_are_preserved_verbatim() {
    let payloads = [
        b"\xef\xbb\xbf{}".to_vec(),
        b"\xff\xfe{\x00}\x00".to_vec(),
        b"\xfe\xff\x00{\x00}".to_vec(),
        b"\xff\xfe\x00\x00{\x00\x00\x00}\x00\x00\x00".to_vec(),
        b"\x00\x00\xfe\xff\x00\x00\x00{\x00\x00\x00}".to_vec(),
        b"[NaN,Infinity,-Infinity,1e10000]".to_vec(),
        b"\"\xed\xa0\x80\"".to_vec(),
        "9".repeat(4300).into_bytes(),
        format!("{}0{}", "[".repeat(200), "]".repeat(200)).into_bytes(),
    ];
    for metadata in payloads {
        let fixture = Fixture::new(response(200, &metadata), response(200, CONTENT));
        assert_success(&fixture.run());
        assert_eq!(fs::read(fixture.sidecar()).unwrap(), metadata);
        assert_eq!(fs::read(fixture.path()).unwrap(), CONTENT);
        fixture.assert_request_order();
        fixture.assert_no_staging_files();
    }
}

#[test]
fn rejected_metadata_preserves_the_old_sidecar_and_does_not_block_content() {
    for metadata in [
        response(404, b"missing"),
        response(500, b"unavailable"),
        response(201, b"{}"),
        response(200, b"not JSON"),
        response(200, &[b'\"', 0xff, b'\"']),
        response(200, b"\xff\xfe{\x00}\x00\x00"),
        response(200, &vec![b'9'; 4301]),
        response(200, br#"{"valid":true} trailing"#),
        response(200, &vec![b' '; 16 * 1024 * 1024 + 1]),
    ] {
        let fixture = Fixture::new(metadata, response(200, CONTENT));
        fixture.seed();
        let output = fixture.run();
        assert_success(&output);
        let diagnostic = format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(diagnostic.contains("metadata fetch failed"), "{diagnostic}");
        assert_eq!(fs::read(fixture.path()).unwrap(), CONTENT);
        assert_eq!(fs::read(fixture.sidecar()).unwrap(), br#"{"old":true}"#);
        fixture.assert_request_order();
        fixture.assert_no_staging_files();
    }
}

#[test]
fn failed_sidecar_publication_does_not_block_content() {
    let fixture = Fixture::new(response(200, b"{}"), response(200, CONTENT));
    fs::create_dir_all(fixture.sidecar()).unwrap();
    // Retention removes empty directories; keep this publication blocker nonempty.
    fs::write(fixture.sidecar().join("retain"), b"fixture").unwrap();
    assert_success(&fixture.run());
    assert_eq!(fs::read(fixture.path()).unwrap(), CONTENT);
    assert!(fixture.sidecar().is_dir());
    assert_eq!(fs::read(fixture.sidecar().join("retain")).unwrap(), b"fixture");
    fixture.assert_request_order();
    fixture.assert_no_staging_files();
}

#[test]
fn a_content_hash_failure_preserves_the_old_idb_after_metadata_publication() {
    let fixture = Fixture::new(response(200, b"{\"new\":true}"), response(200, b"wrong content"));
    fixture.seed();
    let output = fixture.run();
    assert_error(&output, "SHA-256 mismatch");
    assert_eq!(fs::read(fixture.path()).unwrap(), b"old content");
    assert_eq!(fs::read(fixture.sidecar()).unwrap(), b"{\"new\":true}");
    fixture.assert_request_order();
    fixture.assert_no_staging_files();
}

#[test]
fn content_requires_http_200_even_when_the_body_has_the_expected_hash() {
    for status in [201, 204, 206, 500] {
        let fixture = Fixture::new(response(200, b"{}"), response(status, CONTENT));
        fixture.seed();
        let output = fixture.run();
        assert_error(&output, &format!("HTTP {status}"));
        assert_eq!(fs::read(fixture.path()).unwrap(), b"old content");
        fixture.assert_request_order();
        fixture.assert_no_staging_files();
    }
}

#[test]
fn an_exceeded_content_limit_preserves_the_cached_idb() {
    let fixture = Fixture::new(response(200, b"{}"), response(200, &vec![0; 1024 * 1024 + 1]));
    fixture.seed();
    let output = fixture.command().env("HCLI_KE_MAX_DOWNLOAD_MB", "1").output().unwrap();
    assert_error(&output, "size limit");
    assert_eq!(fs::read(fixture.path()).unwrap(), b"old content");
    fixture.assert_no_staging_files();
}

#[cfg(unix)]
#[test]
fn cache_symlinks_are_allowed_only_when_the_destination_resolves_inside_the_root() {
    use std::os::unix::fs::symlink;

    let fixture = Fixture::new(response(200, b"{}"), response(200, CONTENT));
    let actual = fixture.sandbox.path().join("actual-downloads");
    fs::create_dir(&actual).unwrap();
    symlink(&actual, fixture.sandbox.path().join("downloads")).unwrap();
    assert_success(&fixture.run());
    assert_eq!(fs::read(actual.join(&fixture.hash).join(FILENAME)).unwrap(), CONTENT);
    fixture.assert_request_order();

    let fixture = Fixture::new(response(200, b"{}"), response(200, CONTENT));
    let root = fixture.sandbox.path().join("downloads");
    let outside = fixture.sandbox.path().join("outside");
    fs::create_dir(&root).unwrap();
    fs::create_dir(&outside).unwrap();
    fs::write(outside.join(FILENAME), b"retain").unwrap();
    symlink(&outside, root.join(&fixture.hash)).unwrap();
    assert_error(&fixture.run(), "outside the downloads directory");
    assert_eq!(fs::read(outside.join(FILENAME)).unwrap(), b"retain");
    assert!(fixture.server.requests().is_empty());
}

#[cfg(unix)]
#[test]
fn an_internal_directory_alias_can_receive_the_download() {
    use std::os::unix::fs::symlink;
    let fixture = Fixture::new(response(200, b"{}"), response(200, CONTENT));
    let root = fixture.sandbox.path().join("downloads");
    let target = root.join("shared");
    fs::create_dir_all(&target).unwrap();
    // Keep retention from deleting the empty target directory before transfer.
    fs::write(target.join("retain"), b"fixture").unwrap();
    symlink("shared", root.join(&fixture.hash)).unwrap();
    assert_success(&fixture.run());
    assert_eq!(fs::read(target.join(FILENAME)).unwrap(), CONTENT);
    assert_eq!(fs::read(target.join(format!("{FILENAME}.ke.json"))).unwrap(), b"{}");
    assert!(root.join(&fixture.hash).is_symlink());
}

#[cfg(unix)]
#[test]
fn publication_replaces_file_links_without_changing_their_targets() {
    use std::os::unix::fs::symlink;

    let fixture = Fixture::new(response(200, b"{}"), response(200, CONTENT));
    let content_target = fixture.sandbox.path().join("downloads/retained.i64");
    let metadata_target = fixture.sandbox.path().join("outside.json");
    fs::create_dir_all(fixture.path().parent().unwrap()).unwrap();
    fs::write(&content_target, b"retained IDB").unwrap();
    fs::write(&metadata_target, b"retained metadata").unwrap();
    symlink(&content_target, fixture.path()).unwrap();
    symlink(&metadata_target, fixture.sidecar()).unwrap();

    assert_success(&fixture.run());
    assert_eq!(fs::read(fixture.path()).unwrap(), CONTENT);
    assert_eq!(fs::read(fixture.sidecar()).unwrap(), b"{}");
    assert!(!fixture.path().is_symlink());
    assert!(!fixture.sidecar().is_symlink());
    assert_eq!(fs::read(content_target).unwrap(), b"retained IDB");
    assert_eq!(fs::read(metadata_target).unwrap(), b"retained metadata");
    fixture.assert_request_order();
    fixture.assert_no_staging_files();
}

#[cfg(unix)]
#[test]
fn filename_decoding_accepts_upstream_unicode_and_colon_names() {
    for (encoded, decoded) in [
        ("C%3Asample.i64", "C:sample.i64"),
        ("bad%FF.i64", "bad\u{fffd}.i64"),
        ("name%C2%85.i64", "name\u{85}.i64"),
    ] {
        let fixture = Fixture::new(response(200, b"{}"), response(200, CONTENT));
        let mut uri = fixture.uri();
        uri.set_path(&format!("/{encoded}"));
        assert_success(&fixture.command_for_uri(uri.as_str(), true).output().unwrap());
        assert_eq!(fs::read(fixture.path().with_file_name(decoded)).unwrap(), CONTENT);
    }
}

#[cfg(unix)]
#[test]
fn declined_or_failed_confirmation_does_not_touch_downloads_or_send_requests() {
    for (status, seeded) in [("1", false), ("2", true)] {
        let fixture = Fixture::new(response(200, b"{}"), response(200, CONTENT));
        if seeded {
            fixture.seed();
        }
        let output = fixture
            .command_for(false)
            .env("HY_TEST_DIALOG_APPROVE", status)
            .env("HCLI_KE_DOWNLOADS_RETENTION_DAYS", "-1")
            .output()
            .unwrap();
        assert_success(&output);
        assert!(fixture.dialogs.text("confirm").contains("(or where it redirects) in IDA?"));
        assert!(fixture.dialogs.text("confirm").contains(&fixture.hash[..8]));
        assert!(fixture.server.requests().is_empty());
        if seeded {
            assert_eq!(fs::read(fixture.path()).unwrap(), b"old content");
        } else {
            assert!(!fixture.sandbox.path().join("downloads").exists());
        }
        assert!(!fixture.dialogs.directory.join("progress").exists());
        assert!(!fixture.sandbox.config_path().exists());
    }
}

#[cfg(unix)]
#[test]
fn approved_download_shows_and_dismisses_progress_before_launching() {
    let fixture = Fixture::new(response(200, b"{}"), response(200, CONTENT));
    let installation = fixture.installation();
    let output = fixture
        .command_for(false)
        .env("HY_TEST_DIALOG_APPROVE", "0")
        .env("HCLI_CURRENT_IDA_INSTALL_DIR", &installation)
        .output()
        .unwrap();
    assert_success(&output);
    support::assert_file_eventually(
        &installation.join("ida.database"),
        fixture.path().as_os_str().as_encoded_bytes(),
    );
    assert!(fixture.dialogs.directory.join("confirm").exists());
    assert!(fixture.dialogs.text("progress").contains(FILENAME));
    fixture.assert_dialog_dismissed();
    fixture.assert_request_order();
}

#[cfg(unix)]
#[test]
fn failed_transfers_dismiss_progress_and_show_the_native_error() {
    let fixture = Fixture::new(response(200, b"{}"), response(200, b"wrong content"));
    assert_error(&fixture.run(), "SHA-256 mismatch");
    assert!(fixture.dialogs.text("error").contains("SHA-256 mismatch"));
    fixture.assert_dialog_dismissed();
    assert!(!fixture.dialogs.directory.join("confirm").exists());
}

#[cfg(unix)]
#[test]
fn truthy_environment_settings_skip_confirmation_and_allow_private_downloads() {
    for value in ["true", " YES ", "On", "1", "\u{1c}YeS\u{1f}"] {
        let fixture = Fixture::new(response(200, b"{}"), response(200, CONTENT));
        let installation = fixture.installation();
        let output = fixture
            .command_for(false)
            .env("HCLI_KE_SKIP_CONFIRM", value)
            .env("HCLI_KE_ALLOW_PRIVATE_HOSTS", value)
            .env("HCLI_CURRENT_IDA_INSTALL_DIR", installation)
            .output()
            .unwrap();
        assert_success(&output);
        assert!(!fixture.dialogs.directory.join("confirm").exists());
        fixture.assert_dialog_dismissed();
    }
}

#[test]
fn invalid_or_nonpositive_download_limits_use_the_unlimited_default() {
    for value in ["invalid", "-1", "0", " +1_024 ", "-१", "9223372036854775808"] {
        let fixture = Fixture::new(response(200, b"{}"), response(200, CONTENT));
        let output = fixture.command().env("HCLI_KE_MAX_DOWNLOAD_MB", value).output().unwrap();
        assert_success(&output);
        assert_eq!(fs::read(fixture.path()).unwrap(), CONTENT);
    }
}

#[cfg(unix)]
#[test]
fn an_unavailable_confirmation_tool_cancels_without_network_or_cache_changes() {
    let fixture = Fixture::new(response(200, b"{}"), response(200, CONTENT));
    for tool in ["osascript", "zenity", "kdialog"] {
        fs::remove_file(fixture.dialogs.directory.join(tool)).unwrap();
    }
    assert_success(&fixture.command_for(false).output().unwrap());
    assert!(fixture.server.requests().is_empty());
    assert!(!fixture.sandbox.path().join("downloads").exists());
}

#[test]
fn a_rejected_private_host_does_not_clean_the_cache_or_show_progress() {
    let fixture = Fixture::new(response(200, b"{}"), response(200, CONTENT));
    fixture.seed();
    let output = fixture
        .command()
        .env("HCLI_KE_ALLOW_PRIVATE_HOSTS", " false ")
        .env("HCLI_KE_DOWNLOADS_RETENTION_DAYS", "-1")
        .output()
        .unwrap();
    assert_error(&output, "non-public address");
    assert_eq!(fs::read(fixture.path()).unwrap(), b"old content");
    assert!(fixture.server.requests().is_empty());
    assert!(!fixture.dialogs.directory.join("progress").exists());
}

#[test]
fn retention_uses_configured_days_and_invalid_values_fall_back_to_three_days() {
    use std::time::{Duration, SystemTime};
    for (days, removed) in [
        ("1", true),
        ("invalid", false),
        ("-1", true),
        ("\u{1c}१\u{1f}", true),
        ("9223372036854775808", false),
        ("-9223372036854775809", true),
    ] {
        let fixture = Fixture::new(response(200, b"{}"), response(200, CONTENT));
        let old_directory = fixture.sandbox.path().join("downloads/old");
        fs::create_dir_all(&old_directory).unwrap();
        let old = old_directory.join("old.i64");
        fs::write(&old, b"expired fixture").unwrap();
        fs::File::options()
            .write(true)
            .open(&old)
            .unwrap()
            .set_times(
                fs::FileTimes::new()
                    .set_modified(SystemTime::now() - Duration::from_secs(2 * 86_400)),
            )
            .unwrap();
        let output =
            fixture.command().env("HCLI_KE_DOWNLOADS_RETENTION_DAYS", days).output().unwrap();
        assert_success(&output);
        assert_eq!(!old.exists(), removed, "{days}");
        assert_eq!(!old_directory.exists(), removed, "{days}");
        assert_eq!(fs::read(fixture.path()).unwrap(), CONTENT);
    }
}
