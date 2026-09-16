//! Python JSON values must survive discovery until their consuming boundary.

use super::discovery::{empty_repository, snapshot};
use super::*;

fn raw_json(body: Vec<u8>) -> Response {
    Response {
        status: 200,
        content_type: "application/json",
        body,
    }
}

#[test]
fn ignored_python_values_do_not_prevent_discovery_or_cached_key_selection() {
    for cached in [false, true] {
        let sandbox = Sandbox::new();
        let path = sandbox.path().join("cache/candidate_repos.json");
        let cache_text = br#"{"Owner/Repo":[NaN,Infinity,-Infinity,"\ud800"]}"#;
        if cached {
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(&path, cache_text).unwrap();
        }
        let server = Server::start(|request, base| {
            if request.path.starts_with("/search/code?") {
                return raw_json(
                    br#"{
                    "items":[{"repository":{"full_name":"Owner/Repo"}}],
                    "ignored":[NaN,Infinity,-Infinity,"\ud800",1e999]
                }"#
                    .to_vec(),
                );
            }
            assert_eq!(request.path, "/graphql");
            graphql_response(request, |owner, repo| {
                assert_eq!((owner, repo), ("owner", "repo"));
                empty_repository(base)
            })
        });
        assert_success(&snapshot(&sandbox, &server, &[]));
        let expected_requests = if cached {
            1
        } else {
            3
        };
        assert_eq!(server.requests().len(), expected_requests);
        assert_success(&snapshot(&sandbox, &server, &[]));
        assert_eq!(server.requests().len(), expected_requests);
        if cached {
            assert_eq!(fs::read(path).unwrap(), cache_text);
        } else {
            assert_eq!(fs::read_to_string(path).unwrap(), cache_lines("[\n  \"owner/repo\"\n]"));
        }
    }
}

#[test]
fn nonfinite_repository_names_finish_both_queries_before_failing_without_publication() {
    for number in ["NaN", "Infinity", "-Infinity", "1e999"] {
        let sandbox = Sandbox::new();
        let item = format!(r#"{{"repository":{{"full_name":{number}}}}}"#);
        let page = format!(r#"{{"items":[{}]}}"#, vec![item; 100].join(","));
        let server = Server::start(move |request, _| {
            assert!(request.path.starts_with("/search/code?"));
            raw_json(if request.path.ends_with("page=1") {
                page.as_bytes().to_vec()
            } else {
                b"{}".to_vec()
            })
        });
        let output = snapshot(&sandbox, &server, &[]);
        assert!(!output.status.success(), "{number}");
        assert_eq!(server.requests().len(), 4, "{number}");
        assert!(!sandbox.path().join("cache/candidate_repos.json").exists());
    }
}

#[test]
fn surrogate_names_are_published_before_parse_or_component_validation() {
    for (name, expected_cache, warm_first) in [
        (r#"Z/\ud800"#, "[\n  \"a/valid\",\n  \"z/\\ud800\"\n]", true),
        (r#"Z\udfff"#, "[\n  \"a/valid\",\n  \"z\\udfff\"\n]", false),
    ] {
        let sandbox = Sandbox::new();
        let body = format!(
            r#"{{"items":[
            {{"repository":{{"full_name":"A/Valid"}}}},
            {{"repository":{{"full_name":"{name}"}}}}
        ]}}"#
        );
        let server = Server::start(move |request, _| {
            assert!(request.path.starts_with("/search/code?"));
            raw_json(body.as_bytes().to_vec())
        });
        for _ in 0..2 {
            let output = snapshot(&sandbox, &server, &[]);
            assert!(!output.status.success());
            let stderr = String::from_utf8_lossy(&output.stderr);
            assert!(
                stderr.contains(if warm_first {
                    "Must contain only ASCII characters"
                } else {
                    "Expected format: owner/repo"
                }),
                "{stderr}"
            );
            assert_eq!(server.requests().len(), 2);
            assert_eq!(sandbox.path().join("cache/a/valid").is_dir(), warm_first);
            assert!(!sandbox.path().join("cache/z").exists());
            assert_eq!(
                fs::read_to_string(sandbox.path().join("cache/candidate_repos.json")).unwrap(),
                cache_lines(expected_cache)
            );
        }
    }
}

#[test]
fn discovery_requires_utf8_text_without_bom_for_responses_and_cache_files() {
    for body in [
        b"\xef\xbb\xbf{}".to_vec(),
        b"{\0}\0".to_vec(),
        b"\0{\0}".to_vec(),
        b"{\0\0\0}\0\0\0".to_vec(),
        b"\0\0\0{\0\0\0}".to_vec(),
        b"{\"ignored\":\"\xed\xa0\x80\",\"items\":[]}".to_vec(),
    ] {
        for cached in [false, true] {
            let sandbox = Sandbox::new();
            let path = sandbox.path().join("cache/candidate_repos.json");
            if cached {
                fs::create_dir_all(path.parent().unwrap()).unwrap();
                fs::write(&path, &body).unwrap();
            }
            let response_body = body.clone();
            let server = Server::start(move |request, _| {
                assert!(request.path.starts_with("/search/code?"));
                raw_json(response_body.clone())
            });
            assert!(!snapshot(&sandbox, &server, &[]).status.success());
            assert_eq!(server.requests().len(), usize::from(!cached));
            if cached {
                assert_eq!(fs::read(path).unwrap(), body);
            } else {
                assert!(!path.exists());
            }
        }
    }
}

fn cache_lines(text: &str) -> String {
    if cfg!(windows) {
        text.replace('\n', "\r\n")
    } else {
        text.to_owned()
    }
}
