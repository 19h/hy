use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

use serde_json::json;

use crate::guard_fixture::{Fixture, executable};

pub(super) struct Rig {
    pub fixture: Fixture,
    pub idausr: PathBuf,
    pub snapshots: PathBuf,
    counter: PathBuf,
}

impl Rig {
    pub fn new(user_directory: bool) -> Self {
        let fixture = Fixture::new(true);
        executable(&fixture.installation.join("idat"), include_str!("idat.sh"));
        let idausr = fixture.sandbox.path().join("idausr");
        if user_directory {
            for relative in [
                "ida.reg",
                "cfg/idapython.cfg",
                "idapythonrc.py",
                "license.hexlic",
                ".hexlic",
                "plugins/example.py",
                "unrelated.dat",
            ] {
                let path = idausr.join(relative);
                fs::create_dir_all(path.parent().unwrap()).unwrap();
                fs::write(path, relative).unwrap();
            }
        }
        let snapshots = fixture.sandbox.path().join("snapshots");
        fs::create_dir(&snapshots).unwrap();
        let counter = fixture.sandbox.path().join("counter");
        Self {
            fixture,
            idausr,
            snapshots,
            counter,
        }
    }

    pub fn command(&self, mode: &str) -> Command {
        let root = self.fixture.python.parent().unwrap().parent().unwrap();
        let probe = json!({
            "executable": self.fixture.python, "prefix": root, "base_prefix": root,
            "version_major": 3, "version_minor": 13,
        });
        let mut command = self.fixture.command(&[
            "ida",
            "python",
            "--no-python-environment-check",
            "exec",
            "--child-argument",
        ]);
        command
            .env_remove("HCLI_CURRENT_IDA_PYTHON_EXE")
            .env_remove("IDAPYTHON_VENV_EXECUTABLE")
            .env("HY_TEST_IDA_PROBE", probe.to_string())
            .env("HY_TEST_MODE", mode)
            .env("HY_TEST_COUNTER", &self.counter)
            .env("HY_TEST_SNAPSHOTS", &self.snapshots);
        command
    }

    pub fn count(&self) -> usize {
        fs::read_to_string(&self.counter).unwrap().trim().parse().unwrap()
    }

    pub fn environment(&self, attempt: usize) -> HashMap<String, String> {
        fs::read_to_string(self.snapshots.join(format!("env-{attempt}")))
            .unwrap()
            .lines()
            .filter_map(|line| line.split_once('='))
            .map(|(key, value)| (key.into(), value.into()))
            .collect()
    }
}
