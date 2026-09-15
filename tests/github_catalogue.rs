//! GitHub discovery contracts exercised against a local REST/GraphQL fixture.

mod support;

use std::fs;

use serde_json::{Value, json};
use support::http::{Response, Server};
use support::*;

fn commit(base: &str, name: &str) -> Value {
    json!({"oid": name, "zipballUrl": format!("{base}/{name}.zip"), "committedDate": "2026-09-01T00:00:00Z"})
}

fn repository(base: &str, name: &str) -> Value {
    let target = commit(base, "source");
    if name == "extra" {
        return json!({
            "defaultBranchRef": {"target": target},
            "releases": {"nodes": []},
            "refs": {"nodes": [{"name": "v1", "target": commit(base, "extra")}]},
        });
    }
    json!({
        "defaultBranchRef": {"target": commit(base, "default-branch-not-indexed")},
        "releases": {"nodes": [{
            "publishedAt": "2026-09-01T00:00:00Z",
            "tag": {"target": target},
            "releaseAssets": {"nodes": [
                {"name": "PLUGIN.ZIP", "downloadUrl": format!("{base}/asset.zip"), "size": 100, "contentType": "raw"},
                {"name": "large.zip", "downloadUrl": format!("{base}/oversized.zip"), "size": 104857601, "contentType": "application/zip"},
                {"name": "wrong.zip", "downloadUrl": format!("{base}/wrong-type.zip"), "size": 100, "contentType": "text/plain"},
            ]},
        }, {
            "publishedAt": "2025-08-31T23:59:59Z",
            "tag": {"target": commit(base, "old")},
            "releaseAssets": {"nodes": []},
        }]},
        "refs": {"nodes": [
            {"name": "v1", "target": target},
            {"name": "v3", "target": {"target": commit(base, "tag")}},
            {"name": "not-a-version", "target": commit(base, "ignored-tag")},
        ]},
    })
}

#[test]
fn github_discovery_combines_lists_releases_and_tags_with_account_scoped_cache() {
    let sandbox = Sandbox::new();
    let mut archives = std::collections::HashMap::new();
    for (archive_name, plugin_name, version, host) in [
        ("source", "example", "1.0", "main"),
        ("asset", "example", "2.0", "main"),
        ("tag", "example", "3.0", "main"),
        ("extra", "additional", "1.0", "extra"),
    ] {
        let path = sandbox.path().join(format!("{archive_name}.zip"));
        let mut manifest = identity_manifest(version, &format!("https://github.com/owner/{host}"));
        manifest["plugin"]["name"] = json!(plugin_name);
        if archive_name == "asset" {
            let mut foreign = identity_manifest("99.0", "https://github.com/unrelated/repository");
            foreign["plugin"]["name"] = json!("foreign");
            let bytes = serde_json::to_vec(&foreign).unwrap();
            archive_manifest(
                &path,
                &manifest,
                &[
                    ("foreign/ida-plugin.json", &bytes),
                    ("foreign/plugin.py", b"# unrelated identity"),
                ],
            );
        } else {
            archive_manifest(&path, &manifest, &[]);
        }
        archives.insert(format!("/{archive_name}.zip"), fs::read(path).unwrap());
    }
    let server = Server::start(move |request, base| {
        if request.path.starts_with("/search/code?") {
            Response::json(json!({"items": [
                {"repository": {"full_name": "Owner/Main"}},
                {"repository": {"full_name": "Owner/Ignored"}},
            ]}))
        } else if request.path == "/graphql" {
            let body: Value = serde_json::from_slice(&request.body).unwrap();
            Response::json(
                json!({"data": {"repository": repository(base, body["variables"]["name"].as_str().unwrap())}}),
            )
        } else if let Some(bytes) = archives.get(&request.path) {
            Response::zip(bytes.clone())
        } else {
            Response::missing()
        }
    });
    let extra = sandbox.path().join("extra.txt");
    let ignored = sandbox.path().join("ignored.txt");
    fs::write(&extra, "# repository list\nOwner/Extra\nOWNER/MAIN\n").unwrap();
    fs::write(&ignored, "owner/ignored\n").unwrap();
    let args = [
        "plugin",
        "--repo",
        "github",
        "--with-repos-list",
        extra.to_str().unwrap(),
        "--with-ignored-repos-list",
        ignored.to_str().unwrap(),
        "repo",
        "snapshot",
    ];
    let run = |token: &str| {
        sandbox
            .command(&args)
            .env("GITHUB_TOKEN", token)
            .env("GITHUB_API_URL", &server.url)
            .output()
            .unwrap()
    };
    let first = run("fixture-token");
    assert_success(&first);
    let snapshot: Value = serde_json::from_slice(&first.stdout).unwrap();
    assert_eq!(snapshot["plugins"].as_array().unwrap().len(), 2);
    let example = snapshot["plugins"]
        .as_array()
        .unwrap()
        .iter()
        .find(|plugin| plugin["name"] == "example")
        .unwrap();
    assert_eq!(example["versions"].as_object().unwrap().len(), 3);
    assert_eq!(example["versions"]["1.0"].as_array().unwrap().len(), 1);
    let requests = server.requests();
    assert_eq!(
        requests.iter().filter(|request| request.path.starts_with("/search/code?")).count(),
        2
    );
    assert_eq!(requests.iter().filter(|request| request.path == "/graphql").count(), 2);
    assert_eq!(requests.iter().filter(|request| request.path == "/source.zip").count(), 1);
    for request in &requests {
        if request.path.ends_with(".zip") {
            assert!(!request.headers.to_lowercase().contains("authorization:"));
        } else {
            assert!(request.headers.to_lowercase().contains("authorization: bearer fixture-token"));
        }
    }
    assert_success(&run("fixture-token"));
    assert_eq!(
        server.requests().len(),
        requests.len(),
        "warm catalogue should reuse metadata and archive caches"
    );
    assert_success(&run("another-fixture-token"));
    assert_eq!(
        server.requests().len(),
        requests.len() * 2,
        "a different account must not inherit discovery results"
    );
    for entry in fs::read_dir(sandbox.path().join("cache/github-catalogue")).unwrap() {
        let path = entry.unwrap().path();
        fs::File::options()
            .write(true)
            .open(path)
            .unwrap()
            .set_modified(std::time::SystemTime::now() - std::time::Duration::from_secs(172800))
            .unwrap();
    }
    let before = server.requests().len();
    assert_success(&run("fixture-token"));
    assert_eq!(
        server.requests().len(),
        before + 4,
        "expired candidate/release metadata must refresh while archive bytes remain cached"
    );
}

