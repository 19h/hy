//! Real catalogue redirects, including credentials, POST conversion and retries.

use super::*;

fn snapshot(sandbox: &Sandbox, server: &Server) -> std::process::Output {
    sandbox
        .command(&["plugin", "--repo", "github", "repo", "snapshot"])
        .env("GITHUB_TOKEN", "fixture-token")
        .env("GITHUB_API_URL", &server.url)
        .output()
        .unwrap()
}

fn redirect(status: u16, location: &str) -> (Response, Vec<(String, String)>) {
    (
        Response {
            status,
            content_type: "text/plain",
            body: b"redirect body".to_vec(),
        },
        vec![("Location".into(), location.into())],
    )
}

fn found() -> Response {
    Response::json(json!({"items": [{"repository": {"full_name": "owner/main"}}]}))
}

fn empty_repository(base: &str) -> Value {
    json!({"defaultBranchRef": {"target": commit(base, "default")},
        "releases": {"nodes": []}, "refs": {"nodes": []}})
}

#[test]
fn search_and_graphql_redirects_preserve_credentials_and_convert_post_to_get() {
    for status in [301, 302, 303] {
        let sandbox = Sandbox::new();
        let mirror = Server::start(|request, base| {
            assert_eq!(request.path, "/graphql-final");
            assert_eq!(request.method, "GET");
            assert!(request.body.is_empty());
            let headers = request.headers.to_ascii_lowercase();
            assert!(headers.contains("authorization: bearer fixture-token"));
            assert!(headers.contains("user-agent: python-urllib/3.13"));
            assert!(!headers.contains("content-type:"));
            assert!(!headers.contains("content-length:"));
            single_graphql(empty_repository(base))
        });
        let target = format!("{}/graphql-final", mirror.url.replace("127.0.0.1", "localhost"));
        let server = Server::start_with_headers(move |request, _| {
            if request.path.starts_with("/search/code?") {
                return redirect(status, "/search-final");
            }
            if request.path == "/search-final" {
                let headers = request.headers.to_ascii_lowercase();
                assert!(headers.contains("authorization: bearer fixture-token"));
                assert!(headers.contains("user-agent: ida-hcli"));
                return (found(), vec![]);
            }
            assert_eq!(request.path, "/graphql");
            assert_eq!(request.method, "POST");
            assert!(!request.body.is_empty());
            redirect(status, &target)
        });
        assert_success(&snapshot(&sandbox, &server));
        assert_eq!(server.requests().len(), 5);
        assert_eq!(mirror.requests().len(), 1);
    }
}

#[test]
fn graphql_post_rejects_307_and_308_without_follow_up_requests() {
    for status in [307, 308] {
        let sandbox = Sandbox::new();
        let server = Server::start_with_headers(move |request, _| {
            if request.path.starts_with("/search/code?") {
                return (found(), vec![]);
            }
            assert_eq!(request.path, "/graphql");
            redirect(status, "/unexpected")
        });
        let output = snapshot(&sandbox, &server);
        assert!(!output.status.success());
        assert!(String::from_utf8_lossy(&output.stderr).contains(&format!("HTTP {status}")));
        assert_eq!(server.requests().len(), 3);
        assert!(!sandbox.path().join("cache/owner/main/releases.json").exists());
    }
}

#[test]
fn archive_redirects_use_first_location_and_uri_fallback() {
    let sandbox = Sandbox::new();
    let archive_path = sandbox.path().join("plugin.zip");
    archive_manifest(
        &archive_path,
        &identity_manifest("1.0", "https://github.com/owner/main"),
        &[],
    );
    let archive = fs::read(archive_path).unwrap();
    let expected = archive.clone();
    let server = Server::start_with_headers(move |request, base| {
        if request.path.starts_with("/search/code?") {
            return (found(), vec![]);
        }
        if request.path == "/graphql" {
            let mut metadata = repository(base, "main");
            metadata["releases"]["nodes"].as_array_mut().unwrap().truncate(1);
            metadata["refs"] = json!({"nodes": []});
            let release = &mut metadata["releases"]["nodes"][0];
            release["tag"]["target"] = commit(base, "source-start");
            release["releaseAssets"] = json!({"nodes": [{
                "name": "plugin.zip", "downloadUrl": format!("{base}/asset-start.zip"),
                "size": 100, "contentType": "raw",
            }]});
            return (single_graphql(metadata), vec![]);
        }
        let headers = request.headers.to_ascii_lowercase();
        assert!(headers.contains("user-agent: python-urllib/3.13"));
        assert!(!headers.contains("authorization:"));
        match request.path.as_str() {
            "/asset-start.zip" => {
                let (response, mut headers) = redirect(307, "/asset-good.zip");
                headers.push(("Location".into(), "/unexpected".into()));
                (response, headers)
            }
            "/source-start.zip" => {
                let (response, _) = redirect(308, "/unused");
                (response, vec![("URI".into(), "/source-good.zip".into())])
            }
            "/asset-good.zip" | "/source-good.zip" => (Response::zip(archive.clone()), vec![]),
            path => panic!("unexpected archive request: {path}"),
        }
    });
    assert_success(&snapshot(&sandbox, &server));
    assert_eq!(server.requests().len(), 7);
    for path in ["release-assets/v1/plugin.zip", "source-archives/source-start/source.zip"] {
        assert_eq!(fs::read(sandbox.path().join("cache/owner/main").join(path)).unwrap(), expected);
    }
}

#[test]
fn redirect_loops_apply_repeat_and_distinct_target_limits() {
    for distinct in [false, true] {
        let sandbox = Sandbox::new();
        let server = Server::start_with_headers(move |request, _| {
            let next = if distinct {
                request
                    .path
                    .strip_prefix("/target-")
                    .and_then(|index| index.parse::<usize>().ok())
                    .map_or(0, |index| index + 1)
            } else {
                0
            };
            redirect(302, &format!("/target-{next}"))
        });
        let output = snapshot(&sandbox, &server);
        assert!(!output.status.success());
        let error = String::from_utf8_lossy(&output.stderr);
        assert!(error.contains("HTTP Error 302:"));
        assert!(
            error.contains("lead to an infinite loop.\nThe last 30x error message was:\nFixture")
        );
        assert_eq!(
            server.requests().len(),
            if distinct {
                11
            } else {
                5
            }
        );
        assert!(!sandbox.path().join("cache/candidate_repos.json").exists());
    }
}

#[test]
fn retry_attempts_share_redirect_history_with_the_original_request() {
    let sandbox = Sandbox::new();
    let target_visits = Mutex::new(0);
    let server = Server::start_with_headers(move |request, _| {
        if request.path.starts_with("/search/code?") {
            return redirect(302, "/target");
        }
        assert_eq!(request.path, "/target");
        let mut visits = target_visits.lock().unwrap();
        *visits += 1;
        if *visits % 2 == 1 {
            redirect(302, "/target")
        } else {
            (
                Response {
                    status: 503,
                    ..Response::json(json!({}))
                },
                vec![],
            )
        }
    });
    let output = snapshot(&sandbox, &server);
    assert!(!output.status.success());
    let error = String::from_utf8_lossy(&output.stderr);
    assert!(error.contains("HTTP Error 302:"));
    assert!(error.contains("lead to an infinite loop.\nThe last 30x error message was:\nFixture"));
    assert_eq!(server.requests().len(), 7);
    assert!(!sandbox.path().join("cache/candidate_repos.json").exists());
}
