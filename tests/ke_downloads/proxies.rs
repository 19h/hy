//! Environment proxy routing through the actual KE command and cookie session.

use std::fs;

use super::fixture::{CONTENT, Fixture, assert_error, response};
use super::support::{assert_success, http::Server};

#[test]
fn an_environment_proxy_carries_metadata_cookies_and_separate_authentication() {
    let fixture = Fixture::with_handler(|request, _| {
        assert!(request.path.starts_with("http://origin.test/api/objects/"), "{}", request.path);
        let headers = request.headers.to_lowercase();
        assert!(headers.contains("host: origin.test\r\n"));
        assert!(request.headers.contains("authorization: Basic dXNlcjpwYXNz\r\n"));
        assert!(request.headers.contains("proxy-authorization: Basic cHJveHk6c2VjcmV0\r\n"));
        if request.path.split('?').next().unwrap().ends_with("/content") {
            assert!(headers.contains("cookie: session=ready\r\n"));
            (response(200, CONTENT), Vec::new())
        } else {
            (response(200, b"{}"), vec![("Set-Cookie".into(), "session=ready; Path=/".into())])
        }
    });
    let content =
        format!("http://user:pass@origin.test/api/objects/{}/content?token=fixture", fixture.hash);
    let mut uri = fixture.uri();
    uri.query_pairs_mut().clear().append_pair("url", &content);
    let proxy = fixture.server.url.replacen("http://", "http://proxy:secret@", 1);
    let output =
        fixture.command_for_uri(uri.as_str(), true).env("HTTP_PROXY", proxy).output().unwrap();
    assert_success(&output);
    assert_eq!(fs::read(fixture.path()).unwrap(), CONTENT);
    assert_eq!(fs::read(fixture.sidecar()).unwrap(), b"{}");
    assert_eq!(fixture.server.requests().len(), 2);
    fixture.assert_no_staging_files();
}

#[test]
fn no_proxy_bypasses_the_proxy_and_unmatched_hosts_keep_proxy_failures() {
    for (bypass, direct) in [("*", true), ("127.0.0.1", true), ("localhost", false)] {
        let fixture = Fixture::new(response(200, b"{}"), response(200, CONTENT));
        fixture.seed();
        let proxy = Server::start(|request, _| {
            assert!(request.path.starts_with("http://127.0.0.1:"));
            response(503, b"proxy unavailable")
        });
        let output = fixture
            .command()
            .env("HTTP_PROXY", &proxy.url)
            .env("NO_PROXY", bypass)
            .output()
            .unwrap();
        if direct {
            assert_success(&output);
            assert_eq!(fs::read(fixture.path()).unwrap(), CONTENT);
            assert!(proxy.requests().is_empty());
            fixture.assert_request_order();
        } else {
            assert_error(&output, "HTTP 503");
            assert_eq!(fs::read(fixture.path()).unwrap(), b"old content");
            assert_eq!(proxy.requests().len(), 2);
            assert!(fixture.server.requests().is_empty());
        }
        fixture.assert_no_staging_files();
    }
}
