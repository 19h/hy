//! Source CLI parity and byte-preserving no-op behavior under isolated homes.

mod support;

use std::fs;

use serde_json::{Value, json};
use support::*;

fn saved(sandbox: &Sandbox) -> Value {
    serde_json::from_slice(&fs::read(sandbox.config_path()).unwrap()).unwrap()
}

fn text(output: &std::process::Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

#[test]
fn duplicate_add_and_missing_remove_succeed_without_rewriting_configuration() {
    let sandbox = Sandbox::new();
    let first = sandbox.path().join("first");
    let second = sandbox.path().join("second");
    fs::create_dir(&first).unwrap();
    fs::create_dir(&second).unwrap();
    assert_success(&sandbox.run(&["ida", "source", "add", "example", first.to_str().unwrap()]));
    let before = fs::read(sandbox.config_path()).unwrap();
    let duplicate = sandbox.run(&["ke", "source", "add", "example", second.to_str().unwrap()]);
    assert_success(&duplicate);
    assert!(text(&duplicate).contains("already exists"));
    assert_eq!(fs::read(sandbox.config_path()).unwrap(), before);
    let missing = sandbox.run(&["ida", "source", "remove", "missing"]);
    assert_success(&missing);
    assert!(text(&missing).contains("not found"));
    assert_eq!(fs::read(sandbox.config_path()).unwrap(), before);
    assert_success(&sandbox.run(&[
        "ida",
        "source",
        "add",
        "example",
        second.to_str().unwrap(),
        "--force",
    ]));
    assert_eq!(saved(&sandbox)["idb.sources"]["example"], json!(second.canonicalize().unwrap()));
    assert_success(&sandbox.run(&["ke", "source", "remove", "example"]));
    assert_eq!(saved(&sandbox)["idb.sources"], json!({}));
    assert!(first.is_dir() && second.is_dir());
}

#[test]
fn listing_preserves_registration_order_and_marks_missing_paths() {
    let sandbox = Sandbox::new();
    let existing = sandbox.path().join("existing");
    fs::create_dir(&existing).unwrap();
    fs::create_dir_all(sandbox.config_path().parent().unwrap()).unwrap();
    let initial =
        json!({"idb.sources":{"zeta":existing,"alpha":"/missing/source"},"sentinel":"retain"});
    let before = serde_json::to_vec(&initial).unwrap();
    fs::write(sandbox.config_path(), &before).unwrap();
    for group in ["ida", "ke"] {
        let output = sandbox.run(&[group, "source", "list"]);
        assert_success(&output);
        let text = text(&output);
        assert!(text.contains("Sources (2)"), "{text}");
        assert!(text.find("zeta ->").unwrap() < text.find("alpha ->").unwrap(), "{text}");
        assert!(text.contains("alpha -> /missing/source (not found)"), "{text}");
        assert_eq!(fs::read(sandbox.config_path()).unwrap(), before);
    }
}

#[test]
fn names_follow_upstream_ascii_grammar_including_its_final_newline() {
    let sandbox = Sandbox::new();
    let directory = sandbox.path().to_str().unwrap();
    for name in ["a", "0", "a-", "local-host", "a\n", "localhost\n"] {
        assert_success(&sandbox.run(&["ida", "source", "add", name, directory]));
        assert_eq!(
            saved(&sandbox)["idb.sources"][name],
            json!(sandbox.path().canonicalize().unwrap())
        );
    }
    let before = fs::read(sandbox.config_path()).unwrap();
    for name in ["", "localhost", "UPPER", "a_b", "a/b", "a b", "é", "a\n\n", "a\r\n"] {
        let output = sandbox.run(&["ida", "source", "add", name, directory]);
        assert!(!output.status.success(), "accepted {name:?}: {}", text(&output));
        assert_eq!(fs::read(sandbox.config_path()).unwrap(), before);
    }
}

#[test]
fn invalid_paths_fail_before_source_changes() {
    let sandbox = Sandbox::new();
    let file = sandbox.path().join("file");
    fs::write(&file, b"retain").unwrap();
    let missing = sandbox.path().join("missing");
    for path in [&file, &missing] {
        let output = sandbox.run(&["ida", "source", "add", "example", path.to_str().unwrap()]);
        assert!(!output.status.success(), "{}", text(&output));
        assert!(!sandbox.config_path().exists());
    }
    let output = sandbox.run(&["ida", "source", "add", "localhost", missing.to_str().unwrap()]);
    assert!(text(&output).contains("Path does not exist"), "{}", text(&output));
}

#[cfg(unix)]
#[test]
fn registration_resolves_relative_paths_and_symlinks() {
    let sandbox = Sandbox::new();
    let target = sandbox.path().join("target");
    fs::create_dir(&target).unwrap();
    std::os::unix::fs::symlink(&target, sandbox.path().join("link")).unwrap();
    let output = sandbox
        .command(&["ida", "source", "add", "example", "link"])
        .current_dir(sandbox.path())
        .output()
        .unwrap();
    assert_success(&output);
    assert_eq!(saved(&sandbox)["idb.sources"]["example"], json!(target.canonicalize().unwrap()));
}

#[test]
fn protocol_registration_accepts_force_in_help_without_installing_a_handler() {
    let sandbox = Sandbox::new();
    for arguments in [
        vec!["ida", "protocol", "register", "--force", "--help"],
        vec!["ke", "setup", "--force", "--help"],
    ] {
        let output = sandbox.run(&arguments);
        assert_success(&output);
        assert!(text(&output).contains("--force"));
    }
    assert!(!sandbox.config_path().exists());
    assert!(!sandbox.path().join("Applications").exists());
}

#[cfg(unix)]
#[test]
fn database_lookup_uses_the_first_registered_source_unless_a_source_is_named() {
    use std::os::unix::fs::PermissionsExt;

    let sandbox = Sandbox::new();
    let installation = sandbox.path().join("fixture-ida");
    fs::create_dir_all(installation.join("python")).unwrap();
    fs::write(installation.join("python/ida_pro.py"), b"# IDA SDK v9.3\n").unwrap();
    let binary = installation.join("ida");
    fs::write(&binary, b"#!/bin/sh\nprintf '%s\\n' \"$1\" > \"$0.argv\"\n").unwrap();
    fs::set_permissions(binary, fs::Permissions::from_mode(0o755)).unwrap();
    let name = format!("lookup-{}.i64", sandbox.path().file_name().unwrap().to_str().unwrap());
    let first = sandbox.path().join("first");
    let second = sandbox.path().join("second");
    for directory in [&first, &second] {
        fs::create_dir(directory).unwrap();
        fs::write(directory.join(&name), b"fixture database").unwrap();
    }
    assert_success(&sandbox.run(&["ida", "source", "add", "zeta", first.to_str().unwrap()]));
    assert_success(&sandbox.run(&["ida", "source", "add", "alpha", second.to_str().unwrap()]));
    for (source, expected) in [("", &first), ("alpha", &second)] {
        let _ = fs::remove_file(installation.join("ida.argv"));
        let output = sandbox
            .command(&["ida", "open", &format!("ida://{source}/{name}")])
            .env("HCLI_CURRENT_IDA_INSTALL_DIR", &installation)
            .env_remove("HCLI_CURRENT_IDA_VERSION")
            .output()
            .unwrap();
        assert_success(&output);
        let expected = expected.canonicalize().unwrap().join(&name);
        assert_file_eventually(
            &installation.join("ida.argv"),
            format!("{}\n", expected.display()).as_bytes(),
        );
    }
}
