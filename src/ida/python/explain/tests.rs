//! Source differential coverage of context notes and complete plain-text reports.

use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

use serde_json::{Value, json};

#[cfg(unix)]
use super::collect;
use super::{notes, render, types::*};

fn report() -> EnvironmentReport {
    EnvironmentReport {
        experimental: true,
        known_installations: KnownInstallations {
            installations: vec![Installation {
                path: "/ida".into(),
                version: Some("9.4".into()),
            }],
            error: None,
        },
        selected_installation: SelectedInstallation {
            install_dir: Some("/ida".into()),
            install_dir_source: Some("fixture".into()),
            install_dir_error: None,
        },
        architecture_and_version: Some(ArchitectureAndVersion {
            ida_binary: Some("/ida/ida".into()),
            ida_binary_error: None,
            binary_arch: Some("aarch64".into()),
            binary_arch_error: None,
            platform: Some("macos-aarch64".into()),
            platform_error: None,
            ida_version: Some("9.4".into()),
            ida_version_source: Some("fixture".into()),
            ida_version_error: None,
        }),
        python_environment: Some(PythonEnvironment {
            virtual_env: None,
            virtual_env_is_uv_cache: false,
            user_virtual_env: None,
            candidate_virtual_envs: Vec::new(),
            idapython_venv_executable: None,
            idapython_venv_executable_exists: None,
            python_exe: Some("/python".into()),
            python_exe_source: Some("fixture".into()),
            python_exe_error: None,
            externally_managed: false,
            idat_probe: None,
            idat_probe_error: None,
        }),
        idapython_virtualenv: None,
        python_version: Some(PythonVersion {
            final_python_exe: Some("/python".into()),
            final_python_exe_error: None,
            probed_version: Some("3.13".into()),
            probed_version_error: None,
            hcli_interpreter_version: "not applicable (native Rust)",
            hcli_interpreter_path: "/hy".into(),
        }),
        python_version_mismatches: Vec::new(),
        python_version_mismatch_error: None,
        notes: Vec::new(),
    }
}

#[test]
fn notes_match_source_for_native_runtime_context() {
    let mut cases = Vec::new();
    for process in [None, Some(""), Some("/venv"), Some("/uv")] {
        for user in [false, true] {
            for venv in [false, true] {
                for managed in [false, true] {
                    for version in [
                        "",
                        "3.9",
                        "3.10",
                        "2.7",
                        "4.0",
                        "bad",
                        "3",
                        "3.9.1",
                        "03.009",
                        "-1.-2",
                        "٣.٩",
                        "3.\u{1f}9",
                        "\u{1f}3.9",
                    ] {
                        for binary in ["hy", "custom-hy"] {
                            let mut report = report();
                            let environment = report.python_environment.as_mut().unwrap();
                            environment.virtual_env = process.map(String::from);
                            environment.virtual_env_is_uv_cache = process == Some("/uv");
                            environment.user_virtual_env = user.then(|| "/user".into());
                            environment.externally_managed = managed;
                            report.idapython_virtualenv = venv.then(|| VirtualEnvironment {
                                venv: "/ida-venv".into(),
                                home: None,
                                system_site_packages: None,
                                python_version: Some("3.13".into()),
                            });
                            report.python_version.as_mut().unwrap().probed_version =
                                Some(version.into());
                            report.notes = notes::collect(
                                environment,
                                report.idapython_virtualenv.as_ref(),
                                report.python_version.as_ref().unwrap(),
                                binary,
                            );
                            cases.push(json!({"mode": "notes", "binary": binary, "report": report, "expected": report.notes}));
                        }
                    }
                }
            }
        }
    }
    assert_eq!(cases.len(), 832);
    compare(&cases);
}

#[test]
fn complete_plain_text_reports_match_source_sections_and_order() {
    let mut cases = Vec::new();
    for mask in 0..32 {
        let mut report = report();
        if mask & 1 != 0 {
            report.known_installations.installations.clear();
            report.known_installations.error = (mask & 2 != 0).then(|| "scan failed".into());
        }
        if mask & 2 != 0 {
            let environment = report.python_environment.as_mut().unwrap();
            environment.virtual_env = Some("/venv λ".into());
            environment.virtual_env_is_uv_cache = true;
            environment.user_virtual_env = Some("/user".into());
            environment.candidate_virtual_envs = vec![CandidateVirtualEnvironment {
                path: "/candidate".into(),
                source: "PATH",
            }];
            environment.idapython_venv_executable = Some("/missing".into());
            environment.idapython_venv_executable_exists = Some(false);
            environment.externally_managed = true;
            environment.idat_probe = Some(IdatProbe {
                frozen: false,
                externally_managed: true,
                prefix: "/base".into(),
                base_prefix: "/base".into(),
                executable: None,
                virtual_env: None,
                idapython_venv_executable: Some("/requested".into()),
                version_major: 3,
                version_minor: 13,
            });
            report.idapython_virtualenv = Some(VirtualEnvironment {
                venv: "/virtual".into(),
                home: Some("".into()),
                system_site_packages: Some("false".into()),
                python_version: Some("3.12".into()),
            });
        }
        if mask & 4 != 0 {
            let environment = report.python_environment.as_mut().unwrap();
            environment.python_exe = None;
            environment.python_exe_error = Some("PythonNotFoundError: fixture".into());
            environment.idat_probe_error = Some("RuntimeError: fixture".into());
            report.python_version.as_mut().unwrap().probed_version = None;
            report.python_version.as_mut().unwrap().probed_version_error =
                Some("failed to run /python".into());
            report.architecture_and_version.as_mut().unwrap().binary_arch = None;
        }
        if mask & 8 != 0 {
            for version in [
                "3.11",
                if mask & 16 != 0 {
                    "3.12"
                } else {
                    "3.11"
                },
            ] {
                report.python_version_mismatches.push(VersionMismatch {
                    ida_version: "3.13".into(),
                    other_version: version.into(),
                    other_path: "/other".into(),
                    other_source: "fixture source".into(),
                });
            }
        }
        if mask == 31 {
            report.selected_installation.install_dir = None;
            report.selected_installation.install_dir_error = Some("no installation".into());
            report.architecture_and_version = None;
            report.python_environment = None;
            report.python_version = None;
        } else {
            report.notes = notes::collect(
                report.python_environment.as_ref().unwrap(),
                report.idapython_virtualenv.as_ref(),
                report.python_version.as_ref().unwrap(),
                "hy",
            );
        }
        cases.push(json!({"mode": "render", "expected": render::text(&report), "report": report}));
    }
    compare(&cases);
}

