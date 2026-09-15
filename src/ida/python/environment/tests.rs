//! Source comparisons pin complete finding text and classification precedence.

use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use super::{findings, pattern, state::State};

mod reference;

fn baseline() -> State {
    serde_json::from_value(json!({
        "python_exe": "/fixture/venv/bin/python", "python_exe_exists": true,
        "source": "$IDAPYTHON_VENV_EXECUTABLE", "system": "linux", "idausr": "/fixture/idausr",
        "venv_root": "/fixture/venv", "pip_available": true, "python_version": "3.13",
        "ida_python_version": "3.13", "externally_managed": false, "uv_ephemeral": false,
        "idapython_venv_executable": "/fixture/venv/bin/python", "idapython_venv_executable_exists": true,
        "shell_virtual_env": null, "idapythonrc_path": "/fixture/idausr/idapythonrc.py",
        "idapythonrc_activates_venv": false, "base_prefix": null, "conda": false,
        "variable_selects_venv": true, "homebrew": false,
    })).unwrap()
}

#[test]
fn doctor_findings_and_patterns_match_upstream_policy() {
    let mut cases = Vec::new();
    for mask in 0u32..4096 {
        let flag = |bit: u32| mask & (1u32 << bit) != 0;
        let mut state = baseline();
        state.python_exe_exists = flag(0);
        state.venv_root = flag(1).then(|| "/fixture/venv".into());
        state.pip_available = match mask % 3 {
            0 => None,
            1 => Some(false),
            _ => Some(true),
        };
        state.externally_managed = flag(2);
        state.uv_ephemeral = flag(3);
        state.idapython_venv_executable = flag(4).then(|| "/fixture/venv/bin/python".into());
        state.idapython_venv_executable_exists = flag(5);
        state.variable_selects_venv =
            flag(6) && state.venv_root.is_some() && state.idapython_venv_executable.is_some();
        state.shell_virtual_env = flag(7).then(|| "/fixture/shell".into());
        state.idapythonrc_activates_venv = flag(8);
        state.conda = flag(9);
        state.homebrew = flag(10);
        if flag(11) {
            state.source = "$HCLI_CURRENT_IDA_PYTHON_EXE".into();
        }
        state.python_version = match mask % 4 {
            0 => None,
            1 => Some("3.9".into()),
            _ => Some("3.13".into()),
        };
        state.ida_python_version = (mask % 5 != 0).then(|| "3.13".into());
        cases.push(case(state));
    }
    for system in ["windows", "mac", "linux"] {
        for path in [
            "/fixture/python",
            "/fixture/Microsoft/WindowsApps/python",
            "C:\\Microsoft\\WindowsApps\\python.exe",
        ] {
            let mut state = baseline();
            state.system = serde_json::from_value(json!(system)).unwrap();
            state.python_exe = path.into();
            state.venv_root = None;
            state.idausr = None;
            state.python_version = None;
            state.ida_python_version = None;
            cases.push(case(state));
        }
    }
    // Explicit healthy input is independent of bit-mask correlations.
    cases.push(case(baseline()));
    assert_eq!(cases.last().unwrap()["expected"]["pattern"]["id"], "properly-configured");
    reference::compare(&cases);
    let patterns: std::collections::BTreeSet<_> =
        cases.iter().map(|case| case["expected"]["pattern"]["id"].as_str().unwrap()).collect();
    assert_eq!(patterns.len(), 12);
    let mut expected = json!(cases.iter().map(|case| &case["expected"]).collect::<Vec<_>>());
    canonicalize_paths(&mut expected);
    let digest = format!("{:x}", Sha256::digest(serde_json::to_vec(&expected).unwrap()));
    assert_eq!(cases.len(), 4106);
    assert_eq!(digest, "c159a75d03ae7457419acaf44dd232eb48c45e082b57ba27d275ef1584f85686");
}

fn case(state: State) -> Value {
    let findings = findings::check(&state, "hy");
    let pattern = pattern::identify(&state, &findings);
    json!({"state": state, "expected": {"findings": findings, "pattern": pattern}})
}

