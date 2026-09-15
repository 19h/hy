//! Archive ordering, cache identity and acquisition failure boundaries.

use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use super::*;

fn archive_bytes(sandbox: &Sandbox, host: &str, version: &str) -> Vec<u8> {
    let path = sandbox.path().join("fixture.zip");
    archive_manifest(&path, &identity_manifest(version, host), &[]);
    fs::read(path).unwrap()
}

fn asset(name: &str, url: String, size: u64) -> Value {
    json!({"name":name, "downloadUrl":url, "size":size, "contentType":"raw"})
}

pub(super) fn release_metadata(source: Value, tag: &str, assets: Vec<Value>) -> Value {
    json!({
        "defaultBranchRef": {"target": source},
        "releases": {"nodes": [{
            "name": "fixture release",
            "tagName": tag,
            "createdAt": "2026-09-01",
            "publishedAt": "2026-09-01",
            "isPrerelease": false,
            "isDraft": false,
            "url": "https://github.com/owner/repo/releases/fixture",
            "tag": {"target": source},
            "releaseAssets": {"nodes": assets},
        }]},
        "refs": {"nodes": []},
    })
}

fn snapshot(sandbox: &Sandbox, server: &Server) -> std::process::Output {
    sandbox
        .command(&["plugin", "--repo", "github", "repo", "snapshot"])
        .env("GITHUB_TOKEN", "fixture-token")
        .env("GITHUB_API_URL", &server.url)
        .output()
        .unwrap()
}

fn expire_release_metadata(sandbox: &Sandbox) {
    let mut changed = 0;
    for path in cache_files(sandbox) {
        if serde_json::from_slice::<Value>(&fs::read(&path).unwrap())
            .ok()
            .is_some_and(|value| value.get("releases").is_some())
        {
            fs::File::options()
                .write(true)
                .open(path)
                .unwrap()
                .set_modified(std::time::SystemTime::now() - std::time::Duration::from_secs(172800))
                .unwrap();
            changed += 1;
        }
    }
    assert!(changed > 0);
}

#[test]
fn catalogue_preserves_duplicates_and_fetches_global_asset_phase_before_sources() {
    let sandbox = Sandbox::new();
    let archives: HashMap<_, _> = ["a", "a-b"]
        .into_iter()
        .map(|owner| {
            (
                owner.to_owned(),
                archive_bytes(&sandbox, &format!("https://github.com/{owner}/r"), "1"),
            )
        })
        .collect();
    let server = Server::start(move |request, base| {
        if request.path.starts_with("/search/code?") {
            return Response::json(
                json!({"items":[{"repository":{"full_name":"a-b/r"}}, {"repository":{"full_name":"a/r"}}]}),
            );
        }
        if request.path == "/graphql" {
            return graphql_response(request, |owner, _| {
                let source = json!({"oid":"shared-commit", "zipballUrl":format!("{base}/source/{owner}/release"), "committedDate":"2026-09-01"});
                let first = asset("z.zip", format!("{base}/asset/{owner}/z"), 1);
                let mut metadata = release_metadata(
                    source,
                    "v1",
                    vec![
                        first.clone(),
                        asset("a.zip", format!("{base}/asset/{owner}/a"), 1),
                        first,
                    ],
                );
                let release = metadata["releases"]["nodes"][0].clone();
                metadata["releases"]["nodes"].as_array_mut().unwrap().push(release);
                let tag = json!({"name":"vtag", "target":{"oid":"shared-commit", "zipballUrl":format!("{base}/source/{owner}/tag"), "committedDate":"2026-09-01"}});
                metadata["refs"]["nodes"] = json!([tag.clone(), tag]);
                metadata
            });
        }
        let owner = request.path.split('/').nth(2).unwrap();
        Response::zip(archives[owner].clone())
    });
    let output = snapshot(&sandbox, &server);
    assert_success(&output);
    let catalogue: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(catalogue["plugins"].as_array().unwrap().len(), 2);
    for plugin in catalogue["plugins"].as_array().unwrap() {
        assert_eq!(plugin["versions"]["1"].as_array().unwrap().len(), 9);
    }
    let paths: Vec<_> = server
        .requests()
        .into_iter()
        .map(|request| request.path)
        .filter(|path| !path.starts_with("/search/code?"))
        .collect();
    assert_eq!(
        paths,
        [
            "/graphql",
            "/asset/a/z",
            "/asset/a/a",
            "/asset/a-b/z",
            "/asset/a-b/a",
            "/source/a/release",
            "/source/a-b/release"
        ]
    );
}

