use std::fs;
use std::path::PathBuf;
use std::process::Command;

use serde_json::{Value, json};

use crate::guard_fixture::{Fixture, executable};

pub(super) struct Rig {
    pub fixture: Fixture,
    pub root: PathBuf,
    pub package: PathBuf,
    pub bundle: PathBuf,
    stdout: PathBuf,
    stderr: PathBuf,
}

impl Rig {
    pub fn new() -> Self {
        let fixture = Fixture::new(true);
        executable(&fixture.python, include_str!("python.sh"));
        let root = fixture.python.parent().unwrap().parent().unwrap().to_owned();
        let package = fixture.sandbox.path().join("plugin.zip");
        crate::support::archive_with_dependencies(&package, "1", &[], &["fixture"]);
        let bundle = fixture.sandbox.path().join("bundle.zip");
        let stdout = fixture.sandbox.path().join("version-stdout");
        let stderr = fixture.sandbox.path().join("version-stderr");
        fs::write(&stdout, "3.12\n").unwrap();
        fs::write(&stderr, "").unwrap();
        Self {
            fixture,
            root,
            package,
            bundle,
            stdout,
            stderr,
        }
    }

    pub fn streams(&self, stdout: &[u8], stderr: &[u8]) {
        fs::write(&self.stdout, stdout).unwrap();
        fs::write(&self.stderr, stderr).unwrap();
    }

    pub fn command(&self, args: &[&str]) -> Command {
        let mut command = self.fixture.command(args);
        command
            .env("HY_TEST_VERSION_STDOUT", &self.stdout)
            .env("HY_TEST_VERSION_STDERR", &self.stderr)
            .env("HY_TEST_VERSION_COUNT", self.fixture.sandbox.path().join("version-count"));
        command
    }

    pub fn bundle_command(&self) -> Command {
        self.command(&[
            "plugin",
            "bundle",
            "create",
            "--path",
            self.bundle.to_str().unwrap(),
            "--platform",
            "windows",
            "--python",
            "current",
            self.package.to_str().unwrap(),
        ])
    }

    pub fn probe(&self) -> Value {
        json!({
            "executable": self.fixture.python, "prefix": self.root, "base_prefix": self.root,
            "version_major": 3, "version_minor": 13, "virtual_env": self.root,
            "idapython_venv_executable": self.fixture.python, "externally_managed": false,
        })
    }
}
