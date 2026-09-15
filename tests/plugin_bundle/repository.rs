//! Repository bundle reads are hash-verified before descriptor-based naming.

use std::fs;
use std::io::{Cursor, Read, Write};
use std::path::Path;
use std::process::{Command, Stdio};

use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use zip::write::SimpleFileOptions;

use crate::support::http::{Response, Server};
use crate::support::{Sandbox, assert_success, fake_python, identity_manifest};

const PLATFORMS: [&str; 3] = ["linux-x86_64", "macos-aarch64", "windows-x86_64"];

struct Distribution {
    bytes: Vec<u8>,
    checksum: String,
    metadata: Value,
}

impl Distribution {
    fn new(descriptors: &[Value], platforms: &[&'static str]) -> Self {
        let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
        for (index, descriptor) in descriptors.iter().enumerate() {
            writer
                .start_file(format!("{index}/ida-plugin.json"), SimpleFileOptions::default())
                .unwrap();
            writer.write_all(&serde_json::to_vec(descriptor).unwrap()).unwrap();
        }
        let bytes = writer.finish().unwrap().into_inner();
        let mut metadata = identity_manifest("1", "https://github.com/example/original");
        metadata["plugin"]["platforms"] = json!(platforms);
        Self {
            checksum: hash(&bytes),
            bytes,
            metadata,
        }
    }
}

#[test]
fn naming_uses_the_first_exact_name_without_installation_identity_checks() {
    let original = identity_manifest("1", "https://github.com/example/original");
    for change in ["host", "version", "entry", "platforms", "duplicate", "other", "case", "empty"] {
        let mut descriptor = original.clone();
        let mut descriptors = Vec::new();
        match change {
            "host" => {
                descriptor["plugin"]["urls"]["repository"] =
                    json!("https://github.com/example/foreign")
            }
            "version" => descriptor["plugin"]["version"] = json!("2"),
            "entry" => descriptor["plugin"]["entryPoint"] = json!("missing.py"),
            "platforms" => descriptor["plugin"]["platforms"] = json!([]),
            "other" => {
                let mut other = original.clone();
                other["plugin"]["name"] = json!("other");
                descriptors.push(other);
            }
            "case" => descriptor["plugin"]["name"] = json!("Example"),
            _ => {}
        }
        if change != "empty" {
            descriptors.push(descriptor);
        }
        if change == "duplicate" {
            let mut later = original.clone();
            later["plugin"]["version"] = json!("2");
            descriptors.push(later);
        }
        let distribution = Distribution::new(&descriptors, &PLATFORMS);
        let expected = if matches!(change, "case" | "empty") {
            json!({"fetches": [0, 0, 0], "error": "plugin 'example' not found in archive"})
        } else {
            let version = if change == "version" {
                "2"
            } else {
                "1"
            };
            success(&[0, 0, 0], vec![file(&distribution, version, "")], &[])
        };
        check(vec![distribution], expected);
    }
}

#[test]
fn checksums_are_case_sensitive_and_each_platform_is_fetched() {
    let descriptor = identity_manifest("1", "https://github.com/example/original");
    for change in ["valid", "uppercase", "empty", "wrong"] {
        let mut distribution = Distribution::new(std::slice::from_ref(&descriptor), &PLATFORMS);
        match change {
            "uppercase" => distribution.checksum = distribution.checksum.to_uppercase(),
            "empty" => distribution.checksum.clear(),
            "wrong" => distribution.checksum = "0".repeat(64),
            _ => {}
        }
        let expected = if change == "valid" {
            success(&[0, 0, 0], vec![file(&distribution, "1", "")], &[])
        } else {
            json!({"fetches": [0], "error": "hash mismatch"})
        };
        check(vec![distribution], expected);
    }
}

#[test]
fn dependency_order_follows_platform_resolution_not_hash_sorting() {
    let mut descriptor = identity_manifest("1", "https://github.com/example/original");
    descriptor["plugin"]["pythonDependencies"] = json!(["order-first==1"]);
    let mut first = Distribution::new(&[descriptor.clone()], &PLATFORMS[..2]);
    descriptor["plugin"]["pythonDependencies"] = json!(["order-second==2"]);
    let mut second = Distribution::new(&[descriptor], &PLATFORMS[2..]);
    let mut dependencies = ["order-first==1", "order-second==2"];
    // Make hash sorting observably different from target-platform order.
    if first.checksum < second.checksum {
        std::mem::swap(&mut first.bytes, &mut second.bytes);
        std::mem::swap(&mut first.checksum, &mut second.checksum);
        dependencies.swap(0, 1);
    }
    let expected = success(
        &[0, 0, 1],
        vec![
            file(&first, "1", "-linux-x86_64+macos-aarch64"),
            file(&second, "1", "-windows-x86_64"),
        ],
        &dependencies,
    );
    check(vec![first, second], expected);
}

#[test]
fn later_fetch_failure_precedes_earlier_descriptor_failure() {
    let first = Distribution::new(&[], &PLATFORMS[..2]);
    let mut second = Distribution::new(&[], &PLATFORMS[2..]);
    second.checksum = "0".repeat(64);
    check(vec![first, second], json!({"fetches": [0, 0, 1], "error": "hash mismatch"}));
}

#[test]
fn selection_uses_index_identity_and_location_compatibility_independently() {
    for field in ["name", "version", "host"] {
        for qualified in [false, true] {
            let mut metadata = identity_manifest("1", "https://github.com/example/original");
            match field {
                "name" => metadata["plugin"]["name"] = json!("other"),
                "version" => metadata["plugin"]["version"] = json!("2"),
                "host" => {
                    metadata["plugin"]["urls"]["repository"] =
                        json!("https://github.com/example/foreign")
                }
                _ => unreachable!(),
            }
            let mut distribution = Distribution::new(std::slice::from_ref(&metadata), &PLATFORMS);
            distribution.metadata = metadata;
            distribution.metadata["plugin"]["platforms"] = json!(PLATFORMS);
            let name = if field == "name" {
                "other"
            } else {
                "example"
            };
            let version = if field == "version" {
                "2"
            } else {
                "1"
            };
            let expected = success(
                &[0, 0, 0],
                vec![json!({
                    "file": format!("plugins/{name}-{version}.zip"),
                    "sha256": hash(&distribution.bytes),
                })],
                &[],
            );
            let spec = if qualified {
                "example==1@https://github.com/example/original"
            } else {
                "example==1"
            };
            check_spec(vec![distribution], spec, expected);
        }
    }
}

#[test]
fn bundle_preprocessing_drops_scopes_and_preserves_compound_specs() {
    let descriptor = identity_manifest("1", "https://github.com/example/original");
    for spec in [
        "scope/example==1",
        "scope/example==1@https://GitHub.com/Example/Original/",
        "example>=1,==1",
        "example==1,<=2",
        "example!=2,==1",
        "scope/example==1\n",
    ] {
        let distribution = Distribution::new(std::slice::from_ref(&descriptor), &PLATFORMS);
        let expected = success(&[0, 0, 0], vec![file(&distribution, "1", "")], &[]);
        check_spec(vec![distribution], spec, expected);
    }
}

#[test]
fn missing_equality_errors_use_the_preprocessed_reference() {
    let descriptor = identity_manifest("1", "https://github.com/example/original");
    for (spec, example) in [
        ("example", "example==1.0.0"),
        ("scope/example", "example==1.0.0"),
        ("example@invalid", "example@invalid==1.0.0"),
        (
            "scope/example>=1@https://GitHub.com/Example/Original/",
            "example>=1==1.0.0@https://github.com/example/original",
        ),
    ] {
        let distribution = Distribution::new(std::slice::from_ref(&descriptor), &PLATFORMS);
        check_spec(
            vec![distribution],
            spec,
            json!({
                "fetches": [],
                "error": format!("repository plugin specs must include exact version (e.g. {example})"),
            }),
        );
    }
}

fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn file(distribution: &Distribution, version: &str, suffix: &str) -> Value {
    json!({
        "file": format!("plugins/example-{version}{suffix}.zip"),
        "sha256": hash(&distribution.bytes),
    })
}

fn success(fetches: &[usize], mut files: Vec<Value>, dependencies: &[&str]) -> Value {
    files.sort_by(|left, right| left["file"].as_str().cmp(&right["file"].as_str()));
    json!({"fetches": fetches, "files": files, "dependencies": dependencies})
}

fn check(distributions: Vec<Distribution>, expected: Value) {
    check_spec(distributions, "example==1", expected);
}

fn check_spec(distributions: Vec<Distribution>, spec: &str, expected: Value) {
    let sandbox = Sandbox::new();
    let locations = write_locations(&sandbox, &distributions);
    compare_source(&locations, spec, &expected);
    let server = serve(distributions, locations);
    let output = sandbox.path().join("bundle.zip");
    let python = sandbox.path().join("python");
    let calls = sandbox.path().join("pip-calls");
    fake_python(&python);
    let mut command = sandbox.command(&[
        "plugin",
        "--repo",
        &format!("{}/repository.json", server.url),
        "bundle",
        "create",
        "--path",
        output.to_str().unwrap(),
        "--python",
        "3.12",
        spec,
    ]);
    for platform in ["windows", "linux", "macos-arm64"] {
        command.args(["--platform", platform]);
    }
    let result = command
        .env("HCLI_CURRENT_IDA_PYTHON_EXE", python)
        .env("HY_TEST_PIP_ARGUMENTS", &calls)
        .output()
        .unwrap();
    let fetches: Vec<_> = server
        .requests()
        .iter()
        .filter_map(|request| {
            request.path.strip_prefix('/')?.strip_suffix(".zip")?.parse::<usize>().ok()
        })
        .collect();
    assert_eq!(json!(fetches), expected["fetches"]);
    if let Some(error) = expected.get("error").and_then(Value::as_str) {
        assert!(!result.status.success(), "{result:?}");
        assert!(String::from_utf8_lossy(&result.stderr).contains(error), "{result:?}");
        assert!(!output.exists());
        assert!(!calls.exists());
        return;
    }
    assert_success(&result);
    assert_bundle(&output, &expected["files"]);
    assert_downloads(&calls, &expected["dependencies"]);
}

fn write_locations(sandbox: &Sandbox, distributions: &[Distribution]) -> Vec<Value> {
    let mut locations = Vec::new();
    for (index, distribution) in distributions.iter().enumerate() {
        let path = sandbox.path().join(format!("{index}.zip"));
        fs::write(&path, &distribution.bytes).unwrap();
        locations.push(json!({
            "path": path,
            "sha256": distribution.checksum,
            "metadata": distribution.metadata,
        }));
    }
    locations
}

fn serve(distributions: Vec<Distribution>, server_locations: Vec<Value>) -> Server {
    Server::start(move |request, base| {
        if request.path == "/repository.json" {
            let locations: Vec<_> = server_locations
                .iter()
                .enumerate()
                .map(|(index, location)| {
                    json!({
                        "url": format!("{base}/{index}.zip"),
                        "sha256": location["sha256"],
                        "metadata": location["metadata"],
                    })
                })
                .collect();
            return Response::json(json!({"version": 1, "plugins": [{
                "name": "example", "host": "https://github.com/example/original",
                "versions": {"1": locations},
            }]}));
        }
        let index = request
            .path
            .strip_prefix('/')
            .and_then(|path| path.strip_suffix(".zip"))
            .and_then(|index| index.parse::<usize>().ok());
        match index.and_then(|index| distributions.get(index)) {
            Some(distribution) => Response::zip(distribution.bytes.clone()),
            None => Response::missing(),
        }
    })
}

fn assert_bundle(output: &Path, expected: &Value) {
    let mut bundle = zip::ZipArchive::new(fs::File::open(output).unwrap()).unwrap();
    let mut files = Vec::new();
    for index in 0..bundle.len() {
        let mut entry = bundle.by_index(index).unwrap();
        if !entry.name().starts_with("plugins/") {
            continue;
        }
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes).unwrap();
        files.push(json!({"file": entry.name(), "sha256": hash(&bytes)}));
    }
    assert_eq!(&json!(files), expected);
}

