//! Editable src-layout imports and registration cleanup through the CLI.
#![cfg(unix)]

mod support;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::json;
use support::{
    http::{Response, Server},
    *,
};

struct Fixture {
    sandbox: Sandbox,
    python: PathBuf,
    site: PathBuf,
    arguments: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let sandbox = Sandbox::new();
        let python = sandbox.path().join("python");
        fake_python(&python);
        Self {
            python,
            site: sandbox.path().join("site packages"),
            arguments: sandbox.path().join("pip-arguments"),
            sandbox,
        }
    }

    fn run(&self, args: &[&str]) -> Output {
        self.sandbox.run_with_env(
            args,
            &[
                ("HCLI_CURRENT_IDA_PYTHON_EXE", &self.python),
                ("HY_TEST_PURELIB", &self.site),
                ("HY_TEST_PIP_ARGUMENTS", &self.arguments),
            ],
        )
    }

    fn source(&self, directory: &str, version: &str, src_layout: bool) -> PathBuf {
        let source = self.sandbox.path().join(directory);
        fs::create_dir_all(&source).unwrap();
        let descriptor = identity_manifest(version, "https://github.com/example/editable");
        fs::write(source.join("ida-plugin.json"), serde_json::to_vec(&descriptor).unwrap())
            .unwrap();
        fs::write(source.join("plugin.py"), b"# plugin fixture\n").unwrap();
        if src_layout {
            fs::create_dir(source.join("src")).unwrap();
            fs::write(source.join("src/editable_fixture.py"), b"VALUE = 42\n").unwrap();
        }
        source
    }

    fn install(&self, source: &Path) -> Output {
        self.run(&[
            "plugin",
            "--no-python-environment-check",
            "install",
            "--editable",
            source.to_str().unwrap(),
        ])
    }

    fn registration(&self) -> PathBuf {
        self.site.join("_hcli_editable_example.pth")
    }

    fn target(&self) -> PathBuf {
        self.sandbox.path().join("idausr/plugins/example")
    }
}

fn import_value(site: &Path) -> String {
    // Read-only interpreter validation: isolated startup and no bytecode writes.
    let output = Command::new("python3").args([
        "-I", "-B", "-S", "-c",
        "import site, sys; site.addsitedir(sys.argv[1]); import editable_fixture; print(editable_fixture.VALUE)",
    ]).arg(site).output().expect("python3 is required for the editable import regression");
    assert_success(&output);
    String::from_utf8(output.stdout).unwrap().trim().into()
}

#[test]
fn src_layout_is_importable_and_uninstall_preserves_the_source_tree() {
    let fixture = Fixture::new();
    let source = fixture.source("source with spaces", "1", true);
    assert_success(&fixture.install(&source));
    assert_eq!(fs::read_link(fixture.target()).unwrap(), source.canonicalize().unwrap());
    assert_eq!(
        fs::read_to_string(fixture.registration()).unwrap(),
        format!("{}\n", source.canonicalize().unwrap().join("src").display())
    );
    assert_eq!(import_value(&fixture.site), "42");
    fs::write(source.join("src/editable_fixture.py"), b"VALUE = 84\n").unwrap();
    assert_eq!(import_value(&fixture.site), "84");
    assert_success(&fixture.run(&["plugin", "uninstall", "EXAMPLE"]));
    assert!(fs::symlink_metadata(fixture.target()).is_err());
    assert!(!fixture.registration().exists());
    assert_eq!(fs::read(source.join("src/editable_fixture.py")).unwrap(), b"VALUE = 84\n");
    assert!(!source.join("src/__pycache__").exists());
}

#[test]
fn flat_layout_and_regular_replacements_remove_stale_registrations() {
    let fixture = Fixture::new();
    let source = fixture.source("src-layout", "1", true);
    let flat = fixture.source("flat-layout", "1", false);
    assert_success(&fixture.install(&source));
    assert_success(&fixture.install(&flat));
    assert!(!fixture.registration().exists());
    assert_eq!(fs::read_link(fixture.target()).unwrap(), flat.canonicalize().unwrap());
    assert_success(&fixture.install(&source));
    let package = fixture.sandbox.path().join("regular.zip");
    archive_manifest(&package, &identity_manifest("2", "https://github.com/example/editable"), &[]);
    assert_success(&fixture.run(&["plugin", "install", "-U", package.to_str().unwrap()]));
    assert!(!fixture.target().is_symlink());
    assert!(!fixture.registration().exists());
    assert!(source.join("src/editable_fixture.py").exists());
    assert!(flat.join("plugin.py").exists());
}

#[test]
fn registration_failures_preserve_the_previous_plugin() {
    for failure in ["prepare", "publish"] {
        let fixture = Fixture::new();
        let original = fixture.sandbox.path().join("original.zip");
        archive_manifest(
            &original,
            &identity_manifest("1", "https://github.com/example/editable"),
            &[("package/sentinel", b"original")],
        );
        assert_success(&fixture.run(&["plugin", "install", original.to_str().unwrap()]));
        let source = fixture.source("candidate", "2", true);
        let mut descriptor = identity_manifest("2", "https://github.com/example/editable");
        descriptor["plugin"]["pythonDependencies"] = json!(["fixture-dependency"]);
        fs::write(source.join("ida-plugin.json"), serde_json::to_vec(&descriptor).unwrap())
            .unwrap();
        if failure == "prepare" {
            fs::write(&fixture.site, b"not a directory").unwrap();
        } else {
            fs::create_dir_all(fixture.registration()).unwrap();
        }
        let output = fixture.install(&source);
        assert!(!output.status.success(), "{failure}");
        assert_eq!(fixture.arguments.exists(), failure == "publish");
        assert!(!fixture.target().is_symlink());
        assert_eq!(fs::read(fixture.target().join("sentinel")).unwrap(), b"original");
        assert_eq!(installed_manifest(&fixture.sandbox)["plugin"]["version"], "1");
        if failure == "publish" {
            assert!(
                String::from_utf8_lossy(&output.stderr)
                    .contains("cannot publish editable registration")
            );
            assert_eq!(fs::read_dir(&fixture.site).unwrap().count(), 1);
        }
    }
}

