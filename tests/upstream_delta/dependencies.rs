use super::*;
use std::os::unix::fs::PermissionsExt;

fn executable(path: &Path, body: &str) {
    fs::write(path, body).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
}

#[test]
fn full_python_version_is_probed_once_and_failed_probes_keep_unrestricted_versions() {
    for available in [false, true] {
        let sandbox = Sandbox::new();
        let repository = sandbox.path().join("repository");
        fs::create_dir(&repository).unwrap();
        for (version, requirement) in [("2", Some(">=3.12.3")), ("1", Some(">=3.12")), ("0", None)]
        {
            let mut root = descriptor("root", version);
            root["plugin"]["requiresPython"] = json!(requirement);
            if version == "1" {
                root["plugin"]["dependencies"] = json!(["child"]);
            }
            archive(&repository, &format!("root-{version}.zip"), &root);
        }
        let mut child = descriptor("child", "1");
        child["plugin"]["requiresPython"] = json!(">=3.12");
        archive(&repository, "child.zip", &child);
        let python = sandbox.path().join("python");
        executable(
            &python,
            r#"#!/bin/sh
case "$2" in
    *'sys.version_info[:3]'*) ;;
    *) exit 1 ;;
esac
printf 'full\n' >> "$CALLS"
if [ "$AVAILABLE" = true ]; then printf '3.12.2\n'; else exit 1; fi
"#,
        );
        let calls = sandbox.path().join("calls");
        let output = sandbox
            .command(&["plugin", "--repo", repository.to_str().unwrap(), "install", "root"])
            .env("HCLI_CURRENT_IDA_PYTHON_EXE", &python)
            .env("CALLS", &calls)
            .env("AVAILABLE", available.to_string())
            .output()
            .unwrap();
        assert_success(&output);
        assert_eq!(fs::read_to_string(calls).unwrap(), "full\n");
        let metadata: Value = serde_json::from_slice(
            &fs::read(sandbox.path().join("idausr/plugins/root/ida-plugin.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(
            metadata["plugin"]["version"],
            if available {
                "1"
            } else {
                "0"
            }
        );
        assert_eq!(sandbox.path().join("idausr/plugins/child").exists(), available);
    }
}

#[test]
fn unchanged_local_upgrades_do_not_resolve_dependencies_or_probe_python() {
    let sandbox = Sandbox::new();
    let package = sandbox.path().join("root.zip");
    archive_manifest(&package, &descriptor("root", "2"), &[]);
    assert_success(&sandbox.run(&["plugin", "install", package.to_str().unwrap()]));
    let mut candidate = descriptor("root", "1");
    candidate["plugin"]["dependencies"] = json!(["absent"]);
    candidate["plugin"]["requiresPython"] = json!(">=99");
    archive_manifest(&package, &candidate, &[]);
    let python = sandbox.path().join("python");
    executable(&python, "#!/bin/sh\nprintf unexpected > \"$CALLS\"\nexit 1\n");
    let calls = sandbox.path().join("calls");
    let output = sandbox
        .command(&["plugin", "install", "--upgrade", package.to_str().unwrap()])
        .env("HCLI_CURRENT_IDA_PYTHON_EXE", python)
        .env("CALLS", &calls)
        .output()
        .unwrap();
    assert_success(&output);
    assert!(!calls.exists());
}

#[test]
fn scoped_roots_resolve_dependencies_from_other_repositories() {
    let sandbox = Sandbox::new();
    for name in ["community", "hexrays"] {
        assert_success(&sandbox.run(&["plugin", "repo", "remove", name]));
    }
    for name in ["roots", "deps"] {
        let path = sandbox.path().join(name);
        fs::create_dir(&path).unwrap();
        let url = url::Url::from_file_path(&path).unwrap();
        assert_success(&sandbox.run(&["plugin", "repo", "add", name, url.as_str()]));
    }
    let mut root = descriptor("root", "1");
    root["plugin"]["dependencies"] = json!(["child"]);
    archive(&sandbox.path().join("roots"), "root.zip", &root);
    archive(&sandbox.path().join("deps"), "child.zip", &descriptor("child", "1"));
    assert_success(&sandbox.run(&["extension", "install", "roots/root"]));
    assert!(sandbox.path().join("idausr/plugins/child").is_dir());
    assert_success(&sandbox.run(&["extension", "uninstall", "roots/root", "deps/child"]));
    assert!(!sandbox.path().join("idausr/plugins/root").exists());
    assert!(!sandbox.path().join("idausr/plugins/child").exists());
}

