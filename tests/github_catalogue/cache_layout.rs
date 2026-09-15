//! Shared upstream cache locations and filesystem publication boundaries.

use std::path::Path;
use std::process::Command;

use sha2::{Digest, Sha256};

use super::*;

fn snapshot(sandbox: &Sandbox, server: &Server, token: &str) -> std::process::Output {
    sandbox
        .command(&["plugin", "--repo", "github", "repo", "snapshot"])
        .env("GITHUB_TOKEN", token)
        .env("GITHUB_API_URL", &server.url)
        .output()
        .unwrap()
}

fn archive(sandbox: &Sandbox) -> Vec<u8> {
    let path = sandbox.path().join("fixture.zip");
    archive_manifest(&path, &identity_manifest("1.0", "https://github.com/owner/repo"), &[]);
    fs::read(path).unwrap()
}

fn search() -> Response {
    Response::json(json!({"items": [{"repository": {"full_name": "owner/repo"}}]}))
}

fn metadata(base: &str, tag: &str, name: &str) -> Value {
    let mut value = acquisition::release_metadata(
        commit(base, "source"),
        tag,
        vec![json!({
            "name": name, "contentType": "raw", "size": 1, "downloadUrl": format!("{base}/asset"),
        })],
    );
    value["releases"]["nodes"][0]["name"] = json!("Unicode Δ 🧠");
    value
}

#[test]
fn source_getters_read_native_files_and_cache_reuse_crosses_tokens_and_origins() {
    let sandbox = Sandbox::new();
    let bytes = archive(&sandbox);
    let expected_hash = format!("{:x}", Sha256::digest(&bytes));
    let server = Server::start(move |request, base| {
        if request.path.starts_with("/search/code?") {
            return search();
        }
        if request.path == "/graphql" {
            return single_graphql(metadata(base, "v1", "plugin.zip"));
        }
        assert!(matches!(request.path.as_str(), "/asset" | "/source.zip"));
        Response::zip(bytes.clone())
    });
    assert_success(&snapshot(&sandbox, &server, "first-token"));
    let root = sandbox.path().join("cache");
    assert!(root.join("owner/repo/release-assets/v1/plugin.zip").is_file());
    assert!(root.join("owner/repo/source-archives/source/source.zip").is_file());
    let cached_text = fs::read(root.join("owner/repo/releases.json")).unwrap();
    assert!(cached_text.is_ascii());
    assert!(String::from_utf8_lossy(&cached_text).contains("\\u0394"));
    verify_source_reads(&root, &expected_hash);
    let another = Server::start(|_, _| panic!("warm cache must not contact another origin"));
    assert_success(&snapshot(&sandbox, &another, "another-token"));
    assert!(another.requests().is_empty());
    assert_eq!(server.requests().len(), 5);
}

fn verify_source_reads(root: &Path, expected_hash: &str) {
    let Some(python) = std::env::var_os("HY_TEST_BUNDLE_ORACLE_PYTHON") else {
        return;
    };
    let source =
        std::env::var_os("HY_TEST_HCLI_SOURCE").map(std::path::PathBuf::from).unwrap_or_else(
            || Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().join("ida-hcli"),
        );
    let output = Command::new(python)
        .args(["-I", "-B", "-c", include_str!("cache_layout/reference.py")])
        .arg(source)
        .arg(root)
        .output()
        .unwrap();
    assert_success(&output);
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["candidates"], json!(["owner/repo"]));
    assert_eq!(value["release_name"], "Unicode Δ 🧠");
    assert_eq!(value["asset"], expected_hash);
    assert_eq!(value["source"], expected_hash);
}

#[test]
fn preexisting_upstream_metadata_and_archives_need_no_network_requests() {
    let sandbox = Sandbox::new();
    let bytes = archive(&sandbox);
    let root = sandbox.path().join("cache");
    let source = root.join("owner/repo/source-archives/commit");
    fs::create_dir_all(&source).unwrap();
    fs::write(source.join("source.zip"), bytes).unwrap();
    fs::write(root.join("candidate_repos.json"), "[\n  \"owner/repo\"\n]").unwrap();
    let commit = json!({"commit_hash": "commit", "committed_date": "2026-01-01", "zipball_url": "https://unused.test/source"});
    let cached = json!({
        "default_branch": commit,
        "releases": [],
        "tags": [{"tag_name": "v1", "commit_hash": "commit", "committed_date": "2026-01-01", "zipball_url": "https://unused.test/source"}],
    });
    fs::write(root.join("owner/repo/releases.json"), serde_json::to_vec_pretty(&cached).unwrap())
        .unwrap();
    let server = Server::start(|_, _| panic!("upstream cache should satisfy every lookup"));
    let output = snapshot(&sandbox, &server, "fixture-token");
    assert_success(&output);
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["plugins"].as_array().unwrap().len(), 1);
    assert!(server.requests().is_empty());
}

#[test]
fn invalid_release_cache_keys_skip_assets_before_download_and_keep_valid_sources() {
    for tag in ["bad/tag", "", "..", "täg"] {
        let sandbox = Sandbox::new();
        let bytes = archive(&sandbox);
        let server = Server::start(move |request, base| {
            if request.path.starts_with("/search/code?") {
                return search();
            }
            if request.path == "/graphql" {
                return single_graphql(metadata(base, tag, "plugin.zip"));
            }
            assert_eq!(request.path, "/source.zip");
            Response::zip(bytes.clone())
        });
        assert_success(&snapshot(&sandbox, &server, "fixture-token"));
        assert_eq!(server.requests().len(), 4);
        assert!(!sandbox.path().join("cache/owner/repo/release-assets").exists());
    }
}

#[test]
fn asset_filename_parent_directories_are_not_created_during_publication() {
    let sandbox = Sandbox::new();
    let bytes = archive(&sandbox);
    let server = Server::start(move |request, base| {
        if request.path.starts_with("/search/code?") {
            return search();
        }
        if request.path == "/graphql" {
            return single_graphql(metadata(base, "v1", "missing/plugin.zip"));
        }
        assert_eq!(request.path, "/asset");
        Response::zip(bytes.clone())
    });
    assert!(!snapshot(&sandbox, &server, "fixture-token").status.success());
    assert_eq!(server.requests().len(), 4);
    assert!(sandbox.path().join("cache/owner/repo/release-assets/v1").is_dir());
    assert!(!sandbox.path().join("cache/owner/repo/release-assets/v1/missing").exists());
}

#[cfg(unix)]
#[test]
fn candidate_publication_follows_a_dangling_file_symlink() {
    let sandbox = Sandbox::new();
    let root = sandbox.path().join("cache");
    fs::create_dir_all(&root).unwrap();
    let target = sandbox.path().join("target.json");
    let alias = root.join("candidate_repos.json");
    std::os::unix::fs::symlink(&target, &alias).unwrap();
    let server = Server::start(|request, _| {
        assert!(request.path.starts_with("/search/code?"));
        Response::json(json!({"items": []}))
    });
    assert_success(&snapshot(&sandbox, &server, "fixture-token"));
    assert!(fs::symlink_metadata(alias).unwrap().file_type().is_symlink());
    assert_eq!(fs::read_to_string(target).unwrap(), "[]");
}