// Keep the pinned vector portable; the source oracle compares unmodified values.
fn canonicalize_paths(value: &mut Value) {
    match value {
        Value::String(text) => *text = text.replace('\\', "/"),
        Value::Array(values) => values.iter_mut().for_each(canonicalize_paths),
        Value::Object(values) => values.values_mut().for_each(canonicalize_paths),
        _ => (),
    }
}

#[test]
fn environment_guard_messages_match_upstream_warning_and_exception_formats() {
    use super::{
        findings::{Finding, Severity},
        guard,
    };
    let mut cases = Vec::new();
    for binary in ["hy", "custom hcli"] {
        for severities in [
            vec![],
            vec![Severity::Warning],
            vec![Severity::Error],
            vec![Severity::Warning, Severity::Error],
            vec![Severity::Error, Severity::Warning],
        ] {
            let findings: Vec<_> = severities
                .into_iter()
                .map(|severity| {
                    Finding::new(
                        "fixture",
                        severity,
                        "Path [red]日本語[/red] /a b/python",
                        "detail",
                        "hint",
                    )
                })
                .collect();
            let warning = guard::warning_text(&findings, binary);
            assert_eq!(warning.is_empty(), findings.is_empty());
            let error = guard::error_text(&findings, binary);
            assert!(error.contains(&format!("Run '{binary} ida python doctor'")));
            cases.push(json!({"binary": binary, "findings": findings, "expected": {"warning": warning, "error": error}}));
        }
    }
    reference::compare_guard(&cases);
    let expected: Vec<_> = cases.iter().map(|case| &case["expected"]).collect();
    let digest = format!("{:x}", Sha256::digest(serde_json::to_vec(&expected).unwrap()));
    assert_eq!(digest, "6655aed1f88084f185df0d282ef54951cbea93e747ee69cdde79b5c52dbf29be");
}

#[test]
fn doctor_path_observations_match_upstream_helpers() {
    let temporary = tempfile::tempdir().unwrap();
    let mut cases = Vec::new();
    for directory_config in [false, true] {
        let root = temporary.path().join(if directory_config {
            "directory-config"
        } else {
            "file-config"
        });
        std::fs::create_dir_all(&root).unwrap();
        if directory_config {
            std::fs::create_dir(root.join("pyvenv.cfg")).unwrap();
        } else {
            std::fs::write(
                root.join("pyvenv.cfg"),
                b" HOME = first\rHOME=last\nVERSION_INFO = 03.013.1\n# odd = retained\n",
            )
            .unwrap();
        }
        for (name, has_root, selects) in [
            ("bin/python", true, true),
            ("bin/Python3", true, true),
            ("bin/pypy", false, true),
            ("Scripts/python.exe", true, true),
            ("python.exe", false, false),
            ("other/python", false, true),
        ] {
            let executable = root.join(name);
            let expected_root = super::paths::venv_root(&executable);
            let expected = json!({
                "root": expected_root,
                "selects": super::paths::variable_selects(Some(&executable), Some(&root)),
                "config": super::paths::config(&root),
                "homebrew": super::paths::homebrew(&executable),
            });
            assert_eq!(
                expected["root"],
                if has_root {
                    json!(root)
                } else {
                    Value::Null
                }
            );
            assert_eq!(expected["selects"], selects);
            assert_eq!(
                expected["config"],
                if directory_config {
                    json!({})
                } else {
                    json!({"home": "last", "version_info": "03.013.1", "# odd": "retained"})
                }
            );
            cases.push(json!({"executable": executable, "root": root, "expected": expected}));
        }
    }
    for executable in [
        "/opt/homebrew/bin/python",
        "/opt/homebrewish/python",
        "/opt\\homebrew/bin/python",
        "/usr/local/Cellar/python",
        "/home/linuxbrew/.linuxbrew/python",
    ] {
        let path = std::path::Path::new(executable);
        cases.push(json!({
            "executable": path, "root": temporary.path(),
            "expected": {
                "root": super::paths::venv_root(path),
                "selects": super::paths::variable_selects(Some(path), Some(temporary.path())),
                "config": super::paths::config(temporary.path()),
                "homebrew": super::paths::homebrew(path),
            },
        }));
    }
    assert_eq!(cases.len(), 17);
    reference::compare_paths(&cases);
}