#[test]
fn failed_optional_required_dependencies_skip_their_parent() {
    use sha2::{Digest, Sha256};
    let sandbox = Sandbox::new();
    let mut root = descriptor("root", "1");
    root["plugin"]["dependencies"] = json!([{"plugin": "optional", "required": false}]);
    let mut optional = descriptor("optional", "1");
    optional["plugin"]["dependencies"] = json!(["broken"]);
    let mut broken = descriptor("broken", "1");
    broken["plugin"]["entryPoint"] = json!("absent.py");
    let mut plugins = Vec::new();
    for metadata in [&root, &optional, &broken] {
        let name = metadata["plugin"]["name"].as_str().unwrap();
        let path = sandbox.path().join(format!("{name}.zip"));
        archive_manifest(&path, metadata, &[]);
        plugins.push(json!({
            "name": name,
            "host": metadata["plugin"]["urls"]["repository"],
            "versions": {"1": [{
                "url": url::Url::from_file_path(&path).unwrap().as_str(),
                "sha256": format!("{:x}", Sha256::digest(fs::read(path).unwrap())),
                "metadata": metadata,
            }]},
        }));
    }
    let repository = sandbox.path().join("repository.json");
    fs::write(&repository, serde_json::to_vec(&json!({"version": 1, "plugins": plugins})).unwrap())
        .unwrap();
    let output =
        sandbox.run(&["plugin", "--repo", repository.to_str().unwrap(), "install", "root"]);
    assert_success(&output);
    assert!(sandbox.path().join("idausr/plugins/root").is_dir());
    for name in ["optional", "broken"] {
        assert!(!sandbox.path().join("idausr/plugins").join(name).exists());
    }
    assert!(String::from_utf8_lossy(&output.stderr).contains("a required dependency failed"));
}

#[test]
fn installed_python_extras_are_constraints_without_install_requests() {
    let sandbox = Sandbox::new();
    let python = sandbox.path().join("python");
    support::fake_python(&python);
    let calls = sandbox.path().join("calls");
    let mut neighbor = descriptor("neighbor", "1");
    neighbor["plugin"]["pythonDependencies"] = json!(["retained[extra]==1"]);
    let package = sandbox.path().join("plugin.zip");
    archive_manifest(&package, &neighbor, &[]);
    let install = |package: &Path| {
        sandbox.run_with_env(
            &["plugin", "--no-python-environment-check", "install", package.to_str().unwrap()],
            &[("HCLI_CURRENT_IDA_PYTHON_EXE", &python), ("HY_TEST_PIP_ARGUMENTS", &calls)],
        )
    };
    assert_success(&install(&package));
    fs::remove_file(&calls).unwrap();
    let mut root = descriptor("root", "1");
    root["plugin"]["pythonDependencies"] = json!(["requested==2"]);
    archive_manifest(&package, &root, &[]);
    let output = install(&package);
    assert_success(&output);
    assert!(String::from_utf8_lossy(&output.stderr).contains("removing extras"));
    let calls = fs::read_to_string(calls).unwrap();
    assert_eq!(calls.matches("--constraint-contents--\nretained==1\n").count(), 2);
    assert_eq!(calls.matches("requested==2\n").count(), 2);
    assert!(!calls.contains("retained[extra]"));
}

#[test]
fn downloaded_root_dependencies_are_replanned_before_publication() {
    use sha2::{Digest, Sha256};
    use support::http::{Response, Server};
    for available in [false, true] {
        let sandbox = Sandbox::new();
        let catalogue = descriptor("root", "1");
        let mut actual = descriptor("root", "2");
        actual["plugin"]["dependencies"] = json!(["child"]);
        let child = descriptor("child", "1");
        let mut payloads = Vec::new();
        for metadata in [&actual, &child] {
            let path = sandbox.path().join("archive.zip");
            archive_manifest(&path, metadata, &[]);
            payloads.push(fs::read(path).unwrap());
        }
        let responses = payloads.clone();
        let server = Server::start(move |request, _| match request.path.as_str() {
            "/root.zip" => Response::zip(responses[0].clone()),
            "/child.zip" => Response::zip(responses[1].clone()),
            _ => Response::missing(),
        });
        let mut plugins = Vec::new();
        for (index, metadata) in [&catalogue, &child].into_iter().enumerate() {
            if index == 1 && !available {
                continue;
            }
            let name = metadata["plugin"]["name"].as_str().unwrap();
            plugins.push(json!({
                "name": name,
                "host": metadata["plugin"]["urls"]["repository"],
                "versions": {"1": [{
                    "url": format!("{}/{name}.zip", server.url),
                    "sha256": format!("{:x}", Sha256::digest(&payloads[index])),
                    "metadata": metadata,
                }]},
            }));
        }
        let repository = sandbox.path().join("repository.json");
        fs::write(&repository, serde_json::to_vec(&json!({"plugins": plugins})).unwrap()).unwrap();
        let output =
            sandbox.run(&["plugin", "--repo", repository.to_str().unwrap(), "install", "root"]);
        assert_eq!(output.status.success(), available, "{output:?}");
        for name in ["root", "child"] {
            assert_eq!(sandbox.path().join("idausr/plugins").join(name).exists(), available);
        }
        assert_eq!(
            server.requests().iter().filter(|request| request.path == "/root.zip").count(),
            1
        );
    }
}
