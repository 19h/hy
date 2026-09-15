//! Native installer lifecycle, exercised with executable fixture installers.
#![cfg(unix)]

mod support;

use std::fs;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

use support::*;

fn installer(sandbox: &Sandbox) -> PathBuf {
    let script = include_str!("fixtures/installer.sh");
    if cfg!(target_os = "macos") {
        let path = sandbox.path().join("ida-pro_94_armmac.app.zip");
        let mut archive = zip::ZipWriter::new(fs::File::create(&path).unwrap());
        archive
            .start_file(
                "installer.app/Contents/MacOS/osx-arm64",
                zip::write::SimpleFileOptions::default().unix_permissions(0o755),
            )
            .unwrap();
        archive.write_all(script.as_bytes()).unwrap();
        archive.finish().unwrap();
        path
    } else {
        let path = sandbox.path().join("ida-pro_94_x64linux.run");
        fs::write(&path, script).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o640)).unwrap();
        path
    }
}

#[test]
fn dry_run_does_not_execute_or_create_the_destination() {
    let sandbox = Sandbox::new();
    let installer = installer(&sandbox);
    let destination = sandbox.path().join("IDA Fixture.app");
    let arguments = sandbox.path().join("arguments");
    let output = sandbox.run_with_env(
        &[
            "ida",
            "install",
            installer.to_str().unwrap(),
            "--install-dir",
            destination.to_str().unwrap(),
            "--dry-run",
            "--create-python-environment",
        ],
        &[("HY_TEST_INSTALLER_ARGUMENTS", &arguments)],
    );
    assert_success(&output);
    assert!(String::from_utf8_lossy(&output.stdout).contains("Dry run"));
    assert!(!arguments.exists());
    assert!(!destination.exists());
    assert!(!sandbox.path().join("idausr/ida-config.json").exists());
    assert!(!sandbox.path().join("idausr/venv").exists());
}

#[test]
fn installer_application_is_executed_and_its_product_is_published() {
    let sandbox = Sandbox::new();
    let installer = installer(&sandbox);
    let destination = sandbox.path().join("IDA Fixture.app");
    let arguments = sandbox.path().join("arguments");
    assert_success(&sandbox.run_with_env(
        &[
            "ida",
            "install",
            installer.to_str().unwrap(),
            "-i",
            destination.to_str().unwrap(),
            "-y",
            "-A",
            "--no-set-default",
        ],
        &[("HY_TEST_INSTALLER_ARGUMENTS", &arguments)],
    ));
    let executable_directory = if cfg!(target_os = "macos") {
        destination.join("Contents/MacOS")
    } else {
        destination.clone()
    };
    assert!(executable_directory.join("ida.hlp").is_file());
    assert!(executable_directory.join("ida").is_file());
    assert!(!destination.join("installer.app").exists());
    assert!(!sandbox.path().join("idausr/ida-config.json").exists());
    let arguments = fs::read_to_string(arguments).unwrap();
    assert!(arguments.starts_with("--mode\nunattended\n--debugtrace\n"));
    assert!(!arguments.contains("--installpassword"));
    let listing = sandbox.run(&["ida", "list"]);
    assert_success(&listing);
    assert!(!String::from_utf8_lossy(&listing.stdout).contains(destination.to_str().unwrap()));
}

#[test]
fn existing_destination_is_preserved_without_running_the_installer() {
    let sandbox = Sandbox::new();
    let installer = installer(&sandbox);
    let destination = sandbox.path().join("existing");
    fs::create_dir(&destination).unwrap();
    fs::write(destination.join("sentinel"), "retain").unwrap();
    let arguments = sandbox.path().join("arguments");
    assert_success(&sandbox.run_with_env(
        &["ida", "install", installer.to_str().unwrap(), "-i", destination.to_str().unwrap(), "-y"],
        &[("HY_TEST_INSTALLER_ARGUMENTS", &arguments)],
    ));
    assert!(!arguments.exists());
    assert_eq!(fs::read_to_string(destination.join("sentinel")).unwrap(), "retain");
}

#[test]
fn license_installation_requires_authentication_before_executing_the_installer() {
    let sandbox = Sandbox::new();
    let installer = installer(&sandbox);
    let destination = sandbox.path().join("IDA.app");
    let arguments = sandbox.path().join("arguments");
    let output = sandbox.run_with_env(
        &[
            "ida",
            "install",
            installer.to_str().unwrap(),
            "-i",
            destination.to_str().unwrap(),
            "-l",
            "fixture-license",
            "-y",
        ],
        &[("HY_TEST_INSTALLER_ARGUMENTS", &arguments)],
    );
    assert!(!output.status.success());
    assert!(!arguments.exists());
    assert!(!destination.exists());
}

