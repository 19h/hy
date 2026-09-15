//! Explain report observations and their subprocess boundaries.
#![cfg(unix)]

use std::fs;
use std::process::Command;

use serde_json::{Value, json};

#[path = "python_explain/fixture.rs"]
mod fixture;
#[allow(dead_code)]
#[path = "python_environment_guard/fixture.rs"]
mod guard_fixture;
mod support;

use fixture::Rig;
use support::assert_success;

fn report(command: &mut Command) -> Value {
    let output = command.output().unwrap();
    assert_success(&output);
    assert!(!String::from_utf8_lossy(&output.stderr).contains("unexpected-inspection"));
    serde_json::from_slice(&output.stdout).unwrap()
}

#[test]
fn successful_overrides_do_not_add_ida_probes_or_infer_embedded_virtualenvs() {
    for variable in ["HCLI_CURRENT_IDA_PYTHON_EXE", "IDAPYTHON_VENV_EXECUTABLE"] {
        let rig = Rig::new();
        let result = report(rig.command(&rig.probe()).env(variable, &rig.fixture.python));
        assert_eq!(result["python_environment"]["idat_probe"], Value::Null);
        assert_eq!(result["idapython_virtualenv"], Value::Null);
        assert_eq!(result["python_version"]["probed_version"], "3.12");
        assert_eq!(result["python_version_mismatches"], json!([]));
        assert!(result["notes"].as_array().unwrap().iter().any(|note| note["kind"] == "hint"));
        assert_eq!(rig.fixture.calls(), format!("version:{}\n", rig.fixture.python.display()));
    }
}

#[test]
fn embedded_virtualenv_config_and_version_are_reported_with_one_deduplicated_mismatch() {
    let rig = Rig::new();
    let root = rig.fixture.python.parent().unwrap().parent().unwrap();
    fs::write(
        root.join("pyvenv.cfg"),
        "HOME = first\nhome = /base\ninclude-system-site-packages = false\nversion_info = 3.9.7\n",
    )
    .unwrap();
    let result = report(&mut rig.command(&rig.probe()));
    assert_eq!(
        result["idapython_virtualenv"],
        json!({"venv": root, "home": "/base", "system_site_packages": "false", "python_version": "3.12"})
    );
    let mismatches = result["python_version_mismatches"].as_array().unwrap();
    assert_eq!(mismatches.len(), 1);
    assert_eq!(
        mismatches[0],
        json!({"ida_version": "3.13", "other_version": "3.12", "other_path": root, "other_source": "the virtualenv activated inside IDA ($VIRTUAL_ENV)"})
    );
    assert_eq!(result["python_environment"]["idat_probe"]["frozen"], false);
    assert_eq!(result["python_environment"]["idat_probe"]["externally_managed"], false);
    let calls = rig.fixture.calls();
    assert_eq!(calls.matches("idat\n").count(), 1);
    assert_eq!(calls.matches("version:").count(), 3);
}

#[test]
fn mismatch_checks_cover_activated_requested_and_base_environments_in_order() {
    for missing_active_interpreter in [false, true] {
        let rig = Rig::new();
        let active = rig.environment(
            "active",
            (!missing_active_interpreter).then_some("3.11"),
            "version_info = 03.011.9\n",
        );
        let mut probe = rig.probe();
        probe["virtual_env"] = json!(active);
        let result = report(&mut rig.command(&probe));
        let mismatches = result["python_version_mismatches"].as_array().unwrap();
        assert_eq!(mismatches.len(), 2);
        assert_eq!(mismatches[0]["other_path"], json!(active));
        assert_eq!(mismatches[0]["other_version"], "3.11");
        assert_eq!(
            mismatches[1]["other_source"],
            "the virtualenv requested by $IDAPYTHON_VENV_EXECUTABLE"
        );
        assert_eq!(result["idapython_virtualenv"]["python_version"], "3.11");
    }
    let rig = Rig::new();
    let active = rig.environment("active", None, "version = 3.11.1\n");
    let base = rig.fixture.sandbox.path().join("base");
    rig.interpreter(&base.join("bin/python3.13"), Some("3.10"));
    let mut probe = rig.probe();
    probe["virtual_env"] = json!(active);
    probe["prefix"] = json!(base);
    probe["base_prefix"] = json!(base);
    probe["idapython_venv_executable"] = Value::Null;
    probe["executable"] = json!(rig.fixture.installation.join("idat"));
    let result = report(&mut rig.command(&probe));
    let mismatches = result["python_version_mismatches"].as_array().unwrap();
    assert_eq!(mismatches.len(), 2);
    assert_eq!(mismatches[1]["other_version"], "3.10");
    assert_eq!(mismatches[1]["other_path"], json!(base.join("bin/python3.13")));
    assert_eq!(
        mismatches[1]["other_source"],
        "the interpreter HCLI would install plugin dependencies into"
    );
}

