//! Search wire requests and filtering order against owned CLI fixtures.

use super::*;

fn snapshot(sandbox: &Sandbox, server: &Server, extra: &[&str]) -> std::process::Output {
    let mut args = vec!["plugin", "--repo", "github"];
    args.extend_from_slice(extra);
    args.extend(["repo", "snapshot"]);
    sandbox
        .command(&args)
        .env("GITHUB_TOKEN", "fixture-token")
        .env("GITHUB_API_URL", &server.url)
        .output()
        .unwrap()
}

fn empty_repository(base: &str) -> Value {
    json!({
        "defaultBranchRef": {"target": commit(base, "default")},
        "releases": {"nodes": []},
        "refs": {"nodes": []},
    })
}

#[test]
fn full_pages_continue_after_deduplication_and_falsey_items_end_each_query() {
    let sandbox = Sandbox::new();
    let server = Server::start(|request, base| {
        if request.path.starts_with("/search/code?") {
            let headers = request.headers.to_ascii_lowercase();
            assert!(headers.contains("accept: application/vnd.github.v3+json"));
            assert!(headers.contains("user-agent: ida-hcli"));
            assert!(headers.contains("authorization: bearer fixture-token"));
            return if request.path
                == "/search/code?q=filename%3Aida-plugin.json&per_page=100&page=1"
            {
                Response::json(
                    json!({"items": vec![json!({"repository": {"full_name": "Owner/Repo"}}); 100]}),
                )
            } else if request.path.ends_with("page=2") {
                Response::json(json!({}))
            } else {
                assert_eq!(
                    request.path,
                    "/search/code?q=filename%3Aida-plugin.json%20fork%3Atrue&per_page=100&page=1"
                );
                Response::json(json!({"items": null}))
            };
        }
        assert_eq!(request.path, "/graphql");
        graphql_response(request, |owner, name| {
            assert_eq!((owner, name), ("owner", "repo"));
            empty_repository(base)
        })
    });
    assert_success(&snapshot(&sandbox, &server, &[]));
    assert_eq!(server.requests().len(), 4);
    assert_success(&snapshot(&sandbox, &server, &[]));
    assert_eq!(server.requests().len(), 4);
}

#[test]
fn ignored_names_are_removed_before_parsing_and_list_comments_precede_trimming() {
    let sandbox = Sandbox::new();
    let extra = sandbox.path().join("extra.txt");
    let ignored = sandbox.path().join("ignored.txt");
    fs::write(&extra, "# top\n \n #indented\nOwner/Space Name\u{2028}INVALID\r\n").unwrap();
    fs::write(&ignored, " \n #indented\ninvalid\nx/y/z\nunused/invalid/name\n").unwrap();
    let server = Server::start(|request, base| {
        if request.path.starts_with("/search/code?") {
            return Response::json(json!({"items": [
                {"repository": {"full_name": "Owner/Kept"}},
                {"repository": {"full_name": "x/y/z"}},
                {"repository": {"full_name": "INVALID"}},
            ]}));
        }
        let names: Vec<_> =
            graphql_repositories(request).into_iter().map(|entry| entry.name).collect();
        assert_eq!(names, ["kept", "space name"]);
        graphql_response(request, |_, _| empty_repository(base))
    });
    let args = [
        "--with-repos-list",
        extra.to_str().unwrap(),
        "--with-ignored-repos-list",
        ignored.to_str().unwrap(),
    ];
    assert_success(&snapshot(&sandbox, &server, &args));
    assert_eq!(server.requests().len(), 3);
}

#[test]
fn discovery_is_cached_before_selected_name_failures_and_can_be_reused_after_filtering() {
    for name in ["invalid", "../repo"] {
        let sandbox = Sandbox::new();
        let extra = sandbox.path().join("extra.txt");
        let ignored = sandbox.path().join("ignored.txt");
        fs::write(&extra, name).unwrap();
        fs::write(&ignored, name).unwrap();
        let server = Server::start(|request, _| {
            assert!(request.path.starts_with("/search/code?"));
            Response::json(json!({"items": []}))
        });
        let failed = snapshot(&sandbox, &server, &["--with-repos-list", extra.to_str().unwrap()]);
        assert!(!failed.status.success());
        assert_eq!(server.requests().len(), 2);
        assert_success(&snapshot(
            &sandbox,
            &server,
            &[
                "--with-repos-list",
                extra.to_str().unwrap(),
                "--with-ignored-repos-list",
                ignored.to_str().unwrap(),
            ],
        ));
        assert_eq!(server.requests().len(), 2);
    }
}