#[test]
fn dependency_failure_does_not_publish_a_new_registration() {
    let fixture = Fixture::new();
    let first = fixture.source("first", "1", true);
    assert_success(&fixture.install(&first));
    let registration = fs::read(fixture.registration()).unwrap();
    let second = fixture.source("second", "2", true);
    let mut descriptor = identity_manifest("2", "https://github.com/example/editable");
    descriptor["plugin"]["pythonDependencies"] = json!(["fixture-dependency"]);
    fs::write(second.join("ida-plugin.json"), serde_json::to_vec(&descriptor).unwrap()).unwrap();
    fs::write(fixture.arguments.with_extension("fail-resolution"), b"").unwrap();
    assert!(!fixture.install(&second).status.success());
    assert_eq!(fs::read(fixture.registration()).unwrap(), registration);
    assert_eq!(fs::read_link(fixture.target()).unwrap(), first.canonicalize().unwrap());
    assert_eq!(fs::read_dir(&fixture.site).unwrap().count(), 1);
}

#[test]
fn configuration_failure_removes_a_fresh_registration_and_keeps_sources() {
    let fixture = Fixture::new();
    let source = fixture.source("source", "1", true);
    let mut descriptor = identity_manifest("1", "https://github.com/example/editable");
    descriptor["plugin"]["settings"] = json!([{
        "key": "enabled", "type": "boolean", "name": "Enabled", "required": false,
    }]);
    fs::write(source.join("ida-plugin.json"), serde_json::to_vec(&descriptor).unwrap()).unwrap();
    let output = fixture.run(&[
        "plugin",
        "install",
        "--editable",
        "--config",
        "unknown=value",
        source.to_str().unwrap(),
    ]);
    assert!(!output.status.success());
    assert!(fs::symlink_metadata(fixture.target()).is_err());
    assert!(!fixture.registration().exists());
    assert!(source.join("src/editable_fixture.py").is_file());
}

#[test]
fn dangling_editable_links_and_plain_file_remnants_can_be_removed() {
    let fixture = Fixture::new();
    fs::create_dir_all(fixture.target().parent().unwrap()).unwrap();
    fs::create_dir_all(&fixture.site).unwrap();
    std::os::unix::fs::symlink(fixture.sandbox.path().join("absent-source"), fixture.target())
        .unwrap();
    fs::write(fixture.registration(), b"/absent-source/src\n").unwrap();
    assert_success(&fixture.run(&["plugin", "uninstall", "EXAMPLE"]));
    assert!(!fixture.registration().exists());
    fs::write(fixture.target(), b"broken installation").unwrap();
    assert_success(&fixture.run(&["plugin", "uninstall", "example"]));
    assert!(!fixture.target().exists());
}

#[test]
fn editable_urls_are_rejected_before_repository_transport() {
    let fixture = Fixture::new();
    let server = Server::start(|_, _| Response::missing());
    let output = fixture.run(&["plugin", "install", "--editable", &server.url]);
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("--editable requires a local directory")
    );
    assert!(server.requests().is_empty());
}

#[test]
fn editable_install_replaces_broken_entries_without_touching_the_source() {
    for kind in ["directory", "file", "symlink"] {
        let fixture = Fixture::new();
        let source = fixture.source("source", "1", true);
        fs::create_dir_all(fixture.target().parent().unwrap()).unwrap();
        match kind {
            "directory" => {
                fs::create_dir(fixture.target()).unwrap();
                fs::write(fixture.target().join("ida-plugin.json"), b"invalid JSON").unwrap();
            }
            "file" => fs::write(fixture.target(), b"broken installation").unwrap(),
            _ => {
                std::os::unix::fs::symlink(fixture.sandbox.path().join("missing"), fixture.target())
                    .unwrap()
            }
        }
        assert_success(&fixture.install(&source));
        assert_eq!(fs::read_link(fixture.target()).unwrap(), source.canonicalize().unwrap());
        assert!(fixture.registration().is_file());
        assert!(source.join("src/editable_fixture.py").is_file());
    }
}

#[test]
fn source_paths_with_line_breaks_cannot_become_pth_directives() {
    let fixture = Fixture::new();
    let source = fixture.source("source\nsecond-line", "1", true);
    let output = fixture.install(&source);
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("editable source path contains a line break")
    );
    assert!(!fixture.registration().exists());
    assert!(fs::symlink_metadata(fixture.target()).is_err());
}

#[test]
fn uninstall_skips_registration_cleanup_when_the_interpreter_is_unavailable() {
    let fixture = Fixture::new();
    let source = fixture.source("source", "1", true);
    assert_success(&fixture.install(&source));
    // The shared sandbox defaults to a nonexistent interpreter and cannot run IDA.
    assert_success(&fixture.sandbox.run(&["plugin", "uninstall", "example"]));
    assert!(fs::symlink_metadata(fixture.target()).is_err());
    assert!(source.join("src/editable_fixture.py").is_file());
    assert!(fixture.registration().is_file());
}
