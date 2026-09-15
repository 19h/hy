//! GraphQL validation precedes acquisition; normalized metadata survives cache reuse.

use sha2::{Digest, Sha256};

use super::*;

fn snapshot(sandbox: &Sandbox, server: &Server) -> std::process::Output {
    sandbox
        .command(&["plugin", "--repo", "github", "repo", "snapshot"])
        .env("GITHUB_TOKEN", "fixture-token")
        .env("GITHUB_API_URL", &server.url)
        .output()
        .unwrap()
}

fn search() -> Response {
    Response::json(json!({"items": [
        {"repository": {"full_name": "owner/repo"}},
    ]}))
}

fn metadata(base: &str) -> Value {
    acquisition::release_metadata(commit(base, "source"), "v1", vec![])
}

fn metadata_cache(sandbox: &Sandbox) -> std::path::PathBuf {
    let path = sandbox.path().join("cache/owner/repo/releases.json");
    assert!(path.is_file());
    path
}

#[test]
fn invalid_required_release_fields_fail_before_publication_or_download() {
    for field in ["createdAt", "publishedAt", "isPrerelease", "isDraft", "url", "tag"] {
        let sandbox = Sandbox::new();
        let server = Server::start(move |request, base| {
            if request.path.starts_with("/search/code?") {
                return search();
            }
            assert_eq!(request.path, "/graphql");
            let mut value = metadata(base);
            value["releases"]["nodes"][0].as_object_mut().unwrap().remove(field);
            single_graphql(value)
        });
        let output = snapshot(&sandbox, &server);
        assert!(!output.status.success(), "{field}");
        assert!(output.stdout.is_empty());
        assert_eq!(server.requests().len(), 3);
        assert_eq!(cache_files(&sandbox).len(), 1);
    }
}

#[test]
fn coerced_models_keep_alias_precedence_and_signed_sizes_across_cache_roundtrips() {
    let sandbox = Sandbox::new();
    let path = sandbox.path().join("fixture.zip");
    archive_manifest(&path, &identity_manifest("1.0", "https://github.com/owner/repo"), &[]);
    let archive = fs::read(path).unwrap();
    let server = Server::start(move |request, base| {
        if request.path.starts_with("/search/code?") {
            return search();
        }
        if request.path == "/graphql" {
            let mut value = metadata(base);
            let release = &mut value["releases"]["nodes"][0];
            release["name"] = json!([]);
            release["isPrerelease"] = json!("ON");
            release["isDraft"] = json!("n");
            release["tag"]["target"].as_object_mut().unwrap().remove("committedDate");
            release["releaseAssets"]["nodes"] = json!([
                {
                    "name": "negative.zip",
                    "size": " -1 ",
                    "contentType": "raw",
                    "content_type": "ignored",
                    "downloadUrl": format!("{base}/negative"),
                    "download_url": "https://ignored.test/asset",
                },
                {
                    "name": "oversized.zip",
                    "size": "184467440737095516160",
                    "contentType": "raw",
                    "downloadUrl": format!("{base}/oversized"),
                },
            ]);
            return single_graphql(value);
        }
        assert!(matches!(request.path.as_str(), "/negative" | "/source.zip"));
        Response::zip(archive.clone())
    });
    assert_success(&snapshot(&sandbox, &server));
    let requests = server.requests();
    assert_eq!(requests.len(), 5);
    assert_eq!(requests[3].path, "/negative");
    assert_eq!(requests[4].path, "/source.zip");
    let cached: Value =
        serde_json::from_slice(&fs::read(metadata_cache(&sandbox)).unwrap()).unwrap();
    let release = &cached["releases"][0];
    assert_eq!(release["name"], "v1");
    assert_eq!(release["is_prerelease"], true);
    assert_eq!(release["is_draft"], false);
    assert_eq!(release["assets"][0]["size"], -1);
    assert_eq!(release["assets"][0]["content_type"], "raw");
    assert_eq!(release["assets"][1]["size"].to_string(), "184467440737095516160");
    assert_success(&snapshot(&sandbox, &server));
    assert_eq!(server.requests().len(), requests.len());
}

#[test]
fn incomplete_legacy_metadata_is_retained_and_refreshed_into_the_canonical_schema() {
    let sandbox = Sandbox::new();
    let server = Server::start(|request, base| {
        if request.path.starts_with("/search/code?") {
            return search();
        }
        assert_eq!(request.path, "/graphql");
        let mut value = metadata(base);
        value["releases"]["nodes"] = json!([]);
        single_graphql(value)
    });
    assert_success(&snapshot(&sandbox, &server));
    let current = metadata_cache(&sandbox);
    let legacy_key = format!("{}\nfixture-token\nreleases-v2/owner/repo", server.url);
    let legacy_root = sandbox.path().join("cache/github-catalogue");
    fs::create_dir_all(&legacy_root).unwrap();
    let legacy = legacy_root.join(format!("{:x}", Sha256::digest(legacy_key.as_bytes())));
    let old_bytes = br#"{"releases":{"nodes":[]}}"#;
    fs::write(&legacy, old_bytes).unwrap();
    fs::remove_file(&current).unwrap();
    let before = server.requests().len();
    assert_success(&snapshot(&sandbox, &server));
    assert_eq!(server.requests().len(), before + 1);
    assert!(current.is_file());
    assert_eq!(fs::read(legacy).unwrap(), old_bytes);
}
