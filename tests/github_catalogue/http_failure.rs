//! Consumer-specific HTTP errors must preserve messages and publication order.

use super::*;

#[derive(Clone, Copy)]
enum Consumer {
    Search,
    Graphql,
    Asset,
    Source,
}

struct Fixture {
    sandbox: Sandbox,
    server: Server,
}

impl Fixture {
    fn new(consumer: Consumer, status: u16, body: &[u8], headers: Vec<(String, String)>) -> Self {
        let sandbox = Sandbox::new();
        let failure = Response {
            status,
            content_type: "text/plain; charset=iso-8859-1",
            body: body.into(),
        };
        let server = Server::start_with_headers(move |request, base| {
            if request.path.starts_with("/search/code?") {
                if matches!(consumer, Consumer::Search) {
                    return (failure.clone(), headers.clone());
                }
                return (
                    Response::json(json!({"items": [{"repository": {"full_name": "owner/main"}}]})),
                    vec![],
                );
            }
            if request.path == "/graphql" {
                if matches!(consumer, Consumer::Graphql) {
                    return (failure.clone(), headers.clone());
                }
                let mut metadata = repository(base, "main");
                metadata["releases"]["nodes"].as_array_mut().unwrap().truncate(1);
                metadata["refs"] = json!({"nodes": []});
                let release = &mut metadata["releases"]["nodes"][0];
                release["tag"]["target"] = commit(base, "failure");
                release["releaseAssets"] = if matches!(consumer, Consumer::Asset) {
                    json!({"nodes": [{"name": "plugin.zip", "downloadUrl": format!("{base}/failure.zip"), "size": 1, "contentType": "raw"}]})
                } else {
                    json!({"nodes": []})
                };
                return (single_graphql(metadata), vec![]);
            }
            assert_eq!(request.path, "/failure.zip");
            (failure.clone(), headers.clone())
        });
        Self {
            sandbox,
            server,
        }
    }

    fn run(&self) -> std::process::Output {
        self.sandbox
            .command(&["plugin", "--repo", "github", "repo", "snapshot"])
            .env("GITHUB_TOKEN", "fixture-token")
            .env("GITHUB_API_URL", &self.server.url)
            .output()
            .unwrap()
    }

    fn assert_publication_boundary(&self, consumer: Consumer) {
        let root = self.sandbox.path().join("cache");
        let expected_requests = match consumer {
            Consumer::Search => 1,
            Consumer::Graphql => 3,
            Consumer::Asset | Consumer::Source => 4,
        };
        assert_eq!(self.server.requests().len(), expected_requests);
        assert_eq!(
            root.join("candidate_repos.json").is_file(),
            !matches!(consumer, Consumer::Search)
        );
        assert_eq!(
            root.join("owner/main/releases.json").is_file(),
            matches!(consumer, Consumer::Asset | Consumer::Source)
        );
        assert!(!root.join("owner/main/release-assets/v1/plugin.zip").exists());
        assert!(!root.join("owner/main/source-archives/failure/source.zip").exists());
    }
}

#[test]
fn graphql_http_failures_include_unmodified_utf8_bodies_before_metadata_publication() {
    for body in
        ["", "access denied", "拒否 🧠", "first\r\nsecond\rthird", "\u{feff}BOM", "not JSON"]
    {
        let fixture = Fixture::new(Consumer::Graphql, 401, body.as_bytes(), vec![]);
        let output = fixture.run();
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
        let error = String::from_utf8_lossy(&output.stderr);
        assert!(error.contains(&format!("HTTP 401: {body}")), "{error:?}");
        fixture.assert_publication_boundary(Consumer::Graphql);
    }
}

#[test]
fn graphql_http_body_decode_errors_replace_the_status_error_without_refetching() {
    for (bytes, expected) in [
        (b"\xff".as_slice(), "byte 0xff in position 0: invalid start byte"),
        (b"x\xe2\x82", "bytes in position 1-2: unexpected end of data"),
        (b"\xed\xa0\x80", "byte 0xed in position 0: invalid continuation byte"),
    ] {
        let fixture = Fixture::new(Consumer::Graphql, 401, bytes, vec![]);
        let output = fixture.run();
        assert!(!output.status.success());
        let error = String::from_utf8_lossy(&output.stderr);
        assert!(error.contains(&format!("'utf-8' codec can't decode {expected}")), "{error}");
        assert!(!error.contains("HTTP 401"));
        fixture.assert_publication_boundary(Consumer::Graphql);
    }
}

#[test]
fn search_and_archive_failures_report_wire_reason_without_decoding_error_bodies() {
    for consumer in [Consumer::Search, Consumer::Asset, Consumer::Source] {
        let fixture = Fixture::new(consumer, 401, b"\xff", vec![]);
        let output = fixture.run();
        assert!(!output.status.success());
        let error = String::from_utf8_lossy(&output.stderr);
        assert!(error.contains("HTTP Error 401: Fixture"), "{error}");
        assert!(!error.contains("codec can't decode"));
        fixture.assert_publication_boundary(consumer);
    }
}

#[test]
fn rejected_redirects_keep_caller_specific_http_error_messages() {
    for consumer in [Consumer::Search, Consumer::Graphql, Consumer::Asset, Consumer::Source] {
        let target = "file:///unopened-redirect-fixture.zip";
        let fixture = Fixture::new(
            consumer,
            302,
            b"redirect explanation",
            vec![("Location".into(), target.into())],
        );
        let output = fixture.run();
        assert!(!output.status.success());
        let expected = if matches!(consumer, Consumer::Graphql) {
            "HTTP 302: redirect explanation".to_owned()
        } else {
            format!("HTTP Error 302: Fixture - Redirection to url '{target}' is not allowed")
        };
        let error = String::from_utf8_lossy(&output.stderr);
        assert!(error.contains(&expected), "{error}");
        fixture.assert_publication_boundary(consumer);
    }
}