#[test]
fn installer_fetches_the_selected_license_by_key_and_publishes_it_in_the_product() {
    use serde_json::json;
    use support::{
        auth::*,
        http::{Response, Server},
    };

    for missing_asset in [false, true] {
        let sandbox = Sandbox::new();
        write_config(&sandbox, &stored("key", "fixture-api-key"));
        let installer = installer(&sandbox);
        let destination = sandbox.path().join("Licensed IDA.app");
        let arguments = sandbox.path().join("arguments");
        let server = Server::start(move |request, base| match request.path.as_str() {
            "/api/customers" => Response::json(json!([{"id": 17}])),
            "/api/licenses/17?page=1&limit=100" => Response::json(json!({"total": 1, "items": [{
                "pubhash": "96-fixture-01", "license_key": "actual-license-key",
                "status": "active", "product_catalog": "legacy", "asset_types": ["hexlic"]
            }]})),
            "/api/licenses/17/download/hexlic/actual-license-key" => {
                Response::json(json!(format!("{base}/ida_96-fixture-01.hexlic")))
            }
            "/ida_96-fixture-01.hexlic" if !missing_asset => Response {
                status: 200,
                content_type: "application/octet-stream",
                body: b"license fixture".to_vec(),
            },
            _ => Response::missing(),
        });
        let output = command(
            &sandbox,
            &server,
            &[
                "ida",
                "install",
                installer.to_str().unwrap(),
                "-i",
                destination.to_str().unwrap(),
                "-l",
                "96-fixture-01",
                "-y",
                "-A",
                "--no-set-default",
            ],
        )
        .env("HY_TEST_INSTALLER_ARGUMENTS", &arguments)
        .output()
        .unwrap();
        assert_eq!(
            output.status.success(),
            !missing_asset,
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let product = if cfg!(target_os = "macos") {
            destination.join("Contents/MacOS")
        } else {
            destination
        };
        assert!(product.join("ida.hlp").is_file());
        let installed = product.join("ida_96-fixture-01.hexlic");
        assert_eq!(installed.exists(), !missing_asset);
        if !missing_asset {
            assert_eq!(fs::read(installed).unwrap(), b"license fixture");
        }
        let requests = server.requests();
        assert_eq!(requests.len(), 4);
        assert_eq!(requests[2].path, "/api/licenses/17/download/hexlic/actual-license-key");
        assert!(!requests[3].headers.contains("fixture-api-key"));
    }
}

#[test]
fn failed_or_empty_installer_output_does_not_become_the_default() {
    for failure in ["fail", "empty"] {
        let sandbox = Sandbox::new();
        let installer = installer(&sandbox);
        let destination = sandbox.path().join("failed.app");
        let arguments = sandbox.path().join("arguments");
        fs::write(arguments.with_extension(failure), "").unwrap();
        let output = sandbox.run_with_env(
            &[
                "ida",
                "install",
                installer.to_str().unwrap(),
                "-i",
                destination.to_str().unwrap(),
                "-y",
                "-A",
            ],
            &[("HY_TEST_INSTALLER_ARGUMENTS", &arguments)],
        );
        assert!(!output.status.success());
        assert!(arguments.is_file());
        assert!(!sandbox.path().join("idausr/ida-config.json").exists());
    }
}

#[test]
fn successful_install_registers_a_default_without_requiring_idalib() {
    let sandbox = Sandbox::new();
    let installer = installer(&sandbox);
    let destination = sandbox.path().join("IDA Professional 9.4.app");
    let arguments = sandbox.path().join("arguments");
    assert_success(&sandbox.run_with_env(
        &[
            "ida",
            "install",
            installer.to_str().unwrap(),
            "-i",
            destination.to_str().unwrap(),
            "-y",
            "-A",
        ],
        &[("HY_TEST_INSTALLER_ARGUMENTS", &arguments)],
    ));
    let selected: serde_json::Value =
        serde_json::from_slice(&fs::read(sandbox.config_path()).unwrap()).unwrap();
    let default = selected["ida.default"].as_str().unwrap();
    assert_eq!(
        selected["ida.instances"][default],
        serde_json::json!(destination.canonicalize().unwrap())
    );
    assert!(!sandbox.path().join("idausr/ida-config.json").exists());
}

#[test]
fn eula_acceptance_uses_registry_api_with_the_explicit_installation() {
    let sandbox = Sandbox::new();
    let installation = sandbox.path().join("IDA.app");
    let executable_directory = if cfg!(target_os = "macos") {
        installation.join("Contents/MacOS")
    } else {
        installation.clone()
    };
    fs::create_dir_all(&executable_directory).unwrap();
    let interpreter = sandbox.path().join("python");
    let observed = sandbox.path().join("observed");
    fs::write(
        &interpreter,
        "#!/bin/sh\nprintf '%s\\n' \"$IDADIR\" \"$@\" > \"$HY_TEST_EULA_ARGUMENTS\"\n",
    )
    .unwrap();
    fs::set_permissions(&interpreter, fs::Permissions::from_mode(0o755)).unwrap();
    assert_success(&sandbox.run_with_env(
        &["ida", "accept-eula", installation.to_str().unwrap()],
        &[("HCLI_CURRENT_IDA_PYTHON_EXE", &interpreter), ("HY_TEST_EULA_ARGUMENTS", &observed)],
    ));
    let observed = fs::read_to_string(observed).unwrap();
    assert!(observed.starts_with(&format!("{}\n-c\n", executable_directory.display())));
    assert!(observed.contains("import idapro\nimport ida_registry"));
    assert!(observed.contains("range(90, 95)"));
    assert!(observed.contains("ida_registry.reg_write_int"));
}
