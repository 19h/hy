//! Actual link dispatch through isolated launcher programs and owned IPC sockets.
#![cfg(unix)]

mod support;

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::thread;
use std::time::Duration;

use serde_json::{Value, json};
use support::ida_ipc::IpcFixture;
use support::*;

fn launcher(sandbox: &Sandbox) -> PathBuf {
    let installation = sandbox.path().join("fixture-ida");
    fs::create_dir_all(installation.join("python")).unwrap();
    fs::write(installation.join("python/ida_pro.py"), b"# IDA SDK v9.3\n").unwrap();
    let binary = installation.join("ida");
    let script = r#"#!/bin/sh
printf '%s\n' "$1" > "$0.argv"
printf 'launcher stdout\n'
printf 'launcher stderr\n' >&2
"#;
    fs::write(&binary, script).unwrap();
    fs::set_permissions(binary, fs::Permissions::from_mode(0o755)).unwrap();
    installation
}

fn save_config(sandbox: &Sandbox, config: Value) {
    fs::create_dir_all(sandbox.config_path().parent().unwrap()).unwrap();
    fs::write(sandbox.config_path(), serde_json::to_vec(&config).unwrap()).unwrap();
}

#[test]
fn lookup_skips_missing_sources_and_keeps_raw_encoded_names_and_extensions() {
    let sandbox = Sandbox::new();
    let installation = launcher(&sandbox);
    let source = sandbox.path().join("source");
    fs::create_dir(&source).unwrap();
    for name in ["a b.i64", "a%20b.idb", "a%20b.i64"] {
        fs::write(source.join(name), b"fixture").unwrap();
    }
    save_config(&sandbox, json!({"idb.sources":{"missing":"/missing/source","valid":source}}));
    for uri in
        ["ida:///a%20b.i64", "ida:///a%20b.i64?", "ida://VALID:invalid-port/a%20b.i64/functions"]
    {
        let _ = fs::remove_file(installation.join("ida.argv"));
        let output = sandbox
            .command(&["ida", "open", uri])
            .env("HCLI_CURRENT_IDA_INSTALL_DIR", &installation)
            .env_remove("HCLI_CURRENT_IDA_VERSION")
            .output()
            .unwrap();
        assert_success(&output);
        assert_file_eventually(
            &installation.join("ida.argv"),
            format!("{}\n", source.join("a%20b.i64").display()).as_bytes(),
        );
        assert!(!String::from_utf8_lossy(&output.stdout).contains("launcher stdout"));
        assert!(!String::from_utf8_lossy(&output.stderr).contains("launcher stderr"));
    }
    fs::remove_file(installation.join("ida.argv")).unwrap();
    let output = sandbox
        .command(&["ida", "open", "ida:///a%20b"])
        .env("HCLI_CURRENT_IDA_INSTALL_DIR", &installation)
        .env_remove("HCLI_CURRENT_IDA_VERSION")
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(!installation.join("ida.argv").exists());
}

#[test]
fn legacy_sources_migrate_during_glob_lookup_and_launch_the_matching_file() {
    let sandbox = Sandbox::new();
    let installation = launcher(&sandbox);
    let source = sandbox.path().join("source");
    fs::create_dir_all(source.join("nested")).unwrap();
    let database = source.join("nested/match.i64");
    fs::write(&database, b"fixture").unwrap();
    save_config(
        &sandbox,
        json!({"idb.search-paths":["/missing/source",source],"sentinel":"retain"}),
    );
    let output = sandbox
        .command(&["ida", "open", "ida:///m[ab]tch*.i64"])
        .env("HCLI_CURRENT_IDA_INSTALL_DIR", &installation)
        .env_remove("HCLI_CURRENT_IDA_VERSION")
        .output()
        .unwrap();
    assert_success(&output);
    assert_file_eventually(
        &installation.join("ida.argv"),
        format!("{}\n", database.display()).as_bytes(),
    );
    let config: Value = serde_json::from_slice(&fs::read(sandbox.config_path()).unwrap()).unwrap();
    assert_eq!(config["idb.sources"], json!({"source-1":"/missing/source","source-2":source}));
    assert!(config.get("idb.search-paths").is_none());
    assert_eq!(config["sentinel"], "retain");
}

