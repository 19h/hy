//! Cached Python URLs survive indexing; each consumer controls serialization.

use super::*;

fn release(commit: &str, url: &str, asset: bool) -> Value {
    let assets = if asset {
        vec![json!({
            "name": "plugin.zip",
            "download_url": url,
            "size": 1,
            "content_type": "raw",
        })]
    } else {
        vec![]
    };
    json!({
        "name": "fixture",
        "tag_name": commit,
        "commit_hash": if asset { ".." } else { commit },
        "created_at": "2026-01-01",
        "published_at": "2026-01-01",
        "is_prerelease": false,
        "is_draft": false,
        "url": "https://example.test/release",
        "zipball_url": url,
        "assets": assets,
    })
}

fn archive(sandbox: &Sandbox, version: &str, host: &str) -> Vec<u8> {
    let path = sandbox.path().join(format!("fixture-{version}.zip"));
    archive_manifest(&path, &identity_manifest(version, host), &[]);
    fs::read(path).unwrap()
}

fn cache_path(sandbox: &Sandbox, name: &str, asset: bool) -> std::path::PathBuf {
    sandbox.path().join("cache/owner/repo").join(if asset {
        format!("release-assets/{name}/plugin.zip")
    } else {
        format!("source-archives/{name}/source.zip")
    })
}

fn prepare(
    sandbox: &Sandbox,
    server: &Server,
    point: &str,
    asset: bool,
    first: &[u8],
    second: bool,
) {
    let path = cache_path(sandbox, "first", asset);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, first).unwrap();
    fs::write(sandbox.path().join("cache/candidate_repos.json"), br#"["owner/repo"]"#).unwrap();
    let mut releases = vec![release("first", "__RAW__", asset)];
    if second {
        releases.push(release("second", &format!("{}/second", server.url), asset));
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
    .replace("\"__RAW__\"", &format!(r#""https://example.test/\u{point}.zip""#));
    fs::write(sandbox.path().join("cache/owner/repo/releases.json"), metadata).unwrap();
}

fn run(sandbox: &Sandbox, server: &Server, command: &[&str]) -> std::process::Output {
    let mut args = vec!["plugin", "--repo", "github"];
    args.extend_from_slice(command);
    sandbox
        .command(&args)
        .env("GITHUB_TOKEN", "fixture-token")
        .env("GITHUB_API_URL", &server.url)
        .output()
        .unwrap()
}

#[test]
fn cached_urls_survive_all_acquisition_before_snapshot_or_search_serialization() {
    for point in ["d800", "dcff"] {
        for asset in [false, true] {
            let sandbox = Sandbox::new();
            let first = archive(&sandbox, "1.0", "https://github.com/owner/repo");
            let second = archive(&sandbox, "2.0", "https://github.com/owner/repo");
            let server = Server::start(move |request, _| {
                assert_eq!(request.path, "/second");
                Response::zip(second.clone())
            });
            prepare(&sandbox, &server, point, asset, &first, true);
            let snapshot = run(&sandbox, &server, &["repo", "snapshot"]);
            assert!(!snapshot.status.success());
            assert!(String::from_utf8_lossy(&snapshot.stderr).contains("surrogate"));
            assert_eq!(server.requests().len(), 1);
            assert!(cache_path(&sandbox, "second", asset).is_file());
            for command in [vec!["search", "--json"], vec!["search", "example", "--json"]] {
                let output = run(&sandbox, &server, &command);
                assert_success(&output);
                assert!(String::from_utf8_lossy(&output.stdout).contains("example"));
            }
            let exact = run(&sandbox, &server, &["search", "example==1.0", "--json"]);
            assert_success(&exact);
            let expected = format!(r#""url": "https://example.test/\u{point}.zip""#);
            assert!(String::from_utf8_lossy(&exact.stdout).contains(&expected));
            #[cfg(unix)]
            {
                let plain = run(&sandbox, &server, &["search", "example==1.0"]);
                if point == "dcff" {
                    assert_success(&plain);
                    assert!(
                        plain.stdout.windows(b"\xff.zip".len()).any(|part| part == b"\xff.zip")
                    );
                } else {
                    assert!(!plain.status.success());
                }
            }
            assert_eq!(server.requests().len(), 1);
        }
    }
}

#[test]
fn later_transport_failures_precede_snapshot_surrogate_errors() {
    for asset in [false, true] {
        let sandbox = Sandbox::new();
        let first = archive(&sandbox, "1.0", "https://github.com/owner/repo");
        let server = Server::start(|request, _| {
            assert_eq!(request.path, "/second");
            Response {
                status: 401,
                ..Response::json(json!({}))
            }
        });
        prepare(&sandbox, &server, "d800", asset, &first, true);
        let output = run(&sandbox, &server, &["repo", "snapshot"]);
        assert!(!output.status.success());
        assert!(String::from_utf8_lossy(&output.stderr).contains("HTTP Error 401"));
        assert_eq!(server.requests().len(), 1);
        assert!(!cache_path(&sandbox, "second", asset).exists());
    }
}

#[test]
fn filtered_archives_do_not_require_their_url_to_be_serializable() {
    let sandbox = Sandbox::new();
    let first = archive(&sandbox, "1.0", "https://github.com/foreign/repo");
    let server = Server::start(|_, _| panic!("cache-only catalogue must not fetch"));
    prepare(&sandbox, &server, "d800", false, &first, false);
    let output = run(&sandbox, &server, &["repo", "snapshot"]);
    assert_success(&output);
    assert_eq!(serde_json::from_slice::<Value>(&output.stdout).unwrap()["plugins"], json!([]));
    assert!(server.requests().is_empty());
}
