//! Batch boundaries, partial GraphQL results and cache publication.

use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use super::*;

fn empty_repository(base: &str) -> Value {
    json!({
        "defaultBranchRef": {"target": commit(base, "default")},
        "releases": {"nodes": []},
        "refs": {"nodes": []},
    })
}

fn run(sandbox: &Sandbox, server: &Server) -> std::process::Output {
    sandbox
        .command(&["plugin", "--repo", "github", "repo", "snapshot"])
        .env("GITHUB_TOKEN", "fixture-token")
        .env("GITHUB_API_URL", &server.url)
        .output()
        .unwrap()
}

fn search(count: usize) -> Response {
    let items: Vec<_> = (0..count)
        .map(|index| json!({"repository": {"full_name": format!("owner/repo{index:02}")}}))
        .collect();
    Response::json(json!({"items": items}))
}

#[test]
fn only_uncached_metadata_is_warmed_in_batches_of_ten() {
    for count in [0, 1, 9, 10, 11, 20, 21] {
        let sandbox = Sandbox::new();
        let server = Server::start(move |request, base| {
            if request.path.starts_with("/search/code?") {
                return search(count);
            }
            assert_eq!(request.path, "/graphql");
            graphql_response(request, |_, _| empty_repository(base))
        });
        assert_success(&run(&sandbox, &server));
        let initial = server.requests();
        let batches: Vec<_> = initial
            .iter()
            .filter(|request| request.path == "/graphql")
            .map(|request| graphql_repositories(request).len())
            .collect();
        let expected: Vec<_> =
            (0..count).step_by(10).map(|start| (count - start).min(10)).collect();
        assert_eq!(batches, expected, "count {count}");
        assert_success(&run(&sandbox, &server));
        assert_eq!(server.requests().len(), initial.len());
        if count > 0 {
            let entry = cache_files(&sandbox)
                .into_iter()
                .find(|path| {
                    serde_json::from_slice::<Value>(&fs::read(path).unwrap())
                        .ok()
                        .is_some_and(|value| value.get("releases").is_some())
                })
                .unwrap();
            fs::remove_file(entry).unwrap();
            assert_success(&run(&sandbox, &server));
            let requests = server.requests();
            assert_eq!(requests.len(), initial.len() + 1);
            assert_eq!(graphql_repositories(requests.last().unwrap()).len(), 1);
        }
    }
}

#[test]
fn partial_not_found_results_cache_survivors_and_retry_missing_repositories_individually() {
    let sandbox = Sandbox::new();
    let server = Server::start(|request, base| {
        if request.path.starts_with("/search/code?") {
            return Response::json(json!({"items":[
                {"repository":{"full_name":"a/r"}},
                {"repository":{"full_name":"a-b/r"}},
                {"repository":{"full_name":"z/r"}},
            ]}));
        }
        let data: serde_json::Map<_, _> = graphql_repositories(request)
            .into_iter()
            .map(|entry| {
                let value = if entry.owner != "z" {
                    Value::Null
                } else {
                    empty_repository(base)
                };
                (entry.alias, value)
            })
            .collect();
        Response::json(
            json!({"data":data, "errors":[{"type":"NOT_FOUND", "message":"fixture missing"}]}),
        )
    });
    assert_success(&run(&sandbox, &server));
    assert_success(&run(&sandbox, &server));
    let batches: Vec<Vec<String>> = server
        .requests()
        .iter()
        .filter(|request| request.path == "/graphql")
        .map(|request| {
            graphql_repositories(request)
                .into_iter()
                .map(|entry| format!("{}/{}", entry.owner, entry.name))
                .collect()
        })
        .collect();
    assert_eq!(
        batches,
        [
            vec!["a-b/r", "a/r", "z/r"],
            vec!["a/r"],
            vec!["a-b/r"],
            vec!["a-b/r", "a/r"],
            vec!["a/r"],
            vec!["a-b/r"]
        ]
    );
}

#[test]
fn failed_batches_publish_no_entries_but_keep_prior_completed_batches() {
    for malformed_model in [false, true] {
        let sandbox = Sandbox::new();
        let phase = Arc::new(AtomicUsize::new(0));
        let observed = Arc::clone(&phase);
        let server = Server::start(move |request, base| {
            if request.path.starts_with("/search/code?") {
                return search(12);
            }
            let entries = graphql_repositories(request);
            if entries.iter().any(|entry| entry.name == "repo11")
                && observed.load(Ordering::SeqCst) == 0
            {
                let mut data = serde_json::Map::new();
                for entry in entries {
                    let mut repository = empty_repository(base);
                    if entry.name == "repo11" && malformed_model {
                        repository.as_object_mut().unwrap().remove("refs");
                    }
                    data.insert(entry.alias, repository);
                }
                return Response::json(if malformed_model {
                    json!({"data":data})
                } else {
                    json!({"data":data,"errors":[{"type":"FORBIDDEN", "message":"fixture denied"}]})
                });
            }
            graphql_response(request, |_, _| empty_repository(base))
        });
        assert!(!run(&sandbox, &server).status.success());
        let before = server.requests().len();
        phase.store(1, Ordering::SeqCst);
        assert_success(&run(&sandbox, &server));
        let requests = server.requests();
        assert_eq!(requests.len(), before + 1);
        let recovered: Vec<_> = graphql_repositories(requests.last().unwrap())
            .into_iter()
            .map(|entry| entry.name)
            .collect();
        assert_eq!(
            recovered,
            ["repo10", "repo11"],
            "an earlier member of the failed batch was cached"
        );
    }
}