#[test]
fn unknown_source_still_navigates_a_matching_instance_with_the_literal_uri() {
    let sandbox = Sandbox::new();
    let fixture = IpcFixture::start(false);
    let uri = format!("ida://Unknown:invalid-port/{}?", fixture.name());
    assert_success(&sandbox.run(&["ida", "open", "--no-launch", &uri]));
    assert_eq!(
        fixture.navigations(),
        vec![json!({"cmd":"open_ida_link","uri":format!("{uri}/functions")})]
    );
    assert!(!sandbox.config_path().exists());
}

#[test]
fn ipc_navigation_errors_are_returned_without_launching_another_instance() {
    let sandbox = Sandbox::new();
    let fixture = IpcFixture::start(true);
    let uri = format!("ida:///{}/functions?ea=0x10", fixture.name());
    let output = sandbox.run(&["ida", "open", "--no-launch", &uri]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("fixture navigation rejected"));
    assert_eq!(fixture.navigations().len(), 1);
}

#[test]
fn startup_after_glob_lookup_matches_the_selected_filename_before_navigation() {
    let sandbox = Sandbox::new();
    let fixture = IpcFixture::start(false);
    let (installation, uri) = modern_launch(&sandbox, &fixture);
    let database = sandbox.path().join("source").join(fixture.name());
    let output = sandbox
        .command(&["ida", "open", "--skip-analysis", "--timeout", "2", &uri])
        .env("HCLI_CURRENT_IDA_INSTALL_DIR", &installation)
        .env_remove("HCLI_CURRENT_IDA_VERSION")
        .output()
        .unwrap();
    assert_success(&output);
    assert_file_eventually(
        &installation.join("ida.argv"),
        format!("{}\n", database.display()).as_bytes(),
    );
    assert_eq!(fixture.navigations(), vec![json!({"cmd":"open_ida_link","uri":uri})]);
}

fn modern_launch(sandbox: &Sandbox, fixture: &IpcFixture) -> (PathBuf, String) {
    let installation = launcher(sandbox);
    fs::write(installation.join("python/ida_pro.py"), b"# IDA SDK v9.4\n").unwrap();
    let source = sandbox.path().join("source");
    fs::create_dir(&source).unwrap();
    fs::write(source.join(fixture.name()), b"fixture").unwrap();
    save_config(sandbox, json!({"idb.sources":{"fixture":source}}));
    let uri = format!("ida:///hy-fixture-{}-*.i64/functions?ea=0x10", fixture.pid());
    (installation, uri)
}

#[test]
fn analysis_can_outlast_the_startup_timeout_and_then_navigate() {
    let sandbox = Sandbox::new();
    let fixture = IpcFixture::with_analysis(
        false,
        vec![
            json!({"status":"ok","analysis_complete":false}),
            json!({"status":"ok","analysis_complete":true}),
        ],
    );
    let (installation, uri) = modern_launch(&sandbox, &fixture);
    let started = std::time::Instant::now();
    let output = sandbox
        .command(&["ida", "open", "--timeout", "1", &uri])
        .env("HCLI_CURRENT_IDA_INSTALL_DIR", installation)
        .output()
        .unwrap();
    assert_success(&output);
    assert!(started.elapsed() >= Duration::from_secs(5));
    assert_eq!(fixture.analysis_query_count(), 2);
    assert_eq!(fixture.navigations(), vec![json!({"cmd":"open_ida_link","uri":uri})]);
}

#[test]
fn control_c_skips_analysis_and_continues_navigation() {
    let sandbox = Sandbox::new();
    let fixture =
        IpcFixture::with_analysis(false, vec![json!({"status":"ok","analysis_complete":false})]);
    let (installation, uri) = modern_launch(&sandbox, &fixture);
    let mut command = sandbox.command(&["ida", "open", "--timeout", "1", &uri]);
    command.env("HCLI_CURRENT_IDA_INSTALL_DIR", installation);
    let mut terminal = support::terminal::Terminal::start(command);
    terminal.wait_for("Waiting for auto-analysis to complete");
    let deadline = std::time::Instant::now() + Duration::from_secs(2);
    while fixture.analysis_query_count() == 0 {
        assert!(std::time::Instant::now() < deadline, "analysis query did not arrive");
        thread::sleep(Duration::from_millis(5));
    }
    terminal.send("\x03");
    let (status, output) = terminal.finish();
    assert!(status.success(), "{output}");
    assert!(output.contains("Analysis wait cancelled by user"), "{output}");
    assert_eq!(fixture.navigations(), vec![json!({"cmd":"open_ida_link","uri":uri})]);
}

