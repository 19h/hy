use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::{Value, json};

use crate::guard_fixture::{Fixture, executable};

pub(super) struct Rig {
    pub fixture: Fixture,
}

impl Rig {
    pub fn new() -> Self {
        let fixture = Fixture::new(true);
        let rig = Self {
            fixture,
        };
        rig.interpreter(&rig.fixture.python, Some("3.12"));
        rig
    }

    pub fn interpreter(&self, path: &Path, version: Option<&str>) {
        let payload = match version {
            Some(version) => format!("printf '%s\\n' '{version}'\n"),
            None => "exit 1\n".into(),
        };
        executable(
            path,
            &format!(
                "#!/bin/sh\nprintf 'version:%s\\n' \"$0\" >> \"$HY_TEST_EVENTS\"\ncase \"$2\" in\n  *'sys.version_info.major'* ) {payload} ;;\n  * ) echo unexpected-inspection >&2; exit 19 ;;\nesac\n",
            ),
        );
    }

    pub fn environment(&self, name: &str, version: Option<&str>, config: &str) -> PathBuf {
        let root = self.fixture.sandbox.path().join(name);
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("pyvenv.cfg"), config).unwrap();
        if let Some(version) = version {
            self.interpreter(&root.join("bin/python"), Some(version));
        }
        root
    }

    pub fn probe(&self) -> Value {
        let root = self.fixture.python.parent().unwrap().parent().unwrap();
        json!({"executable": self.fixture.python, "prefix": root, "base_prefix": root,
            "version_major": 3, "version_minor": 13, "virtual_env": root,
            "idapython_venv_executable": self.fixture.python, "externally_managed": false})
    }

    pub fn command(&self, probe: &Value) -> Command {
        let mut command = self.fixture.command(&["ida", "python", "explain-environment", "--json"]);
        command
            .env_remove("HCLI_CURRENT_IDA_PYTHON_EXE")
            .env_remove("IDAPYTHON_VENV_EXECUTABLE")
            .env("HY_TEST_IDA_PROBE", probe.to_string());
        command
    }
}
