//! Session cookies must survive metadata failures and metadata redirects.

use std::fs;

use chrono::Datelike;

use super::fixture::{CONTENT, Fixture, assert_error, response};
use super::support::assert_success;
use super::support::http::Request;

fn cookie(request: &Request) -> Option<&str> {
    request
        .headers
        .lines()
        .filter_map(|line| line.split_once(':'))
        .find(|(name, _)| name.eq_ignore_ascii_case("cookie"))
        .map(|(_, value)| value.trim())
}

#[test]
fn metadata_cookies_authorize_content_even_when_metadata_fails() {
    let rolling_year = (chrono::Local::now().year() + 50) % 100;
    for (status, expiry) in [
        (200, String::new()),
        (404, String::new()),
        (200, format!("; Expires=01 Jan {rolling_year:02}")),
        (200, "; Expires=Wed, 01 Jan 1969 00:00:00 GMT".into()),
        (200, "; Max-Age=١_۰۰۰".into()),
        (200, "; Max-Age=9223372036854775808".into()),
    ] {
        let fixture = Fixture::with_handler(move |request, _| {
            if request.path.split('?').next().unwrap().ends_with("/content") {
                let content = if cookie(request) == Some("session=ready") {
                    response(200, CONTENT)
                } else {
                    response(403, b"missing session")
                };
                (content, Vec::new())
            } else {
                assert_eq!(cookie(request), None);
                (
                    response(status, b"{}"),
                    vec![("Set-Cookie".into(), format!("session=ready; Path=/{expiry}"))],
                )
            }
        });
        fixture.seed();
        assert_success(&fixture.run());
        assert_eq!(fs::read(fixture.path()).unwrap(), CONTENT);
        let metadata = if status == 200 {
            b"{}".as_slice()
        } else {
            br#"{"old":true}"#
        };
        assert_eq!(fs::read(fixture.sidecar()).unwrap(), metadata);
        fixture.assert_request_order();
        fixture.assert_no_staging_files();
    }
}

#[test]
fn metadata_redirect_cookie_updates_reach_the_original_content_transport() {
    let fixture = Fixture::with_handler(|request, _| {
        let path = request.path.split('?').next().unwrap();
        if path == "/metadata-ready" {
            assert_eq!(cookie(request), Some("session=initial"));
            (response(200, b"{}"), vec![("Set-Cookie".into(), "session=ready; Path=/".into())])
        } else if path.ends_with("/content") {
            assert_eq!(cookie(request), Some("session=ready"));
            (response(200, CONTENT), Vec::new())
        } else {
            assert_eq!(cookie(request), None);
            (
                response(302, b""),
                vec![
                    ("Location".into(), "/metadata-ready".into()),
                    ("Set-Cookie".into(), "session=initial; Path=/".into()),
                ],
            )
        }
    });
    assert_success(&fixture.run());
    assert_eq!(fs::read(fixture.path()).unwrap(), CONTENT);
    assert_eq!(fs::read(fixture.sidecar()).unwrap(), b"{}");
    assert_eq!(fixture.server.requests().len(), 3);
    fixture.assert_no_staging_files();
}

#[test]
fn failed_cookie_batches_preserve_the_cached_idb_when_content_requires_a_session() {
    for invalid in ["Max-Age".to_owned(), format!("Max-Age={}", "9".repeat(309))] {
        let fixture = Fixture::with_handler(move |request, _| {
            if request.path.split('?').next().unwrap().ends_with("/content") {
                assert_eq!(cookie(request), None);
                (response(403, b"missing session"), Vec::new())
            } else {
                (
                    response(200, b"{}"),
                    vec![
                        ("Set-Cookie".into(), "session=ready; Path=/".into()),
                        ("Set-Cookie".into(), format!("invalid=value; {invalid}")),
                    ],
                )
            }
        });
        fixture.seed();
        assert_error(&fixture.run(), "HTTP 403");
        assert_eq!(fs::read(fixture.path()).unwrap(), b"old content");
        assert_eq!(fs::read(fixture.sidecar()).unwrap(), b"{}");
        fixture.assert_request_order();
        fixture.assert_no_staging_files();
    }
}

#[test]
fn legacy_response_cookies_follow_the_combined_header_rule() {
    for (netscape, expected) in [
        (None, None),
        (Some(""), Some("session=ready")),
        (Some("a=one"), Some("a=one; session=ready")),
        (Some("a=one; Max-Age"), Some("session=ready")),
    ] {
        let fixture = Fixture::with_handler(move |request, _| {
            if request.path.split('?').next().unwrap().ends_with("/content") {
                assert_eq!(cookie(request), expected);
                let response = if expected.is_some() {
                    response(200, CONTENT)
                } else {
                    response(403, b"missing session")
                };
                (response, Vec::new())
            } else {
                let mut headers = Vec::new();
                if let Some(value) = netscape {
                    headers.push(("Set-Cookie".into(), value.into()));
                }
                headers.push(("Set-Cookie2".into(), "session=ready; Version=0; Path=/".into()));
                (response(200, b"{}"), headers)
            }
        });
        fixture.seed();
        let output = fixture.run();
        if expected.is_some() {
            assert_success(&output);
            assert_eq!(fs::read(fixture.path()).unwrap(), CONTENT);
        } else {
            assert_error(&output, "HTTP 403");
            assert_eq!(fs::read(fixture.path()).unwrap(), b"old content");
        }
        assert_eq!(fs::read(fixture.sidecar()).unwrap(), b"{}");
        fixture.assert_request_order();
        fixture.assert_no_staging_files();
    }
}