#[cfg(unix)]
#[tokio::test]
async fn mismatch_observations_match_source_precedence_and_deduplication() {
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    let mut fixtures = Vec::new();
    let mut cases = Vec::new();
    for profile in 0..4 {
        let directory = tempfile::tempdir().unwrap();
        let first = directory.path().join("first");
        let second = directory.path().join("second");
        let base = directory.path().join("base/bin/python");
        let alias = directory.path().join("alias");
        let mut versions = serde_json::Map::new();
        for (path, version) in [
            (
                first.join("bin/python"),
                if profile & 1 == 0 {
                    Some("3.12")
                } else {
                    None
                },
            ),
            (
                second.join("bin/python"),
                if profile & 2 == 0 {
                    Some("3.13")
                } else {
                    Some("3.9")
                },
            ),
            (base.clone(), Some("3.10")),
        ] {
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            let script = version
                .map(|version| format!("#!/bin/sh\nprintf '%s\\n' '{version}'\n"))
                .unwrap_or_else(|| "#!/bin/sh\nexit 1\n".into());
            fs::write(&path, script).unwrap();
            fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
            versions.insert(path.display().to_string(), json!(version));
        }
        fs::write(first.join("pyvenv.cfg"), "version_info = 03.011.9\n").unwrap();
        fs::write(second.join("pyvenv.cfg"), "version = 3.13.9\n").unwrap();
        std::os::unix::fs::symlink(&first, &alias).unwrap();
        versions.insert(
            alias.join("bin/python").display().to_string(),
            versions[&first.join("bin/python").display().to_string()].clone(),
        );
        for active in [None, Some(first.clone()), Some(alias)] {
            for requested in [None, Some(first.join("bin/python")), Some(second.join("bin/python"))]
            {
                for executable in [
                    None,
                    Some(first.join("bin/python")),
                    Some(second.join("bin/python")),
                    Some(base.clone()),
                ] {
                    let probe = IdatProbe {
                        frozen: false,
                        externally_managed: false,
                        prefix: "/base".into(),
                        base_prefix: "/base".into(),
                        executable: None,
                        virtual_env: active.as_ref().map(|path| path.display().to_string()),
                        idapython_venv_executable: requested
                            .as_ref()
                            .map(|path| path.display().to_string()),
                        version_major: 3,
                        version_minor: 13,
                    };
                    let expected =
                        collect::mismatches(Some(&probe), executable.as_deref()).await.unwrap();
                    cases.push(json!({"mode": "mismatches", "probe": probe, "executable": executable, "versions": versions, "expected": expected}));
                }
            }
        }
        fixtures.push(directory);
    }
    assert_eq!(cases.len(), 144);
    compare(&cases);
}

fn compare(cases: &[Value]) {
    use sha2::{Digest, Sha256};
    let expected: Vec<_> = cases.iter().map(|case| &case["expected"]).collect();
    if cases.first().is_some_and(|case| case["mode"] != "mismatches") {
        let digest = if cases[0]["mode"] == "notes" {
            "93d534110426832b3bc9831df8a4eb3c246f8008f869ccc8df1e51c5f79232e1"
        } else {
            "7a54f5b3acf37ceb05e8f4aa5772840460927c877900c647411b3bdf4cd8631a"
        };
        assert_eq!(format!("{:x}", Sha256::digest(serde_json::to_vec(&expected).unwrap())), digest);
    }
    let Some(python) = std::env::var_os("HY_TEST_VENV_ORACLE_PYTHON") else {
        return;
    };
    let source =
        std::env::var_os("HY_TEST_HCLI_SOURCE").map(std::path::PathBuf::from).unwrap_or_else(
            || Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().join("ida-hcli"),
        );
    let mut child = Command::new(python)
        .args(["-I", "-B", "-c", include_str!("tests/reference.py")])
        .arg(source)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(&serde_json::to_vec(cases).unwrap()).unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success(), "{output:?}");
    let actual: Vec<Value> = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(actual.len(), cases.len());
    for (actual, case) in actual.iter().zip(cases) {
        assert_eq!(*actual, case["expected"], "{case}");
    }
}