#[test]
fn cached_assets_precede_size_checks_and_commit_identity_survives_url_changes() {
    let sandbox = Sandbox::new();
    let old = archive_bytes(&sandbox, "https://github.com/owner/main", "1");
    let new = archive_bytes(&sandbox, "https://github.com/owner/main", "2");
    let phase = Arc::new(AtomicUsize::new(0));
    let observed_phase = Arc::clone(&phase);
    let server = Server::start(move |request, base| {
        let phase = observed_phase.load(Ordering::SeqCst);
        if request.path.starts_with("/search/code?") {
            return Response::json(json!({"items":[{"repository":{"full_name":"owner/main"}}]}));
        }
        if request.path == "/graphql" {
            let asset_url = format!(
                "{base}/{}",
                if phase == 1 {
                    "new"
                } else {
                    "old"
                }
            );
            let source = json!({"oid":"same-commit", "zipballUrl":format!("{base}/source-{}", if phase == 0 { "old" } else { "new" }), "committedDate":"2026-09-01"});
            return single_graphql(release_metadata(
                source,
                if phase == 2 {
                    "v2"
                } else {
                    "v1"
                },
                vec![asset(
                    "plugin.zip",
                    asset_url,
                    if phase == 1 {
                        104_857_601
                    } else {
                        1
                    },
                )],
            ));
        }
        assert!(
            matches!(request.path.as_str(), "/old" | "/source-old"),
            "cache miss at {}",
            request.path
        );
        Response::zip(if phase == 2 {
            new.clone()
        } else {
            old.clone()
        })
    });
    assert_success(&snapshot(&sandbox, &server));
    assert_eq!(server.requests().len(), 5);
    phase.store(1, Ordering::SeqCst);
    expire_release_metadata(&sandbox);
    let output = snapshot(&sandbox, &server);
    assert_success(&output);
    let catalogue: Value = serde_json::from_slice(&output.stdout).unwrap();
    let locations = catalogue["plugins"][0]["versions"]["1"].as_array().unwrap();
    assert_eq!(locations.len(), 2);
    assert!(locations.iter().any(|location| location["url"] == format!("{}/new", server.url)));
    assert!(
        locations.iter().any(|location| location["url"] == format!("{}/source-new", server.url))
    );
    assert_eq!(server.requests().len(), 6, "only metadata should refresh");
    phase.store(2, Ordering::SeqCst);
    expire_release_metadata(&sandbox);
    let output = snapshot(&sandbox, &server);
    assert_success(&output);
    let catalogue: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(catalogue["plugins"][0]["versions"].get("2").is_some());
    assert_eq!(server.requests().len(), 8, "a new release must not reuse the old URL cache");
}

#[test]
fn archive_value_errors_are_skipped_but_other_acquisition_errors_propagate() {
    for mode in 0..4 {
        let sandbox = Sandbox::new();
        let archive = archive_bytes(&sandbox, "https://github.com/owner/main", "1");
        let server = Server::start_with_headers(move |request, base| {
            if request.path.starts_with("/search/code?") {
                return (
                    Response::json(json!({"items":[{"repository":{"full_name":"owner/main"}}]})),
                    vec![],
                );
            }
            if request.path == "/graphql" {
                let source = commit(base, "source");
                return (
                    single_graphql(release_metadata(
                        source,
                        "v1",
                        vec![
                            asset("bad.zip", format!("{base}/bad"), 1),
                            asset("good.zip", format!("{base}/good"), 1),
                        ],
                    )),
                    vec![],
                );
            }
            let failure_path = if mode == 1 {
                "/source.zip"
            } else {
                "/bad"
            };
            if request.path == failure_path {
                let (status, headers) = match mode {
                    0 => (403, vec![("Retry-After".into(), "invalid".into())]),
                    1 => (
                        200,
                        vec![
                            ("X-RateLimit-Remaining".into(), "0".into()),
                            ("X-RateLimit-Reset".into(), "invalid".into()),
                        ],
                    ),
                    2 => (403, vec![("X-RateLimit-Reset".into(), "9".repeat(309))]),
                    _ => (404, vec![]),
                };
                return (
                    Response {
                        status,
                        ..Response::json(json!({}))
                    },
                    headers,
                );
            }
            (Response::zip(archive.clone()), vec![])
        });
        let output = snapshot(&sandbox, &server);
        assert_eq!(output.status.success(), mode < 2, "{output:?}");
        if mode < 2 {
            let catalogue: Value = serde_json::from_slice(&output.stdout).unwrap();
            assert_eq!(catalogue["plugins"][0]["versions"]["1"].as_array().unwrap().len(), 2);
            assert_eq!(server.requests().len(), 6);
        } else {
            assert_eq!(server.requests().len(), 4);
        }
    }
}

#[test]
fn a_metadata_batch_failure_prevents_all_archive_acquisition() {
    let sandbox = Sandbox::new();
    let server = Server::start(|request, base| {
        if request.path.starts_with("/search/code?") {
            return Response::json(
                json!({"items":[{"repository":{"full_name":"a/r"}}, {"repository":{"full_name":"z/r"}}]}),
            );
        }
        assert_eq!(request.path, "/graphql");
        if graphql_repositories(request).iter().any(|entry| entry.owner == "z") {
            return Response {
                status: 401,
                ..Response::json(json!({}))
            };
        }
        single_graphql(release_metadata(
            commit(base, "source"),
            "v1",
            vec![asset("plugin.zip", format!("{base}/asset"), 1)],
        ))
    });
    assert!(!snapshot(&sandbox, &server).status.success());
    assert_eq!(server.requests().len(), 3);
}
