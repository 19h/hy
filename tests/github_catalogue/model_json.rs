//! Model JSON preservation through publication, filtering and archive cache access.

use super::models::{search, snapshot};
use super::*;

fn raw_response(value: Value, replacement: &str) -> Response {
    Response {
        status: 200,
        content_type: "application/json",
        body: value.to_string().replace("\"__RAW__\"", replacement).into_bytes(),
    }
}

fn archive(sandbox: &Sandbox) -> Vec<u8> {
    let path = sandbox.path().join("fixture.zip");
    archive_manifest(&path, &identity_manifest("1.0", "https://github.com/owner/repo"), &[]);
    fs::read(path).unwrap()
}

fn metadata_path(sandbox: &Sandbox) -> std::path::PathBuf {
    sandbox.path().join("cache/owner/repo/releases.json")
}

#[test]
fn surrogate_model_strings_and_ignored_nonfinite_values_survive_cache_reuse() {
    let sandbox = Sandbox::new();
    let server = Server::start(|request, base| {
        if request.path.starts_with("/search/code?") {
            return search();
        }
        assert_eq!(request.path, "/graphql");
        let mut metadata = acquisition::release_metadata(commit(base, "source"), "__RAW__", vec![]);
        let release = &mut metadata["releases"]["nodes"][0];
        release["name"] = json!("__RAW__");
        release["url"] = json!("__RAW__");
        release["publishedAt"] = json!("2020-01-01");
        metadata["ignored"] = json!("__NONFINITE__");
        let mut response =
            raw_response(json!({"data":{"repo0":metadata}}), r#""\ud800name\udfff""#);
        response.body = String::from_utf8(response.body)
            .unwrap()
            .replace("\"__NONFINITE__\"", "[NaN,Infinity,-Infinity,1e999]")
            .into_bytes();
        response
    });
    assert_success(&snapshot(&sandbox, &server));
    let path = metadata_path(&sandbox);
    let published = fs::read_to_string(&path).unwrap();
    assert!(published.contains(r#""name": "\ud800name\udfff""#));
    assert!(!published.contains("ignored"));
    assert_eq!(server.requests().len(), 3);
    // Unknown cached fields are ignored after Python JSON decoding as well.
    let cached = published.replacen('{', "{\"ignored\":[NaN,Infinity,\"\\ud800\"],", 1);
    fs::write(&path, &cached).unwrap();
    assert_success(&snapshot(&sandbox, &server));
    assert_eq!(server.requests().len(), 3);
    assert_eq!(fs::read_to_string(path).unwrap(), cached);
}

#[test]
fn surrogate_cache_keys_skip_only_their_archive_after_metadata_publication() {
    for invalid_tag in [false, true] {
        let sandbox = Sandbox::new();
        let bytes = archive(&sandbox);
        let server = Server::start(move |request, base| {
            if request.path.starts_with("/search/code?") {
                return search();
            }
            if request.path == "/graphql" {
                let mut source = commit(base, "source");
                if !invalid_tag {
                    source["oid"] = json!("__RAW__");
                }
                let tag = if invalid_tag {
                    "__RAW__"
                } else {
                    "v1"
                };
                let assets = vec![json!({
                    "name":"plugin.zip", "downloadUrl":format!("{base}/asset"), "size":1, "contentType":"raw"
                })];
                let metadata = acquisition::release_metadata(source, tag, assets);
                return raw_response(json!({"data":{"repo0":metadata}}), r#""\ud800""#);
            }
            assert_eq!(
                request.path,
                if invalid_tag {
                    "/source.zip"
                } else {
                    "/asset"
                }
            );
            Response::zip(bytes.clone())
        });
        assert_success(&snapshot(&sandbox, &server));
        assert!(fs::read_to_string(metadata_path(&sandbox)).unwrap().contains("\\ud800"));
        assert_eq!(server.requests().len(), 4);
        assert_success(&snapshot(&sandbox, &server));
        assert_eq!(server.requests().len(), 4);
    }
}

#[cfg(unix)]
#[test]
fn surrogate_asset_filenames_preserve_filesystem_encoding_and_publication_timing() {
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;

    for (name, encodable) in [(r#""\ud800.zip""#, false), (r#""\udcff.zip""#, true)] {
        let sandbox = Sandbox::new();
        let bytes = archive(&sandbox);
        let directory = sandbox.path().join("cache/owner/repo/release-assets/v1");
        fs::create_dir_all(&directory).unwrap();
        let raw_path = directory.join(OsStr::from_bytes(b"\xff.zip"));
        let probe_error = fs::metadata(&raw_path).unwrap_err();
        let expected_error = if !encodable {
            None
        } else if probe_error.kind() != std::io::ErrorKind::NotFound {
            Some((probe_error, 3))
        } else {
            // A filesystem can allow a missing-name stat but reject its creation.
            // Probe only this Rust-owned fixture, removing it if creation succeeds.
            match fs::write(&raw_path, b"probe") {
                Ok(()) => {
                    fs::remove_file(&raw_path).unwrap();
                    None
                }
                Err(error) => Some((error, 4)),
            }
        };
        let server = Server::start(move |request, base| {
            if request.path.starts_with("/search/code?") {
                return search();
            }
            if request.path == "/graphql" {
                let mut source = commit(base, "source");
                source["oid"] = json!("non-ascii-Δ");
                let assets = vec![json!({
                    "name":"__RAW__", "downloadUrl":format!("{base}/asset"), "size":1, "contentType":"raw"
                })];
                let metadata = acquisition::release_metadata(source, "v1", assets);
                return raw_response(json!({"data":{"repo0":metadata}}), name);
            }
            assert_eq!(request.path, "/asset");
            Response::zip(bytes.clone())
        });
        let output = snapshot(&sandbox, &server);
        if let Some((error, requests)) = expected_error {
            assert!(!output.status.success());
            assert_eq!(server.requests().len(), requests);
            assert!(String::from_utf8_lossy(&output.stderr).contains(&error.to_string()));
            assert!(metadata_path(&sandbox).is_file());
            assert!(!raw_path.exists());
            continue;
        }
        assert_success(&output);
        assert_eq!(server.requests().len(), 4);
        assert!(directory.is_dir());
        assert_eq!(raw_path.is_file(), encodable);
        assert_success(&snapshot(&sandbox, &server));
        assert_eq!(
            server.requests().len(),
            if encodable {
                4
            } else {
                5
            }
        );
    }
}

#[test]
fn nonfinite_model_fields_fail_before_any_metadata_in_the_batch_is_published() {
    for token in ["NaN", "Infinity", "-Infinity"] {
        let sandbox = Sandbox::new();
        let server = Server::start(move |request, base| {
            if request.path.starts_with("/search/code?") {
                return Response::json(json!({"items":[
                    {"repository":{"full_name":"owner/a"}},
                    {"repository":{"full_name":"owner/b"}}
                ]}));
            }
            assert_eq!(request.path, "/graphql");
            let good = acquisition::release_metadata(commit(base, "source"), "v1", vec![]);
            let mut bad = good.clone();
            bad["releases"]["nodes"][0]["isDraft"] = json!("__RAW__");
            raw_response(json!({"data":{"repo0":good,"repo1":bad}}), token)
        });
        assert!(!snapshot(&sandbox, &server).status.success());
        assert_eq!(server.requests().len(), 3);
        assert_eq!(cache_files(&sandbox), [sandbox.path().join("cache/candidate_repos.json")]);
    }
}

#[test]
fn graphql_python_error_values_are_rendered_before_data_validation() {
    let sandbox = Sandbox::new();
    let server = Server::start(|request, _| {
        if request.path.starts_with("/search/code?") {
            return search();
        }
        assert_eq!(request.path, "/graphql");
        raw_response(
            json!({"data":null, "errors":[{"type":"FORBIDDEN","message":"__RAW__"}]}),
            r#"[NaN,Infinity,-Infinity,"\ud800"]"#,
        )
    });
    let output = snapshot(&sandbox, &server);
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("'message': [nan, inf, -inf, '\\ud800']"), "{stderr}");
    assert_eq!(server.requests().len(), 3);
    assert!(!metadata_path(&sandbox).exists());
}
