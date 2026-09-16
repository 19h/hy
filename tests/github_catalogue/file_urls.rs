//! Catalogue file downloads use urllib request/open semantics before caching.

use super::index_urls::{archive, cache_path, release};
use super::*;

fn prepare(sandbox: &Sandbox, document: &str, asset: bool, next: Option<&str>) {
    let root = sandbox.path().join("cache/owner/repo");
    fs::create_dir_all(&root).unwrap();
    fs::write(sandbox.path().join("cache/candidate_repos.json"), br#"["owner/repo"]"#).unwrap();
    let mut releases = vec![release("first", "__URL__", asset)];
    if let Some(next) = next {
        releases.push(release("second", next, asset));
    }
    let metadata = json!({
        "default_branch": {
            "commit_hash": "default",
            "committed_date": "2026-01-01",
            "zipball_url": "unused",
        },
        "releases": releases,
        "tags": [],
    })
    .to_string()
    .replace("\"__URL__\"", document);
    fs::write(root.join("releases.json"), metadata).unwrap();
}

fn snapshot(sandbox: &Sandbox, server: &Server) -> std::process::Output {
    sandbox
        .command(&["plugin", "--repo", "github", "repo", "snapshot"])
        .env("GITHUB_TOKEN", "fixture-token")
        .env("GITHUB_API_URL", &server.url)
        .output()
        .unwrap()
}

#[test]
fn file_requests_keep_query_and_control_text_and_accept_wrapped_schemes() {
    for asset in [false, true] {
        for form in ["opaque", "upper", "wrapped", "localhost"] {
            let sandbox = Sandbox::new();
            let plain = archive(&sandbox, "1.0", "https://github.com/owner/repo");
            let query = archive(&sandbox, "2.0", "https://github.com/owner/repo");
            let path = sandbox.path().join("payload\t.zip");
            fs::write(&path, plain).unwrap();
            fs::write(format!("{}?variant", path.display()), &query).unwrap();
            let url = match form {
                "opaque" => format!("file:{}?variant#fragment", path.display()),
                "upper" => format!("FILE:{}?variant", path.display()),
                "wrapped" => format!(" <URL:file:{}?variant> ", path.display()),
                "localhost" => format!("file://localhost{}?variant", path.display()),
                _ => unreachable!(),
            };
            let server = Server::start(|_, _| panic!("file request must not use HTTP"));
            prepare(&sandbox, &serde_json::to_string(&url).unwrap(), asset, None);
            let output = snapshot(&sandbox, &server);
            assert_success(&output);
            let document: Value = serde_json::from_slice(&output.stdout).unwrap();
            assert!(document["plugins"][0]["versions"].get("2.0").is_some());
            assert!(document["plugins"][0]["versions"].get("1.0").is_none());
            assert_eq!(fs::read(cache_path(&sandbox, "first", asset)).unwrap(), query);
            assert!(server.requests().is_empty());
        }
    }
}

#[test]
fn invalid_file_path_values_skip_only_their_archive() {
    for asset in [false, true] {
        for document in [r#""file:/missing/\ud800""#, r#""file:/missing/%00""#] {
            let sandbox = Sandbox::new();
            let bytes = archive(&sandbox, "2.0", "https://github.com/owner/repo");
            let server = Server::start(move |request, _| {
                assert_eq!(request.path, "/second");
                Response::zip(bytes.clone())
            });
            prepare(&sandbox, document, asset, Some(&format!("{}/second", server.url)));
            let output = snapshot(&sandbox, &server);
            assert_success(&output);
            assert!(!cache_path(&sandbox, "first", asset).exists());
            assert!(cache_path(&sandbox, "second", asset).is_file());
            assert_eq!(server.requests().len(), 1);
        }
    }
}

#[test]
fn discarded_surrogate_fragments_do_not_prevent_file_cache_publication() {
    for asset in [false, true] {
        let sandbox = Sandbox::new();
        let bytes = archive(&sandbox, "1.0", "https://github.com/owner/repo");
        let path = sandbox.path().join("fixture-1.0.zip");
        let document = format!(r#""file:{}#\ud800""#, path.display());
        let server = Server::start(|_, _| panic!("file request must not use HTTP"));
        prepare(&sandbox, &document, asset, None);
        let output = snapshot(&sandbox, &server);
        assert!(!output.status.success());
        assert!(String::from_utf8_lossy(&output.stderr).contains("surrogate"));
        assert_eq!(fs::read(cache_path(&sandbox, "first", asset)).unwrap(), bytes);
        assert!(server.requests().is_empty());
    }
}

#[test]
fn trailing_file_separators_fail_without_publishing_or_fetching_later_archives() {
    let sandbox = Sandbox::new();
    archive(&sandbox, "1.0", "https://github.com/owner/repo");
    let url = format!("file:{}/fixture-1.0.zip/", sandbox.path().display());
    let server = Server::start(|_, _| panic!("failed file open must stop acquisition"));
    prepare(
        &sandbox,
        &serde_json::to_string(&url).unwrap(),
        false,
        Some(&format!("{}/second", server.url)),
    );
    let output = snapshot(&sandbox, &server);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("urlopen error"));
    assert!(!cache_path(&sandbox, "first", false).exists());
    assert!(!cache_path(&sandbox, "second", false).exists());
    assert!(server.requests().is_empty());
}
