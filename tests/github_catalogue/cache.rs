//! Cache failures must preserve entries and stop before network fallback.

use std::path::PathBuf;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::{Duration, SystemTime};

use super::*;

struct Fixture {
    sandbox: Sandbox,
    server: Server,
    reject_requests: Arc<AtomicBool>,
    candidates: PathBuf,
    releases: PathBuf,
    archive: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let sandbox = Sandbox::new();
        let path = sandbox.path().join("source.zip");
        archive_manifest(&path, &identity_manifest("1.0", "https://github.com/owner/repo"), &[]);
        let archive = fs::read(path).unwrap();
        let reject_requests = Arc::new(AtomicBool::new(false));
        let rejected = Arc::clone(&reject_requests);
        let server = Server::start(move |request, base| {
            if rejected.load(Ordering::SeqCst) {
                return Response {
                    status: 401,
                    ..Response::json(json!({}))
                };
            }
            if request.path.starts_with("/search/code?") {
                Response::json(json!({"items": [
                    {"repository": {"full_name": "owner/repo"}},
                ]}))
            } else if request.path == "/graphql" {
                single_graphql(json!({
                    "defaultBranchRef": {"target": commit(base, "default")},
                    "releases": {"nodes": []},
                    "refs": {"nodes": [{"name": "v1", "target": commit(base, "source")}]},
                }))
            } else {
                assert_eq!(request.path, "/source.zip");
                Response::zip(archive.clone())
            }
        });
        assert_success(&run(&sandbox, &server));
        let mut candidates = None;
        let mut releases = None;
        let mut archive = None;
        for entry in fs::read_dir(sandbox.path().join("cache/github-catalogue")).unwrap() {
            let path = entry.unwrap().path();
            match serde_json::from_slice::<Value>(&fs::read(&path).unwrap()) {
                Ok(Value::Array(_)) => candidates = Some(path),
                Ok(Value::Object(_)) => releases = Some(path),
                _ => archive = Some(path),
            }
        }
        Self {
            sandbox,
            server,
            reject_requests,
            candidates: candidates.unwrap(),
            releases: releases.unwrap(),
            archive: archive.unwrap(),
        }
    }

    fn run(&self) -> std::process::Output {
        run(&self.sandbox, &self.server)
    }
}

fn run(sandbox: &Sandbox, server: &Server) -> std::process::Output {
    sandbox
        .command(&["plugin", "--repo", "github", "repo", "snapshot"])
        .env("GITHUB_TOKEN", "fixture-token")
        .env("GITHUB_API_URL", &server.url)
        .output()
        .unwrap()
}

fn set_modified(path: &std::path::Path, time: SystemTime) {
    fs::File::options().write(true).open(path).unwrap().set_modified(time).unwrap();
}

#[test]
fn corrupt_metadata_fails_without_refetching_or_overwriting_the_entry() {
    for (release, contents) in [(false, "not JSON"), (true, "not JSON"), (true, "{}")] {
        let fixture = Fixture::new();
        let path = if release {
            &fixture.releases
        } else {
            &fixture.candidates
        };
        fs::write(path, contents).unwrap();
        let before = fixture.server.requests().len();
        let output = fixture.run();
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
        assert_eq!(fixture.server.requests().len(), before);
        assert_eq!(fs::read_to_string(path).unwrap(), contents);
    }
}

#[test]
fn unreadable_cache_entries_fail_before_network_fallback() {
    for kind in ["candidates", "releases", "archive"] {
        let fixture = Fixture::new();
        let path = match kind {
            "candidates" => &fixture.candidates,
            "releases" => &fixture.releases,
            _ => &fixture.archive,
        };
        fs::remove_file(path).unwrap();
        fs::create_dir(path).unwrap();
        let before = fixture.server.requests().len();
        assert!(!fixture.run().status.success(), "{kind}");
        assert_eq!(fixture.server.requests().len(), before, "{kind}");
        assert!(path.is_dir());
    }
}

#[test]
fn future_dated_cache_entries_are_reused_without_requests() {
    let fixture = Fixture::new();
    let future = SystemTime::now() + Duration::from_secs(172_800);
    for path in [&fixture.candidates, &fixture.releases, &fixture.archive] {
        set_modified(path, future);
    }
    let before = fixture.server.requests().len();
    assert_success(&fixture.run());
    assert_eq!(fixture.server.requests().len(), before);
}

#[test]
fn expired_metadata_is_removed_even_when_refresh_fails() {
    for release in [false, true] {
        let fixture = Fixture::new();
        let path = if release {
            &fixture.releases
        } else {
            &fixture.candidates
        };
        // Expiry must be applied before attempting to decode the stale bytes.
        fs::write(path, "invalid JSON").unwrap();
        set_modified(path, SystemTime::now() - Duration::from_secs(172_800));
        fixture.reject_requests.store(true, Ordering::SeqCst);
        let before = fixture.server.requests().len();
        assert!(!fixture.run().status.success());
        assert_eq!(fixture.server.requests().len(), before + 1);
        assert!(!path.exists());
        assert!(fixture.archive.is_file());
    }
}