#[test]
fn recursive_directory_catalogues_index_every_plugin_in_each_archive() {
    let sandbox = Sandbox::new();
    let directory = sandbox.path().join("repository/nested");
    fs::create_dir_all(&directory).unwrap();
    let host = "https://github.com/example/plugins";
    let manifest = identity_manifest("1.0", host);
    let mut other = manifest.clone();
    other["plugin"]["name"] = json!("other");
    let bytes = serde_json::to_vec(&other).unwrap();
    archive_manifest(
        &directory.join("plugins.zip"),
        &manifest,
        &[("other/ida-plugin.json", &bytes), ("other/plugin.py", b"# fixture")],
    );
    fs::write(directory.join("invalid.zip"), "not an archive").unwrap();
    let invalid = sandbox.run(&[
        "plugin",
        "--repo",
        directory.parent().unwrap().to_str().unwrap(),
        "repo",
        "snapshot",
    ]);
    assert!(!invalid.status.success());
    assert!(invalid.stdout.is_empty());
    fs::remove_file(directory.join("invalid.zip")).unwrap();
    let output = sandbox.run(&[
        "plugin",
        "--repo",
        directory.parent().unwrap().to_str().unwrap(),
        "repo",
        "snapshot",
    ]);
    assert_success(&output);
    let snapshot: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(snapshot["plugins"].as_array().unwrap().len(), 2);
    assert_eq!(snapshot["plugins"][0]["name"], "example");
    assert_eq!(snapshot["plugins"][1]["name"], "other");
}

#[test]
fn explicit_github_source_reports_missing_token_and_invalid_lists() {
    let sandbox = Sandbox::new();
    let missing_token = sandbox
        .command(&["plugin", "--repo", "github", "search", "example"])
        .env_remove("GITHUB_TOKEN")
        .env_remove("GH_TOKEN")
        .output()
        .unwrap();
    assert!(!missing_token.status.success());
    assert!(String::from_utf8_lossy(&missing_token.stderr).contains("GitHub token required"));
    let list = sandbox.path().join("repositories.txt");
    fs::write(&list, "../invalid\n").unwrap();
    let output = sandbox
        .command(&[
            "plugin",
            "--repo",
            "github",
            "--with-repos-list",
            list.to_str().unwrap(),
            "repo",
            "snapshot",
        ])
        .env("GITHUB_TOKEN", "fixture-token")
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("invalid GitHub repository"));
}

#[test]
fn github_archive_indexing_errors_fail_the_catalogue() {
    let sandbox = Sandbox::new();
    let server = Server::start(|request, base| {
        if request.path.starts_with("/search/code?") {
            Response::json(json!({"items": [{"repository": {"full_name": "Owner/Main"}}]}))
        } else if request.path == "/graphql" {
            Response::json(json!({"data": {"repository": repository(base, "main")}}))
        } else if request.path == "/asset.zip" {
            Response::zip(b"not ZIP".to_vec())
        } else {
            Response::missing()
        }
    });
    let output = sandbox
        .command(&["plugin", "--repo", "github", "repo", "snapshot"])
        .env("GITHUB_TOKEN", "fixture-token")
        .env("GITHUB_API_URL", &server.url)
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("ZIP"));
    assert!(server.requests().iter().any(|request| request.path == "/asset.zip"));
    assert!(!server.requests().iter().any(|request| request.path == "/source.zip"));
}

#[test]
fn direct_github_install_rejects_ambiguous_and_oversized_release_assets() {
    for oversized in [false, true] {
        let sandbox = Sandbox::new();
        let server = Server::start(move |request, base| {
            if request.path != "/repos/owner/plugin/releases/latest" {
                return Response::missing();
            }
            let mut assets = vec![
                json!({"name": "one.ZIP", "browser_download_url": format!("{base}/one.zip"), "size": if oversized {104857601} else {100}}),
            ];
            if !oversized {
                assets.push(json!({"name": "two.zip", "browser_download_url": format!("{base}/two.zip"), "size": 100}));
            }
            Response::json(json!({"assets": assets}))
        });
        let output = sandbox
            .command(&["plugin", "install", "https://github.com/owner/plugin.git"])
            .env("GITHUB_API_URL", &server.url)
            .output()
            .unwrap();
        assert!(!output.status.success());
        let error = String::from_utf8_lossy(&output.stderr);
        let expected = if oversized {
            "Asset one.ZIP (104857601 bytes) exceeds maximum size limit (104857600 bytes)"
        } else {
            "Multiple .zip assets found in release: one.ZIP, two.zip. Cannot determine which to install."
        };
        assert!(error.contains(expected), "{error}");
        assert_eq!(
            server.requests().len(),
            1,
            "invalid asset selection must not download any archive"
        );
        assert!(!sandbox.path().join("idausr/plugins").exists());
    }
}