#[test]
fn unavailable_versions_remain_unknown_and_missing_installations_have_no_notes() {
    let rig = Rig::new();
    rig.interpreter(&rig.fixture.python, None);
    let result = report(&mut rig.command(&rig.probe()));
    assert_eq!(result["idapython_virtualenv"]["python_version"], Value::Null);
    assert_eq!(result["python_version"]["probed_version"], Value::Null);
    assert_eq!(
        result["python_version"]["probed_version_error"],
        format!("failed to run {}", rig.fixture.python.display())
    );
    assert_eq!(result["python_version_mismatches"], json!([]));
    let result = report(rig.command(&rig.probe()).env_remove("HCLI_CURRENT_IDA_INSTALL_DIR"));
    assert_eq!(result["architecture_and_version"], Value::Null);
    assert_eq!(result["python_environment"], Value::Null);
    assert_eq!(result["notes"], json!([]));
}

#[test]
fn path_candidates_preserve_order_deduplicate_aliases_and_filter_uv_caches() {
    for active in ["unset", "empty", "uv", "user"] {
        let rig = Rig::new();
        let first = rig.environment("first", None, "version = 3.11\n");
        let second = rig.environment("second", None, "version = 3.12\n");
        let uv = rig.environment("uv/archive-v0/overlay", None, "version = 3.13\n");
        let alias = rig.fixture.sandbox.path().join("alias");
        std::os::unix::fs::symlink(&first, &alias).unwrap();
        let path = std::env::join_paths([
            uv.join("bin"),
            first.join("bin"),
            alias.join("bin"),
            second.join("Scripts"),
        ])
        .unwrap();
        let mut command = rig.command(&rig.probe());
        command.env("PATH", path).env("HCLI_CURRENT_IDA_PYTHON_EXE", &rig.fixture.python);
        match active {
            "empty" => {
                command.env("VIRTUAL_ENV", "");
            }
            "uv" => {
                command.env("VIRTUAL_ENV", &uv);
            }
            "user" => {
                command.env("VIRTUAL_ENV", &second);
            }
            _ => (),
        }
        let result = report(&mut command);
        let environment = &result["python_environment"];
        assert_eq!(
            environment["candidate_virtual_envs"],
            json!([{"path": first, "source": "PATH"}, {"path": second, "source": "PATH"}])
        );
        assert_eq!(
            environment["user_virtual_env"],
            match active {
                "uv" => json!(first),
                "user" => json!(second),
                _ => Value::Null,
            }
        );
        assert_eq!(
            environment["virtual_env"],
            match active {
                "empty" => json!(""),
                "uv" => json!(uv),
                "user" => json!(second),
                _ => Value::Null,
            }
        );
    }
}

#[test]
fn installation_versions_keep_metadata_provenance_and_ignore_overrides_when_listing() {
    for source in ["override", "sdk", "binary", "directory"] {
        let rig = Rig::new();
        let mut installation = rig.fixture.installation.clone();
        if source == "directory" {
            installation = rig.fixture.sandbox.path().join("IDA Professional 9.0.app");
            fs::create_dir(&installation).unwrap();
        } else if source == "binary" {
            fs::write(installation.join("ida"), b"fixture 9.1.123456").unwrap();
        } else {
            fs::create_dir_all(installation.join("python")).unwrap();
            fs::write(installation.join("python/ida_pro.py"), "IDA SDK v9.2.\n").unwrap();
        }
        let mut command = rig.command(&rig.probe());
        command
            .env("HCLI_CURRENT_IDA_PYTHON_EXE", &rig.fixture.python)
            .env("HCLI_CURRENT_IDA_INSTALL_DIR", &installation);
        if source != "override" {
            command.env_remove("HCLI_CURRENT_IDA_VERSION");
        }
        let result = report(&mut command);
        let (version, provenance, listed_version) = match source {
            "override" => ("9.4", "$HCLI_CURRENT_IDA_VERSION", Some("9.2")),
            "sdk" => ("9.2", "python/ida_pro.py SDK docstring", Some("9.2")),
            "binary" => ("9.1", "IDA binary version metadata", None),
            _ => ("9.0", "installation directory name", Some("9.0")),
        };
        assert_eq!(result["architecture_and_version"]["ida_version"], version);
        assert_eq!(result["architecture_and_version"]["ida_version_source"], provenance);
        let listed = result["known_installations"]["installations"]
            .as_array()
            .unwrap()
            .iter()
            .find(|entry| entry["path"] == json!(installation))
            .unwrap();
        assert_eq!(listed["version"], json!(listed_version));
    }
}