fn assert_downloads(calls: &Path, expected: &Value) {
    let calls = fs::read_to_string(calls).unwrap_or_default();
    let invocations: Vec<_> = calls.split("--invocation--\n").skip(1).collect();
    if expected.as_array().unwrap().is_empty() {
        assert!(invocations.is_empty());
    } else {
        assert_eq!(invocations.len(), 3);
        for invocation in invocations {
            let dependencies: Vec<_> =
                invocation.lines().filter(|line| line.starts_with("order-")).collect();
            assert_eq!(&json!(dependencies), expected);
        }
    }
}

fn compare_source(locations: &[Value], spec: &str, expected: &Value) {
    let Some(python) = std::env::var_os("HY_TEST_BUNDLE_ORACLE_PYTHON") else {
        return;
    };
    let source =
        std::env::var_os("HY_TEST_HCLI_SOURCE").map(std::path::PathBuf::from).unwrap_or_else(
            || Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().join("ida-hcli"),
        );
    let mut child = Command::new(python)
        .args(["-I", "-B", "-c", include_str!("repository.py")])
        .arg(source)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(
            &serde_json::to_vec(&json!({
                "locations": locations, "platforms": PLATFORMS, "spec": spec,
            }))
            .unwrap(),
        )
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    let actual: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(&actual, expected);
}
