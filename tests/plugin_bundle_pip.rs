//! Bundle pip behavior is exercised with an owned shell executable and no network.
#![cfg(unix)]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::{Command, Stdio};

mod support;

use support::{Sandbox, archive_with_dependencies, assert_success};

struct Fixture {
    sandbox: Sandbox,
    events: PathBuf,
    command: Command,
}

impl Fixture {
    fn new(links: &[&str]) -> Self {
        let sandbox = Sandbox::new();
        let repository = sandbox.path().join("repository");
        fs::create_dir(&repository).unwrap();
        archive_with_dependencies(
            &repository.join("plugin.zip"),
            "1.0",
            &[],
            &["fixture[extra]>=1", "--pre"],
        );
        let python = sandbox.path().join("python");
        fs::write(&python, include_str!("plugin_bundle_pip/python.sh")).unwrap();
        fs::set_permissions(&python, fs::Permissions::from_mode(0o755)).unwrap();
        let events = sandbox.path().join("events");
        let output = sandbox.path().join("bundle.zip");
        let mut args = vec![
            "plugin",
            "--repo",
            repository.to_str().unwrap(),
            "--pip-index-url",
            "https://index/simple",
            "--pip-extra-index-url",
            "https://extra/simple",
        ];
        if !links.is_empty() {
            args.push("--offline");
        }
        for link in links {
            args.extend(["--pip-find-links", link]);
        }
        args.extend([
            "bundle",
            "create",
            "--path",
            output.to_str().unwrap(),
            "--platform",
            "windows",
            "--python",
            "3.12",
            "example==1",
        ]);
        let mut command = sandbox.command(&args);
        command
            .env("HCLI_CURRENT_IDA_PYTHON_EXE", python)
            .env("HY_TEST_EVENTS", &events)
            .current_dir(sandbox.path());
        Self {
            sandbox,
            events,
            command,
        }
    }

    fn events(&self) -> String {
        fs::read_to_string(&self.events).unwrap()
    }
}

#[test]
fn bundle_download_inherits_process_state_and_captures_success_streams() {
    let mut fixture = Fixture::new(&["~/cache/./wheels/../", "./wheels//", "", "odd://a//./b"]);
    let input = fixture.sandbox.path().join("stdin");
    fs::write(&input, "bundle-input\n").unwrap();
    let output = fixture
        .command
        .stdin(Stdio::from(fs::File::open(input).unwrap()))
        .env("HY_TEST_READ_STDIN", "1")
        .env("PYTHONHOME", "/fixture/base python")
        .env("VIRTUAL_ENV", "/fixture/active")
        .env("PYTHONUTF8", "0")
        .env("PATH", "/fixture/bin:/other path/bin")
        .env("HY_TEST_PIP_STDOUT", "captured bundle stdout")
        .env("HY_TEST_PIP_STDERR", "captured bundle stderr")
        .output()
        .unwrap();
    assert_success(&output);
    let events = fixture.events();
    assert!(
        events.contains(
            "environment:/fixture/base python|/fixture/active|0|/fixture/bin:/other path/bin\n"
        ),
        "{events}"
    );
    assert!(events.contains("stdin:bundle-input\n"), "{events}");
    let cwd = fs::canonicalize(fixture.sandbox.path()).unwrap();
    assert!(events.contains(&format!("cwd:{}\n", cwd.display())), "{events}");
    let home = fixture.sandbox.path().display();
    let expected = format!(
        "arg:--index-url\narg:https://index/simple\n\
         arg:--extra-index-url\narg:https://extra/simple\n\
         arg:--find-links\narg:{home}/cache/wheels/..\n\
         arg:--find-links\narg:wheels\narg:--find-links\narg:.\n\
         arg:--find-links\narg:odd://a//./b\narg:--no-index\n\
         arg:fixture[extra]>=1\narg:--pre\n"
    );
    assert!(events.contains(&expected), "{events}");
    assert!(!String::from_utf8_lossy(&output.stdout).contains("captured bundle stdout"));
    assert!(!String::from_utf8_lossy(&output.stderr).contains("captured bundle stderr"));
    assert!(fixture.sandbox.path().join("bundle.zip").is_file());
}

#[test]
fn bundle_failures_keep_stream_whitespace_and_leave_existing_output_intact() {
    for status in [7, 17] {
        let mut fixture = Fixture::new(&[]);
        let destination = fixture.sandbox.path().join("bundle.zip");
        fs::write(&destination, "existing bundle").unwrap();
        let output = fixture
            .command
            .env("HY_TEST_PIP_STATUS", status.to_string())
            .env("HY_TEST_PIP_STDOUT", " \r\nstdout \n")
            .env("HY_TEST_PIP_STDERR", "\tstderr \r\n")
            .output()
            .unwrap();
        assert!(!output.status.success());
        assert!(String::from_utf8_lossy(&output.stderr).contains(
            "pip download failed for target windows-x86_64-cp312:\n \r\nstdout \n\n\tstderr \r\n"
        ), "{output:?}");
        assert_eq!(fs::read(destination).unwrap(), b"existing bundle");
        assert_eq!(fixture.events().matches("arg:download\n").count(), 1);
    }
}

#[test]
fn empty_and_relative_home_values_reach_bundle_pip_unchanged_in_kind() {
    for (home, link) in [("", "/wheels"), ("relative/home/", "relative/home/wheels")] {
        let mut fixture = Fixture::new(&["~/wheels"]);
        let output = fixture.command.env("HOME", home).output().unwrap();
        assert_success(&output);
        assert!(fixture.events().contains(&format!("arg:--find-links\narg:{link}\n")));
    }
}