#[test]
fn existing_instances_ignore_launch_timeouts_and_do_not_poll_analysis() {
    let sandbox = Sandbox::new();
    let fixture = IpcFixture::start(false);
    let uri = format!("ida:///{}/functions?ea=1", fixture.name());
    for timeout in ["0", "-1", "NaN", "inf", "-inf"] {
        assert_success(&sandbox.run(&["ida", "open", "--timeout", timeout, "--no-launch", &uri]));
    }
    assert_eq!(fixture.navigations().len(), 5);
    assert_eq!(fixture.analysis_query_count(), 0);
    assert!(!sandbox.config_path().exists());
}

#[test]
fn launch_prefers_the_registered_default_and_ignores_the_global_version_override() {
    let sandbox = Sandbox::new();
    let created = launcher(&sandbox);
    let registered = sandbox.path().join("registered");
    fs::rename(created, &registered).unwrap();
    let environment = launcher(&sandbox);
    fs::write(environment.join("python/ida_pro.py"), b"# IDA SDK v9.4\n").unwrap();
    let source = sandbox.path().join("source");
    fs::create_dir(&source).unwrap();
    let database = source.join("selection.i64");
    fs::write(&database, b"fixture").unwrap();
    save_config(
        &sandbox,
        json!({"ida.instances":{"registered":registered},"ida.default":"registered","idb.sources":{"source":source}}),
    );
    let before = fs::read(sandbox.config_path()).unwrap();
    let output = sandbox
        .command(&["ida", "open", "--timeout", "0", "ida:///selection.i64"])
        .env("HCLI_CURRENT_IDA_INSTALL_DIR", &environment)
        .env("HCLI_CURRENT_IDA_VERSION", "9.4")
        .output()
        .unwrap();
    assert_success(&output);
    assert_file_eventually(
        &registered.join("ida.argv"),
        format!("{}\n", database.display()).as_bytes(),
    );
    assert!(!environment.join("ida.argv").exists());

    fs::remove_file(registered.join("ida")).unwrap();
    fs::write(environment.join("python/ida_pro.py"), b"# IDA SDK v9.3\n").unwrap();
    let output = sandbox
        .command(&["ida", "open", "--timeout", "0", "ida:///selection.i64"])
        .env("HCLI_CURRENT_IDA_INSTALL_DIR", &environment)
        .output()
        .unwrap();
    assert_success(&output);
    assert_file_eventually(
        &environment.join("ida.argv"),
        format!("{}\n", database.display()).as_bytes(),
    );
    assert_eq!(fs::read(sandbox.config_path()).unwrap(), before);
}

#[test]
fn launching_detaches_the_session_and_suppresses_child_output() {
    let sandbox = Sandbox::new();
    let installation = launcher(&sandbox);
    let script = r#"#!/bin/sh
printf '%s\n' "$$" > "$0.pid"
printf 'child stdout\n'
printf 'child stderr\n' >&2
exec /bin/sleep 30
"#;
    fs::write(installation.join("ida"), script).unwrap();
    let database = sandbox.path().join("detached.i64");
    fs::write(&database, b"fixture").unwrap();
    save_config(&sandbox, json!({"idb.sources":{"fixture":sandbox.path()}}));
    let output = sandbox
        .command(&["ida", "open", "ida:///detached.i64"])
        .env("HCLI_CURRENT_IDA_INSTALL_DIR", &installation)
        .output()
        .unwrap();
    assert_success(&output);
    let deadline = std::time::Instant::now() + Duration::from_secs(2);
    let pid = loop {
        if let Ok(text) = fs::read_to_string(installation.join("ida.pid"))
            && let Ok(pid) = text.trim().parse::<i32>()
        {
            break pid;
        }
        assert!(std::time::Instant::now() < deadline, "detached fixture did not start");
        thread::sleep(Duration::from_millis(5));
    };
    struct DetachedFixture(i32);
    impl Drop for DetachedFixture {
        fn drop(&mut self) {
            // SAFETY: this PID was recorded by the executable in this private fixture.
            unsafe {
                libc::kill(self.0, libc::SIGTERM);
            }
        }
    }
    let process = DetachedFixture(pid);
    // SAFETY: getsid reads the session identifier of this owned fixture process.
    assert_eq!(unsafe { libc::getsid(process.0) }, process.0);
    assert!(!String::from_utf8_lossy(&output.stdout).contains("child stdout"));
    assert!(!String::from_utf8_lossy(&output.stderr).contains("child stderr"));
}
